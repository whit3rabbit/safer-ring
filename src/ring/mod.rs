//! Safe io_uring wrapper with compile-time safety guarantees.
//!
//! This module provides the main [`Ring`] type that wraps io_uring operations
//! with lifetime management to prevent use-after-free bugs and ensure buffer safety.

pub mod batch;
mod completion;
mod core;

#[cfg(test)]
mod tests;

pub use batch::{Batch, BatchConfig, BatchResult, OperationResult};
pub use completion::CompletionResult;
pub use core::Ring;
