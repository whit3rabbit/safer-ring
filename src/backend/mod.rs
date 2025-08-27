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
pub trait Backend {
    /// Submit an operation to the backend.
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
    fn try_complete(&mut self) -> Result<Vec<(u64, io::Result<i32>)>>;

    /// Wait for at least one operation to complete.
    fn wait_for_completion(&mut self) -> Result<Vec<(u64, io::Result<i32>)>>;

    /// Get the number of operations currently in flight.
    fn operations_in_flight(&self) -> usize;

    /// Get backend name for debugging.
    fn name(&self) -> &'static str;

    /// Register file descriptors for optimized access.
    /// Returns the starting index where files were registered.
    fn register_files(&mut self, fds: &[RawFd]) -> Result<u32>;

    /// Unregister all registered file descriptors.
    fn unregister_files(&mut self) -> Result<()>;

    /// Register buffers for optimized access.
    /// Returns the starting index where buffers were registered.
    fn register_buffers(&mut self, buffers: &[Pin<Box<[u8]>>]) -> Result<u32>;

    /// Unregister all registered buffers.
    fn unregister_buffers(&mut self) -> Result<()>;

    /// Get the capacity of the submission queue.
    fn capacity(&self) -> u32;

    /// Get completion queue statistics (ready count, total capacity).
    fn completion_queue_stats(&mut self) -> (usize, usize);
}

/// Detect the best available backend for the current system.
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
