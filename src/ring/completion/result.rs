//! Completion result types and utilities.

use std::io;
use std::pin::Pin;

use crate::operation::{Completed, Operation};

/// Result of a completed operation.
///
/// Contains the operation result and allows extraction of the buffer
/// ownership for reuse or cleanup. This type ensures that completed
/// operations can be safely consumed to retrieve both the result
/// and the buffer ownership.
#[derive(Debug)]
pub struct CompletionResult<'ring, 'buf> {
    /// The completed operation with its result
    pub(super) operation: Operation<'ring, 'buf, Completed<io::Result<i32>>>,
    /// Operation ID for tracking purposes
    pub(super) operation_id: u64,
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
        Self {
            operation,
            operation_id,
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
    /// // Buffer can now be reused for another operation
    /// ```
    pub fn into_result(self) -> (io::Result<i32>, Option<Pin<&'buf mut [u8]>>) {
        self.operation.into_result_with_buffer()
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
        self.operation.result()
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
        self.operation.fd()
    }

    /// Get the operation type.
    ///
    /// Returns the type of I/O operation that was performed (read, write, etc.).
    #[inline]
    pub fn op_type(&self) -> crate::operation::OperationType {
        self.operation.op_type()
    }

    /// Check if this operation used a buffer.
    ///
    /// Returns `true` if the operation had a buffer associated with it.
    /// This can be useful for determining whether buffer ownership will
    /// be returned when consuming the completion.
    #[inline]
    pub fn has_buffer(&self) -> bool {
        self.operation.has_buffer()
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
