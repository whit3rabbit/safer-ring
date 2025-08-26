//! Standalone batch future that doesn't hold Ring reference.
//!
//! This solves the lifetime constraint issue by allowing the batch future
//! to exist without borrowing the Ring mutably. Instead, polling operations
//! can take Ring references as needed.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use crate::error::Result;
use crate::future::WakerRegistry;
use crate::ring::batch::{BatchResult, OperationResult};
use crate::ring::Ring;

/// A batch future that doesn't hold a reference to the Ring.
///
/// This future can be created and stored independently of Ring lifetime,
/// resolving the API ergonomics issues with the original BatchFuture.
pub struct StandaloneBatchFuture {
    /// Operation IDs for tracking completions
    operation_ids: Vec<Option<u64>>,
    /// Results collected so far
    results: Vec<Option<OperationResult>>,
    /// Dependencies between operations (dependent -> dependencies)
    dependencies: HashMap<usize, Vec<usize>>,
    /// Whether to fail fast on first error
    fail_fast: bool,
    /// Whether the batch has completed
    completed: bool,
    /// Fast lookup map: operation_id -> batch_index for O(1) completion matching
    id_to_index: HashMap<u64, usize>,
    /// Waker registry for async notification
    waker_registry: Rc<WakerRegistry>,
}

impl StandaloneBatchFuture {
    /// Create a new standalone batch future.
    pub(crate) fn new(
        operation_ids: Vec<Option<u64>>,
        dependencies: HashMap<usize, Vec<usize>>,
        waker_registry: Rc<WakerRegistry>,
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
            operation_ids,
            results,
            dependencies,
            fail_fast,
            completed: false,
            id_to_index,
            waker_registry,
        }
    }

    /// Poll the batch future with an externally provided Ring reference.
    ///
    /// This method allows the caller to provide Ring access only when polling,
    /// avoiding the lifetime constraint issues of holding a Ring reference.
    pub fn poll_with_ring(
        &mut self,
        ring: &mut Ring<'_>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<BatchResult>> {
        if self.completed {
            // Collect all results
            let results: Vec<OperationResult> = self
                .results
                .iter()
                .map(|opt| opt.as_ref().cloned().unwrap_or(OperationResult::Cancelled))
                .collect();

            return Poll::Ready(Ok(BatchResult::new(results)));
        }

        // Poll for completions using the provided Ring
        match self.poll_completions_with_ring(ring, cx) {
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

    /// Poll for completion of submitted operations using the provided Ring.
    fn poll_completions_with_ring(
        &mut self,
        ring: &mut Ring<'_>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        let mut any_completed = false;
        let mut any_failed = false;
        let mut completed_operations = Vec::new();

        // Process ALL available completions in one batch for efficiency
        match ring.try_complete() {
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
        Poll::Pending
    }

    /// Check if operations that were waiting for dependencies are now ready.
    fn check_ready_operations(&mut self, completed_index: usize) {
        // Placeholder for future dependency handling logic
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
}

impl Future for StandaloneBatchFuture {
    type Output = Result<BatchResult>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // This implementation cannot work because we don't have access to a Ring
        // The caller must use poll_with_ring instead
        let _ = cx;
        Poll::Pending
    }
}

// Implement Drop to ensure proper cleanup
impl Drop for StandaloneBatchFuture {
    fn drop(&mut self) {
        // Cancel any remaining operations to prevent resource leaks
        self.cancel_all_remaining_operations();

        // Clean up wakers from the registry
        for id_opt in &self.operation_ids {
            if let Some(id) = id_opt {
                self.waker_registry.remove_waker(*id);
            }
        }
    }
}