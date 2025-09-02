//! I/O operations (read, write, vectored) for the Ring.

use super::Ring;
use crate::error::{Result, SaferRingError};
use crate::future::{
    OperationFuture, ReadFuture, VectoredReadFuture, VectoredWriteFuture, WriteFuture,
};
use crate::operation::{Building, Operation, OperationType};

impl<'ring> Ring<'ring> {
    /// Read from a file descriptor at offset 0.
    ///
    /// ⚠️ **NOT RECOMMENDED**: This API is fundamentally limited and should not be used in application code.
    /// **Always prefer [`read_owned()`](Ring::read_owned) or [`read_at_owned()`](Ring::read_at_owned).**
    ///
    /// This pin-based API returns a `Future` that holds a mutable borrow of the `Ring`.
    /// Due to Rust's lifetime rules, this makes it **impossible to use this method in a loop**
    /// or for multiple concurrent operations on the same `Ring`, as the borrow checker will
    /// prevent subsequent calls.
    ///
    /// This method exists for educational purposes and for building low-level abstractions.
    /// For all practical application logic, use the "hot potato" pattern with [`OwnedBuffer`](crate::OwnedBuffer):
    ///
    /// ```rust,no_run  
    /// # use safer_ring::{Ring, OwnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let buffer = OwnedBuffer::new(1024);
    ///
    /// // Recommended: ownership transfer API
    /// let (bytes_read, buffer) = ring.read_owned(0, buffer).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// This is a convenience method for read operations that don't need a specific offset.
    /// It's equivalent to calling `read_at(fd, buffer, 0)`.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `buffer` - Pinned buffer to read into
    ///
    /// # Returns
    ///
    /// Returns a ReadFuture that can be awaited to get (bytes_read, buffer).
    ///
    /// # Security Considerations
    ///
    /// **IMPORTANT**: The caller is responsible for ensuring the file descriptor is:
    /// - Valid and open for reading
    /// - Owned by the current process with appropriate read permissions  
    /// - Not subject to race conditions (e.g., concurrent closes by other threads)
    /// - Properly validated for the intended use case
    ///
    /// This library does **NOT** perform permission checks, file descriptor validation,
    /// or access control. Passing an invalid, unauthorized, or malicious file descriptor
    /// may result in:
    /// - Reading from unintended files or devices
    /// - Security vulnerabilities or privilege escalation
    /// - System errors or crashes
    ///
    /// Always validate file descriptors at the application security boundary.
    ///
    /// # Legacy Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// let future = ring.read(0, buffer.as_mut_slice())?;
    /// let (bytes_read, buffer) = future.await?;
    /// println!("Read {} bytes", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<ReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        self.read_at(fd, buffer, 0)
    }

    /// Write to a file descriptor at offset 0.
    ///
    /// ⚠️ **NOT RECOMMENDED**: This API is fundamentally limited and should not be used in application code.
    /// **Always prefer [`write_owned()`](Ring::write_owned) or [`write_at_owned()`](Ring::write_at_owned).**
    ///
    /// This pin-based API returns a `Future` that holds a mutable borrow of the `Ring`.
    /// Due to Rust's lifetime rules, this makes it **impossible to use this method in a loop**
    /// or for multiple concurrent operations on the same `Ring`, as the borrow checker will
    /// prevent subsequent calls.
    ///
    /// This method exists for educational purposes and for building low-level abstractions.
    /// For all practical application logic, use the "hot potato" pattern with [`OwnedBuffer`](crate::OwnedBuffer):
    ///
    /// ```rust,no_run  
    /// # use safer_ring::{Ring, OwnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let buffer = OwnedBuffer::from_slice(b"Hello, world!");
    ///
    /// // Recommended: ownership transfer API
    /// let (bytes_written, buffer) = ring.write_owned(1, buffer).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// This is a convenience method for write operations that don't need a specific offset.
    /// It's equivalent to calling `write_at(fd, buffer, 0)`.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `buffer` - Pinned buffer containing data to write
    ///
    /// # Returns
    ///
    /// Returns a WriteFuture that can be awaited to get (bytes_written, buffer).
    ///
    /// # Security Considerations
    ///
    /// **IMPORTANT**: The caller is responsible for ensuring the file descriptor is:
    /// - Valid and open for writing
    /// - Owned by the current process with appropriate write permissions
    /// - Not subject to race conditions (e.g., concurrent closes by other threads)
    /// - Properly validated for the intended use case
    /// - Not pointing to sensitive system files or devices without authorization
    ///
    /// This library does **NOT** perform permission checks, file descriptor validation,
    /// or access control. Passing an invalid, unauthorized, or malicious file descriptor
    /// may result in:
    /// - Writing to unintended files, devices, or system resources
    /// - Data corruption or security vulnerabilities
    /// - Privilege escalation or unauthorized system modifications
    /// - System errors or crashes
    ///
    /// Always validate file descriptors at the application security boundary.
    ///
    /// # Legacy Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let data = b"Hello, world!";
    /// let mut buffer = PinnedBuffer::from_slice(data);
    ///
    /// let future = ring.write(1, buffer.as_mut_slice())?;
    /// let (bytes_written, buffer) = future.await?;
    /// println!("Wrote {} bytes", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<WriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        self.write_at(fd, buffer, 0)
    }

    /// Submit a read operation and return a future.
    ///
    /// This is a convenience method that combines operation submission with
    /// future creation for read operations. The returned future can be awaited
    /// to get the result and buffer ownership.
    ///
    /// # Arguments
    ///
    /// * `operation` - Read operation in Building state
    ///
    /// # Returns
    ///
    /// Returns a ReadFuture that can be awaited to get (bytes_read, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if operation submission fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, Operation, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    /// let ring = Ring::new(32)?;
    ///
    /// let operation = Operation::read()
    ///     .fd(0)
    ///     .buffer(buffer.as_mut_slice());
    ///
    /// // Example usage - actual future processing would be more complex
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit_read<'buf>(
        &'ring mut self,
        operation: Operation<'ring, 'buf, Building>,
    ) -> Result<ReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        // Validate that this is actually a read operation
        if operation.get_type() != OperationType::Read {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Operation must be a read operation",
            )));
        }

        let submitted = self.submit(operation)?;
        Ok(ReadFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Submit a write operation and return a future.
    ///
    /// This is a convenience method that combines operation submission with
    /// future creation for write operations. The returned future can be awaited
    /// to get the result and buffer ownership.
    ///
    /// # Arguments
    ///
    /// * `operation` - Write operation in Building state
    ///
    /// # Returns
    ///
    /// Returns a WriteFuture that can be awaited to get (bytes_written, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if operation submission fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, Operation, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buffer = b"Hello, world!".to_vec();
    /// let ring = Ring::new(32)?;
    ///
    /// let operation = Operation::write()
    ///     .fd(1)
    ///     .buffer(Pin::new(buffer.as_mut_slice()));
    ///
    /// // Example usage - actual future processing would be more complex
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit_write<'buf>(
        &'ring mut self,
        operation: Operation<'ring, 'buf, Building>,
    ) -> Result<WriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        // Validate that this is actually a write operation
        if operation.get_type() != OperationType::Write {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Operation must be a write operation",
            )));
        }

        let submitted = self.submit(operation)?;
        Ok(WriteFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Submit any operation and return a generic future.
    ///
    /// This method provides a unified interface for submitting any type of
    /// operation and getting a future. The returned future provides the raw
    /// result without type-specific conversions.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation in Building state
    ///
    /// # Returns
    ///
    /// Returns an OperationFuture that can be awaited to get (result, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if operation submission fails.
    pub fn submit_operation<'buf>(
        &'ring mut self,
        operation: Operation<'ring, 'buf, Building>,
    ) -> Result<OperationFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let submitted = self.submit(operation)?;
        Ok(OperationFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Read data from a file descriptor into a buffer at a specific offset.
    ///
    /// ⚠️ **NOT RECOMMENDED**: This API is fundamentally limited and should not be used in application code.
    /// **Always prefer [`read_at_owned()`](Ring::read_at_owned) instead.**
    ///
    /// This pin-based API returns a `Future` that holds a mutable borrow of the `Ring`.
    /// Due to Rust's lifetime rules, this makes it **impossible to use this method in a loop**
    /// or for multiple concurrent operations on the same `Ring`, as the borrow checker will
    /// prevent subsequent calls.
    ///
    /// This method exists for educational purposes and for building low-level abstractions.
    /// For all practical application logic, use the "hot potato" pattern with [`OwnedBuffer`](crate::OwnedBuffer).
    ///
    /// This is a high-level convenience method that creates a read operation,
    /// submits it, and returns a future that can be awaited.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `buffer` - Pinned buffer to read data into
    ///
    /// # Returns
    ///
    /// Returns a ReadFuture that resolves to (bytes_read, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// // Read 1024 bytes starting at offset 4096
    /// let future = ring.read_at(3, buffer.as_mut_slice(), 4096)?;
    /// let (bytes_read, buffer) = future.await?;
    /// println!("Read {} bytes from offset 4096", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_at<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
        offset: u64,
    ) -> Result<ReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::read().fd(fd).buffer(buffer).offset(offset);
        self.submit_read(operation)
    }

    /// Read data using a registered file descriptor.
    ///
    /// This method uses a pre-registered file descriptor for improved performance
    /// by avoiding kernel fd lookups on each operation.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - Registered file descriptor to read from
    /// * `buffer` - Pinned buffer to read data into
    ///
    /// # Returns
    ///
    /// Returns a ReadFuture that resolves to (bytes_read, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer, Registry};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut registry = Registry::new();
    /// let registered_fd = registry.register_fd(0)?;
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// let future = ring.read_registered(registered_fd, buffer.as_mut_slice())?;
    /// let (bytes_read, buffer) = future.await?;
    /// println!("Read {} bytes using registered fd", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_registered<'buf>(
        &'ring mut self,
        registered_fd: crate::registry::RegisteredFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<ReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::read()
            .registered_fd(registered_fd)
            .buffer(buffer);
        self.submit_read(operation)
    }

    /// Read data using a registered buffer.
    ///
    /// This method uses a pre-registered buffer for improved performance
    /// by avoiding kernel buffer validation on each operation.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `registered_buffer` - Registered buffer to read data into
    ///
    /// # Returns
    ///
    /// Returns a ReadFuture that resolves to (bytes_read, registered_buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, Registry};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut registry = Registry::new();
    /// let buffer = Pin::new(Box::new([0u8; 1024]));
    /// let registered_buffer = registry.register_buffer(buffer)?;
    ///
    /// // Note: This is a conceptual example - the actual return type would need
    /// // to be adjusted to handle registered buffers properly
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_with_registered_buffer<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        registered_buffer: crate::registry::RegisteredBuffer,
    ) -> Result<OperationFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::read()
            .fd(fd)
            .registered_buffer(registered_buffer);
        self.submit_operation(operation)
    }

    /// Perform a vectored read operation (readv).
    ///
    /// This reads data into multiple buffers in a single system call,
    /// implementing scatter-gather I/O for improved efficiency when
    /// working with non-contiguous data.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `buffers` - Vector of pinned buffers to read data into
    ///
    /// # Returns
    ///
    /// Returns a VectoredReadFuture that resolves to (bytes_read, buffers).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted or if the
    /// buffers vector is empty.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::with_capacity(512);
    /// let mut buffer2 = PinnedBuffer::with_capacity(512);
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// let future = ring.read_vectored(0, buffers)?;
    /// let (bytes_read, buffers) = future.await?;
    /// println!("Read {} bytes into {} buffers", bytes_read, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_vectored<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffers: Vec<std::pin::Pin<&'buf mut [u8]>>,
    ) -> Result<VectoredReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        if buffers.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Vectored read requires at least one buffer",
            )));
        }

        let operation = Operation::read_vectored().fd(fd).buffers(buffers);
        let submitted = self.submit(operation)?;
        Ok(VectoredReadFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Perform a vectored read operation at a specific offset.
    ///
    /// This combines vectored I/O with positioned reads, reading data into
    /// multiple buffers starting at a specific file offset.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `buffers` - Vector of pinned buffers to read data into
    /// * `offset` - Byte offset in the file to start reading from
    ///
    /// # Returns
    ///
    /// Returns a VectoredReadFuture that resolves to (bytes_read, buffers).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted or if the
    /// buffers vector is empty.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::with_capacity(512);
    /// let mut buffer2 = PinnedBuffer::with_capacity(512);
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// // Read starting at offset 4096
    /// let future = ring.read_vectored_at(3, buffers, 4096)?;
    /// let (bytes_read, buffers) = future.await?;
    /// println!("Read {} bytes into {} buffers from offset 4096", bytes_read, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_vectored_at<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffers: Vec<std::pin::Pin<&'buf mut [u8]>>,
        offset: u64,
    ) -> Result<VectoredReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        if buffers.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Vectored read requires at least one buffer",
            )));
        }

        let operation = Operation::read_vectored()
            .fd(fd)
            .buffers(buffers)
            .offset(offset);
        let submitted = self.submit(operation)?;
        Ok(VectoredReadFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Write data from a buffer to a file descriptor at a specific offset.
    ///
    /// ⚠️ **NOT RECOMMENDED**: This API is fundamentally limited and should not be used in application code.
    /// **Always prefer [`write_at_owned()`](Ring::write_at_owned) instead.**
    ///
    /// This pin-based API returns a `Future` that holds a mutable borrow of the `Ring`.
    /// Due to Rust's lifetime rules, this makes it **impossible to use this method in a loop**
    /// or for multiple concurrent operations on the same `Ring`, as the borrow checker will
    /// prevent subsequent calls.
    ///
    /// This method exists for educational purposes and for building low-level abstractions.
    /// For all practical application logic, use the "hot potato" pattern with [`OwnedBuffer`](crate::OwnedBuffer).
    ///
    /// This is a high-level convenience method that creates a write operation,
    /// submits it, and returns a future that can be awaited.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `buffer` - Pinned buffer containing data to write
    ///
    /// # Returns
    ///
    /// Returns a WriteFuture that resolves to (bytes_written, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::from_slice(b"Hello, world!");
    ///
    /// // Write 13 bytes starting at offset 4096
    /// let future = ring.write_at(3, buffer.as_mut_slice(), 4096)?;
    /// let (bytes_written, buffer) = future.await?;
    /// println!("Wrote {} bytes at offset 4096", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_at<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
        offset: u64,
    ) -> Result<WriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::write().fd(fd).buffer(buffer).offset(offset);
        self.submit_write(operation)
    }

    /// Write data using a registered file descriptor.
    ///
    /// This method uses a pre-registered file descriptor for improved performance
    /// by avoiding kernel fd lookups on each operation.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - Registered file descriptor to write to
    /// * `buffer` - Pinned buffer containing data to write
    ///
    /// # Returns
    ///
    /// Returns a WriteFuture that resolves to (bytes_written, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer, Registry};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut registry = Registry::new();
    /// let registered_fd = registry.register_fd(1)?;
    /// let mut buffer = PinnedBuffer::from_slice(b"Hello, world!");
    ///
    /// let future = ring.write_registered(registered_fd, buffer.as_mut_slice())?;
    /// let (bytes_written, buffer) = future.await?;
    /// println!("Wrote {} bytes using registered fd", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_registered<'buf>(
        &'ring mut self,
        registered_fd: crate::registry::RegisteredFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<WriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::write()
            .registered_fd(registered_fd)
            .buffer(buffer);
        self.submit_write(operation)
    }

    /// Write data using a registered buffer.
    ///
    /// This method uses a pre-registered buffer for improved performance
    /// by avoiding kernel buffer validation on each operation.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `registered_buffer` - Registered buffer containing data to write
    ///
    /// # Returns
    ///
    /// Returns a WriteFuture that resolves to (bytes_written, registered_buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, Registry};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut registry = Registry::new();
    /// let buffer = Pin::new(Box::new(*b"Hello, world!"));
    /// let registered_buffer = registry.register_buffer(buffer)?;
    ///
    /// // Note: This is a conceptual example - the actual return type would need
    /// // to be adjusted to handle registered buffers properly
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_with_registered_buffer<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        registered_buffer: crate::registry::RegisteredBuffer,
    ) -> Result<OperationFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::write()
            .fd(fd)
            .registered_buffer(registered_buffer);
        self.submit_operation(operation)
    }

    /// Perform a vectored write operation (writev).
    ///
    /// This writes data from multiple buffers in a single system call,
    /// implementing gather I/O for improved efficiency when
    /// working with non-contiguous data.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `buffers` - Vector of pinned buffers containing data to write
    ///
    /// # Returns
    ///
    /// Returns a VectoredWriteFuture that resolves to (bytes_written, buffers).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted or if the
    /// buffers vector is empty.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::from_slice(b"Hello, ");
    /// let mut buffer2 = PinnedBuffer::from_slice(b"world!");
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// let future = ring.write_vectored(1, buffers)?;
    /// let (bytes_written, buffers) = future.await?;
    /// println!("Wrote {} bytes from {} buffers", bytes_written, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_vectored<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffers: Vec<std::pin::Pin<&'buf mut [u8]>>,
    ) -> Result<VectoredWriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        if buffers.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Vectored write requires at least one buffer",
            )));
        }

        let operation = Operation::write_vectored().fd(fd).buffers(buffers);
        let submitted = self.submit(operation)?;
        Ok(VectoredWriteFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Perform a vectored write operation at a specific offset.
    ///
    /// This combines vectored I/O with positioned writes, writing data from
    /// multiple buffers starting at a specific file offset.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `buffers` - Vector of pinned buffers containing data to write
    /// * `offset` - Byte offset in the file to start writing at
    ///
    /// # Returns
    ///
    /// Returns a VectoredWriteFuture that resolves to (bytes_written, buffers).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted or if the
    /// buffers vector is empty.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::from_slice(b"Hello, ");
    /// let mut buffer2 = PinnedBuffer::from_slice(b"world!");
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// // Write starting at offset 4096
    /// let future = ring.write_vectored_at(3, buffers, 4096)?;
    /// let (bytes_written, buffers) = future.await?;
    /// println!("Wrote {} bytes from {} buffers at offset 4096", bytes_written, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_vectored_at<'buf>(
        &'ring mut self,
        fd: std::os::unix::io::RawFd,
        buffers: Vec<std::pin::Pin<&'buf mut [u8]>>,
        offset: u64,
    ) -> Result<VectoredWriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        if buffers.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Vectored write requires at least one buffer",
            )));
        }

        let operation = Operation::write_vectored()
            .fd(fd)
            .buffers(buffers)
            .offset(offset);
        let submitted = self.submit(operation)?;
        Ok(VectoredWriteFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }
}
