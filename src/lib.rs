//! # Safer-Ring
//!
//! A safe Rust wrapper around io_uring that provides zero-cost abstractions while preventing
//! common memory safety issues. The library uses Rust's type system, lifetime management,
//! and pinning to ensure that buffers remain valid during asynchronous I/O operations.
//!
//! ## Features
//!
//! - **Memory Safety**: Compile-time guarantees that buffers outlive their operations
//! - **Type Safety**: State machines prevent operations in invalid states
//! - **Zero-Cost**: No runtime overhead compared to raw io_uring
//! - **Async/Await**: Seamless integration with Rust's async ecosystem
//! - **Buffer Management**: Efficient buffer pooling and reuse
//! - **Batch Operations**: Submit multiple operations efficiently with dependency support
//!
//! ## Example
//!
//! ```rust,no_run
//! use safer_ring::{Ring, PinnedBuffer};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new ring with 32 entries
//! let ring = Ring::new(32)?;
//!
//! // Create a pinned buffer for I/O operations
//! let buffer = PinnedBuffer::with_capacity(1024);
//!
//! println!("Ring created with {} operations in flight", ring.operations_in_flight());
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Batch Operations
//!
//! For high-throughput applications, you can submit multiple operations in a single batch:
//!
//! ```rust,no_run
//! use safer_ring::{Ring, Batch, Operation, PinnedBuffer, BatchConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let ring = Ring::new(32)?;
//! let mut batch = Batch::new();
//! let mut buffer1 = PinnedBuffer::with_capacity(1024);
//! let mut buffer2 = PinnedBuffer::with_capacity(1024);
//!
//! // Add operations to the batch
//! batch.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))?;
//! batch.add_operation(Operation::write().fd(1).buffer(buffer2.as_mut_slice()))?;
//!
//! // Submit all operations at once
//! let results = ring.submit_batch(batch).await?;
//! println!("Batch completed: {} succeeded, {} failed", 
//!          results.successful_count, results.failed_count);
//!
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Core modules - fundamental building blocks for safe io_uring operations
pub mod buffer;
pub mod error;
pub mod operation;
pub mod ring;

// Advanced features - performance optimizations and convenience APIs
pub mod future;
pub mod pool; // Buffer pooling for reduced allocations
pub mod registry; // FD/buffer registration for kernel optimization // async/await integration

// Re-exports for convenience - commonly used types at crate root
pub use buffer::PinnedBuffer;
pub use error::{Result, SaferRingError};
pub use operation::{Operation, OperationState};
pub use pool::BufferPool;
pub use registry::Registry;
pub use ring::{Batch, BatchConfig, BatchResult, CompletionResult, OperationResult, Ring};

// Type aliases for common patterns - reduces verbosity in user code
/// Future type for read operations that can be awaited.
pub type ReadFuture<'ring, 'buf> = future::ReadFuture<'ring, 'buf>;
/// Future type for write operations that can be awaited.
pub type WriteFuture<'ring, 'buf> = future::WriteFuture<'ring, 'buf>;
/// Future type for accept operations that can be awaited.
pub type AcceptFuture<'ring> = future::AcceptFuture<'ring>;
/// Future type for send operations that can be awaited.
pub type SendFuture<'ring, 'buf> = future::SendFuture<'ring, 'buf>;
/// Future type for receive operations that can be awaited.
pub type RecvFuture<'ring, 'buf> = future::RecvFuture<'ring, 'buf>;
/// Future type for batch operations that can be awaited.
pub type BatchFuture<'ring> = future::BatchFuture<'ring>;
