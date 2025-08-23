//! Completed state implementation for finished operations.
//!
//! This module contains functionality for operations that have completed
//! and contain their results.

use std::os::unix::io::RawFd;
use std::pin::Pin;

use super::core::{BufferType, FdType, Operation};
use crate::operation::{Completed, OperationType};

impl<'ring, 'buf, T> Operation<'ring, 'buf, Completed<T>> {
    /// Extract the result from the completed operation.
    ///
    /// This consumes the operation and returns both the result and the buffer
    /// ownership, ensuring no resource leaks. This is the primary way to
    /// retrieve results from completed operations.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - The operation result
    /// - The buffer (if one was used), returned to the caller for reuse
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # let completed_op: Operation<'_, '_, safer_ring::operation::Completed<i32>> = todo!();
    /// let (bytes_read, buffer) = completed_op.into_result();
    /// println!("Read {} bytes", bytes_read);
    /// // Buffer can now be reused for another operation
    /// ```
    pub fn into_result(self) -> (T, BufferType<'buf>) {
        (self.state.result, self.buffer)
    }

    /// Extract the result and return a pinned buffer if available.
    ///
    /// This is a convenience method for operations that use regular pinned buffers.
    /// For other buffer types, use `into_result()` instead.
    pub fn into_result_with_buffer(self) -> (T, Option<Pin<&'buf mut [u8]>>) {
        let (result, buffer_type) = self.into_result();
        let buffer = match buffer_type {
            BufferType::Pinned(buf) => Some(buf),
            _ => None,
        };
        (result, buffer)
    }

    /// Extract the result and return vectored buffers if available.
    ///
    /// This is a convenience method for vectored operations.
    pub fn into_result_with_vectored_buffers(self) -> (T, Option<Vec<Pin<&'buf mut [u8]>>>) {
        let (result, buffer_type) = self.into_result();
        let buffers = match buffer_type {
            BufferType::Vectored(bufs) => Some(bufs),
            _ => None,
        };
        (result, buffers)
    }

    /// Get a reference to the result without consuming the operation.
    ///
    /// This allows inspecting the result while keeping the operation intact.
    #[inline]
    pub fn result(&self) -> &T {
        &self.state.result
    }

    /// Get the file descriptor for this operation.
    #[inline]
    pub fn fd(&self) -> RawFd {
        match &self.fd {
            FdType::Raw(fd) => *fd,
            FdType::Registered(reg_fd) => reg_fd.raw_fd(),
        }
    }

    /// Get the offset for this operation.
    #[inline]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Get the operation type.
    #[inline]
    pub fn op_type(&self) -> OperationType {
        self.op_type
    }
}
