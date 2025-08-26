//! Utility methods for Ring management and monitoring.

use super::Ring;
use crate::error::Result;
use crate::ring::completion::CompletionResult;

impl<'ring> Ring<'ring> {
    /// Get the number of operations currently in flight.
    pub fn operations_in_flight(&self) -> usize {
        self.operations.borrow().count()
    }

    /// Check if there are any operations currently in flight.
    pub fn has_operations_in_flight(&self) -> bool {
        self.operations.borrow().has_operations()
    }

    /// Get the capacity of the submission queue.
    #[cfg(target_os = "linux")]
    pub fn capacity(&mut self) -> u32 {
        // TODO: Add capacity method to backend trait
        1024 // Placeholder capacity
    }

    /// Get the capacity of the submission queue (stub for non-Linux).
    #[cfg(not(target_os = "linux"))]
    pub fn capacity(&self) -> u32 {
        0
    }

    /// Try to complete an operation by ID and return its result.
    ///
    /// This method checks if an operation with the given ID has completed.
    /// It's used by batch futures to poll for completion.
    ///
    /// # Arguments
    ///
    /// * `operation_id` - The ID of the operation to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(result))` if completed, `Ok(None)` if still in progress,
    /// or `Err` if the operation failed.
    pub fn poll_operation_completion(
        &mut self,
        _operation_id: u64,
    ) -> Result<Option<CompletionResult<'ring, 'static>>> {
        // Check the completion queue for this operation
        #[cfg(target_os = "linux")]
        {
            // TODO: Fix completion queue access through backend
            Ok(None) // Placeholder implementation
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux platforms, simulate completion for testing
            // On non-Linux platforms, return None to indicate no completion available
            Ok(None)
        }
    }

    /// Process completed operations, handling both active and orphaned operations.
    ///
    /// This method should be called periodically to clean up orphaned operations
    /// and ensure proper resource management.
    ///
    /// # Returns
    ///
    /// The number of operations processed.
    pub fn process_completions(&self) -> Result<usize> {
        #[cfg(target_os = "linux")]
        {
            // In a real implementation, this would:
            // 1. Poll io_uring completion queue
            // 2. For each completion, check if operation is orphaned
            // 3. If orphaned, clean up via orphan_tracker.handle_completion()
            // 4. If active, wake the appropriate future

            // Placeholder implementation
            let mut tracker = self.orphan_tracker.lock().unwrap();
            Ok(tracker.cleanup_all_orphans())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(0)
        }
    }

    /// Get the number of currently orphaned operations.
    ///
    /// Useful for monitoring and debugging.
    pub fn orphan_count(&self) -> usize {
        self.orphan_tracker.lock().unwrap().orphan_count()
    }
}
