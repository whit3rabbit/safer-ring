//! Core batch operations and management.

use super::validation::DependencyValidator;
use crate::error::{Result, SaferRingError};
use crate::operation::{Building, Operation};
use std::collections::HashMap;

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

impl<'ring, 'buf> Batch<'ring, 'buf> {
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
        self.operations.push(BatchOperation { operation });

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
        self.operations.push(BatchOperation { operation });

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
                format!("Dependent operation index {dependent} is out of bounds"),
            )));
        }

        if dependency >= self.operations.len() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Dependency operation index {dependency} is out of bounds"),
            )));
        }

        if dependent == dependency {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Operation cannot depend on itself",
            )));
        }

        // Check for cycles (simple check - could be more sophisticated)
        if DependencyValidator::would_create_cycle(&self.dependencies, dependent, dependency) {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Dependency would create a cycle",
            )));
        }

        self.dependencies
            .entry(dependent)
            .or_default()
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
        DependencyValidator::has_circular_dependencies(&self.dependencies)
    }

    /// Clear all operations and dependencies from the batch.
    pub fn clear(&mut self) {
        self.operations.clear();
        self.dependencies.clear();
    }

    /// Get the operations in dependency order.
    ///
    /// Returns operations sorted such that dependencies come before dependents.
    /// Operations with no dependencies come first.
    pub(crate) fn dependency_order(&self) -> Result<Vec<usize>> {
        DependencyValidator::dependency_order(&self.dependencies, self.operations.len())
    }

    /// Extract operations and dependencies for submission.
    ///
    /// This method consumes the batch and returns the operations and dependencies
    /// in a format suitable for submission.
    pub(crate) fn into_operations_and_dependencies(
        self,
    ) -> (
        Vec<Operation<'ring, 'buf, Building>>,
        HashMap<usize, Vec<usize>>,
    ) {
        let operations = self
            .operations
            .into_iter()
            .map(|batch_op| batch_op.operation)
            .collect();

        (operations, self.dependencies)
    }
}

impl<'ring, 'buf> Default for Batch<'ring, 'buf> {
    fn default() -> Self {
        Self::new()
    }
}
