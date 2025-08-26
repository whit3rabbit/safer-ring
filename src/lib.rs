//! # Safer-Ring: Safe io_uring for Rust
//!
//! A comprehensive, safe Rust wrapper around io_uring that provides zero-cost abstractions
//! while preventing common memory safety issues. The library uses Rust's type system,
//! lifetime management, and pinning to ensure that buffers remain valid during asynchronous
//! I/O operations, eliminating use-after-free bugs and data races.
//!
//! ## Key Features
//!
//! ### Safety Guarantees
//! - **Memory Safety**: Compile-time guarantees that buffers outlive their operations
//! - **Type Safety**: State machines prevent operations in invalid states
//! - **Lifetime Safety**: Automatic enforcement of buffer and operation lifetimes
//! - **Thread Safety**: Safe sharing of resources across async tasks
//!
//! ### Performance Optimizations
//! - **Zero-Cost Abstractions**: No runtime overhead compared to raw io_uring
//! - **Batch Operations**: Submit multiple operations efficiently with dependency support
//! - **Buffer Pooling**: Efficient buffer reuse to minimize allocations
//! - **Advanced Features**: Support for buffer selection, multi-shot operations, and more
//!
//! ### Developer Experience
//! - **Async/Await**: Seamless integration with Rust's async ecosystem
//! - **Comprehensive Logging**: Structured logging and performance metrics
//! - **Flexible Configuration**: Optimized presets for different use cases
//! - **Graceful Degradation**: Automatic fallback for older kernel versions
//!
//! ## Quick Start
//!
//! ### Recommended: Ownership Transfer API
//!
//! ```rust,no_run
//! use safer_ring::{Ring, OwnedBuffer};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new ring with 32 entries
//! let mut ring = Ring::new(32)?;
//!
//! // Create a buffer - ownership will be transferred during I/O
//! let buffer = OwnedBuffer::new(1024);
//!
//! // Safe read with ownership transfer ("hot potato" pattern)
//! let (bytes_read, buffer) = ring.read_owned(0, buffer).await?;
//! println!("Read {} bytes", bytes_read);
//!
//! // Buffer is safely returned and can be reused  
//! let (bytes_written, _buffer) = ring.write_owned(1, buffer).await?;
//! println!("Wrote {} bytes", bytes_written);
//!
//! # Ok(())
//! # }
//! ```
//!
//! ### Alternative: Pin-based API (Advanced)
//!
//! ```rust,no_run
//! use safer_ring::{Ring, PinnedBuffer};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let ring = Ring::new(32)?;
//! let mut buffer = PinnedBuffer::with_capacity(1024);
//!
//! // Lower-level pin-based API for advanced use cases
//! let (bytes_read, buffer) = ring.read(0, buffer.as_mut_slice()).await?;
//! println!("Read {} bytes", bytes_read);
//!
//! # Ok(())
//! # }
//! ```
//!
//! ### High-Performance Batch Operations
//!
//! For high-throughput applications, submit multiple operations in a single batch:
//!
//! ```rust,no_run
//! use safer_ring::{Ring, Batch, Operation, PinnedBuffer, BatchConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Note: Batch operations currently require careful lifetime management
//! // See examples/async_demo.rs for working patterns
//! let ring = Ring::new(128)?;
//! let mut batch = Batch::new();
//!
//! // Prepare multiple buffers
//! let mut read_buffer = PinnedBuffer::with_capacity(4096);
//! let mut write_buffer = PinnedBuffer::from_slice(b"Hello, world!");
//!
//! // Add operations to the batch
//! batch.add_operation(Operation::read().fd(0).buffer(read_buffer.as_mut_slice()))?;
//! batch.add_operation(Operation::write().fd(1).buffer(write_buffer.as_mut_slice()))?;
//!
//! // Submit all operations at once  
//! let results = ring.submit_batch(batch).await?;
//! println!("Batch completed: {} operations", results.results.len());
//!
//! # Ok(())
//! # }
//! ```
//!
//! > **Note**: Batch operations are fully implemented but have some API ergonomics
//! > limitations. See `examples/async_demo.rs` for working usage patterns.
//!
//! ### Network Server Example
//!
//! ```rust,no_run
//! use safer_ring::{Ring, PinnedBuffer, BufferPool};
//! use std::os::unix::io::RawFd;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let ring = Ring::new(256)?;
//! let buffer_pool = BufferPool::new(100, 4096)?;
//! let listening_fd: RawFd = 3; // Assume we have a listening socket
//!
//! loop {
//!     // Accept a new connection
//!     let client_fd = ring.accept(listening_fd).await?;
//!     
//!     // Get a buffer from the pool
//!     let buffer = buffer_pool.get().await?;
//!     
//!     // Read data from the client
//!     let (bytes_read, buffer) = ring.recv(client_fd, buffer.as_mut_slice()).await?;
//!     
//!     // Echo the data back
//!     let (bytes_written, _buffer) = ring.send(client_fd, buffer.as_mut_slice()).await?;
//!     
//!     println!("Echoed {} bytes", bytes_written);
//!     // Buffer is automatically returned to pool when dropped
//! }
//! # }
//! ```
//!
//! ## Configuration and Optimization
//!
//! Safer-ring provides pre-configured setups for different use cases:
//!
//! ```rust,no_run
//! use safer_ring::{Ring, SaferRingConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Low-latency configuration for real-time applications
//! let config = SaferRingConfig::low_latency();
//! let ring = Ring::with_config(config)?;
//!
//! // High-throughput configuration for batch processing
//! let config = SaferRingConfig::high_throughput();
//! let ring = Ring::with_config(config)?;
//!
//! // Auto-detect optimal configuration for current system
//! let config = SaferRingConfig::auto_detect()?;
//! let ring = Ring::with_config(config)?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Advanced Features
//!
//! ### Buffer Selection and Provided Buffers
//!
//! ```rust,no_run
//! use safer_ring::{Ring, BufferGroup, AdvancedConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a buffer group for kernel buffer selection
//! let mut buffer_group = BufferGroup::new(1, 64, 4096)?;
//!
//! // Configure ring with advanced features
//! let mut config = AdvancedConfig::default();
//! config.buffer_selection = true;
//! config.provided_buffers = true;
//!
//! let ring = Ring::with_advanced_config(config)?;
//! // Use buffer selection for zero-copy reads
//! # Ok(())
//! # }
//! ```
//!
//! ### Comprehensive Logging and Metrics
//!
//! ```rust,no_run
//! use safer_ring::{Ring, SaferRingConfig, LogLevel};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Enable detailed logging and metrics
//! let mut config = SaferRingConfig::development();
//! config.logging.enabled = true;
//! config.logging.level = LogLevel::Debug;
//! config.logging.metrics = true;
//!
//! let ring = Ring::with_config(config)?;
//!
//! // Operations are automatically logged with timing information
//! let mut buffer = safer_ring::PinnedBuffer::with_capacity(1024);
//! let (bytes_read, _) = ring.read(0, buffer.as_mut_slice()).await?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Platform Support
//!
//! - **Linux**: Full io_uring support with all advanced features
//! - **Other platforms**: Graceful degradation with stub implementations for testing
//!
//! ## Minimum Kernel Requirements
//!
//! - **Basic functionality**: Linux 5.1+
//! - **Advanced features**: Linux 5.19+ (buffer selection, multi-shot operations)
//! - **Optimal performance**: Linux 6.0+ (cooperative task running, defer taskrun)
//!
//! The library automatically detects available features and gracefully degrades
//! functionality on older kernels while maintaining API compatibility.
//!
//! ## Security Considerations
//!
//! ### File Descriptor Responsibility
//!
//! **⚠️ CRITICAL SECURITY NOTICE**: This library accepts raw file descriptors (`RawFd`) 
//! from user code and does **NOT** perform any validation, permission checks, or access
//! control. The application is **entirely responsible** for ensuring file descriptor security.
//!
//! ### Security Responsibilities
//!
//! **The calling application must ensure that all file descriptors:**
//! - Are valid and owned by the current process
//! - Have appropriate permissions for the intended operation (read/write/accept)
//! - Are not subject to race conditions from concurrent access
//! - Point to intended resources (files, sockets, devices)
//! - Have been properly authenticated and authorized
//! - Are not being used maliciously by untrusted code
//!
//! ### Potential Security Risks
//!
//! **Using invalid, unauthorized, or malicious file descriptors can result in:**
//! - Reading from or writing to unintended files, sockets, or devices
//! - Information disclosure or data corruption
//! - Privilege escalation or unauthorized system access
//! - Buffer overflow attacks or memory corruption (from network data)
//! - Denial of service or system instability
//!
//! ### Security Best Practices
//!
//! **Always implement these security controls at the application level:**
//! - Validate file descriptors at security boundaries
//! - Use proper access control and permission checks
//! - Implement input validation for all received data
//! - Consider using sandboxing or privilege isolation
//! - Apply principle of least privilege for file access
//! - Use secure network protocols and authentication
//! - Monitor and log suspicious file descriptor usage
//!
//! ### Data Validation
//!
//! **For network operations, the application must:**
//! - Validate all received data before processing
//! - Implement proper input sanitization and bounds checking  
//! - Use secure parsing for untrusted input data
//! - Consider data confidentiality and integrity requirements
//! - Protect against injection attacks and protocol exploitation
//!
//! This library provides **memory safety** and **async safety** but does **NOT** provide
//! **security boundaries** or **access control**. Security must be implemented at the
//! application layer.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Core modules - fundamental building blocks for safe io_uring operations
pub mod backend; // Backend abstraction for io_uring and epoll
pub mod buffer;
pub mod error;
pub mod operation;
pub mod ownership; // Buffer ownership management for safety
pub mod ring;
pub mod runtime; // Runtime detection and fallback system
pub mod safety; // Cancellation safety and orphaned operation tracking

// Advanced features - performance optimizations and convenience APIs
pub mod advanced; // Advanced io_uring features (buffer selection, multi-shot, etc.)
pub mod compat; // AsyncRead/AsyncWrite compatibility adapters
pub mod config; // Configuration options for different use cases
pub mod future; // async/await integration
pub mod logging; // Comprehensive logging and debugging support
pub mod perf; // Performance profiling and optimization utilities
pub mod pool; // Buffer pooling for reduced allocations
pub mod registry; // FD/buffer registration for kernel optimization

// Re-exports for convenience - commonly used types at crate root
pub use advanced::{AdvancedConfig, BufferGroup, FeatureDetector, MultiShotConfig};
pub use buffer::PinnedBuffer;
pub use compat::{AsyncCompat, AsyncReadAdapter, AsyncWriteAdapter, File, Socket}; // Async compatibility
pub use config::{
    BufferConfig, ConfigBuilder, ErrorHandlingConfig, LoggingConfig, PerformanceConfig, RingConfig,
    SaferRingConfig,
};
pub use error::{Result, SaferRingError};
pub use logging::{LogLevel, Logger, PerformanceMetrics};
pub use operation::{Operation, OperationState};
pub use ownership::{BufferOwnership, OwnedBuffer, SafeBuffer}; // Core ownership types
pub use pool::{BufferPool, PooledBuffer};
pub use registry::{FixedFile, RegisteredBufferSlot, Registry};
pub use ring::{Batch, BatchConfig, BatchResult, CompletionResult, OperationResult, Ring};
pub use runtime::{get_environment_info, is_io_uring_available, Backend, EnvironmentInfo, Runtime}; // Runtime system
pub use safety::{OrphanTracker, SafeOperation, SafeOperationFuture, SubmissionId}; // Cancellation safety

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
/// Standalone future type for batch operations that doesn't hold Ring references.
pub type StandaloneBatchFuture = future::StandaloneBatchFuture;
