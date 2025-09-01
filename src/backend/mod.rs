//! Backend abstraction for different I/O mechanisms.
//!
//! This module provides a unified interface for different I/O backends,
//! allowing the library to fall back from io_uring to epoll when necessary.

use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;

use crate::error::Result;
use crate::operation::OperationType;

pub mod epoll;
pub mod io_uring;

/// Trait for I/O backends that can execute operations.
///
/// This trait provides a unified interface for different I/O mechanisms,
/// allowing the library to abstract over io_uring and epoll backends.
/// Each backend must implement all methods to provide a complete I/O
/// abstraction layer.
///
/// # Backend Types
///
/// - **io_uring**: High-performance backend for Linux 5.1+ with kernel-level
///   asynchronous I/O support
/// - **epoll**: Fallback backend using traditional epoll for older systems
///   or restricted environments
///
/// # Safety
///
/// Implementations must ensure that buffer pointers remain valid during
/// the lifetime of operations and that file descriptors are properly
/// managed to prevent use-after-close bugs.
pub trait Backend {
    /// Submit an operation to the backend.
    ///
    /// Queues an I/O operation for execution by the backend. The operation
    /// is identified by the user_data parameter, which will be returned
    /// when the operation completes.
    ///
    /// # Arguments
    ///
    /// * `op_type` - The type of I/O operation to perform
    /// * `fd` - File descriptor for the operation
    /// * `offset` - Byte offset for file operations (0 for sockets)
    /// * `buffer_ptr` - Pointer to the buffer for data transfer
    /// * `buffer_len` - Size of the buffer in bytes
    /// * `user_data` - Unique identifier for this operation
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `buffer_ptr` points to a valid buffer of at least `buffer_len` bytes
    /// - The buffer remains valid until the operation completes
    /// - `fd` is a valid, open file descriptor
    /// - `user_data` is unique among pending operations
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The backend's submission queue is full
    /// - The file descriptor is invalid
    /// - The operation type is unsupported by the backend
    fn submit_operation(
        &mut self,
        op_type: OperationType,
        fd: RawFd,
        offset: u64,
        buffer_ptr: *mut u8,
        buffer_len: usize,
        user_data: u64,
    ) -> Result<()>;

    /// Try to complete operations without blocking.
    ///
    /// Polls the backend for completed operations and returns them immediately.
    /// This method will not block if no operations are ready.
    ///
    /// # Returns
    ///
    /// Returns a vector of (user_data, result) tuples for all completed
    /// operations. The result contains either the number of bytes
    /// transferred or an I/O error.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters a system-level error
    /// while checking for completions.
    fn try_complete(&mut self) -> Result<Vec<(u64, io::Result<i32>)>>;

    /// Wait for at least one operation to complete.
    ///
    /// Blocks until at least one operation completes, then returns all
    /// currently completed operations. This is more efficient than
    /// polling when you know operations are pending.
    ///
    /// # Returns
    ///
    /// Returns a vector of (user_data, result) tuples for completed
    /// operations. At least one completion will be returned unless
    /// an error occurs.
    ///
    /// # Errors
    ///
    /// - Returns `InvalidInput` if no operations are in flight
    /// - Returns backend-specific errors for system-level failures
    ///
    /// # Panics
    ///
    /// May panic if called when no operations are pending on some backends.
    fn wait_for_completion(&mut self) -> Result<Vec<(u64, io::Result<i32>)>>;

    /// Get the number of operations currently in flight.
    ///
    /// Returns the count of operations that have been submitted but not
    /// yet completed. This includes operations waiting in submission
    /// queues and those actively being processed by the kernel.
    ///
    /// This is useful for flow control and debugging.
    fn operations_in_flight(&self) -> usize;

    /// Get backend name for debugging.
    ///
    /// Returns a static string identifying the backend type,
    /// such as "io_uring" or "epoll". Useful for logging,
    /// debugging, and performance analysis.
    fn name(&self) -> &'static str;

    /// Register file descriptors for optimized access.
    ///
    /// Pre-registers file descriptors with the backend for improved
    /// performance on subsequent operations. Some backends (like io_uring)
    /// can use registered files more efficiently by avoiding descriptor
    /// lookups and validations.
    ///
    /// # Arguments
    ///
    /// * `fds` - Slice of file descriptors to register
    ///
    /// # Returns
    ///
    /// Returns the starting index where files were registered in the
    /// backend's file table. This index can be used to reference
    /// registered files in operations.
    ///
    /// # Errors
    ///
    /// - Returns `InvalidInput` if the file descriptor array is empty
    /// - Returns backend-specific errors for registration failures
    ///
    /// # Note
    ///
    /// Some backends (like epoll) may not support file registration
    /// and will return success without doing anything.
    fn register_files(&mut self, fds: &[RawFd]) -> Result<u32>;

    /// Unregister all registered file descriptors.
    ///
    /// Removes all previously registered file descriptors from the
    /// backend's optimization tables. This should be called before
    /// closing registered file descriptors to avoid resource leaks.
    ///
    /// # Errors
    ///
    /// Returns backend-specific errors if unregistration fails.
    /// Some backends may return success even if no files were registered.
    fn unregister_files(&mut self) -> Result<()>;

    /// Register buffers for optimized access.
    ///
    /// Pre-registers buffers with the backend for improved performance.
    /// Registered buffers can be referenced by index in operations,
    /// avoiding the need to pass buffer addresses each time.
    ///
    /// # Arguments
    ///
    /// * `buffers` - Slice of pinned buffers to register
    ///
    /// # Returns
    ///
    /// Returns the starting index where buffers were registered in the
    /// backend's buffer table.
    ///
    /// # Safety
    ///
    /// The buffers must remain pinned and valid for the entire duration
    /// they are registered. Moving or deallocating registered buffers
    /// while they are registered will cause undefined behavior.
    ///
    /// # Errors
    ///
    /// - Returns `InvalidInput` if the buffer array is empty
    /// - Returns backend-specific errors for registration failures
    fn register_buffers(&mut self, buffers: &[Pin<Box<[u8]>>]) -> Result<u32>;

    /// Unregister all registered buffers.
    ///
    /// Removes all previously registered buffers from the backend.
    /// After this call, the buffers can be safely moved or deallocated.
    ///
    /// # Errors
    ///
    /// Returns backend-specific errors if unregistration fails.
    fn unregister_buffers(&mut self) -> Result<()>;

    /// Get the capacity of the submission queue.
    ///
    /// Returns the maximum number of operations that can be queued
    /// for submission at one time. This represents the backend's
    /// internal queue size and affects how many operations can be
    /// batched together.
    ///
    /// For backends without explicit submission queues, this returns
    /// a reasonable default value.
    fn capacity(&self) -> u32;

    /// Get completion queue statistics (ready count, total capacity).
    ///
    /// Returns a tuple of (ready_count, total_capacity) where:
    /// - ready_count: Number of completed operations waiting to be processed
    /// - total_capacity: Maximum number of completions the queue can hold
    ///
    /// This is useful for monitoring queue utilization and detecting
    /// when the completion queue might be getting full.
    ///
    /// For backends without explicit completion queues, returns
    /// reasonable approximations based on pending operations.
    fn completion_queue_stats(&mut self) -> (usize, usize);

    /// Get a reference to the backend as Any for downcasting.
    ///
    /// This allows safe downcasting to specific backend types for
    /// specialized functionality like AsyncFd integration.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Detect the best available backend for the current system.
///
/// Attempts to create the most performant backend available on the current
/// system, falling back to less optimal but more compatible backends if
/// necessary.
///
/// # Backend Selection Priority
///
/// 1. **io_uring** (Linux 5.1+): Highest performance, kernel-level async I/O
/// 2. **epoll** (Linux): Traditional event-driven I/O, widely compatible
///
/// # Arguments
///
/// * `entries` - Desired capacity for the backend's operation queues
///
/// # Returns
///
/// Returns a boxed Backend trait object for the best available backend.
///
/// # Errors
///
/// Returns an error only if no backends are available on the current system,
/// which should not happen on supported platforms.
///
/// # Examples
///
/// ```rust,no_run
/// # use safer_ring::backend::detect_backend;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = detect_backend(32)?;
/// println!("Using backend: {}", backend.name());
/// # Ok(())
/// # }
/// ```
pub fn detect_backend(_entries: u32) -> Result<Box<dyn Backend>> {
    // Try io_uring first on Linux
    #[cfg(target_os = "linux")]
    {
        match io_uring::IoUringBackend::new(_entries) {
            Ok(backend) => {
                return Ok(Box::new(backend));
            }
            Err(_e) => {
                // io_uring failed, fall back to epoll
            }
        }
    }

    // Fall back to epoll
    match epoll::EpollBackend::new() {
        Ok(backend) => Ok(Box::new(backend)),
        Err(e) => Err(e),
    }
}
