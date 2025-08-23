//! Future implementations for async/await support.
//!
//! This module provides Future implementations that integrate io_uring operations
//! with Rust's async/await ecosystem. The futures handle polling the completion
//! queue and managing wakers for efficient async operation.
//!
//! # Example
//!
//! ```rust,no_run
//! use safer_ring::{Ring, PinnedBuffer, Operation};
//! use std::pin::Pin;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut buffer = PinnedBuffer::with_capacity(1024);
//! let ring = Ring::new(32)?;
//!
//! // Create a read operation
//! let operation = Operation::read()
//!     .fd(0)
//!     .buffer(buffer.as_mut_slice());
//!
//! // Example usage - actual async processing would be more complex
//! # Ok(())
//! # }
//! ```

mod batch_future;
mod io_futures;
mod operation_future;
mod waker;

#[cfg(test)]
mod tests;

// Re-export public types
pub use batch_future::BatchFuture;
pub use io_futures::{AcceptFuture, ReadFuture, RecvFuture, SendFuture, VectoredReadFuture, VectoredWriteFuture, WriteFuture};
pub use operation_future::OperationFuture;

// Re-export internal types for crate use
pub(crate) use waker::WakerRegistry;
