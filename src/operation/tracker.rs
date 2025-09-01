//! Operation tracking for ensuring ring safety.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::os::unix::io::RawFd;
use std::pin::Pin;

use crate::operation::OperationType;

/// Tracks in-flight operations to ensure safety.
///
/// Prevents the ring from being dropped while operations are pending,
/// which would leave dangling pointers in the kernel.
#[derive(Debug)]
pub(crate) struct OperationTracker<'ring> {
    in_flight: HashMap<u64, OperationHandle<'ring>>,
    next_id: u64,
}

/// Handle for tracking an in-flight operation.
#[derive(Debug)]
pub(crate) struct OperationHandle<'ring> {
    pub(crate) id: u64,
    pub(crate) op_type: OperationType,
    pub(crate) fd: RawFd,
    /// Buffer ownership for operations that use buffers
    pub(crate) buffer: Option<BufferOwnership>,
    _phantom: PhantomData<&'ring ()>,
}

/// Represents ownership of a buffer used in an operation.
/// This allows the completion API to return buffer ownership to the caller.
///
/// Note: This is designed for use with owned buffers (like PinnedBuffer) that can
/// be transferred between the submission and completion phases. The future-based API
/// manages ownership through lifetimes and doesn't need this mechanism.
#[derive(Debug)]
pub enum BufferOwnership {
    /// Single owned buffer (e.g. from PinnedBuffer)
    Single(Pin<Box<[u8]>>),
    /// Multiple owned buffers for vectored operations
    Vectored(Vec<Pin<Box<[u8]>>>),
}

impl<'ring> OperationTracker<'ring> {
    /// Create a new operation tracker.
    #[allow(dead_code)] // Used by ring initialization
    pub(crate) fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            next_id: 1, // Start from 1, reserve 0 for special cases
        }
    }

    /// Register a new operation and return its ID.
    pub(crate) fn register_operation(&mut self, op_type: OperationType, fd: RawFd) -> u64 {
        self.register_operation_with_buffer(op_type, fd, None)
    }

    /// Register a new operation with buffer ownership and return its ID.
    pub(crate) fn register_operation_with_buffer(
        &mut self,
        op_type: OperationType,
        fd: RawFd,
        buffer: Option<BufferOwnership>,
    ) -> u64 {
        let id = self.next_id;
        // Wrapping add prevents overflow panics in long-running applications
        self.next_id = self.next_id.wrapping_add(1);

        let handle = OperationHandle {
            id,
            op_type,
            fd,
            buffer,
            _phantom: PhantomData,
        };

        self.in_flight.insert(id, handle);
        id
    }

    /// Mark an operation as completed and remove it from tracking.
    pub(crate) fn complete_operation(&mut self, id: u64) -> Option<OperationHandle<'ring>> {
        self.in_flight.remove(&id)
    }

    /// Check if an operation is currently tracked.
    pub(crate) fn is_operation_tracked(&self, id: u64) -> bool {
        self.in_flight.contains_key(&id)
    }

    /// Get the number of operations currently in flight.
    pub(crate) fn count(&self) -> usize {
        self.in_flight.len()
    }

    /// Check if there are any operations in flight.
    pub(crate) fn has_operations(&self) -> bool {
        !self.in_flight.is_empty()
    }

    /// Get debug information about all in-flight operations.
    pub(crate) fn debug_info(&self) -> Vec<(u64, OperationType, RawFd)> {
        self.in_flight
            .values()
            .map(|handle| (handle.id, handle.op_type, handle.fd))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_is_empty() {
        let tracker = OperationTracker::new();
        assert_eq!(tracker.count(), 0);
        assert!(!tracker.has_operations());
    }

    #[test]
    fn register_and_complete_operations() {
        let mut tracker = OperationTracker::new();

        let id1 = tracker.register_operation(OperationType::Read, 0);
        assert_eq!(tracker.count(), 1);
        assert!(tracker.has_operations());
        assert_eq!(id1, 1);

        let id2 = tracker.register_operation(OperationType::Write, 1);
        assert_eq!(tracker.count(), 2);
        assert_eq!(id2, 2);

        let handle = tracker.complete_operation(id1);
        assert!(handle.is_some());
        assert_eq!(handle.unwrap().id, id1);
        assert_eq!(tracker.count(), 1);

        tracker.complete_operation(id2);
        assert_eq!(tracker.count(), 0);
        assert!(!tracker.has_operations());
    }

    #[test]
    fn complete_nonexistent_operation() {
        let mut tracker = OperationTracker::new();
        let handle = tracker.complete_operation(999);
        assert!(handle.is_none());
    }

    #[test]
    fn id_wraparound() {
        let mut tracker = OperationTracker::new();
        tracker.next_id = u64::MAX;

        let id1 = tracker.register_operation(OperationType::Read, 0);
        assert_eq!(id1, u64::MAX);

        let id2 = tracker.register_operation(OperationType::Read, 0);
        assert_eq!(id2, 0); // Should wrap around
    }

    #[test]
    fn debug_info() {
        let mut tracker = OperationTracker::new();

        tracker.register_operation(OperationType::Read, 3);
        tracker.register_operation(OperationType::Write, 4);

        let info = tracker.debug_info();
        assert_eq!(info.len(), 2);

        // Sort by ID for consistent testing
        let mut sorted_info = info;
        sorted_info.sort_by_key(|&(id, _, _)| id);

        assert_eq!(sorted_info[0], (1, OperationType::Read, 3));
        assert_eq!(sorted_info[1], (2, OperationType::Write, 4));
    }
}
