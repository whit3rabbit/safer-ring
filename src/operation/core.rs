//! Core Operation struct definition and common functionality.
//!
//! This module contains the main Operation struct and methods that are
//! available across all states.

use std::marker::PhantomData;
use std::os::unix::io::RawFd;
use std::pin::Pin;

use crate::operation::{OperationState, OperationType};
use crate::registry::{RegisteredBuffer, RegisteredFd};

/// Buffer type for operations.
///
/// This enum represents the different types of buffers that can be used
/// with operations, supporting both regular pinned buffers and registered buffers.
#[derive(Debug)]
pub enum BufferType<'buf> {
    /// No buffer (for operations like accept)
    None,
    /// Regular pinned buffer
    Pinned(Pin<&'buf mut [u8]>),
    /// Registered buffer with the registry
    Registered(RegisteredBuffer),
    /// Multiple pinned buffers for vectored I/O
    Vectored(Vec<Pin<&'buf mut [u8]>>),
}

/// File descriptor type for operations.
///
/// This enum represents the different types of file descriptors that can be used
/// with operations, supporting both regular file descriptors and registered ones.
#[derive(Debug)]
pub enum FdType {
    /// Regular file descriptor
    Raw(RawFd),
    /// Registered file descriptor
    Registered(RegisteredFd),
}

/// Type-safe operation with compile-time state tracking.
///
/// The operation struct uses phantom types to track its current state and enforce
/// valid state transitions at compile time. The lifetime parameters ensure that:
/// - `'ring`: The operation cannot outlive the ring that will execute it
/// - `'buf`: The buffer cannot be dropped while the operation is in flight
///
/// # Type Parameters
///
/// - `'ring`: Lifetime of the ring that will execute this operation
/// - `'buf`: Lifetime of the buffer used by this operation
/// - `S`: Current state of the operation ([`crate::operation::Building`], [`crate::operation::Submitted`], or [`crate::operation::Completed<T>`])
#[derive(Debug)]
pub struct Operation<'ring, 'buf, S> {
    // PhantomData is zero-sized, so this doesn't add runtime overhead
    pub(crate) ring: PhantomData<&'ring ()>,
    // Buffer type supports different buffer configurations
    pub(crate) buffer: BufferType<'buf>,
    // File descriptor type supports both raw and registered fds
    pub(crate) fd: FdType,
    // u64 offset supports large files (>4GB) on all platforms
    pub(crate) offset: u64,
    pub(crate) op_type: OperationType,
    pub(crate) state: S,
}

impl<'ring, 'buf, S: OperationState> Operation<'ring, 'buf, S> {
    /// Get the operation type.
    ///
    /// Returns the type of I/O operation this will perform.
    #[inline]
    pub fn get_type(&self) -> OperationType {
        self.op_type
    }

    /// Check if the operation has a buffer set.
    ///
    /// Returns `true` if a buffer has been configured for this operation.
    #[inline]
    pub fn has_buffer(&self) -> bool {
        !matches!(self.buffer, BufferType::None)
    }

    /// Get the file descriptor for this operation.
    ///
    /// Returns the raw file descriptor, resolving registered fds if necessary.
    #[inline]
    pub fn get_fd(&self) -> RawFd {
        match &self.fd {
            FdType::Raw(fd) => *fd,
            FdType::Registered(reg_fd) => reg_fd.raw_fd(),
        }
    }


}
