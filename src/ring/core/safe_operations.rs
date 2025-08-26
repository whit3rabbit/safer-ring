//! Safe ownership transfer operations (hot potato pattern) for the Ring.

use super::Ring;
use crate::error::{Result, SaferRingError};
use crate::ownership::OwnedBuffer;
use crate::safety::{SafeOperation, SafeOperationFuture};
use std::os::unix::io::RawFd;
use std::sync::Arc;

impl<'ring> Ring<'ring> {
    /// Read with ownership transfer (hot potato pattern).
    ///
    /// You give the buffer, the kernel uses it, and you get it back when done.
    /// This is the core safe API pattern that prevents use-after-free bugs.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `buffer` - Buffer to read into (ownership transferred)
    ///
    /// # Returns
    ///
    /// A future that resolves to `(bytes_read, buffer)` when the operation completes.
    /// The buffer is returned with the result, implementing the hot potato pattern.
    ///
    /// # Safety
    ///
    /// This method is completely safe. The buffer ownership is transferred to the
    /// kernel during the operation, preventing any use-after-free issues.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use safer_ring::{Ring, OwnedBuffer};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let buffer = OwnedBuffer::new(1024);
    ///
    /// // Hot potato: give buffer, get it back
    /// let (bytes_read, buffer) = ring.read_owned(0, buffer).await?;
    /// println!("Read {} bytes", bytes_read);
    ///
    /// // Can reuse the same buffer
    /// let (bytes_read2, _buffer) = ring.read_owned(0, buffer).await?;
    /// println!("Read {} more bytes", bytes_read2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_owned(&self, fd: RawFd, buffer: OwnedBuffer) -> SafeOperationFuture<'_> {
        // Generate unique submission ID
        let submission_id = {
            let mut tracker = self.orphan_tracker.lock().unwrap();
            tracker.next_submission_id()
        };

        // Submit the operation to the backend
        match self.submit_safe_read(fd, &buffer, submission_id) {
            Ok(_) => {
                // Create safe operation with ownership transfer
                let operation =
                    SafeOperation::new(buffer, submission_id, Arc::downgrade(&self.orphan_tracker));

                operation.into_future(self, self.waker_registry.clone())
            }
            Err(_e) => {
                // If submission fails, create a failed future
                SafeOperation::failed(buffer, submission_id, Arc::downgrade(&self.orphan_tracker))
                    .into_future(self, self.waker_registry.clone())
            }
        }
    }

    /// Write with ownership transfer (hot potato pattern).
    ///
    /// You give the buffer, the kernel uses it, and you get it back when done.
    /// This is the core safe API pattern that prevents use-after-free bugs.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `buffer` - Buffer to write from (ownership transferred)
    ///
    /// # Returns
    ///
    /// A future that resolves to `(bytes_written, buffer)` when the operation completes.
    /// The buffer is returned with the result, implementing the hot potato pattern.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use safer_ring::{Ring, OwnedBuffer};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let buffer = OwnedBuffer::from_slice(b"Hello, world!");
    ///
    /// // Hot potato: give buffer, get it back
    /// let (bytes_written, buffer) = ring.write_owned(1, buffer).await?;
    /// println!("Wrote {} bytes", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_owned(&self, fd: RawFd, buffer: OwnedBuffer) -> SafeOperationFuture<'_> {
        // Generate unique submission ID
        let submission_id = {
            let mut tracker = self.orphan_tracker.lock().unwrap();
            tracker.next_submission_id()
        };

        // Submit the operation to the backend
        match self.submit_safe_write(fd, &buffer, submission_id) {
            Ok(_) => {
                // Create safe operation with ownership transfer
                let operation =
                    SafeOperation::new(buffer, submission_id, Arc::downgrade(&self.orphan_tracker));

                operation.into_future(self, self.waker_registry.clone())
            }
            Err(_e) => {
                // If submission fails, create a failed future
                SafeOperation::failed(buffer, submission_id, Arc::downgrade(&self.orphan_tracker))
                    .into_future(self, self.waker_registry.clone())
            }
        }
    }

    /// Accept connection with safe operation tracking.
    ///
    /// Unlike buffer operations, accept doesn't need buffer ownership transfer,
    /// but still uses the safe operation pattern for consistency.
    ///
    /// # Arguments
    ///
    /// * `fd` - Listening socket file descriptor
    ///
    /// # Returns
    ///
    /// A future that resolves to the accepted client file descriptor.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use safer_ring::Ring;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    /// let listening_fd = 3; // Assume we have a listening socket
    ///
    /// let client_fd = ring.accept_safe(listening_fd).await?;
    /// println!("Accepted client on fd {}", client_fd);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn accept_safe(&self, _fd: RawFd) -> Result<RawFd> {
        // Generate unique submission ID
        let _submission_id = {
            let mut tracker = self.orphan_tracker.lock().unwrap();
            tracker.next_submission_id()
        };

        // In a real implementation, we would:
        // 1. Submit io_uring accept operation with submission_id
        // 2. Return future that polls completion queue
        // 3. Handle orphaned operations if future is dropped

        // For now, return an error indicating this is not implemented
        Err(SaferRingError::Io(std::io::Error::other(
            "accept_safe not yet fully implemented - demonstration API only",
        )))
    }

    /// Get ring-managed buffer for operations.
    ///
    /// This provides a buffer from the ring's internal pool, eliminating
    /// the need for users to manage buffer allocation and ownership.
    ///
    /// # Arguments
    ///
    /// * `size` - Size of buffer to allocate
    ///
    /// # Returns
    ///
    /// A buffer owned by the ring that can be used in operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use safer_ring::Ring;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ring = Ring::new(32)?;
    ///
    /// // Ring provides the buffer
    /// let buffer = ring.get_buffer(4096)?;
    /// let (bytes_read, buffer) = ring.read_owned(0, buffer).await?;
    /// println!("Read {} bytes using ring buffer", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_buffer(&self, size: usize) -> Result<OwnedBuffer> {
        // In a real implementation, this would use a buffer pool
        // For now, just create a new buffer
        Ok(OwnedBuffer::new(size))
    }

    /// Submit a safe read operation to the backend.
    ///
    /// This method submits the read operation directly to the backend using
    /// the buffer's raw pointer, since we have ownership transfer semantics.
    fn submit_safe_read(&self, fd: RawFd, buffer: &OwnedBuffer, submission_id: u64) -> Result<()> {
        let (buffer_ptr, buffer_len) = buffer.as_ptr_and_len();

        self.backend.borrow_mut().submit_operation(
            crate::operation::OperationType::Read,
            fd,
            0, // offset 0 for simple reads
            buffer_ptr,
            buffer_len,
            submission_id,
        )
    }

    /// Submit a safe write operation to the backend.
    ///
    /// This method submits the write operation directly to the backend using
    /// the buffer's raw pointer, since we have ownership transfer semantics.
    fn submit_safe_write(&self, fd: RawFd, buffer: &OwnedBuffer, submission_id: u64) -> Result<()> {
        let (buffer_ptr, buffer_len) = buffer.as_ptr_and_len();

        self.backend.borrow_mut().submit_operation(
            crate::operation::OperationType::Write,
            fd,
            0, // offset 0 for simple writes
            buffer_ptr,
            buffer_len,
            submission_id,
        )
    }
}
