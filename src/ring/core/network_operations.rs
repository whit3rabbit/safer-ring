//! Network operations (accept, send, recv) for the Ring.

use super::Ring;
use crate::error::{Result, SaferRingError};
use crate::future::{AcceptFuture, RecvFuture, SendFuture};
use crate::operation::{Building, Operation, OperationType};
use std::os::unix::io::RawFd;
use std::pin::Pin;

impl<'ring> Ring<'ring> {
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
    /// # Security Considerations
    ///
    /// **IMPORTANT**: The caller is responsible for ensuring the file descriptor is:
    /// - A valid socket file descriptor in listening state
    /// - Owned by the current process with appropriate permissions
    /// - Properly bound to the intended network interface and port
    /// - Configured with appropriate socket options (e.g., SO_REUSEADDR)
    /// - Not subject to race conditions from other threads
    ///
    /// This library does **NOT** perform socket validation, permission checks,
    /// or network security controls. Using an invalid or malicious file descriptor
    /// may result in:
    /// - Accepting connections on unintended sockets or ports
    /// - Security vulnerabilities or unauthorized network access
    /// - System errors or crashes
    ///
    /// Always validate socket file descriptors and implement proper network
    /// security controls at the application level.
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
    pub fn accept(&'ring mut self, fd: std::os::unix::io::RawFd) -> Result<AcceptFuture<'ring>> {
        let operation = Operation::accept().fd(fd);
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
    /// # Security Considerations
    ///
    /// **IMPORTANT**: The caller is responsible for ensuring the file descriptor is:
    /// - A valid, connected socket file descriptor
    /// - Owned by the current process with appropriate send permissions
    /// - Connected to the intended remote endpoint
    /// - Not subject to race conditions from other threads
    /// - Properly authenticated and authorized for data transmission
    ///
    /// The caller must also ensure the buffer contains only authorized data:
    /// - No sensitive information that should not be transmitted
    /// - Properly validated and sanitized content
    /// - Data appropriate for the connected peer
    ///
    /// This library does **NOT** perform socket validation, connection verification,
    /// data filtering, or access control. Using an invalid file descriptor or 
    /// sending unauthorized data may result in:
    /// - Data transmission to unintended recipients
    /// - Information disclosure or data leaks
    /// - Security vulnerabilities or protocol violations
    /// - System errors or network failures
    ///
    /// Always implement proper network security controls and data validation.
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
        &'ring mut self,
        fd: RawFd,
        buffer: Pin<&'buf mut [u8]>,
    ) -> Result<SendFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::send().fd(fd).buffer(buffer);
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
    /// # Security Considerations
    ///
    /// **IMPORTANT**: The caller is responsible for ensuring the file descriptor is:
    /// - A valid, connected socket file descriptor
    /// - Owned by the current process with appropriate receive permissions
    /// - Connected to a trusted or properly authenticated remote endpoint
    /// - Not subject to race conditions from other threads
    /// - Properly configured for the expected protocol and data format
    ///
    /// After receiving data, the caller must:
    /// - Validate all received data before processing
    /// - Implement proper input sanitization and bounds checking
    /// - Handle untrusted data appropriately for the application protocol
    /// - Consider data confidentiality and integrity requirements
    ///
    /// This library does **NOT** perform socket validation, connection verification,
    /// data validation, or content filtering. Using an invalid file descriptor or
    /// processing untrusted received data may result in:
    /// - Buffer overflow or memory corruption vulnerabilities
    /// - Injection attacks or protocol exploitation
    /// - Information disclosure or data corruption
    /// - System compromise or privilege escalation
    ///
    /// Always implement proper input validation and network security controls.
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
        &'ring mut self,
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
        &'ring mut self,
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
        &'ring mut self,
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
        &'ring mut self,
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
}
