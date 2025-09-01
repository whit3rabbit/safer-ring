//! Completion queue processing for the Ring.

use crate::error::{Result, SaferRingError};
use std::io;

use super::core::Ring;

mod result;

pub use result::CompletionResult;

impl<'ring> Ring<'ring> {
    /// Try to complete operations by checking the completion queue.
    ///
    /// Non-blocking check for completed operations. Processes all available
    /// completions and returns them as a vector of results. Each completion
    /// contains the operation result.
    ///
    /// **Note**: Buffer ownership is not currently returned due to the borrowed
    /// reference API design. Users retain ownership of their buffers.
    ///
    /// # Returns
    ///
    /// Returns a vector of completion results. The vector may be empty if no
    /// operations have completed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The completion queue is in an invalid state
    /// - A completion references an unknown operation ID
    /// - System error occurs while processing completions
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::Ring;
    /// # #[cfg(target_os = "linux")]
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    ///
    /// // Submit some operations...
    ///
    /// // Check for completions
    /// let completions = ring.try_complete()?;
    /// for completion in completions {
    ///     let (result, buffer) = completion.into_result();
    ///     match result {
    ///         Ok(bytes) => println!("Operation completed: {} bytes", bytes),
    ///         Err(e) => eprintln!("Operation failed: {}", e),
    ///     }
    ///     // buffer is currently always None - users retain buffer ownership
    ///     assert!(buffer.is_none());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_complete(&mut self) -> Result<Vec<CompletionResult<'ring, '_>>> {
        self.process_completion_queue(false)
    }
    /// Wait for at least one operation to complete.
    ///
    /// Blocks until at least one operation completes, then processes all
    /// available completions. This is more efficient than polling when you
    /// need to wait for operations to finish.
    ///
    /// # Returns
    ///
    /// Returns a vector of completion results with at least one element
    /// (unless an error occurs).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No operations are in flight (nothing to wait for)
    /// - The completion queue is in an invalid state
    /// - System error occurs while waiting or processing
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::Ring;
    /// # #[cfg(target_os = "linux")]
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    ///
    /// // Submit some operations...
    ///
    /// // Wait for at least one to complete
    /// let completions = ring.wait_for_completion()?;
    /// println!("Got {} completions", completions.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn wait_for_completion(&mut self) -> Result<Vec<CompletionResult<'ring, '_>>> {
        // Check if we have any operations to wait for
        if !self.has_operations_in_flight() {
            return Err(SaferRingError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No operations in flight to wait for",
            )));
        }

        self.process_completion_queue(true)
    }

    /// Check a specific operation for completion.
    ///
    /// This method allows checking if a specific operation has completed
    /// without processing all completions. Useful when you only care about
    /// one particular operation.
    ///
    /// # Arguments
    ///
    /// * `operation_id` - The ID of the operation to check
    ///
    /// # Returns
    ///
    /// Returns `Some(result)` if the operation has completed, `None` if it's
    /// still in flight.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation ID is not recognized or if there's
    /// a system error while checking completions.
    pub fn try_complete_by_id(&mut self, operation_id: u64) -> Result<Option<io::Result<i32>>> {
        // Track polling for debugging
        static POLL_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let count = POLL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count % 1000 == 0 {
            eprintln!("[DEBUG] try_complete_by_id called {count} times for op {operation_id}");
        }

        // First check the completion cache
        {
            let mut cache = self.completion_cache.borrow_mut();
            if let Some(result) = cache.remove(&operation_id) {
                // Found in cache, remove from tracker and return
                let mut tracker = self.operations.borrow_mut();
                tracker.complete_operation(operation_id);
                self.waker_registry.wake_operation(operation_id);
                return Ok(Some(result));
            }
        }

        // Not in cache, try blocking wait instead of busy polling
        let completions = if count > 50 {
            // After many polls with no result, wait for completion
            self.backend
                .borrow_mut()
                .wait_for_completion()
                .unwrap_or_else(|_| {
                    // If wait fails (e.g., no operations), fall back to try_complete
                    self.backend.borrow_mut().try_complete().unwrap_or_default()
                })
        } else {
            // First few attempts use try_complete
            self.backend.borrow_mut().try_complete()?
        };

        let mut target_result = None;

        // Process all completions
        {
            let mut tracker = self.operations.borrow_mut();
            let mut cache = self.completion_cache.borrow_mut();

            for (completed_id, result) in completions {
                // Check if this operation is being tracked
                if tracker.is_operation_tracked(completed_id) {
                    if completed_id == operation_id {
                        // This is the one we're looking for - complete it immediately
                        tracker.complete_operation(completed_id);
                        self.waker_registry.wake_operation(completed_id);
                        target_result = Some(result);
                    } else {
                        // Cache this completion for later retrieval
                        cache.insert(completed_id, result);
                        // Wake up the future that might be waiting
                        self.waker_registry.wake_operation(completed_id);
                    }
                }
                // Ignore completions for untracked operations (orphans)
            }
        }

        Ok(target_result)
    }

    /// Get completion queue statistics.
    ///
    /// Returns information about the current state of the completion queue,
    /// useful for monitoring and debugging.
    ///
    /// # Returns
    ///
    /// Returns a tuple of (ready_count, capacity) where:
    /// - `ready_count` is the number of completions ready to be processed
    /// - `capacity` is the total capacity of the completion queue
    pub fn completion_queue_stats(&mut self) -> (usize, usize) {
        self.backend.borrow_mut().completion_queue_stats()
    }

    /// Process completions from the completion queue.
    ///
    /// Internal method that handles the actual completion processing logic.
    /// Can operate in blocking or non-blocking mode.
    fn process_completion_queue(&mut self, wait: bool) -> Result<Vec<CompletionResult<'ring, '_>>> {
        // First check if we have any cached completions
        let mut cached_completions = Vec::new();
        {
            let mut cache = self.completion_cache.borrow_mut();
            if !cache.is_empty() {
                // Drain all cached completions
                cached_completions = cache.drain().collect();
            }
        }

        // Get new completions from backend if needed
        let backend_completions = if wait || cached_completions.is_empty() {
            if wait {
                self.backend.borrow_mut().wait_for_completion()?
            } else {
                self.backend.borrow_mut().try_complete()?
            }
        } else {
            // We have cached completions, don't poll backend unless waiting
            Vec::new()
        };

        // Combine cached and new completions
        let mut all_completions = cached_completions;
        all_completions.extend(backend_completions);

        // Process all completions
        self.process_completed_operations(all_completions)
    }

    /// Convert a completion queue entry result to an io::Result.
    ///
    /// Centralizes the logic for converting raw io_uring results to Rust's
    /// io::Result type. Negative values are errno codes.
    #[cfg(target_os = "linux")]
    #[inline]
    #[allow(dead_code)] // Will be used when completion queue is implemented
    fn convert_cqe_result(result_value: i32) -> io::Result<i32> {
        if result_value < 0 {
            // Negative values are errno codes, convert to io::Error
            Err(io::Error::from_raw_os_error(-result_value))
        } else {
            // Non-negative values are success (bytes transferred)
            Ok(result_value)
        }
    }

    /// Process completed operations and remove them from tracking.
    ///
    /// This method processes completions and wakes up any futures that are
    /// waiting for these operations to complete. It also removes completed
    /// operations from tracking.
    fn process_completed_operations(
        &self,
        completed_operations: Vec<(u64, io::Result<i32>)>,
    ) -> Result<Vec<CompletionResult<'ring, '_>>> {
        let mut completions = Vec::with_capacity(completed_operations.len());

        // Remove completed operations from tracking and wake futures
        {
            let mut tracker = self.operations.borrow_mut();

            for (operation_id, io_result) in completed_operations {
                if let Some(handle) = tracker.complete_operation(operation_id) {
                    // Wake up any future waiting for this operation
                    self.waker_registry.wake_operation(operation_id);

                    // Create completion result with buffer ownership
                    let completion = CompletionResult::new_with_buffer(
                        operation_id,
                        handle.op_type,
                        handle.fd,
                        io_result,
                        handle.buffer,
                    );

                    completions.push(completion);
                } else {
                    // Unknown operation ID shouldn't happen in normal operation
                    return Err(SaferRingError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Completion for unknown operation ID: {operation_id}"),
                    )));
                }
            }
        }

        Ok(completions)
    }

    /// Remove a single operation from tracking.
    ///
    /// Helper method to remove an operation from the tracker and handle
    /// the case where the operation ID is not found. Also wakes up any
    /// future waiting for this operation.
    #[cfg(target_os = "linux")]
    #[allow(dead_code)] // Will be used when completion queue is implemented
    fn remove_operation_from_tracking(&self, operation_id: u64) -> Result<()> {
        let mut tracker = self.operations.borrow_mut();
        if tracker.complete_operation(operation_id).is_none() {
            return Err(SaferRingError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Completion for unknown operation ID: {operation_id}"),
            )));
        }

        // Wake up any future waiting for this operation
        self.waker_registry.wake_operation(operation_id);

        Ok(())
    }
}
