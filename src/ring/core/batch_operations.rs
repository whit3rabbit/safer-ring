//! Batch operation submission and management for the Ring.

use super::Ring;
use crate::error::{Result, SaferRingError};
use crate::future::{BatchFuture, StandaloneBatchFuture};
use crate::ring::batch::{Batch, BatchConfig};

impl<'ring> Ring<'ring> {
    /// Submit a batch of operations efficiently.
    ///
    /// This method submits multiple operations in a single batch, which can
    /// significantly improve performance by reducing the number of system calls.
    /// Operations can have dependencies and will be submitted in the correct order.
    ///
    /// # Arguments
    ///
    /// * `batch` - Batch of operations to submit
    ///
    /// # Returns
    ///
    /// Returns a BatchFuture that resolves to the results of all operations.
    ///
    /// # Errors
    ///
    /// Returns an error if batch validation fails or submission fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, Batch, Operation, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut batch = Batch::new();
    /// let mut buffer1 = PinnedBuffer::with_capacity(1024);
    /// let mut buffer2 = PinnedBuffer::with_capacity(1024);
    ///
    /// batch.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))?;
    /// batch.add_operation(Operation::write().fd(1).buffer(buffer2.as_mut_slice()))?;
    ///
    /// let results = ring.submit_batch(batch).await?;
    /// println!("Batch completed with {} operations", results.results.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit_batch<'buf>(
        &'ring mut self,
        batch: Batch<'ring, 'buf>,
    ) -> Result<BatchFuture<'ring>>
    where
        'buf: 'ring, // Buffer must outlive ring operations
    {
        self.submit_batch_with_config(batch, BatchConfig::default())
    }

    /// Submit a batch of operations with custom configuration.
    ///
    /// This method provides more control over batch submission behavior,
    /// including fail-fast mode and dependency handling.
    ///
    /// # Arguments
    ///
    /// * `batch` - Batch of operations to submit
    /// * `config` - Configuration for batch submission behavior
    ///
    /// # Returns
    ///
    /// Returns a BatchFuture that resolves to the results of all operations.
    ///
    /// # Errors
    ///
    /// Returns an error if batch validation fails or submission fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, Batch, BatchConfig, Operation, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut batch = Batch::new();
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// batch.add_operation(Operation::read().fd(0).buffer(buffer.as_mut_slice()))?;
    ///
    /// let config = BatchConfig {
    ///     fail_fast: true,
    ///     max_batch_size: 100,
    ///     enforce_dependencies: true,
    /// };
    ///
    /// let results = ring.submit_batch_with_config(batch, config).await?;
    /// println!("Batch completed");
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit_batch_with_config<'buf>(
        &'ring mut self,
        batch: Batch<'ring, 'buf>,
        config: BatchConfig,
    ) -> Result<BatchFuture<'ring>>
    where
        'buf: 'ring, // Buffer must outlive ring operations
    {
        if batch.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot submit empty batch",
            )));
        }

        if batch.len() > config.max_batch_size {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Batch size {} exceeds maximum {}",
                    batch.len(),
                    config.max_batch_size
                ),
            )));
        }

        // Get operations in dependency order if dependencies are enforced
        let _submission_order = if config.enforce_dependencies {
            batch.dependency_order()?
        } else {
            (0..batch.len()).collect()
        };

        // Submit operations that have no dependencies first
        let mut _submitted_operations: Vec<Option<()>> = vec![None; batch.len()];
        let mut dependencies = std::collections::HashMap::new();

        // Extract operations and dependencies from the batch
        let (operations, batch_dependencies) = batch.into_operations_and_dependencies();

        // Submit all operations and collect their IDs
        let mut operation_ids = vec![None; operations.len()];
        for (index, operation) in operations.into_iter().enumerate() {
            let submitted = self.submit(operation)?;
            operation_ids[index] = Some(submitted.id());

            if batch_dependencies.contains_key(&index) {
                dependencies.insert(index, batch_dependencies[&index].clone());
            }
        }

        // Create the batch future
        Ok(BatchFuture::new(
            operation_ids,
            dependencies,
            self,
            self.waker_registry.clone(),
            config.fail_fast,
        ))
    }

    /// Submit a batch of operations and return a standalone future.
    ///
    /// This method provides an alternative to `submit_batch` that returns a future
    /// which doesn't hold a mutable reference to the Ring. This solves lifetime
    /// constraint issues that can occur when composing batch operations with other
    /// operations on the same ring.
    ///
    /// The returned future must be polled using a custom polling method that
    /// provides Ring access when needed, but doesn't hold the Ring reference.
    ///
    /// # Arguments
    ///
    /// * `batch` - Batch of operations to submit
    ///
    /// # Returns
    ///
    /// Returns a StandaloneBatchFuture that can be polled independently of Ring lifetime.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, Batch, Operation, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut batch = Batch::new();
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// batch.add_operation(Operation::read().fd(0).buffer(buffer.as_mut_slice()))?;
    ///
    /// // This doesn't hold a mutable reference to Ring
    /// let mut batch_future = ring.submit_batch_standalone(batch)?;
    /// 
    /// // Can perform other operations on ring here
    /// // let other_result = ring.read(...).await?;
    ///
    /// // Poll the batch future manually providing Ring access
    /// let results = std::future::poll_fn(|cx| {
    ///     batch_future.poll_with_ring(&mut ring, cx)
    /// }).await?;
    ///
    /// println!("Batch completed with {} operations", results.results.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit_batch_standalone<'buf>(
        &mut self,
        batch: Batch<'ring, 'buf>,
    ) -> Result<StandaloneBatchFuture>
    where
        'buf: 'ring, // Buffer must outlive ring operations
    {
        self.submit_batch_standalone_with_config(batch, BatchConfig::default())
    }

    /// Submit a batch of operations with custom configuration and return a standalone future.
    ///
    /// This provides the same functionality as `submit_batch_with_config` but returns
    /// a standalone future that doesn't hold Ring references.
    pub fn submit_batch_standalone_with_config<'buf>(
        &mut self,
        batch: Batch<'ring, 'buf>,
        config: BatchConfig,
    ) -> Result<StandaloneBatchFuture>
    where
        'buf: 'ring, // Buffer must outlive ring operations
    {
        if batch.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot submit empty batch",
            )));
        }

        if batch.len() > config.max_batch_size {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Batch size {} exceeds maximum {}",
                    batch.len(),
                    config.max_batch_size
                ),
            )));
        }

        // Get operations in dependency order if dependencies are enforced
        let _submission_order = if config.enforce_dependencies {
            batch.dependency_order()?
        } else {
            (0..batch.len()).collect()
        };

        // Submit operations that have no dependencies first
        let mut _submitted_operations: Vec<Option<()>> = vec![None; batch.len()];
        let mut dependencies = std::collections::HashMap::new();

        // Extract operations and dependencies from the batch
        let (operations, batch_dependencies) = batch.into_operations_and_dependencies();

        // Submit all operations and collect their IDs
        let mut operation_ids = vec![None; operations.len()];
        for (index, operation) in operations.into_iter().enumerate() {
            let submitted = self.submit(operation)?;
            operation_ids[index] = Some(submitted.id());

            if batch_dependencies.contains_key(&index) {
                dependencies.insert(index, batch_dependencies[&index].clone());
            }
        }

        // Create the standalone batch future
        Ok(StandaloneBatchFuture::new(
            operation_ids,
            dependencies,
            self.waker_registry.clone(),
            config.fail_fast,
        ))
    }
}
