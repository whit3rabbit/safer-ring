//! Cancellation safety mechanisms for io_uring operations.
//!
//! This module implements the core safety feature that makes safer-ring truly safe:
//! proper handling of cancelled futures and orphaned operations. When a future is
//! dropped before completion, the kernel may still be using the buffer, so we track
//! these "orphaned" operations and safely handle their completion.
//!
//! # Core Safety Principle
//!
//! When an io_uring operation future is dropped (cancelled):
//! 1. The buffer ownership remains with the kernel
//! 2. The operation is marked as "orphaned"
//! 3. When the operation completes, the buffer is safely returned
//! 4. No use-after-free or double-free can occur
//!
//! # Example
//!
//! ```rust,no_run
//! use safer_ring::safety::SafeOperation;
//! use safer_ring::ownership::OwnedBuffer;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let buffer = OwnedBuffer::new(1024);
//!
//! // Create a safe operation
//! let operation = SafeOperation::new(buffer, 123);
//!
//! // If this future is dropped, the buffer stays with kernel
//! let _future = operation.into_future();
//! // drop(_future); // Safe - buffer ownership tracked
//!
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::pin::Pin;
#[cfg(test)]
use std::sync::Arc;
use std::sync::{Mutex, Weak};
use std::task::{Context, Poll};

use crate::error::{Result, SaferRingError};
use crate::ownership::OwnedBuffer;

/// A submission ID for tracking operations.
pub type SubmissionId = u64;

/// Trait for checking completion of operations.
///
/// This trait allows SafeOperationFuture to check for completions
/// without holding a direct reference to the Ring.
pub trait CompletionChecker: Sync {
    /// Try to complete a specific operation by its submission ID.
    ///
    /// Returns:
    /// - `Ok(Some(result))` if the operation completed with the given result
    /// - `Ok(None)` if the operation is still pending
    /// - `Err(e)` if there was an error checking completion
    fn try_complete_safe_operation(
        &self,
        submission_id: SubmissionId,
    ) -> Result<Option<std::io::Result<i32>>>;
}

/// An operation that can be safely cancelled without causing memory safety issues.
///
/// When a `SafeOperation` is dropped before completion, the buffer ownership
/// remains with the kernel until the operation actually completes. This prevents
/// use-after-free bugs that can occur with raw io_uring usage.
///
/// The operation is tied to a specific ring via the orphan tracker, ensuring
/// that cancelled operations are properly handled when the ring processes completions.
pub struct SafeOperation {
    submission_id: SubmissionId,
    buffer: Option<OwnedBuffer>,
    orphan_tracker: Weak<Mutex<OrphanTracker>>,
    completed: bool,
}

impl SafeOperation {
    /// Create a new safe operation.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to use for the operation
    /// * `submission_id` - Unique ID for tracking the operation
    /// * `orphan_tracker` - Weak reference to the ring's orphan tracker
    ///
    /// # Returns
    ///
    /// A new safe operation that can be cancelled without memory safety issues.
    pub fn new(
        buffer: OwnedBuffer,
        submission_id: SubmissionId,
        orphan_tracker: Weak<Mutex<OrphanTracker>>,
    ) -> Self {
        Self {
            submission_id,
            buffer: Some(buffer),
            orphan_tracker,
            completed: false,
        }
    }

    /// Create a failed safe operation.
    ///
    /// This is used when submission fails but we still need to return the buffer
    /// to the user in a failed state.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to return to the user
    /// * `submission_id` - Unique ID that was assigned (but not used)
    /// * `orphan_tracker` - Weak reference to the ring's orphan tracker
    ///
    /// # Returns
    ///
    /// A safe operation that will immediately fail when polled.
    pub fn failed(
        buffer: OwnedBuffer,
        submission_id: SubmissionId,
        orphan_tracker: Weak<Mutex<OrphanTracker>>,
    ) -> Self {
        Self {
            submission_id,
            buffer: Some(buffer),
            orphan_tracker,
            completed: true, // Mark as completed so it fails immediately
        }
    }

    /// Get the submission ID for this operation.
    pub fn submission_id(&self) -> SubmissionId {
        self.submission_id
    }

    /// Check if the operation has completed.
    pub fn is_completed(&self) -> bool {
        self.completed
    }

    /// Mark the operation as completed and return the buffer.
    ///
    /// This should only be called when the operation has actually completed
    /// and the kernel has finished using the buffer.
    ///
    /// # Returns
    ///
    /// The buffer that was used for the operation.
    pub fn complete(mut self) -> Result<OwnedBuffer> {
        self.completed = true;
        // We need to move the buffer out before drop runs
        self.buffer.take().ok_or_else(|| {
            SaferRingError::Io(std::io::Error::other("Operation buffer already taken"))
        })
    }

    /// Convert this operation into a future that can be awaited.
    ///
    /// The returned future will resolve when the operation completes.
    ///
    /// # Arguments
    ///
    /// * `ring` - Ring reference for checking operation completion
    pub(crate) fn into_future<'ring>(
        self,
        ring: &'ring dyn CompletionChecker,
        waker_registry: std::sync::Arc<crate::future::WakerRegistry>,
    ) -> SafeOperationFuture<'ring> {
        SafeOperationFuture {
            operation: Some(self),
            ring,
            waker_registry,
        }
    }

    /// Get the buffer size for debugging.
    pub fn buffer_size(&self) -> Option<usize> {
        self.buffer.as_ref().map(|b| b.size())
    }

    /// Get buffer pointer and size for operation submission.
    ///
    /// Returns (ptr, size) tuple for the buffer by transferring ownership to kernel.
    pub fn buffer_info(&self) -> Result<(*mut u8, usize)> {
        if let Some(buffer) = &self.buffer {
            buffer.give_to_kernel(self.submission_id)
        } else {
            Ok((std::ptr::null_mut(), 0))
        }
    }
}

impl Drop for SafeOperation {
    fn drop(&mut self) {
        if !self.completed && self.buffer.is_some() {
            // Operation was cancelled - register as orphaned
            if let Some(tracker) = self.orphan_tracker.upgrade() {
                if let Ok(mut tracker) = tracker.lock() {
                    if let Some(buffer) = &self.buffer {
                        tracker.register_orphan(self.submission_id, buffer.clone_handle());
                    }
                }
            }
        }
    }
}

/// Future representing a safe io_uring operation.
///
/// This future resolves to `(bytes_transferred, buffer)` when the operation completes.
/// If the future is dropped before completion, the operation becomes orphaned and
/// the buffer is safely handled by the orphan tracking system.
pub struct SafeOperationFuture<'ring> {
    operation: Option<SafeOperation>,
    ring: &'ring dyn CompletionChecker,
    waker_registry: std::sync::Arc<crate::future::WakerRegistry>,
}

impl<'ring> std::future::Future for SafeOperationFuture<'ring> {
    type Output = Result<(usize, OwnedBuffer)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(mut operation) = self.operation.take() {
            // If operation was marked as failed during creation, return error immediately
            if operation.completed && operation.buffer.is_some() {
                let _buffer = operation.buffer.take().unwrap();
                return Poll::Ready(Err(SaferRingError::Io(std::io::Error::other(
                    "Operation failed during submission",
                ))));
            }

            // Check if the operation has completed
            match self
                .ring
                .try_complete_safe_operation(operation.submission_id)
            {
                Ok(Some(completion_result)) => {
                    // Operation completed - extract the buffer and return result
                    operation.completed = true;
                    let buffer = operation.buffer.take().ok_or_else(|| {
                        SaferRingError::Io(std::io::Error::other(
                            "Operation buffer missing after completion",
                        ))
                    })?;

                    let bytes_transferred = completion_result.map_err(SaferRingError::Io)?;
                    Poll::Ready(Ok((bytes_transferred as usize, buffer)))
                }
                Ok(None) => {
                    // Operation still pending - store operation back and register waker
                    let submission_id = operation.submission_id;
                    self.operation = Some(operation);

                    // Register the waker so it gets notified when the operation completes
                    self.waker_registry
                        .register_waker(submission_id, cx.waker().clone());

                    Poll::Pending
                }
                Err(e) => {
                    // Error checking for completion
                    Poll::Ready(Err(e))
                }
            }
        } else {
            // Future was already completed or operation was taken
            Poll::Ready(Err(SaferRingError::Io(std::io::Error::other(
                "Operation future polled after completion",
            ))))
        }
    }
}

/// Future representing a safe accept operation.
///
/// This future resolves to `(bytes_transferred, buffer)` when the accept completes,
/// but for accept operations, the useful result is extracted from the completion.
pub struct SafeAcceptFuture<'ring> {
    operation: Option<SafeOperation>,
    ring: &'ring dyn CompletionChecker,
    waker_registry: std::sync::Arc<crate::future::WakerRegistry>,
}

impl<'ring> SafeAcceptFuture<'ring> {
    /// Create a new safe accept future.
    pub(crate) fn new(
        operation: SafeOperation,
        ring: &'ring dyn CompletionChecker,
        waker_registry: std::sync::Arc<crate::future::WakerRegistry>,
    ) -> Self {
        Self {
            operation: Some(operation),
            ring,
            waker_registry,
        }
    }
}

impl<'ring> std::future::Future for SafeAcceptFuture<'ring> {
    type Output = Result<(usize, OwnedBuffer)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(mut operation) = self.operation.take() {
            // If operation was marked as failed during creation, return error immediately
            if operation.completed && operation.buffer.is_some() {
                let _buffer = operation.buffer.take().unwrap();
                return Poll::Ready(Err(SaferRingError::Io(std::io::Error::other(
                    "Accept operation failed during submission",
                ))));
            }

            // Check if the operation has completed
            match self
                .ring
                .try_complete_safe_operation(operation.submission_id)
            {
                Ok(Some(completion_result)) => {
                    // Operation completed - extract the buffer and return result
                    operation.completed = true;
                    let buffer = operation.buffer.take().ok_or_else(|| {
                        SaferRingError::Io(std::io::Error::other(
                            "Accept operation buffer missing after completion",
                        ))
                    })?;

                    let bytes_transferred = completion_result.map_err(SaferRingError::Io)?;
                    Poll::Ready(Ok((bytes_transferred as usize, buffer)))
                }
                Ok(None) => {
                    // Operation still pending - store operation back and register waker
                    let submission_id = operation.submission_id;
                    self.operation = Some(operation);

                    // Register the waker so it gets notified when the operation completes
                    self.waker_registry
                        .register_waker(submission_id, cx.waker().clone());

                    Poll::Pending
                }
                Err(e) => {
                    // Error checking for completion
                    Poll::Ready(Err(e))
                }
            }
        } else {
            // Future was already completed or operation was taken
            Poll::Ready(Err(SaferRingError::Io(std::io::Error::other(
                "Accept future polled after completion",
            ))))
        }
    }
}

/// Tracks orphaned operations whose futures were dropped before completion.
///
/// The orphan tracker maintains a mapping from submission IDs to buffer handles
/// for operations that were cancelled. When these operations complete, the
/// tracker ensures the buffers are properly cleaned up.
///
/// This is a critical safety component that prevents memory leaks and ensures
/// that cancelled operations don't cause issues when they eventually complete.
#[derive(Debug)]
pub struct OrphanTracker {
    /// Map from submission ID to orphaned buffer handle
    orphaned_operations: HashMap<SubmissionId, OwnedBuffer>,
    /// Counter for generating unique submission IDs
    next_submission_id: SubmissionId,
}

impl OrphanTracker {
    /// Create a new orphan tracker.
    pub fn new() -> Self {
        Self {
            orphaned_operations: HashMap::new(),
            next_submission_id: 1, // Start at 1, 0 reserved for invalid
        }
    }

    /// Generate a new unique submission ID.
    pub fn next_submission_id(&mut self) -> SubmissionId {
        let id = self.next_submission_id;
        self.next_submission_id = self.next_submission_id.wrapping_add(1);
        id
    }

    /// Register an orphaned operation.
    ///
    /// Called when a `SafeOperation` is dropped before completion.
    /// The buffer handle is stored until the operation actually completes.
    ///
    /// # Arguments
    ///
    /// * `submission_id` - ID of the orphaned operation
    /// * `buffer` - Buffer handle for the operation
    pub fn register_orphan(&mut self, submission_id: SubmissionId, buffer: OwnedBuffer) {
        self.orphaned_operations.insert(submission_id, buffer);
    }

    /// Process a completed operation, handling both active and orphaned operations.
    ///
    /// If the operation was orphaned, this cleans up the buffer handle.
    /// If the operation was active, this returns information for waking the future.
    ///
    /// # Arguments
    ///
    /// * `submission_id` - ID of the completed operation
    /// * `result` - Result of the operation (bytes transferred or error)
    ///
    /// # Returns
    ///
    /// - `Some(buffer)` if this was an orphaned operation
    /// - `None` if this was an active operation (should wake the future)
    pub fn handle_completion(
        &mut self,
        submission_id: SubmissionId,
        result: std::io::Result<i32>,
    ) -> Option<(OwnedBuffer, std::io::Result<i32>)> {
        self.orphaned_operations
            .remove(&submission_id)
            .map(|buffer| (buffer, result))
    }

    /// Get the number of currently orphaned operations.
    ///
    /// Useful for debugging and monitoring.
    pub fn orphan_count(&self) -> usize {
        self.orphaned_operations.len()
    }

    /// Check if an operation is orphaned.
    pub fn is_orphaned(&self, submission_id: SubmissionId) -> bool {
        self.orphaned_operations.contains_key(&submission_id)
    }

    /// Clean up all orphaned operations.
    ///
    /// This should be called when the ring is being dropped to ensure
    /// all buffers are properly released.
    ///
    /// # Returns
    ///
    /// The number of orphaned operations that were cleaned up.
    pub fn cleanup_all_orphans(&mut self) -> usize {
        let count = self.orphaned_operations.len();
        self.orphaned_operations.clear();
        count
    }
}

impl Default for OrphanTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for safe operations with validation.
///
/// Provides a safe way to construct `SafeOperation` instances with proper
/// validation and error handling.
pub struct SafeOperationBuilder {
    buffer: Option<OwnedBuffer>,
    submission_id: Option<SubmissionId>,
    orphan_tracker: Option<Weak<Mutex<OrphanTracker>>>,
}

impl SafeOperationBuilder {
    /// Create a new operation builder.
    pub fn new() -> Self {
        Self {
            buffer: None,
            submission_id: None,
            orphan_tracker: None,
        }
    }

    /// Set the buffer for the operation.
    pub fn buffer(mut self, buffer: OwnedBuffer) -> Self {
        self.buffer = Some(buffer);
        self
    }

    /// Set the submission ID for the operation.
    pub fn submission_id(mut self, id: SubmissionId) -> Self {
        self.submission_id = Some(id);
        self
    }

    /// Set the orphan tracker for the operation.
    pub fn orphan_tracker(mut self, tracker: Weak<Mutex<OrphanTracker>>) -> Self {
        self.orphan_tracker = Some(tracker);
        self
    }

    /// Build the safe operation.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are not set.
    pub fn build(self) -> Result<SafeOperation> {
        let buffer = self.buffer.ok_or_else(|| {
            SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Buffer is required for safe operation",
            ))
        })?;

        let submission_id = self.submission_id.ok_or_else(|| {
            SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Submission ID is required for safe operation",
            ))
        })?;

        let orphan_tracker = self.orphan_tracker.ok_or_else(|| {
            SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Orphan tracker is required for safe operation",
            ))
        })?;

        Ok(SafeOperation::new(buffer, submission_id, orphan_tracker))
    }
}

impl Default for SafeOperationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orphan_tracker_creation() {
        let tracker = OrphanTracker::new();
        assert_eq!(tracker.orphan_count(), 0);
    }

    #[test]
    fn test_submission_id_generation() {
        let mut tracker = OrphanTracker::new();
        let id1 = tracker.next_submission_id();
        let id2 = tracker.next_submission_id();
        assert_ne!(id1, id2);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_orphan_registration() {
        let mut tracker = OrphanTracker::new();
        let buffer = OwnedBuffer::new(1024);
        let submission_id = 123;

        tracker.register_orphan(submission_id, buffer);
        assert_eq!(tracker.orphan_count(), 1);
        assert!(tracker.is_orphaned(submission_id));
    }

    #[test]
    fn test_orphan_completion_handling() {
        let mut tracker = OrphanTracker::new();
        let buffer = OwnedBuffer::new(1024);
        let submission_id = 123;

        // Register orphan
        tracker.register_orphan(submission_id, buffer);
        assert_eq!(tracker.orphan_count(), 1);

        // Handle completion
        let result = Ok(100); // 100 bytes transferred
        let completed = tracker.handle_completion(submission_id, result);

        assert!(completed.is_some());
        assert_eq!(tracker.orphan_count(), 0);
        assert!(!tracker.is_orphaned(submission_id));

        if let Some((buffer, result)) = completed {
            assert_eq!(buffer.size(), 1024);
            assert_eq!(result.unwrap(), 100);
        }
    }

    #[test]
    fn test_active_operation_completion() {
        let mut tracker = OrphanTracker::new();
        let submission_id = 456;

        // Handle completion of non-orphaned operation
        let result = Ok(50);
        let completed = tracker.handle_completion(submission_id, result);

        assert!(completed.is_none()); // Active operation, not orphaned
    }

    #[test]
    fn test_cleanup_all_orphans() {
        let mut tracker = OrphanTracker::new();

        // Register multiple orphans
        for i in 1..=5 {
            let buffer = OwnedBuffer::new(1024);
            tracker.register_orphan(i, buffer);
        }

        assert_eq!(tracker.orphan_count(), 5);

        let cleaned_up = tracker.cleanup_all_orphans();
        assert_eq!(cleaned_up, 5);
        assert_eq!(tracker.orphan_count(), 0);
    }

    #[test]
    fn test_safe_operation_creation() {
        let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
        let buffer = OwnedBuffer::new(1024);
        let submission_id = 789;

        let operation = SafeOperation::new(buffer, submission_id, Arc::downgrade(&tracker));

        assert_eq!(operation.submission_id(), submission_id);
        assert!(!operation.is_completed());
        assert_eq!(operation.buffer_size(), Some(1024));
    }

    #[test]
    fn test_safe_operation_completion() {
        let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
        let buffer = OwnedBuffer::new(512);
        let submission_id = 999;

        let operation = SafeOperation::new(buffer, submission_id, Arc::downgrade(&tracker));

        let returned_buffer = operation.complete().unwrap();
        assert_eq!(returned_buffer.size(), 512);

        // No orphan should be registered since operation completed normally
        let tracker_guard = tracker.lock().unwrap();
        assert_eq!(tracker_guard.orphan_count(), 0);
    }

    #[test]
    fn test_safe_operation_drop_registers_orphan() {
        let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
        let buffer = OwnedBuffer::new(256);
        let submission_id = 111;

        {
            let _operation = SafeOperation::new(buffer, submission_id, Arc::downgrade(&tracker));
            // operation is dropped here without completion
        }

        // Should have registered as orphan
        let tracker_guard = tracker.lock().unwrap();
        assert_eq!(tracker_guard.orphan_count(), 1);
        assert!(tracker_guard.is_orphaned(submission_id));
    }

    #[test]
    fn test_safe_operation_builder() {
        let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
        let buffer = OwnedBuffer::new(1024);

        let operation = SafeOperationBuilder::new()
            .buffer(buffer)
            .submission_id(42)
            .orphan_tracker(Arc::downgrade(&tracker))
            .build()
            .unwrap();

        assert_eq!(operation.submission_id(), 42);
        assert_eq!(operation.buffer_size(), Some(1024));
    }

    #[test]
    fn test_safe_operation_builder_missing_fields() {
        let result = SafeOperationBuilder::new()
            .submission_id(42)
            // Missing buffer and tracker
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_submission_id_wrapping() {
        let mut tracker = OrphanTracker::new();
        tracker.next_submission_id = u64::MAX;

        let id1 = tracker.next_submission_id();
        let id2 = tracker.next_submission_id();

        assert_eq!(id1, u64::MAX);
        assert_eq!(id2, 0); // Should wrap around
    }
}
