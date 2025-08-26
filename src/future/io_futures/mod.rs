//! I/O-specific future implementations.
//!
//! This module contains futures for different types of I/O operations,
//! organized by operation type for better maintainability.

mod common;
mod file_io;
mod network_io;
mod vectored_io;

// Re-export all future types
pub use file_io::{ReadFuture, WriteFuture};
pub use network_io::{AcceptFuture, RecvFuture, SendFuture};
pub use vectored_io::{VectoredReadFuture, VectoredWriteFuture};
