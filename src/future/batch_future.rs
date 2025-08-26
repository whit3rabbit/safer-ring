//! Future implementation for batch operations.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use crate::error::Result;
use crate::future::WakerRegistry;

use crate::ring::batch::{BatchResult, OperationResult};
use crate::ring::Ring;

/// Future for batch operations that can be awaited.
///
/// This future manages the completion of multiple operations submitted as a batch,
/// handling dependencies and partial failures according to the batch configuration.
///
/// # Example
///
/// ```rust,no_run
/// # use safer_ring::{Ring, Batch, Operation, PinnedBuffer};
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let ring = Ring::new(32)?;
/// let mut batch = Batch::new();
/// let mut buffer = PinnedBuffer::with_capacity(1024);
///
/// batch.add_operation(Operation::read().fd(0).buffer(buffer.as_mut_slice()))?;
///
/// let results = ring.submit_batch(batch).await?;
/// println!("Batch completed with {} operations", results.results.len());
/// # Ok(())
/// # }
/// ```
pub struct BatchFuture<'ring> {
    /// Ring reference for polling completions
    ring: &'ring mut Ring<'ring>,
    /// Results collected so far
    results: Vec<Option<OperationResult>>,
    /// Dependencies between operations (dependent -> dependencies)
    dependencies: HashMap<usize, Vec<usize>>,
    /// Whether to fail fast on first error
    fail_fast: bool,
    /// Whether the batch has completed
    completed: bool,
    /// Operation IDs for tracking completions
    #[allow(dead_code)] // Used for tracking operations, may be needed for debugging
    operation_ids: Vec<Option<u64>>,
    /// Fast lookup map: operation_id -> batch_index for O(1) completion matching
    id_to_index: HashMap<u64, usize>,
}

impl<'ring> BatchFuture<'ring> {
    /// Create a new batch future.
    ///
    /// # Arguments
    ///
    /// * `operation_ids` - Vector of operation IDs that have been submitted
    /// * `dependencies` - Map of operation dependencies
    /// * `ring` - Ring reference for completion polling
    /// * `waker_registry` - Waker registry for async coordination
    /// * `fail_fast` - Whether to cancel remaining operations on first failure
    pub(crate) fn new(
        operation_ids: Vec<Option<u64>>,
        dependencies: HashMap<usize, Vec<usize>>,
        ring: &'ring mut Ring<'ring>,
        _waker_registry: Rc<WakerRegistry>,
        fail_fast: bool,
    ) -> Self {
        let operation_count = operation_ids.len();
        let results = (0..operation_count).map(|_| None).collect();

        // Build the fast lookup map for O(1) operation_id -> batch_index mapping
        let mut id_to_index = HashMap::new();
        for (index, id_opt) in operation_ids.iter().enumerate() {
            if let Some(id) = id_opt {
                id_to_index.insert(*id, index);
            }
        }

        Self {
            ring,
            results,
            dependencies,
            fail_fast,
            completed: false,
            operation_ids,
            id_to_index,
        }
    }

    /// Poll for completion of submitted operations.
    fn poll_completions(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut any_completed = false;
        let mut any_failed = false;
        let mut completed_operations = Vec::new();

        // Process ALL available completions in one batch for efficiency
        // This is much more efficient than checking each operation individually
        match self.ring.try_complete() {
            Ok(completions) => {
                // Process each completion and match it to our pending operations
                for completion in completions {
                    let operation_id = completion.id();

                    // Use O(1) HashMap lookup instead of O(N) linear search
                    if let Some(&index) = self.id_to_index.get(&operation_id) {
                        if self.results[index].is_some() {
                            continue; // Already completed (shouldn't happen, but defensive)
                        }

                        // Extract the result from the completion
                        let result = match completion.result() {
                            Ok(bytes) => OperationResult::Success(*bytes),
                            Err(e) => {
                                let error_msg = e.to_string();
                                OperationResult::Error(error_msg)
                            }
                        };

                        let is_error = matches!(result, OperationResult::Error(_));
                        self.results[index] = Some(result);
                        any_completed = true;
                        if is_error {
                            any_failed = true;
                        }
                        completed_operations.push(index);
                    }
                    // If we can't find the operation, it might be from a different batch
                    // or completed operation - ignore it
                }
            }
            Err(e) => {
                // Error polling completions - this might be a system error
                return Poll::Ready(Err(e));
            }
        }

        // Process completed operations to check for ready dependencies
        for completed_index in completed_operations {
            self.check_ready_operations(completed_index);

            // Cancel dependent operations if fail_fast is enabled and this operation failed
            if self.fail_fast
                && matches!(
                    self.results[completed_index],
                    Some(OperationResult::Error(_))
                )
            {
                self.cancel_dependent_operations(completed_index);
            }
        }

        // If we're in fail_fast mode and something failed, cancel everything
        if self.fail_fast && any_failed {
            self.cancel_all_remaining_operations();
            return Poll::Ready(Ok(()));
        }

        // Check if all operations have completed
        if self.all_operations_completed() {
            self.completed = true;
            return Poll::Ready(Ok(()));
        }

        // If we made progress, continue polling
        if any_completed {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        // For batch operations, we'll use a simple polling approach
        // In a more sophisticated implementation, we could register wakers
        // for individual operations, but for now we'll just return Pending
        Poll::Pending
    }

    /// Check if operations that were waiting for dependencies are now ready.
    fn check_ready_operations(&mut self, completed_index: usize) {
        // For now, we submit all operations immediately, so dependency handling
        // is simplified. In a more sophisticated implementation, we would
        // track which operations are waiting for dependencies and submit them
        // when their dependencies complete.

        // This is a placeholder for future dependency handling logic
        let _newly_ready: Vec<usize> = Vec::new();

        // Find operations that were waiting for this one to complete
        for (&_dependent_index, dependencies) in &self.dependencies {
            if dependencies.contains(&completed_index) {
                // Check if all dependencies for this operation are now satisfied
                let _all_deps_satisfied = dependencies.iter().all(|&dep_index| {
                    self.results[dep_index].is_some()
                        && self.results[dep_index].as_ref().unwrap().is_success()
                });

                // In the current implementation, all operations are submitted immediately
                // so we don't need to track ready operations
            }
        }
    }

    /// Cancel operations that depend on a failed operation.
    fn cancel_dependent_operations(&mut self, failed_index: usize) {
        let mut to_cancel = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![failed_index];

        // Find all operations that transitively depend on the failed operation
        while let Some(current) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            for (&dependent, dependencies) in &self.dependencies {
                if dependencies.contains(&current) && !visited.contains(&dependent) {
                    to_cancel.push(dependent);
                    stack.push(dependent);
                }
            }
        }

        // Cancel the dependent operations
        for &index in &to_cancel {
            if self.results[index].is_none() {
                self.results[index] = Some(OperationResult::Cancelled);
            }
        }
    }

    /// Cancel all remaining operations (used in fail_fast mode).
    fn cancel_all_remaining_operations(&mut self) {
        for result in self.results.iter_mut() {
            if result.is_none() {
                *result = Some(OperationResult::Cancelled);
            }
        }
    }

    /// Check if all operations have completed (successfully, failed, or cancelled).
    fn all_operations_completed(&self) -> bool {
        self.results.iter().all(|result| result.is_some())
    }

    /// Submit operations that are ready (have no pending dependencies).
    fn submit_ready_operations(&mut self) -> Result<()> {
        // This would be called during initial setup or when dependencies are satisfied
        // For now, we assume operations are submitted externally
        Ok(())
    }
}

impl<'ring> Future for BatchFuture<'ring> {
    type Output = Result<BatchResult>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.completed {
            // Collect all results
            let results: Vec<OperationResult> = self
                .results
                .iter()
                .map(|opt| opt.as_ref().cloned().unwrap_or(OperationResult::Cancelled))
                .collect();

            return Poll::Ready(Ok(BatchResult::new(results)));
        }

        // Submit any operations that are ready
        if let Err(e) = self.submit_ready_operations() {
            return Poll::Ready(Err(e));
        }

        // Poll for completions
        match self.poll_completions(cx) {
            Poll::Ready(Ok(())) => {
                // All operations completed, collect results
                let results: Vec<OperationResult> = self
                    .results
                    .iter()
                    .map(|opt| opt.as_ref().cloned().unwrap_or(OperationResult::Cancelled))
                    .collect();

                Poll::Ready(Ok(BatchResult::new(results)))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

// Implement Drop to ensure proper cleanup
impl<'ring> Drop for BatchFuture<'ring> {
    fn drop(&mut self) {
        // Cancel any remaining operations to prevent resource leaks
        self.cancel_all_remaining_operations();
    }
}
