//! Completion queue processing for the Ring.

use std::io;

#[cfg(target_os = "linux")]
use std::collections::HashMap;

use crate::error::Result;

use super::core::Ring;
#[cfg(target_os = "linux")]
use crate::error::SaferRingError;

mod result;

pub use result::CompletionResult;

impl<'ring> Ring<'ring> {
    /// Try to complete operations by checking the completion queue.
    ///
    /// Non-blocking check for completed operations. Processes all available
    /// completions and returns them as a vector of results. Each completion
    /// contains the operation result and returns buffer ownership to the caller.
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
    ///     // Buffer can be reused here
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_complete(&self) -> Result<Vec<CompletionResult<'ring, '_>>> {
        #[cfg(target_os = "linux")]
        {
            self.process_completions(false)
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(Vec::new())
        }
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
    pub fn wait_for_completion(&self) -> Result<Vec<CompletionResult<'ring, '_>>> {
        #[cfg(target_os = "linux")]
        {
            // Check if we have any operations to wait for
            if !self.has_operations_in_flight() {
                return Err(SaferRingError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No operations in flight to wait for",
                )));
            }

            self.process_completions(true)
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(Vec::new())
        }
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
    pub fn try_complete_by_id(&self, operation_id: u64) -> Result<Option<io::Result<i32>>> {
        #[cfg(target_os = "linux")]
        {
            let mut cq = self.inner.completion();

            // Look for our specific operation in the completion queue
            for cqe in &mut cq {
                if cqe.user_data() == operation_id {
                    let result_value = cqe.result();

                    // Convert the raw result to an io::Result
                    let io_result = Self::convert_cqe_result(result_value);

                    // Mark this completion as seen
                    cq.sync();

                    // Remove from tracking
                    self.remove_operation_from_tracking(operation_id)?;

                    return Ok(Some(io_result));
                }
            }

            // Operation not found in completion queue
            Ok(None)
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = operation_id; // Suppress unused parameter warning
            Ok(None)
        }
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
    pub fn completion_queue_stats(&self) -> (usize, usize) {
        #[cfg(target_os = "linux")]
        {
            let cq = self.inner.completion();
            (cq.len(), cq.capacity())
        }

        #[cfg(not(target_os = "linux"))]
        {
            (0, 0)
        }
    }

    /// Process completions from the completion queue.
    ///
    /// Internal method that handles the actual completion processing logic.
    /// Can operate in blocking or non-blocking mode.
    #[cfg(target_os = "linux")]
    fn process_completions(&self, wait: bool) -> Result<Vec<CompletionResult<'ring, '_>>> {
        let mut completed_operations = HashMap::new();

        // Get completion queue entries
        let mut cq = self.inner.completion();

        if wait && cq.is_empty() {
            // Drop the completion queue to release the borrow before waiting
            drop(cq);

            // Wait for at least one completion
            self.inner.submit_and_wait(0)?;

            // Re-acquire the completion queue
            cq = self.inner.completion();
        }

        // Process all available completions in a single pass
        for cqe in &mut cq {
            let user_data = cqe.user_data();
            let result_value = cqe.result();

            // Convert the raw result to an io::Result
            let io_result = Self::convert_cqe_result(result_value);

            // Store the completion result for this operation ID
            completed_operations.insert(user_data, io_result);
        }

        // Mark completions as seen
        cq.sync();

        // Remove completed operations from tracking
        self.process_completed_operations(completed_operations)
    }

    /// Convert a completion queue entry result to an io::Result.
    ///
    /// Centralizes the logic for converting raw io_uring results to Rust's
    /// io::Result type. Negative values are errno codes.
    #[cfg(target_os = "linux")]
    #[inline]
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
    #[cfg(target_os = "linux")]
    fn process_completed_operations(
        &self,
        completed_operations: HashMap<u64, io::Result<i32>>,
    ) -> Result<Vec<CompletionResult<'ring, '_>>> {
        let mut completions = Vec::with_capacity(completed_operations.len());

        // Remove completed operations from tracking and wake futures
        {
            let mut tracker = self.operations.borrow_mut();

            for (operation_id, _io_result) in completed_operations {
                if let Some(_handle) = tracker.complete_operation(operation_id) {
                    // Wake up any future waiting for this operation
                    self.waker_registry.wake_operation(operation_id);

                    // TODO: Reconstruct full operation with buffer ownership
                    // This is a known limitation of the current design.
                    // To properly return buffer ownership, we would need to:
                    // 1. Store buffer references in the operation tracker
                    // 2. Modify the tracker to return more than just handles
                    // 3. Restructure how operations are managed throughout their lifecycle
                    //
                    // For now, we acknowledge this limitation and return empty results.
                } else {
                    // Unknown operation ID shouldn't happen in normal operation
                    return Err(SaferRingError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Completion for unknown operation ID: {}", operation_id),
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
    fn remove_operation_from_tracking(&self, operation_id: u64) -> Result<()> {
        let mut tracker = self.operations.borrow_mut();
        if tracker.complete_operation(operation_id).is_none() {
            return Err(SaferRingError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Completion for unknown operation ID: {}", operation_id),
            )));
        }

        // Wake up any future waiting for this operation
        self.waker_registry.wake_operation(operation_id);

        Ok(())
    }
}
