//! Completion result types and utilities.

use std::io;

use crate::operation::{tracker::BufferOwnership, Completed, Operation, OperationType};
use std::os::unix::io::RawFd;

/// Result of a completed operation.
///
/// Contains the operation result and allows extraction of the buffer
/// ownership for reuse or cleanup. This type ensures that completed
/// operations can be safely consumed to retrieve both the result
/// and the buffer ownership.
#[derive(Debug)]
pub struct CompletionResult<'ring, 'buf> {
    /// The completed operation with its result (if using the future API)
    pub(super) operation: Option<Operation<'ring, 'buf, Completed<io::Result<i32>>>>,
    /// Operation ID for tracking purposes
    pub(super) operation_id: u64,
    /// Operation type
    pub(super) op_type: OperationType,
    /// File descriptor used
    pub(super) fd: RawFd,
    /// Operation result
    pub(super) result: io::Result<i32>,
    /// Buffer ownership (if any)
    pub(super) buffer: Option<BufferOwnership>,
}

impl<'ring, 'buf> CompletionResult<'ring, 'buf> {
    /// Create a new completion result.
    ///
    /// This is used internally by the completion processor to wrap
    /// completed operations with their tracking information.
    #[allow(dead_code)] // Used by completion processor
    pub(super) fn new(
        operation: Operation<'ring, 'buf, Completed<io::Result<i32>>>,
        operation_id: u64,
    ) -> Self {
        // Extract information from the operation
        let op_type = operation.op_type();
        let fd = operation.fd();
        let result = match operation.result() {
            Ok(bytes) => Ok(*bytes),
            Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
        };

        Self {
            operation: Some(operation),
            operation_id,
            op_type,
            fd,
            result,
            buffer: None,
        }
    }

    /// Create a new completion result with buffer ownership.
    ///
    /// This is used by the polling API to create completion results
    /// that include buffer ownership for proper cleanup.
    pub(super) fn new_with_buffer(
        operation_id: u64,
        op_type: OperationType,
        fd: RawFd,
        result: io::Result<i32>,
        buffer: Option<BufferOwnership>,
    ) -> Self {
        Self {
            operation: None,
            operation_id,
            op_type,
            fd,
            result,
            buffer,
        }
    }

    /// Extract the result and buffer from the completed operation.
    ///
    /// Returns a tuple of (result, buffer) where the buffer can be reused
    /// for subsequent operations. This consumes the completion result,
    /// ensuring proper ownership transfer.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - The I/O operation result (bytes transferred or error)
    /// - The buffer (if one was used), returned for reuse
    ///
    /// # Note
    ///
    /// **Current Limitation**: The current implementation always returns `None`
    /// for buffer ownership because the API uses borrowed references rather than
    /// owned buffers. This is a known limitation that affects the polling API.
    ///
    /// Future enhancements will add an owned buffer API that can properly transfer
    /// buffer ownership in the polling API. For now, users retain ownership of
    /// their buffers throughout the operation lifecycle.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::CompletionResult;
    /// # let completion: CompletionResult<'_, '_> = todo!();
    /// let (result, buffer) = completion.into_result();
    /// match result {
    ///     Ok(bytes) => println!("Transferred {} bytes", bytes),
    ///     Err(e) => eprintln!("I/O error: {}", e),
    /// }
    /// // Note: buffer is currently always None due to the borrowed reference API
    /// assert!(buffer.is_none());
    /// // Users retain ownership of their original buffers
    /// ```
    pub fn into_result(self) -> (io::Result<i32>, Option<BufferOwnership>) {
        // Return the stored result and buffer ownership
        // Currently self.buffer is always None due to the borrowed reference API
        // This is a known limitation that will be addressed in future versions
        (self.result, self.buffer)
    }

    /// Get a reference to the result without consuming the completion.
    ///
    /// This allows inspecting the result while keeping the completion intact.
    /// Useful when you need to check the result before deciding whether to
    /// consume the completion.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::CompletionResult;
    /// # let completion: CompletionResult<'_, '_> = todo!();
    /// match completion.result() {
    ///     Ok(bytes) => println!("Operation succeeded with {} bytes", bytes),
    ///     Err(e) => println!("Operation failed: {}", e),
    /// }
    /// // Completion is still available for consumption
    /// ```
    #[inline]
    pub fn result(&self) -> &io::Result<i32> {
        if let Some(ref operation) = self.operation {
            operation.result()
        } else {
            &self.result
        }
    }

    /// Get the operation ID.
    ///
    /// Returns the unique identifier that was assigned to this operation
    /// when it was submitted. This can be useful for correlating completions
    /// with submitted operations in logging or debugging scenarios.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::CompletionResult;
    /// # let completion: CompletionResult<'_, '_> = todo!();
    /// println!("Operation {} completed", completion.id());
    /// ```
    #[inline]
    pub fn id(&self) -> u64 {
        self.operation_id
    }

    /// Get the file descriptor used by this operation.
    ///
    /// Returns the file descriptor that was used for the I/O operation.
    /// This can be useful for logging or debugging purposes.
    #[inline]
    pub fn fd(&self) -> std::os::unix::io::RawFd {
        if let Some(ref operation) = self.operation {
            operation.fd()
        } else {
            self.fd
        }
    }

    /// Get the operation type.
    ///
    /// Returns the type of I/O operation that was performed (read, write, etc.).
    #[inline]
    pub fn op_type(&self) -> crate::operation::OperationType {
        if let Some(ref operation) = self.operation {
            operation.op_type()
        } else {
            self.op_type
        }
    }

    /// Check if this operation used a buffer.
    ///
    /// Returns `true` if the operation had a buffer associated with it.
    /// This can be useful for determining whether buffer ownership will
    /// be returned when consuming the completion.
    #[inline]
    pub fn has_buffer(&self) -> bool {
        if let Some(ref operation) = self.operation {
            operation.has_buffer()
        } else {
            self.buffer.is_some()
        }
    }

    /// Check if the operation was successful.
    ///
    /// Returns `true` if the operation completed without an error.
    /// This is a convenience method that's equivalent to checking
    /// `completion.result().is_ok()`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::CompletionResult;
    /// # let completion: CompletionResult<'_, '_> = todo!();
    /// if completion.is_success() {
    ///     println!("Operation completed successfully");
    /// } else {
    ///     println!("Operation failed");
    /// }
    /// ```
    #[inline]
    pub fn is_success(&self) -> bool {
        self.result().is_ok()
    }

    /// Get the number of bytes transferred (if successful).
    ///
    /// Returns `Some(bytes)` if the operation was successful, `None` if it failed.
    /// This is a convenience method for extracting the byte count from successful
    /// operations without having to match on the result.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::CompletionResult;
    /// # let completion: CompletionResult<'_, '_> = todo!();
    /// if let Some(bytes) = completion.bytes_transferred() {
    ///     println!("Transferred {} bytes", bytes);
    /// }
    /// ```
    #[inline]
    pub fn bytes_transferred(&self) -> Option<i32> {
        match self.result() {
            Ok(bytes) => Some(*bytes),
            Err(_) => None,
        }
    }
}
