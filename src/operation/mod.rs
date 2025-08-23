//! Type-safe operation state management.
//!
//! This module provides a type-safe state machine for io_uring operations that prevents
//! common usage errors at compile time. Operations progress through well-defined states:
//!
//! - [`Building`]: Initial state where the operation can be configured
//! - [`Submitted`]: Operation has been submitted to the kernel and is in flight
//! - [`Completed<T>`]: Operation has finished and contains the result
//!
//! State transitions are enforced at compile time, making it impossible to:
//! - Submit an operation twice
//! - Poll an operation that hasn't been submitted
//! - Extract results from an incomplete operation
//!
//! # Example
//!
//! ```rust,no_run
//! # use safer_ring::operation::{Operation, OperationType};
//! # use std::pin::Pin;
//! let mut buffer = vec![0u8; 1024];
//! let pinned = Pin::new(buffer.as_mut_slice());
//!
//! let operation = Operation::read()
//!     .fd(0)
//!     .buffer(pinned)
//!     .offset(0);
//! ```

// Public modules
pub mod types;

// Internal modules
mod building;
mod completed;
mod core;
mod states;
mod submitted;

// Internal operation tracking module
pub(crate) mod tracker;

// Test module
#[cfg(test)]
mod tests;

// Re-exports for public API
pub use core::{BufferType, FdType, Operation};
pub use states::{Building, Completed, OperationState, Submitted};
pub use types::OperationType;
