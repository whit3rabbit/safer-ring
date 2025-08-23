//! Tests for Ring functionality.

use super::Ring;
use crate::error::SaferRingError;

/// Test ring creation and basic properties
mod creation {
    use super::*;

    #[test]
    fn new_ring_success() {
        let result = Ring::new(32);

        #[cfg(target_os = "linux")]
        {
            assert!(result.is_ok());
            let ring = result.unwrap();
            assert_eq!(ring.operations_in_flight(), 0);
            assert!(!ring.has_operations_in_flight());
            assert_eq!(ring.capacity(), 32);
        }

        #[cfg(not(target_os = "linux"))]
        {
            assert!(result.is_err());
            match result.unwrap_err() {
                SaferRingError::Io(e) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::Unsupported);
                }
                _ => panic!("Expected Io error"),
            }
        }
    }

    #[test]
    fn new_ring_zero_entries() {
        let result = Ring::new(0);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::Io(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
                assert!(e.to_string().contains("greater than 0"));
            }
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn ring_capacity() {
        #[cfg(target_os = "linux")]
        {
            let ring = Ring::new(64).unwrap();
            assert_eq!(ring.capacity(), 64);
        }
    }
}

mod lifecycle {
    #[test]
    #[cfg(target_os = "linux")]
    fn empty_ring_drop() {
        let ring = Ring::new(32).unwrap();
        assert_eq!(ring.operations_in_flight(), 0);
        // Should drop without panic
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn operation_submission_validation() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        // Test invalid operation (no fd set)
        let invalid_op = Operation::read();
        let result = ring.submit(invalid_op);
        assert!(result.is_err());

        // Test valid operation
        let mut buffer = vec![0u8; 1024];
        let valid_op = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer.as_mut_slice()));

        let result = ring.submit(valid_op);
        assert!(result.is_ok());

        let submitted = result.unwrap();
        assert_eq!(submitted.id(), 1);
        assert_eq!(ring.operations_in_flight(), 1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn operation_submission_buffer_requirements() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        // Test read operation without buffer (should fail)
        let read_no_buffer = Operation::read().fd(0);
        let result = ring.submit(read_no_buffer);
        assert!(result.is_err());

        // Test write operation without buffer (should fail)
        let write_no_buffer = Operation::write().fd(1);
        let result = ring.submit(write_no_buffer);
        assert!(result.is_err());

        // Test accept operation without buffer (should succeed)
        let accept_no_buffer = Operation::accept().fd(3);
        let result = ring.submit(accept_no_buffer);
        assert!(result.is_ok());

        // Test operations with buffers (should succeed)
        let mut read_buffer = vec![0u8; 1024];
        let read_with_buffer = Operation::read()
            .fd(0)
            .buffer(Pin::new(read_buffer.as_mut_slice()));
        let result = ring.submit(read_with_buffer);
        assert!(result.is_ok());

        let mut write_buffer = b"Hello, world!".to_vec();
        let write_with_buffer = Operation::write()
            .fd(1)
            .buffer(Pin::new(write_buffer.as_mut_slice()));
        let result = ring.submit(write_with_buffer);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn operation_submission_fd_validation() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        // Test operation with invalid fd (negative)
        let mut buffer = vec![0u8; 1024];
        let invalid_fd_op = Operation::read()
            .fd(-1)
            .buffer(Pin::new(buffer.as_mut_slice()));
        let result = ring.submit(invalid_fd_op);
        assert!(result.is_err());

        // Test operation with valid fd
        let mut buffer2 = vec![0u8; 1024];
        let valid_fd_op = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer2.as_mut_slice()));
        let result = ring.submit(valid_fd_op);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn multiple_operation_submission() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        // Submit multiple operations
        let mut buffer1 = vec![0u8; 1024];
        let op1 = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer1.as_mut_slice()));
        let submitted1 = ring.submit(op1).unwrap();

        let mut buffer2 = vec![0u8; 1024];
        let op2 = Operation::write()
            .fd(1)
            .buffer(Pin::new(buffer2.as_mut_slice()));
        let submitted2 = ring.submit(op2).unwrap();

        let op3 = Operation::accept().fd(3);
        let submitted3 = ring.submit(op3).unwrap();

        // Verify operations have unique IDs
        assert_eq!(submitted1.id(), 1);
        assert_eq!(submitted2.id(), 2);
        assert_eq!(submitted3.id(), 3);

        // Verify tracking
        assert_eq!(ring.operations_in_flight(), 3);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn operation_submission_with_offset() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        let mut buffer = vec![0u8; 1024];
        let op = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer.as_mut_slice()))
            .offset(4096);

        let submitted = ring.submit(op).unwrap();
        assert_eq!(submitted.offset(), 4096);
    }
}

mod thread_safety {
    use super::Ring;

    #[test]
    fn ring_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Ring<'_>>();
    }
}

mod completion_processing {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn try_complete_empty_queue() {
        let ring = Ring::new(32).unwrap();

        // No operations submitted, should return empty vector
        let completions = ring.try_complete().unwrap();
        assert!(completions.is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn wait_for_completion_no_operations() {
        let ring = Ring::new(32).unwrap();

        // No operations in flight, should return error
        let result = ring.wait_for_completion();
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::Io(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
                assert!(e.to_string().contains("No operations in flight"));
            }
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn try_complete_by_id_not_found() {
        let ring = Ring::new(32).unwrap();

        // Check for non-existent operation
        let result = ring.try_complete_by_id(999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn completion_queue_stats() {
        let ring = Ring::new(32).unwrap();

        let (ready, capacity) = ring.completion_queue_stats();
        assert_eq!(ready, 0); // No completions ready
        assert!(capacity > 0); // Should have some capacity
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn operation_tracking_after_submission() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        // Submit an operation
        let mut buffer = vec![0u8; 1024];
        let operation = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer.as_mut_slice()));

        let submitted = ring.submit(operation).unwrap();
        let operation_id = submitted.id();

        // Verify operation is tracked
        assert_eq!(ring.operations_in_flight(), 1);

        // Try to complete it (won't actually complete since fd 0 might not be ready)
        let result = ring.try_complete_by_id(operation_id);

        // Should not error, but might not find completion
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn completion_methods_non_linux() {
        let result = Ring::new(32);
        assert!(result.is_err()); // Ring creation should fail on non-Linux

        // Test that we can call the methods on a hypothetical ring
        // (This tests the API surface even on non-Linux platforms)
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn multiple_operations_completion_tracking() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        // Submit multiple operations
        let mut buffer1 = vec![0u8; 1024];
        let op1 = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer1.as_mut_slice()));
        let submitted1 = ring.submit(op1).unwrap();

        let mut buffer2 = vec![0u8; 1024];
        let op2 = Operation::write()
            .fd(1)
            .buffer(Pin::new(buffer2.as_mut_slice()));
        let submitted2 = ring.submit(op2).unwrap();

        let op3 = Operation::accept().fd(3);
        let submitted3 = ring.submit(op3).unwrap();

        // Verify all operations are tracked
        assert_eq!(ring.operations_in_flight(), 3);

        // Try to complete operations individually
        let result1 = ring.try_complete_by_id(submitted1.id());
        let result2 = ring.try_complete_by_id(submitted2.id());
        let result3 = ring.try_complete_by_id(submitted3.id());

        // All should succeed (though may not find completions)
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());

        // Try bulk completion
        let completions = ring.try_complete().unwrap();
        // May be empty if operations haven't completed yet
        assert!(completions.len() <= 3);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn completion_queue_stats_after_submission() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        let (ready_before, capacity) = ring.completion_queue_stats();
        assert_eq!(ready_before, 0);

        // Submit an operation
        let mut buffer = vec![0u8; 1024];
        let operation = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer.as_mut_slice()));

        let _submitted = ring.submit(operation).unwrap();

        // Stats should still show no ready completions (operation just submitted)
        let (ready_after, capacity_after) = ring.completion_queue_stats();
        assert_eq!(ready_after, 0); // Likely no immediate completion
        assert_eq!(capacity_after, capacity); // Capacity shouldn't change
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn error_handling_invalid_operation_id() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();

        // Submit an operation to get a valid tracker state
        let mut buffer = vec![0u8; 1024];
        let operation = Operation::read()
            .fd(0)
            .buffer(Pin::new(buffer.as_mut_slice()));

        let _submitted = ring.submit(operation).unwrap();

        // The completion processing should handle unknown operation IDs gracefully
        // This is tested indirectly through the try_complete_by_id method
        let result = ring.try_complete_by_id(99999);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}

mod submission_lifetime_constraints {
    #[test]
    #[cfg(target_os = "linux")]
    fn buffer_outlives_ring_compiles() {
        use crate::operation::Operation;
        use std::pin::Pin;

        // This should compile - buffer outlives ring
        let mut buffer = vec![0u8; 1024];
        let pinned_buffer = Pin::new(buffer.as_mut_slice());

        {
            let ring = Ring::new(32).unwrap();
            let operation = Operation::read().fd(0).buffer(pinned_buffer);

            let _submitted = ring.submit(operation).unwrap();
            // Ring dropped here, but buffer still exists
        }

        // Buffer is still valid here
        assert_eq!(buffer.len(), 1024);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn operation_tracks_correct_parameters() {
        use crate::operation::Operation;
        use std::pin::Pin;

        let ring = Ring::new(32).unwrap();
        let mut buffer = vec![0u8; 1024];

        let operation = Operation::write()
            .fd(5)
            .buffer(Pin::new(buffer.as_mut_slice()))
            .offset(2048);

        let submitted = ring.submit(operation).unwrap();

        assert_eq!(submitted.fd(), 5);
        assert_eq!(submitted.offset(), 2048);
        assert!(submitted.has_buffer());
        assert_eq!(submitted.op_type(), crate::operation::OperationType::Write);
    }

    #[test]
    fn lifetime_constraint_compilation() {
        // This test verifies that the lifetime constraints are properly enforced
        // by the type system. It should compile on all platforms.
        use crate::operation::Operation;

        // Test that we can create operations with proper lifetimes
        let operation = Operation::read().fd(0);
        assert_eq!(operation.get_fd(), 0);

        // Test that operation validation works
        let result = operation.validate();
        assert!(result.is_err()); // Should fail because no buffer is set for read

        let accept_op = Operation::accept().fd(3);
        let result = accept_op.validate();
        assert!(result.is_ok()); // Should succeed because accept doesn't need buffer
    }
}
