//! Operation state types and state machine definitions.
//!
//! This module defines the state types used in the Operation state machine
//! and the sealed OperationState trait that ensures type safety.

/// Marker trait for operation states.
///
/// This trait is sealed to prevent external implementations and ensure
/// that only valid states can be used with operations.
pub trait OperationState: private::Sealed {}

/// Operation is being built and can be configured.
///
/// In this state, the operation can have its parameters modified but cannot
/// be submitted to the kernel. This prevents incomplete operations from being
/// submitted accidentally.
#[derive(Debug, Clone, Copy)]
pub struct Building;

/// Operation has been submitted and is in flight.
///
/// In this state, the operation cannot be modified but can be polled for
/// completion. The operation ID is used to track the operation in the kernel.
#[derive(Debug, Clone, Copy)]
pub struct Submitted {
    /// Operation ID for tracking in the completion queue
    /// This ID is assigned by the ring when the operation is submitted
    pub(crate) id: u64,
}

/// Operation has completed with a result.
///
/// In this state, the operation result can be extracted along with the
/// buffer ownership. This is the terminal state for operations.
#[derive(Debug)]
pub struct Completed<T> {
    /// The result of the operation
    pub(crate) result: T,
}

// Implement the sealed trait for all valid states
impl OperationState for Building {}
impl OperationState for Submitted {}
impl<T> OperationState for Completed<T> {}

/// Private module to seal the OperationState trait.
///
/// This prevents external crates from implementing OperationState,
/// ensuring type safety of the state machine.
mod private {
    pub trait Sealed {}
    impl Sealed for super::Building {}
    impl Sealed for super::Submitted {}
    impl<T> Sealed for super::Completed<T> {}
}
