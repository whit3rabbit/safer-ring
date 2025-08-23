//! Batch operation support for efficient submission of multiple operations.
//!
//! This module provides functionality to submit multiple operations efficiently
//! in a single batch, with support for operation dependencies and proper error
//! handling for partial batch failures.

use std::collections::HashMap;

use crate::error::{Result, SaferRingError};
use crate::operation::{Building, Operation};

/// A batch of operations to be submitted together.
///
/// Batches allow efficient submission of multiple operations with a single
/// kernel syscall, reducing overhead for high-throughput applications.
/// Operations in a batch can have dependencies and ordering constraints.
///
/// # Example
///
/// ```rust,no_run
/// # use safer_ring::{Ring, Operation, PinnedBuffer, Batch};
/// # use std::pin::Pin;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let ring = Ring::new(32)?;
/// let mut buffer1 = PinnedBuffer::with_capacity(1024);
/// let mut buffer2 = PinnedBuffer::with_capacity(1024);
///
/// let mut batch = Batch::new();
/// batch.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))?;
/// batch.add_operation(Operation::write().fd(1).buffer(buffer2.as_mut_slice()))?;
///
/// let results = ring.submit_batch(batch).await?;
/// println!("Batch completed with {} results", results.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Batch<'ring, 'buf> {
    operations: Vec<BatchOperation<'ring, 'buf>>,
    dependencies: HashMap<usize, Vec<usize>>, // operation_index -> depends_on_indices
    max_operations: usize,
}

/// A single operation within a batch.
#[derive(Debug)]
struct BatchOperation<'ring, 'buf> {
    operation: Operation<'ring, 'buf, Building>,
}

/// Result of a batch submission.
///
/// Contains the results of all operations in the batch, including both
/// successful completions and errors. Operations are indexed by their
/// position in the original batch.
#[derive(Debug)]
pub struct BatchResult {
    /// Results indexed by operation position in the batch
    pub results: Vec<OperationResult>,
    /// Number of operations that completed successfully
    pub successful_count: usize,
    /// Number of operations that failed
    pub failed_count: usize,
}

/// Result of a single operation within a batch.
#[derive(Debug, Clone)]
pub enum OperationResult {
    /// Operation completed successfully with the given result
    Success(i32),
    /// Operation failed with the given error
    Error(String), // Use String instead of std::io::Error for cloneability
    /// Operation was cancelled due to batch failure or dependency failure
    Cancelled,
}

/// Configuration for batch submission behavior.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Whether to continue submitting operations if one fails
    pub fail_fast: bool,
    /// Maximum number of operations to submit in a single batch
    pub max_batch_size: usize,
    /// Whether to enforce dependency ordering
    pub enforce_dependencies: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            fail_fast: false,
            max_batch_size: 256, // Reasonable default for most use cases
            enforce_dependencies: true,
        }
    }
}

impl<'ring, 'buf> Batch<'ring, 'buf> {
    /// Extract operations and dependencies for submission.
    ///
    /// This method consumes the batch and returns the operations and dependencies
    /// in a format suitable for submission.
    pub(crate) fn into_operations_and_dependencies(
        self,
    ) -> (Vec<Operation<'ring, 'buf, Building>>, HashMap<usize, Vec<usize>>) {
        let operations = self
            .operations
            .into_iter()
            .map(|batch_op| batch_op.operation)
            .collect();
        
        (operations, self.dependencies)
    }
    /// Create a new empty batch.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use safer_ring::Batch;
    /// let batch = Batch::new();
    /// assert_eq!(batch.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self::with_capacity(16) // Default capacity
    }

    /// Create a new batch with the specified initial capacity.
    ///
    /// Pre-allocating capacity can improve performance when the number
    /// of operations is known in advance.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Initial capacity for the batch
    ///
    /// # Example
    ///
    /// ```rust
    /// # use safer_ring::Batch;
    /// let batch = Batch::with_capacity(100);
    /// assert_eq!(batch.len(), 0);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            operations: Vec::with_capacity(capacity),
            dependencies: HashMap::new(),
            max_operations: 1024, // Reasonable upper limit
        }
    }

    /// Add an operation to the batch.
    ///
    /// Operations are added in the order they should be submitted.
    /// Returns the index of the added operation for use in dependency tracking.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation in Building state to add to the batch
    ///
    /// # Returns
    ///
    /// Returns the index of the operation in the batch.
    ///
    /// # Errors
    ///
    /// Returns an error if the batch is full or the operation is invalid.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Batch, Operation, PinnedBuffer};
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut batch = Batch::new();
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// let index = batch.add_operation(
    ///     Operation::read().fd(0).buffer(buffer.as_mut_slice())
    /// )?;
    /// assert_eq!(index, 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_operation(&mut self, operation: Operation<'ring, 'buf, Building>) -> Result<usize> {
        if self.operations.len() >= self.max_operations {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Batch is full (max {} operations)", self.max_operations),
            )));
        }

        // Validate the operation before adding
        operation.validate().map_err(|msg| {
            SaferRingError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, msg))
        })?;

        let index = self.operations.len();
        self.operations.push(BatchOperation {
            operation,
        });

        Ok(index)
    }

    /// Add an operation with user-provided data.
    ///
    /// The user data can be used to identify operations in the results.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation in Building state to add to the batch
    /// * `user_data` - User-provided identifier for this operation
    ///
    /// # Returns
    ///
    /// Returns the index of the operation in the batch.
    ///
    /// # Errors
    ///
    /// Returns an error if the batch is full or the operation is invalid.
    pub fn add_operation_with_data(
        &mut self,
        operation: Operation<'ring, 'buf, Building>,
        _user_data: u64,
    ) -> Result<usize> {
        if self.operations.len() >= self.max_operations {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Batch is full (max {} operations)", self.max_operations),
            )));
        }

        // Validate the operation before adding
        operation.validate().map_err(|msg| {
            SaferRingError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, msg))
        })?;

        let index = self.operations.len();
        self.operations.push(BatchOperation {
            operation,
        });

        Ok(index)
    }

    /// Add a dependency between operations.
    ///
    /// The dependent operation will not be submitted until all its dependencies
    /// have completed successfully. If any dependency fails, the dependent
    /// operation will be cancelled.
    ///
    /// # Arguments
    ///
    /// * `dependent` - Index of the operation that depends on others
    /// * `dependency` - Index of the operation that must complete first
    ///
    /// # Errors
    ///
    /// Returns an error if either index is invalid or if the dependency
    /// would create a cycle.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Batch, Operation, PinnedBuffer};
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut batch = Batch::new();
    /// let mut buffer1 = PinnedBuffer::with_capacity(1024);
    /// let mut buffer2 = PinnedBuffer::with_capacity(1024);
    ///
    /// let read_idx = batch.add_operation(
    ///     Operation::read().fd(0).buffer(buffer1.as_mut_slice())
    /// )?;
    /// let write_idx = batch.add_operation(
    ///     Operation::write().fd(1).buffer(buffer2.as_mut_slice())
    /// )?;
    ///
    /// // Write depends on read completing first
    /// batch.add_dependency(write_idx, read_idx)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_dependency(&mut self, dependent: usize, dependency: usize) -> Result<()> {
        if dependent >= self.operations.len() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Dependent operation index {} is out of bounds", dependent),
            )));
        }

        if dependency >= self.operations.len() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Dependency operation index {} is out of bounds", dependency),
            )));
        }

        if dependent == dependency {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Operation cannot depend on itself",
            )));
        }

        // Check for cycles (simple check - could be more sophisticated)
        if self.would_create_cycle(dependent, dependency) {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Dependency would create a cycle",
            )));
        }

        self.dependencies
            .entry(dependent)
            .or_insert_with(Vec::new)
            .push(dependency);

        Ok(())
    }

    /// Get the number of operations in the batch.
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Get the maximum number of operations this batch can hold.
    pub fn max_operations(&self) -> usize {
        self.max_operations
    }

    /// Check if the batch has circular dependencies.
    ///
    /// This method performs a topological sort to detect cycles in the
    /// dependency graph.
    pub fn has_circular_dependencies(&self) -> bool {
        // Implement cycle detection using DFS
        use std::collections::HashSet;
        
        fn has_cycle(
            node: usize,
            graph: &HashMap<usize, Vec<usize>>,
            visited: &mut HashSet<usize>,
            rec_stack: &mut HashSet<usize>,
        ) -> bool {
            visited.insert(node);
            rec_stack.insert(node);
            
            if let Some(neighbors) = graph.get(&node) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) && has_cycle(neighbor, graph, visited, rec_stack) {
                        return true;
                    } else if rec_stack.contains(&neighbor) {
                        return true;
                    }
                }
            }
            
            rec_stack.remove(&node);
            false
        }
        
        let mut visited = HashSet::new();
        
        for &node in self.dependencies.keys() {
            if !visited.contains(&node) {
                let mut rec_stack = HashSet::new();
                if has_cycle(node, &self.dependencies, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }
        
        false
    }

    /// Clear all operations and dependencies from the batch.
    pub fn clear(&mut self) {
        self.operations.clear();
        self.dependencies.clear();
    }

    /// Check if adding a dependency would create a cycle.
    fn would_create_cycle(&self, dependent: usize, new_dependency: usize) -> bool {
        // Simple cycle detection: check if new_dependency transitively depends on dependent
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![new_dependency];

        while let Some(current) = stack.pop() {
            if current == dependent {
                return true; // Found a cycle
            }

            if visited.contains(&current) {
                continue; // Already processed this node
            }
            visited.insert(current);

            // Add all dependencies of current to the stack
            if let Some(deps) = self.dependencies.get(&current) {
                stack.extend(deps.iter().copied());
            }
        }

        false
    }

    /// Get the operations in dependency order.
    ///
    /// Returns operations sorted such that dependencies come before dependents.
    /// Operations with no dependencies come first.
    pub(crate) fn dependency_order(&self) -> Result<Vec<usize>> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut temp_visited = std::collections::HashSet::new();

        // Topological sort using DFS
        for i in 0..self.operations.len() {
            if !visited.contains(&i) {
                self.visit_node(i, &mut visited, &mut temp_visited, &mut result)?;
            }
        }

        result.reverse(); // Reverse to get correct order
        Ok(result)
    }

    /// Visit a node in the dependency graph for topological sorting.
    fn visit_node(
        &self,
        node: usize,
        visited: &mut std::collections::HashSet<usize>,
        temp_visited: &mut std::collections::HashSet<usize>,
        result: &mut Vec<usize>,
    ) -> Result<()> {
        if temp_visited.contains(&node) {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Circular dependency detected",
            )));
        }

        if visited.contains(&node) {
            return Ok(());
        }

        temp_visited.insert(node);

        // Visit all dependencies first
        if let Some(deps) = self.dependencies.get(&node) {
            for &dep in deps {
                self.visit_node(dep, visited, temp_visited, result)?;
            }
        }

        temp_visited.remove(&node);
        visited.insert(node);
        result.push(node);

        Ok(())
    }
}

impl<'ring, 'buf> Default for Batch<'ring, 'buf> {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchResult {
    /// Create a new batch result.
    pub fn new(results: Vec<OperationResult>) -> Self {
        let successful_count = results
            .iter()
            .filter(|r| matches!(r, OperationResult::Success(_)))
            .count();
        let failed_count = results
            .iter()
            .filter(|r| matches!(r, OperationResult::Error(_)))
            .count();

        Self {
            results,
            successful_count,
            failed_count,
        }
    }

    /// Get the result for a specific operation by index.
    pub fn get(&self, index: usize) -> Option<&OperationResult> {
        self.results.get(index)
    }

    /// Check if all operations in the batch succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.failed_count == 0 && self.results.iter().all(|r| !matches!(r, OperationResult::Cancelled))
    }

    /// Check if any operations in the batch failed.
    pub fn any_failed(&self) -> bool {
        self.failed_count > 0
    }

    /// Get an iterator over successful results.
    pub fn successes(&self) -> impl Iterator<Item = (usize, i32)> + '_ {
        self.results.iter().enumerate().filter_map(|(i, r)| {
            if let OperationResult::Success(value) = r {
                Some((i, *value))
            } else {
                None
            }
        })
    }

    /// Get an iterator over failed results.
    pub fn failures(&self) -> impl Iterator<Item = (usize, &String)> + '_ {
        self.results.iter().enumerate().filter_map(|(i, r)| {
            if let OperationResult::Error(error) = r {
                Some((i, error))
            } else {
                None
            }
        })
    }
}

impl OperationResult {
    /// Check if this result represents a successful operation.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Check if this result represents a failed operation.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Check if this result represents a cancelled operation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Get the success value if this result is successful.
    pub fn success_value(&self) -> Option<i32> {
        if let Self::Success(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Get the error if this result is an error.
    pub fn error(&self) -> Option<&String> {
        if let Self::Error(error) = self {
            Some(error)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::Operation;
    use crate::PinnedBuffer;

    #[test]
    fn new_batch_is_empty() {
        let batch: Batch<'_, '_> = Batch::new();
        assert_eq!(batch.len(), 0);
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_with_capacity() {
        let batch: Batch<'_, '_> = Batch::with_capacity(100);
        assert_eq!(batch.len(), 0);
        assert!(batch.is_empty());
    }

    #[test]
    fn add_operation() {
        let mut batch = Batch::new();
        let mut buffer = PinnedBuffer::with_capacity(1024);

        let operation = Operation::read().fd(0).buffer(buffer.as_mut_slice());
        let index = batch.add_operation(operation).unwrap();

        assert_eq!(index, 0);
        assert_eq!(batch.len(), 1);
        assert!(!batch.is_empty());
    }

    #[test]
    fn add_multiple_operations() {
        let mut batch = Batch::new();
        let mut buffer1 = PinnedBuffer::with_capacity(1024);
        let mut buffer2 = PinnedBuffer::with_capacity(1024);

        let op1 = Operation::read().fd(0).buffer(buffer1.as_mut_slice());
        let op2 = Operation::write().fd(1).buffer(buffer2.as_mut_slice());

        let idx1 = batch.add_operation(op1).unwrap();
        let idx2 = batch.add_operation(op2).unwrap();

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn add_operation_with_user_data() {
        let mut batch = Batch::new();
        let mut buffer = PinnedBuffer::with_capacity(1024);

        let operation = Operation::read().fd(0).buffer(buffer.as_mut_slice());
        let index = batch.add_operation_with_data(operation, 42).unwrap();

        assert_eq!(index, 0);
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn add_dependency() {
        let mut batch = Batch::new();
        let mut buffer1 = PinnedBuffer::with_capacity(1024);
        let mut buffer2 = PinnedBuffer::with_capacity(1024);

        let op1 = Operation::read().fd(0).buffer(buffer1.as_mut_slice());
        let op2 = Operation::write().fd(1).buffer(buffer2.as_mut_slice());

        let idx1 = batch.add_operation(op1).unwrap();
        let idx2 = batch.add_operation(op2).unwrap();

        batch.add_dependency(idx2, idx1).unwrap();
    }

    #[test]
    fn self_dependency_error() {
        let mut batch = Batch::new();
        let mut buffer = PinnedBuffer::with_capacity(1024);

        let operation = Operation::read().fd(0).buffer(buffer.as_mut_slice());
        let index = batch.add_operation(operation).unwrap();

        let result = batch.add_dependency(index, index);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_dependency_indices() {
        let mut batch = Batch::new();
        let mut buffer = PinnedBuffer::with_capacity(1024);

        let operation = Operation::read().fd(0).buffer(buffer.as_mut_slice());
        let _index = batch.add_operation(operation).unwrap();

        // Test invalid dependent index
        let result = batch.add_dependency(10, 0);
        assert!(result.is_err());

        // Test invalid dependency index
        let result = batch.add_dependency(0, 10);
        assert!(result.is_err());
    }

    #[test]
    fn clear_batch() {
        let mut batch = Batch::new();
        let mut buffer = PinnedBuffer::with_capacity(1024);

        let operation = Operation::read().fd(0).buffer(buffer.as_mut_slice());
        let _index = batch.add_operation(operation).unwrap();

        assert_eq!(batch.len(), 1);

        batch.clear();
        assert_eq!(batch.len(), 0);
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_result_creation() {
        let results = vec![
            OperationResult::Success(100),
            OperationResult::Error("File not found".to_string()),
            OperationResult::Cancelled,
        ];

        let batch_result = BatchResult::new(results);
        assert_eq!(batch_result.successful_count, 1);
        assert_eq!(batch_result.failed_count, 1);
        assert!(!batch_result.all_succeeded());
        assert!(batch_result.any_failed());
    }

    #[test]
    fn operation_result_methods() {
        let success = OperationResult::Success(42);
        assert!(success.is_success());
        assert!(!success.is_error());
        assert!(!success.is_cancelled());
        assert_eq!(success.success_value(), Some(42));
        assert!(success.error().is_none());

        let error = OperationResult::Error("Not found".to_string());
        assert!(!error.is_success());
        assert!(error.is_error());
        assert!(!error.is_cancelled());
        assert_eq!(error.success_value(), None);
        assert!(error.error().is_some());

        let cancelled = OperationResult::Cancelled;
        assert!(!cancelled.is_success());
        assert!(!cancelled.is_error());
        assert!(cancelled.is_cancelled());
        assert_eq!(cancelled.success_value(), None);
        assert!(cancelled.error().is_none());
    }

    #[test]
    fn dependency_order_simple() {
        let mut batch = Batch::new();
        let mut buffer1 = PinnedBuffer::with_capacity(1024);
        let mut buffer2 = PinnedBuffer::with_capacity(1024);
        let mut buffer3 = PinnedBuffer::with_capacity(1024);

        let op1 = Operation::read().fd(0).buffer(buffer1.as_mut_slice());
        let op2 = Operation::write().fd(1).buffer(buffer2.as_mut_slice());
        let op3 = Operation::read().fd(2).buffer(buffer3.as_mut_slice());

        let idx1 = batch.add_operation(op1).unwrap();
        let idx2 = batch.add_operation(op2).unwrap();
        let idx3 = batch.add_operation(op3).unwrap();

        // op2 depends on op1, op3 depends on op2
        batch.add_dependency(idx2, idx1).unwrap();
        batch.add_dependency(idx3, idx2).unwrap();

        let order = batch.dependency_order().unwrap();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn cycle_detection() {
        let mut batch = Batch::new();
        let mut buffer1 = PinnedBuffer::with_capacity(1024);
        let mut buffer2 = PinnedBuffer::with_capacity(1024);

        let op1 = Operation::read().fd(0).buffer(buffer1.as_mut_slice());
        let op2 = Operation::write().fd(1).buffer(buffer2.as_mut_slice());

        let idx1 = batch.add_operation(op1).unwrap();
        let idx2 = batch.add_operation(op2).unwrap();

        // Create a cycle: op1 depends on op2, op2 depends on op1
        batch.add_dependency(idx1, idx2).unwrap();
        let result = batch.add_dependency(idx2, idx1);
        assert!(result.is_err());
    }
}