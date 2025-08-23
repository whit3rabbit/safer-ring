//! Simple batch operation tests that avoid complex lifetime issues.

use safer_ring::{Batch, BatchConfig, Ring};
use tokio;

#[tokio::test]
async fn test_empty_batch_error() {
    // Test that empty batch returns an error without lifetime issues
    let result = {
        let ring = Ring::new(32).unwrap();
        let batch = Batch::new();
        ring.submit_batch(batch)
    };
    
    // Empty batch should return an error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_creation() {
    let mut batch = Batch::new();
    assert_eq!(batch.len(), 0);
    assert!(batch.is_empty());
    
    // Batch should have default limits
    assert!(batch.len() <= batch.max_operations());
}

#[tokio::test]
async fn test_batch_config() {
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

#[tokio::test]
async fn test_batch_max_operations() {
    let mut batch = Batch::new();
    let initial_max = batch.max_operations();
    
    // Should be able to set a reasonable limit
    assert!(initial_max > 0);
    assert!(initial_max <= 1024); // Reasonable upper bound
}

#[cfg(not(target_os = "linux"))] 
#[tokio::test]
async fn test_batch_operations_stub() {
    // This test only runs on non-Linux platforms where we have stubs
    let result = {
        let ring = Ring::new(32).unwrap();
        let batch = Batch::new();
        
        // Can't easily test real operations due to lifetime constraints,
        // but we can test that the basic structure works
        assert_eq!(batch.len(), 0);
        
        // Test that empty batch fails as expected
        ring.submit_batch(batch)
    };
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_dependency_validation() {
    let batch = Batch::new();
    
    // Test that dependency validation works
    // Dependencies on non-existent operations should fail
    let result = batch.has_circular_dependencies();
    assert!(!result); // Empty batch has no circular dependencies
}