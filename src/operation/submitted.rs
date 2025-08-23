//! Submitted state implementation for in-flight operations.
//!
//! This module contains functionality for operations that have been submitted
//! to the kernel and are currently in flight.

use std::os::unix::io::RawFd;

use super::core::{BufferType, FdType, Operation};
use crate::operation::{Completed, OperationType, Submitted};

impl<'ring, 'buf> Operation<'ring, 'buf, Submitted> {
    /// Get the operation ID.
    ///
    /// This ID is used to track the operation in the completion queue
    /// and match completions to their corresponding operations.
    #[inline]
    pub fn id(&self) -> u64 {
        self.state.id
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

    /// Get buffer information for kernel submission.
    ///
    /// Returns a tuple of (pointer, length) for the buffer if present.
    /// This is used internally by the Ring for kernel submission.
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid while the buffer remains pinned
    /// and the operation is in flight. The caller must ensure the buffer
    /// is not moved or dropped until the operation completes.
    #[allow(dead_code)] // Used by ring submission logic
    pub(crate) fn buffer_info(&self) -> Option<(*mut u8, usize)> {
        match &self.buffer {
            BufferType::None => None,
            BufferType::Pinned(buf) => {
                let slice = buf.as_ref();
                Some((slice.as_ptr() as *mut u8, slice.len()))
            }
            BufferType::Registered(_) => {
                // For registered buffers, we need to get the buffer info from the registry
                // This will be handled by the ring during submission
                None
            }
            BufferType::Vectored(_) => {
                // For vectored operations, buffer info is handled differently
                // This will be handled by the ring during submission
                None
            }
        }
    }

    /// Get vectored buffer information for kernel submission.
    ///
    /// Returns a vector of (pointer, length) tuples for vectored operations.
    /// This is used internally by the Ring for vectored operation submission.
    ///
    /// # Safety
    ///
    /// The returned pointers are only valid while the buffers remain pinned
    /// and the operation is in flight.
    #[allow(dead_code)] // Used by ring submission logic
    pub(crate) fn vectored_buffer_info(&self) -> Option<Vec<(*mut u8, usize)>> {
        match &self.buffer {
            BufferType::Vectored(buffers) => {
                let info: Vec<_> = buffers
                    .iter()
                    .map(|buf| {
                        let slice = buf.as_ref();
                        (slice.as_ptr() as *mut u8, slice.len())
                    })
                    .collect();
                Some(info)
            }
            _ => None,
        }
    }

    /// Check if this operation uses a registered buffer.
    #[inline]
    pub fn uses_registered_buffer(&self) -> bool {
        matches!(self.buffer, BufferType::Registered(_))
    }

    /// Check if this operation uses a registered file descriptor.
    #[inline]
    pub fn uses_registered_fd(&self) -> bool {
        matches!(self.fd, FdType::Registered(_))
    }

    /// Check if this operation is vectored.
    #[inline]
    pub fn is_vectored(&self) -> bool {
        matches!(self.buffer, BufferType::Vectored(_))
    }

    /// Convert this submitted operation to a completed operation.
    ///
    /// This is typically called by the Ring when processing completions.
    /// The state transition is zero-cost at runtime.
    ///
    /// # Arguments
    ///
    /// * `result` - The result of the completed operation
    #[allow(dead_code)]
    pub(crate) fn complete_with_result<T>(self, result: T) -> Operation<'ring, 'buf, Completed<T>> {
        // Zero-cost state transition - just change the type parameter
        Operation {
            ring: self.ring,
            buffer: self.buffer,
            fd: self.fd,
            offset: self.offset,
            op_type: self.op_type,
            state: Completed { result },
        }
    }
}
