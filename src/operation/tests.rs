//! Comprehensive tests for operation state management.

use super::*;
use std::pin::Pin;

// Compile-time assertions to ensure our types have the expected properties
#[cfg(test)]
mod compile_time_tests {
    use super::*;
    use static_assertions::*;

    // Ensure operations are Send when appropriate
    assert_impl_all!(Operation<'static, 'static, Building>: Send);
    assert_impl_all!(Operation<'static, 'static, Submitted>: Send);
    assert_impl_all!(Operation<'static, 'static, Completed<i32>>: Send);

    // Ensure state types are the expected size for optimal memory layout
    assert_eq_size!(Building, ()); // Zero-sized type
    assert_eq_size!(Submitted, u64); // Just the operation ID
}

#[test]
fn test_operation_creation() {
    let op = Operation::<'_, '_, Building>::new();
    assert_eq!(op.get_fd(), -1);
    assert_eq!(op.get_offset(), 0);
    assert_eq!(op.get_type(), OperationType::Read);
    assert!(!op.has_buffer());
}

#[test]
fn test_operation_builder_methods() {
    let op = Operation::read().fd(5).offset(1024);

    assert_eq!(op.get_fd(), 5);
    assert_eq!(op.get_offset(), 1024);
    assert_eq!(op.get_type(), OperationType::Read);
}

#[test]
fn test_operation_types() {
    assert_eq!(Operation::read().get_type(), OperationType::Read);
    assert_eq!(Operation::write().get_type(), OperationType::Write);
    assert_eq!(Operation::accept().get_type(), OperationType::Accept);
    assert_eq!(Operation::send().get_type(), OperationType::Send);
    assert_eq!(Operation::recv().get_type(), OperationType::Recv);
}

#[test]
fn test_operation_validation() {
    // Invalid: no fd set
    let op = Operation::read();
    assert!(op.validate().is_err());

    // Invalid: fd set but no buffer for I/O operation
    let op = Operation::read().fd(0);
    assert!(op.validate().is_err());

    // Valid: accept operation doesn't need buffer
    let op = Operation::accept().fd(0);
    assert!(op.validate().is_ok());
}

#[test]
fn test_operation_with_buffer() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());

    let op = Operation::read().fd(0).buffer(pinned);

    assert!(op.has_buffer());
    assert!(op.validate().is_ok());
}

#[test]
fn test_state_transitions() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());

    let op = Operation::read().fd(0).buffer(pinned);

    // Building -> Submitted
    let submitted = op.submit_with_id(42).unwrap();
    assert_eq!(submitted.id(), 42);
    assert_eq!(submitted.fd(), 0);

    // Submitted -> Completed
    let completed = submitted.complete_with_result(100i32);
    assert_eq!(*completed.result(), 100);

    // Completed -> extracted
    let (result, _buffer) = completed.into_result();
    assert_eq!(result, 100);
}

#[test]
fn test_operation_properties() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());

    let submitted = Operation::write()
        .fd(1)
        .offset(512)
        .buffer(pinned)
        .submit_with_id(123)
        .unwrap();

    assert_eq!(submitted.id(), 123);
    assert_eq!(submitted.fd(), 1);
    assert_eq!(submitted.offset(), 512);
    assert_eq!(submitted.op_type(), OperationType::Write);
}

#[test]
fn test_completed_operation_access() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());

    let completed = Operation::read()
        .fd(0)
        .buffer(pinned)
        .submit_with_id(1)
        .unwrap()
        .complete_with_result("test result".to_string());

    // Can access result by reference
    assert_eq!(completed.result(), "test result");
    assert_eq!(completed.fd(), 0);
    assert_eq!(completed.op_type(), OperationType::Read);

    // Can extract result by value
    let (result, _buffer) = completed.into_result();
    assert_eq!(result, "test result");
}

#[test]
fn test_operation_state_machine_invariants() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());

    let building = Operation::read().fd(0).buffer(pinned);

    // Building -> Submitted
    let submitted = building.submit_with_id(1).unwrap();
    assert_eq!(submitted.id(), 1);

    // Submitted -> Completed
    let completed = submitted.complete_with_result(42);
    assert_eq!(*completed.result(), 42);

    // Completed -> extracted
    let (result, _) = completed.into_result();
    assert_eq!(result, 42);
}

#[test]
fn test_operation_validation_rules() {
    // Missing FD
    let op = Operation::read();
    assert!(op.validate().is_err());

    // Missing buffer for I/O operations
    let op = Operation::read().fd(0);
    assert!(op.validate().is_err());

    let op = Operation::write().fd(0);
    assert!(op.validate().is_err());

    // Accept doesn't need buffer
    let op = Operation::accept().fd(0);
    assert!(op.validate().is_ok());
}

#[test]
fn test_builder_pattern_fluency() {
    // Test that the builder pattern works fluently
    let op = Operation::write().fd(1).offset(1024);

    assert_eq!(op.get_fd(), 1);
    assert_eq!(op.get_offset(), 1024);
    assert_eq!(op.get_type(), OperationType::Write);
}

#[test]
fn test_default_implementation() {
    let op1 = Operation::<'_, '_, Building>::default();
    let op2 = Operation::<'_, '_, Building>::new();

    // Both should have the same initial state
    assert_eq!(op1.get_fd(), op2.get_fd());
    assert_eq!(op1.get_offset(), op2.get_offset());
    assert_eq!(op1.get_type(), op2.get_type());
    assert_eq!(op1.has_buffer(), op2.has_buffer());
}
