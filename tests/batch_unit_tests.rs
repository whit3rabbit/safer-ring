//! Unit tests for batch operations that don't require async execution.
//!
//! These tests focus on batch data structures and logic that work
//! identically across all platforms. They don't perform actual I/O
//! operations, so they're safe to run on macOS, Linux, and Windows.

use safer_ring::{Batch, BatchConfig};

#[test]
fn test_batch_creation() {
    let batch = Batch::new();
    assert_eq!(batch.len(), 0);
    assert!(batch.is_empty());
    assert!(batch.max_operations() > 0);
}

#[test]
fn test_batch_config() {
    let config = BatchConfig::default();
    assert_eq!(config.max_batch_size, 256);
    assert!(!config.fail_fast);
    assert!(config.enforce_dependencies);

    let custom_config = BatchConfig {
        max_batch_size: 10,
        fail_fast: true,
        enforce_dependencies: false,
    };
    assert_eq!(custom_config.max_batch_size, 10);
    assert!(custom_config.fail_fast);
    assert!(!custom_config.enforce_dependencies);
}

#[test]
fn test_batch_dependency_validation() {
    let batch = Batch::new();

    // Empty batch has no circular dependencies
    assert!(!batch.has_circular_dependencies());
}

#[test]
fn test_batch_clear() {
    let mut batch = Batch::new();

    // Initially empty
    assert_eq!(batch.len(), 0);

    // Clear should work on empty batch
    batch.clear();
    assert_eq!(batch.len(), 0);
    assert!(batch.is_empty());
}

#[test]
fn test_batch_max_operations() {
    let batch = Batch::new();
    let max_ops = batch.max_operations();

    // Should have reasonable bounds
    assert!(max_ops > 0);
    assert!(max_ops <= 2048); // Reasonable upper bound
}
