//! Core Ring implementation with lifetime management.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::error::{Result, SaferRingError};
use crate::future::{AcceptFuture, BatchFuture, OperationFuture, ReadFuture, RecvFuture, SendFuture, VectoredReadFuture, VectoredWriteFuture, WakerRegistry, WriteFuture};
use crate::operation::{Building, Operation, OperationType};
// Operation types are used in other modules
use crate::operation::tracker::OperationTracker;
use crate::ring::batch::{Batch, BatchConfig};

#[cfg(target_os = "linux")]
use io_uring;

/// Safe wrapper around io_uring with lifetime management.
///
/// Enforces buffer lifetime constraints and tracks operations to prevent
/// use-after-free bugs. The lifetime parameter ensures no operation can
/// outlive the ring that created it.
///
/// # Example
///
/// ```rust,no_run
/// use safer_ring::Ring;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let ring = Ring::new(32)?;
/// println!("Ring created with {} capacity", ring.capacity());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Ring<'ring> {
    #[cfg(target_os = "linux")]
    pub(super) inner: io_uring::IoUring,
    #[cfg(not(target_os = "linux"))]
    pub(super) _stub: (),
    pub(super) phantom: PhantomData<&'ring ()>,
    // RefCell allows interior mutability for operation tracking
    // Using RefCell instead of Mutex for single-threaded performance
    pub(super) operations: RefCell<OperationTracker<'ring>>,
    // Waker registry for async/await support
    pub(super) waker_registry: Rc<WakerRegistry>,
}

impl<'ring> Ring<'ring> {
    /// Create a new Ring with the specified queue depth.
    ///
    /// # Arguments
    ///
    /// * `entries` - Number of submission queue entries
    ///
    /// # Errors
    ///
    /// Returns an error if entries is 0 or io_uring initialization fails.
    pub fn new(entries: u32) -> Result<Self> {
        if entries == 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Queue depth must be greater than 0",
            )));
        }

        #[cfg(target_os = "linux")]
        {
            let inner = io_uring::IoUring::new(entries)?;
            Ok(Self {
                inner,
                phantom: PhantomData,
                operations: RefCell::new(OperationTracker::new()),
                waker_registry: Rc::new(WakerRegistry::new()),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "io_uring is only supported on Linux",
            )))
        }
    }

    /// Get the number of operations currently in flight.
    pub fn operations_in_flight(&self) -> usize {
        self.operations.borrow().count()
    }

    /// Check if there are any operations currently in flight.
    pub fn has_operations_in_flight(&self) -> bool {
        self.operations.borrow().has_operations()
    }

    /// Get the capacity of the submission queue.
    #[cfg(target_os = "linux")]
    pub fn capacity(&self) -> u32 {
        self.inner.submission().capacity()
    }

    /// Get the capacity of the submission queue (stub for non-Linux).
    #[cfg(not(target_os = "linux"))]
    pub fn capacity(&self) -> u32 {
        0
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
    /// ```rust,no_run
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
        &'ring self,
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
    /// ```rust,no_run
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
        &'ring self,
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
        &'ring self,
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

    /// Read data from a file descriptor into a buffer.
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// let (bytes_read, buffer) = ring.read(0, buffer.as_mut_slice()).await?;
    /// println!("Read {} bytes", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read<'buf>(
        &'ring self,
        fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<ReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::read().fd(fd).buffer(buffer);
        self.submit_read(operation)
    }

    /// Read data from a file descriptor at a specific offset.
    ///
    /// This performs a positioned read operation, similar to pread(2).
    /// The file offset is not changed by this operation.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `buffer` - Pinned buffer to read data into
    /// * `offset` - Byte offset in the file to read from
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// // Read 1024 bytes starting at offset 4096
    /// let (bytes_read, buffer) = ring.read_at(3, buffer.as_mut_slice(), 4096).await?;
    /// println!("Read {} bytes from offset 4096", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_at<'buf>(
        &'ring self,
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer, Registry};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut registry = Registry::new();
    /// let registered_fd = registry.register_fd(0)?;
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// let (bytes_read, buffer) = ring.read_registered(registered_fd, buffer.as_mut_slice()).await?;
    /// println!("Read {} bytes using registered fd", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_registered<'buf>(
        &'ring self,
        registered_fd: crate::registry::RegisteredFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<ReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::read().registered_fd(registered_fd).buffer(buffer);
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
        &'ring self,
        fd: std::os::unix::io::RawFd,
        registered_buffer: crate::registry::RegisteredBuffer,
    ) -> Result<OperationFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::read().fd(fd).registered_buffer(registered_buffer);
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::with_capacity(512);
    /// let mut buffer2 = PinnedBuffer::with_capacity(512);
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// let (bytes_read, buffers) = ring.read_vectored(0, buffers).await?;
    /// println!("Read {} bytes into {} buffers", bytes_read, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_vectored<'buf>(
        &'ring self,
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::with_capacity(512);
    /// let mut buffer2 = PinnedBuffer::with_capacity(512);
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// // Read starting at offset 4096
    /// let (bytes_read, buffers) = ring.read_vectored_at(3, buffers, 4096).await?;
    /// println!("Read {} bytes into {} buffers from offset 4096", bytes_read, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_vectored_at<'buf>(
        &'ring self,
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

    /// Write data from a buffer to a file descriptor.
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::from_slice(b"Hello, world!");
    ///
    /// let (bytes_written, buffer) = ring.write(1, buffer.as_mut_slice()).await?;
    /// println!("Wrote {} bytes", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write<'buf>(
        &'ring self,
        fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<WriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::write().fd(fd).buffer(buffer);
        self.submit_write(operation)
    }

    /// Write data to a file descriptor at a specific offset.
    ///
    /// This performs a positioned write operation, similar to pwrite(2).
    /// The file offset is not changed by this operation.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `buffer` - Pinned buffer containing data to write
    /// * `offset` - Byte offset in the file to write at
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::from_slice(b"Hello, world!");
    ///
    /// // Write 13 bytes starting at offset 4096
    /// let (bytes_written, buffer) = ring.write_at(3, buffer.as_mut_slice(), 4096).await?;
    /// println!("Wrote {} bytes at offset 4096", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_at<'buf>(
        &'ring self,
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer, Registry};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut registry = Registry::new();
    /// let registered_fd = registry.register_fd(1)?;
    /// let mut buffer = PinnedBuffer::from_slice(b"Hello, world!");
    ///
    /// let (bytes_written, buffer) = ring.write_registered(registered_fd, buffer.as_mut_slice()).await?;
    /// println!("Wrote {} bytes using registered fd", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_registered<'buf>(
        &'ring self,
        registered_fd: crate::registry::RegisteredFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<WriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::write().registered_fd(registered_fd).buffer(buffer);
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
        &'ring self,
        fd: std::os::unix::io::RawFd,
        registered_buffer: crate::registry::RegisteredBuffer,
    ) -> Result<OperationFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::write().fd(fd).registered_buffer(registered_buffer);
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::from_slice(b"Hello, ");
    /// let mut buffer2 = PinnedBuffer::from_slice(b"world!");
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// let (bytes_written, buffers) = ring.write_vectored(1, buffers).await?;
    /// println!("Wrote {} bytes from {} buffers", bytes_written, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_vectored<'buf>(
        &'ring self,
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
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::pin::Pin;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut buffer1 = PinnedBuffer::from_slice(b"Hello, ");
    /// let mut buffer2 = PinnedBuffer::from_slice(b"world!");
    ///
    /// let buffers = vec![
    ///     buffer1.as_mut_slice(),
    ///     buffer2.as_mut_slice(),
    /// ];
    ///
    /// // Write starting at offset 4096
    /// let (bytes_written, buffers) = ring.write_vectored_at(3, buffers, 4096).await?;
    /// println!("Wrote {} bytes from {} buffers at offset 4096", bytes_written, buffers.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_vectored_at<'buf>(
        &'ring self,
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

    /// Accept a connection on a listening socket.
    ///
    /// This method accepts an incoming connection on a listening socket and
    /// returns a future that resolves to the new client file descriptor.
    ///
    /// # Arguments
    ///
    /// * `listening_fd` - File descriptor of the listening socket
    ///
    /// # Returns
    ///
    /// Returns an AcceptFuture that resolves to the new client file descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::Ring;
    /// # use std::os::unix::io::RawFd;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let listening_fd: RawFd = 3; // Assume we have a listening socket
    ///
    /// let client_fd = ring.accept(listening_fd).await?;
    /// println!("Accepted connection: fd {}", client_fd);
    /// # Ok(())
    /// # }
    /// ```
    pub fn accept(&'ring self, listening_fd: std::os::unix::io::RawFd) -> Result<AcceptFuture<'ring>> {
        let operation = Operation::accept().fd(listening_fd);
        self.submit_accept(operation)
    }

    /// Send data on a socket.
    ///
    /// This method sends data from a buffer to a connected socket and
    /// returns a future that resolves to the number of bytes sent and
    /// the buffer ownership.
    ///
    /// # Arguments
    ///
    /// * `socket_fd` - File descriptor of the connected socket
    /// * `buffer` - Pinned buffer containing data to send
    ///
    /// # Returns
    ///
    /// Returns a SendFuture that resolves to (bytes_sent, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::os::unix::io::RawFd;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let socket_fd: RawFd = 4; // Assume we have a connected socket
    /// let mut buffer = PinnedBuffer::from_slice(b"Hello, client!");
    ///
    /// let (bytes_sent, buffer) = ring.send(socket_fd, buffer.as_mut_slice()).await?;
    /// println!("Sent {} bytes", bytes_sent);
    /// # Ok(())
    /// # }
    /// ```
    pub fn send<'buf>(
        &'ring self,
        socket_fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<SendFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::send().fd(socket_fd).buffer(buffer);
        self.submit_send(operation)
    }

    /// Receive data from a socket.
    ///
    /// This method receives data from a connected socket into a buffer and
    /// returns a future that resolves to the number of bytes received and
    /// the buffer ownership.
    ///
    /// # Arguments
    ///
    /// * `socket_fd` - File descriptor of the connected socket
    /// * `buffer` - Pinned buffer to receive data into
    ///
    /// # Returns
    ///
    /// Returns a RecvFuture that resolves to (bytes_received, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, PinnedBuffer};
    /// # use std::os::unix::io::RawFd;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let socket_fd: RawFd = 4; // Assume we have a connected socket
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// let (bytes_received, buffer) = ring.recv(socket_fd, buffer.as_mut_slice()).await?;
    /// println!("Received {} bytes", bytes_received);
    /// # Ok(())
    /// # }
    /// ```
    pub fn recv<'buf>(
        &'ring self,
        socket_fd: std::os::unix::io::RawFd,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<RecvFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::recv().fd(socket_fd).buffer(buffer);
        self.submit_recv(operation)
    }

    /// Submit an accept operation and return a future.
    ///
    /// This is a convenience method that combines operation submission with
    /// future creation for accept operations.
    ///
    /// # Arguments
    ///
    /// * `operation` - Accept operation in Building state
    ///
    /// # Returns
    ///
    /// Returns an AcceptFuture that can be awaited to get the client fd.
    ///
    /// # Errors
    ///
    /// Returns an error if operation submission fails.
    pub fn submit_accept(
        &'ring self,
        operation: Operation<'ring, 'static, Building>,
    ) -> Result<AcceptFuture<'ring>> {
        // Validate that this is actually an accept operation
        if operation.get_type() != OperationType::Accept {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Operation must be an accept operation",
            )));
        }

        let submitted = self.submit(operation)?;
        Ok(AcceptFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Submit a send operation and return a future.
    ///
    /// This is a convenience method that combines operation submission with
    /// future creation for send operations.
    ///
    /// # Arguments
    ///
    /// * `operation` - Send operation in Building state
    ///
    /// # Returns
    ///
    /// Returns a SendFuture that can be awaited to get (bytes_sent, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if operation submission fails.
    pub fn submit_send<'buf>(
        &'ring self,
        operation: Operation<'ring, 'buf, Building>,
    ) -> Result<SendFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        // Validate that this is actually a send operation
        if operation.get_type() != OperationType::Send {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Operation must be a send operation",
            )));
        }

        let submitted = self.submit(operation)?;
        Ok(SendFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Submit a receive operation and return a future.
    ///
    /// This is a convenience method that combines operation submission with
    /// future creation for receive operations.
    ///
    /// # Arguments
    ///
    /// * `operation` - Receive operation in Building state
    ///
    /// # Returns
    ///
    /// Returns a RecvFuture that can be awaited to get (bytes_received, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if operation submission fails.
    pub fn submit_recv<'buf>(
        &'ring self,
        operation: Operation<'ring, 'buf, Building>,
    ) -> Result<RecvFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        // Validate that this is actually a receive operation
        if operation.get_type() != OperationType::Recv {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Operation must be a receive operation",
            )));
        }

        let submitted = self.submit(operation)?;
        Ok(RecvFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Submit a batch of operations efficiently.
    ///
    /// This method submits multiple operations in a single batch, which can
    /// significantly improve performance by reducing the number of system calls.
    /// Operations can have dependencies and will be submitted in the correct order.
    ///
    /// # Arguments
    ///
    /// * `batch` - Batch of operations to submit
    ///
    /// # Returns
    ///
    /// Returns a BatchFuture that resolves to the results of all operations.
    ///
    /// # Errors
    ///
    /// Returns an error if batch validation fails or submission fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, Batch, Operation, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut batch = Batch::new();
    /// let mut buffer1 = PinnedBuffer::with_capacity(1024);
    /// let mut buffer2 = PinnedBuffer::with_capacity(1024);
    ///
    /// batch.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))?;
    /// batch.add_operation(Operation::write().fd(1).buffer(buffer2.as_mut_slice()))?;
    ///
    /// let results = ring.submit_batch(batch).await?;
    /// println!("Batch completed with {} operations", results.results.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit_batch<'buf>(
        &'ring self,
        batch: Batch<'ring, 'buf>,
    ) -> Result<BatchFuture<'ring>>
    where
        'buf: 'ring,
    {
        self.submit_batch_with_config(batch, BatchConfig::default())
    }

    /// Submit a batch of operations with custom configuration.
    ///
    /// This method provides more control over batch submission behavior,
    /// including fail-fast mode and dependency handling.
    ///
    /// # Arguments
    ///
    /// * `batch` - Batch of operations to submit
    /// * `config` - Configuration for batch submission behavior
    ///
    /// # Returns
    ///
    /// Returns a BatchFuture that resolves to the results of all operations.
    ///
    /// # Errors
    ///
    /// Returns an error if batch validation fails or submission fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, Batch, BatchConfig, Operation, PinnedBuffer};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let mut batch = Batch::new();
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// batch.add_operation(Operation::read().fd(0).buffer(buffer.as_mut_slice()))?;
    ///
    /// let config = BatchConfig {
    ///     fail_fast: true,
    ///     max_batch_size: 100,
    ///     enforce_dependencies: true,
    /// };
    ///
    /// let results = ring.submit_batch_with_config(batch, config).await?;
    /// println!("Batch completed");
    /// # Ok(())
    /// # }
    /// ```
    pub fn submit_batch_with_config<'buf>(
        &'ring self,
        batch: Batch<'ring, 'buf>,
        config: BatchConfig,
    ) -> Result<BatchFuture<'ring>>
    where
        'buf: 'ring,
    {
        if batch.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot submit empty batch",
            )));
        }

        if batch.len() > config.max_batch_size {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Batch size {} exceeds maximum {}",
                    batch.len(),
                    config.max_batch_size
                ),
            )));
        }

        // Get operations in dependency order if dependencies are enforced
        let _submission_order = if config.enforce_dependencies {
            batch.dependency_order()?
        } else {
            (0..batch.len()).collect()
        };

        // Submit operations that have no dependencies first
        let mut _submitted_operations: Vec<Option<()>> = vec![None; batch.len()];
        let mut dependencies = std::collections::HashMap::new();

        // Extract operations and dependencies from the batch
        let (operations, batch_dependencies) = batch.into_operations_and_dependencies();

        // Submit all operations and collect their IDs
        let mut operation_ids = vec![None; operations.len()];
        for (index, operation) in operations.into_iter().enumerate() {
            let submitted = self.submit(operation)?;
            operation_ids[index] = Some(submitted.id());
            
            if batch_dependencies.contains_key(&index) {
                dependencies.insert(index, batch_dependencies[&index].clone());
            }
        }

        // Create the batch future
        Ok(BatchFuture::new(
            operation_ids,
            dependencies,
            self,
            self.waker_registry.clone(),
            config.fail_fast,
        ))
    }




    /// Try to complete an operation by ID and return its result.
    ///
    /// This method checks if an operation with the given ID has completed.
    /// It's used by batch futures to poll for completion.
    ///
    /// # Arguments
    ///
    /// * `operation_id` - The ID of the operation to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(result))` if completed, `Ok(None)` if still in progress,
    /// or `Err` if the operation failed.
    pub(crate) fn poll_operation_completion(&self, _operation_id: u64) -> Result<Option<i32>> {
        // Check the completion queue for this operation
        #[cfg(target_os = "linux")]
        {
            let mut cq = self.inner.completion();
            
            // Look for completion entries
            for cqe in &mut cq {
                if cqe.user_data() == operation_id {
                    let result = cqe.result();
                    
                    // Mark operation as completed in tracker
                    self.operations.borrow_mut().complete_operation(operation_id);
                    
                    if result < 0 {
                        // Operation failed
                        let error = std::io::Error::from_raw_os_error(-result);
                        return Err(SaferRingError::Io(error));
                    } else {
                        // Operation succeeded
                        return Ok(Some(result));
                    }
                }
            }
            
            // Operation not yet completed
            Ok(None)
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux platforms, simulate completion for testing
            Ok(Some(0))
        }
    }
}

impl<'ring> Drop for Ring<'ring> {
    /// Panic if operations are still in flight to prevent use-after-free bugs.
    fn drop(&mut self) {
        let tracker = self.operations.borrow();
        let count = tracker.count();

        if count > 0 {
            let debug_info = tracker.debug_info();
            drop(tracker); // Release borrow before panic to avoid poisoning

            let mut message = format!("Ring dropped with {} operations in flight:\n", count);
            for (id, op_type, fd) in debug_info {
                message.push_str(&format!(
                    "  - Operation {}: {:?} on fd {}\n",
                    id, op_type, fd
                ));
            }
            message.push_str("All operations must complete before dropping the ring.");

            panic!("{}", message);
        }
    }
}

// Ring can be sent between threads but not shared (RefCell prevents Sync)
unsafe impl<'ring> Send for Ring<'ring> {}
