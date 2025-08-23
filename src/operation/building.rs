//! Building state implementation and operation constructors.
//!
//! This module contains all the functionality for operations in the Building state,
//! including constructors and configuration methods.

use std::marker::PhantomData;
use std::os::unix::io::RawFd;
use std::pin::Pin;

use super::core::{BufferType, FdType, Operation};
use crate::operation::{Building, OperationType, Submitted};
use crate::registry::{RegisteredBuffer, RegisteredFd};

impl<'ring, 'buf> Default for Operation<'ring, 'buf, Building> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ring, 'buf> Operation<'ring, 'buf, Building> {
    /// Create a new operation in the building state.
    ///
    /// The operation starts with default values and must be configured
    /// before it can be submitted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// let op = Operation::new()
    ///     .fd(0)
    ///     .offset(1024);
    /// ```
    pub fn new() -> Self {
        Self::with_type(OperationType::Read) // Default to read for backwards compatibility
    }

    /// Create a new read operation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut buffer = vec![0u8; 1024];
    /// let op = Operation::read()
    ///     .fd(0)
    ///     .buffer(Pin::new(buffer.as_mut_slice()));
    /// ```
    #[inline]
    pub fn read() -> Self {
        Self::with_type(OperationType::Read)
    }

    /// Create a new vectored read operation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut buffer1 = vec![0u8; 512];
    /// let mut buffer2 = vec![0u8; 512];
    /// let buffers = vec![
    ///     Pin::new(buffer1.as_mut_slice()),
    ///     Pin::new(buffer2.as_mut_slice()),
    /// ];
    /// let op = Operation::read_vectored()
    ///     .fd(0)
    ///     .buffers(buffers);
    /// ```
    #[inline]
    pub fn read_vectored() -> Self {
        Self::with_type(OperationType::ReadVectored)
    }

    /// Create a new write operation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut buffer = b"Hello, world!".to_vec();
    /// let op = Operation::write()
    ///     .fd(1)
    ///     .buffer(Pin::new(buffer.as_mut_slice()));
    /// ```
    #[inline]
    pub fn write() -> Self {
        Self::with_type(OperationType::Write)
    }

    /// Create a new vectored write operation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut buffer1 = b"Hello, ".to_vec();
    /// let mut buffer2 = b"world!".to_vec();
    /// let buffers = vec![
    ///     Pin::new(buffer1.as_mut_slice()),
    ///     Pin::new(buffer2.as_mut_slice()),
    /// ];
    /// let op = Operation::write_vectored()
    ///     .fd(1)
    ///     .buffers(buffers);
    /// ```
    #[inline]
    pub fn write_vectored() -> Self {
        Self::with_type(OperationType::WriteVectored)
    }

    /// Create a new accept operation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// let op = Operation::accept().fd(3); // listening socket fd
    /// ```
    #[inline]
    pub fn accept() -> Self {
        Self::with_type(OperationType::Accept)
    }

    /// Create a new send operation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut buffer = b"Hello, client!".to_vec();
    /// let op = Operation::send()
    ///     .fd(4)
    ///     .buffer(Pin::new(buffer.as_mut_slice()));
    /// ```
    #[inline]
    pub fn send() -> Self {
        Self::with_type(OperationType::Send)
    }

    /// Create a new receive operation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut buffer = vec![0u8; 1024];
    /// let op = Operation::recv()
    ///     .fd(4)
    ///     .buffer(Pin::new(buffer.as_mut_slice()));
    /// ```
    #[inline]
    pub fn recv() -> Self {
        Self::with_type(OperationType::Recv)
    }

    /// Create a new operation with the specified type.
    ///
    /// This is a helper method to reduce code duplication in the constructor methods.
    #[inline]
    fn with_type(op_type: OperationType) -> Self {
        Self {
            ring: PhantomData,
            buffer: BufferType::None,
            fd: FdType::Raw(-1), // Invalid fd that must be set before submission
            offset: 0,
            op_type,
            state: Building,
        }
    }

    /// Set the file descriptor for this operation.
    ///
    /// This is required for all operations and must be a valid file descriptor.
    ///
    /// # Arguments
    ///
    /// * `fd` - A valid file descriptor (>= 0)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// let op = Operation::read().fd(0); // stdin
    /// ```
    #[inline]
    pub fn fd(mut self, fd: RawFd) -> Self {
        self.fd = FdType::Raw(fd);
        self
    }

    /// Set a registered file descriptor for this operation.
    ///
    /// Using registered file descriptors can improve performance for frequently
    /// used file descriptors by avoiding kernel lookups.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - A registered file descriptor
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{operation::Operation, Registry};
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let registered_fd = registry.register_fd(0)?;
    /// let op = Operation::read().registered_fd(registered_fd);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn registered_fd(mut self, registered_fd: RegisteredFd) -> Self {
        self.fd = FdType::Registered(registered_fd);
        self
    }

    /// Set the buffer for this operation.
    ///
    /// The buffer lifetime must be at least as long as the ring lifetime
    /// to ensure memory safety during the operation. The buffer must remain
    /// pinned in memory until the operation completes.
    ///
    /// # Arguments
    ///
    /// * `buffer` - A pinned mutable slice that will be used for I/O
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut data = vec![0u8; 4096];
    /// let op = Operation::read()
    ///     .fd(0)
    ///     .buffer(Pin::new(data.as_mut_slice()));
    /// ```
    #[inline]
    pub fn buffer(mut self, buffer: Pin<&'buf mut [u8]>) -> Self {
        self.buffer = BufferType::Pinned(buffer);
        self
    }

    /// Set a registered buffer for this operation.
    ///
    /// Using registered buffers can improve performance by avoiding kernel
    /// buffer validation and setup overhead.
    ///
    /// # Arguments
    ///
    /// * `registered_buffer` - A registered buffer
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{operation::Operation, Registry};
    /// # use std::pin::Pin;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let buffer = Pin::new(Box::new([0u8; 1024]));
    /// let registered_buffer = registry.register_buffer(buffer)?;
    /// let op = Operation::read()
    ///     .fd(0)
    ///     .registered_buffer(registered_buffer);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn registered_buffer(mut self, registered_buffer: RegisteredBuffer) -> Self {
        self.buffer = BufferType::Registered(registered_buffer);
        self
    }

    /// Set multiple buffers for vectored I/O operations.
    ///
    /// This enables scatter-gather I/O where data can be read into or written
    /// from multiple non-contiguous buffers in a single operation.
    ///
    /// # Arguments
    ///
    /// * `buffers` - A vector of pinned mutable slices
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// # use std::pin::Pin;
    /// let mut buffer1 = vec![0u8; 512];
    /// let mut buffer2 = vec![0u8; 512];
    /// let buffers = vec![
    ///     Pin::new(buffer1.as_mut_slice()),
    ///     Pin::new(buffer2.as_mut_slice()),
    /// ];
    /// let op = Operation::read_vectored()
    ///     .fd(0)
    ///     .buffers(buffers);
    /// ```
    #[inline]
    pub fn buffers(mut self, buffers: Vec<Pin<&'buf mut [u8]>>) -> Self {
        self.buffer = BufferType::Vectored(buffers);
        self
    }

    /// Set the offset for this operation.
    ///
    /// This is used for file operations to specify the position in the file.
    /// For socket operations, this parameter is typically ignored by the kernel.
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte offset in the file (0-based)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// let op = Operation::read()
    ///     .fd(3)
    ///     .offset(1024); // Read starting at byte 1024
    /// ```
    #[inline]
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }



    /// Get the current offset.
    ///
    /// Returns the byte offset for file operations, or 0 if not set.
    #[inline]
    pub fn get_offset(&self) -> u64 {
        self.offset
    }

    /// Validate that the operation is ready for submission.
    ///
    /// This checks that all required fields are set for the operation type.
    /// Using the type system's knowledge about operation requirements for
    /// more efficient validation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File descriptor is not set (< 0)
    /// - Buffer is required but not set
    /// - Vectored operation has empty buffer list
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::operation::Operation;
    /// let op = Operation::read().fd(0);
    /// assert!(op.validate().is_err()); // Missing buffer
    /// ```
    pub fn validate(&self) -> Result<(), &'static str> {
        // Check file descriptor
        match &self.fd {
            FdType::Raw(fd) if *fd < 0 => return Err("File descriptor must be set"),
            _ => {}
        }

        // Use the type system's knowledge about buffer requirements
        if self.op_type.requires_buffer() {
            match &self.buffer {
                BufferType::None => return Err("Buffer must be set for I/O operations"),
                BufferType::Vectored(buffers) if buffers.is_empty() => {
                    return Err("Vectored operations require at least one buffer")
                }
                _ => {}
            }
        }

        // Validate operation type matches buffer type
        if self.op_type.is_vectored() && !matches!(self.buffer, BufferType::Vectored(_)) {
            return Err("Vectored operation types require vectored buffers");
        }

        if !self.op_type.is_vectored() && matches!(self.buffer, BufferType::Vectored(_)) {
            return Err("Non-vectored operation types cannot use vectored buffers");
        }

        Ok(())
    }

    /// Convert this building operation to a submitted operation.
    ///
    /// This is typically called by the Ring when submitting the operation.
    /// It validates the operation and transitions to the Submitted state.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique operation ID assigned by the ring
    ///
    /// # Errors
    ///
    /// Returns an error if the operation is not properly configured.
    #[allow(dead_code)]
    pub(crate) fn submit_with_id(
        self,
        id: u64,
    ) -> Result<Operation<'ring, 'buf, Submitted>, &'static str> {
        self.validate()?;

        // Zero-cost state transition - just change the type parameter
        Ok(Operation {
            ring: self.ring,
            buffer: self.buffer,
            fd: self.fd,
            offset: self.offset,
            op_type: self.op_type,
            state: Submitted { id },
        })
    }
}
