//! Batch operation support for efficient submission of multiple operations.
//!
//! This module provides functionality to submit multiple operations efficiently
//! in a single batch, with support for operation dependencies and proper error
//! handling for partial batch failures.
//!
//! # Module Organization
//!
//! - `core` - Main Batch struct and core operations
//! - `result` - BatchResult and OperationResult types  
//! - `config` - BatchConfig configuration type
//! - `validation` - Dependency validation and cycle detection

// Internal modules

/// Configuration types for controlling batch submission behavior.
///
/// Contains `BatchConfig` for fine-tuning how batches are processed,
/// including error handling strategies, size limits, and dependency enforcement.
pub mod config;

/// Core batch functionality and the main `Batch` type.
///
/// Contains the `Batch` struct for collecting operations and managing
/// dependencies, along with methods for adding operations and configuring
/// batch behavior.
pub mod core;

/// Result types for batch operations.
///
/// Contains `BatchResult` and `OperationResult` types that represent
/// the outcomes of batch submissions, including success/failure information
/// and detailed per-operation results.
pub mod result;
mod validation;

// Re-export public types for backward compatibility
pub use config::BatchConfig;
pub use core::Batch;
pub use result::{BatchResult, OperationResult};

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
