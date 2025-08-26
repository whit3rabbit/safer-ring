//! Buffer ownership management for safe io_uring operations.
//!
//! This module implements the core safety mechanism of safer-ring: explicit buffer ownership
//! tracking. When a buffer is used in an io_uring operation, ownership is transferred to the
//! kernel for the duration of the operation, preventing use-after-free bugs and data races.
//!
//! # Core Principles
//!
//! 1. **Ownership Transfer**: Buffers are explicitly owned by either userspace or kernel
//! 2. **Cancellation Safety**: Dropped futures leave buffers with kernel until completion
//! 3. **Memory Safety**: No access to buffers while kernel owns them
//! 4. **Hot Potato Pattern**: Buffers are given and returned explicitly
//!
//! # Example
//!
//! ```rust,no_run
//! use safer_ring::ownership::OwnedBuffer;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a buffer that can be safely transferred to kernel
//! let buffer = OwnedBuffer::new(1024);
//!
//! // Buffer can be accessed when owned by userspace
//! if let Some(mut guard) = buffer.try_access() {
//!     guard[0] = 42; // Safe to access
//! }
//!
//! // When used in io_uring operations, kernel takes ownership
//! // buffer.give_to_kernel(operation_id)?;
//! // No access possible until kernel returns ownership
//! # Ok(())
//! # }
//! ```

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use crate::error::{Result, SaferRingError};

/// Buffer ownership states during io_uring operations.
///
/// This enum tracks the explicit ownership of buffers between userspace and kernel,
/// ensuring memory safety during asynchronous operations.
#[derive(Debug)]
pub enum BufferOwnership {
    /// Buffer is owned by userspace and can be safely accessed.
    User(Box<[u8]>),
    /// Buffer is owned by kernel for an ongoing operation.
    /// The buffer is preserved along with the submission ID for tracking.
    Kernel(Box<[u8]>, u64),
    /// Buffer ownership is being transferred back from kernel.
    /// Temporary state during completion processing.
    Returning,
}

/// A handle to a buffer that can be safely transferred between userspace and kernel.
///
/// This is the fundamental abstraction that makes io_uring safe. It ensures that:
/// - Buffers cannot be accessed while owned by the kernel
/// - Kernel operations cannot use freed buffers
/// - Cancellation cannot cause use-after-free bugs
///
/// # Lifetime Safety
///
/// The buffer handle maintains memory safety through explicit ownership tracking.
/// When an operation is cancelled (future dropped), the buffer remains with the
/// kernel until the operation completes, at which point ownership is returned.
#[derive(Debug)]
pub struct OwnedBuffer {
    inner: Arc<Mutex<BufferOwnership>>,
    size: usize,
    generation: u64, // For debugging and lifecycle tracking
}

impl OwnedBuffer {
    /// Create a new owned buffer with the specified size.
    ///
    /// The buffer is initially owned by userspace and can be accessed immediately.
    ///
    /// # Arguments
    ///
    /// * `size` - Size of the buffer in bytes
    ///
    /// # Example
    ///
    /// ```rust
    /// use safer_ring::ownership::OwnedBuffer;
    ///
    /// let buffer = OwnedBuffer::new(4096);
    /// println!("Created buffer of {} bytes", buffer.size());
    /// ```
    pub fn new(size: usize) -> Self {
        let buffer = vec![0u8; size].into_boxed_slice();
        Self {
            inner: Arc::new(Mutex::new(BufferOwnership::User(buffer))),
            size,
            generation: 0, // TODO: Use atomic counter for unique generations
        }
    }

    /// Create a new owned buffer from existing data.
    ///
    /// Takes ownership of the provided data and makes it available for io_uring operations.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to take ownership of
    ///
    /// # Example
    ///
    /// ```rust
    /// use safer_ring::ownership::OwnedBuffer;
    ///
    /// let data = b"Hello, world!";
    /// let buffer = OwnedBuffer::from_slice(data);
    /// ```
    pub fn from_slice(data: &[u8]) -> Self {
        let buffer = data.to_vec().into_boxed_slice();
        let size = buffer.len();
        Self {
            inner: Arc::new(Mutex::new(BufferOwnership::User(buffer))),
            size,
            generation: 0,
        }
    }

    /// Get the size of the buffer in bytes.
    ///
    /// This returns the allocated size regardless of current ownership state.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the generation counter for debugging.
    ///
    /// Each buffer has a unique generation ID for lifecycle tracking.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Try to get exclusive access to the buffer.
    ///
    /// Returns `Some(BufferAccessGuard)` if the buffer is currently owned by userspace,
    /// or `None` if the kernel currently owns it.
    ///
    /// The guard provides RAII access to the buffer data and automatically returns
    /// ownership to the buffer when dropped.
    ///
    /// # Example
    ///
    /// ```rust
    /// use safer_ring::ownership::OwnedBuffer;
    ///
    /// let mut buffer = OwnedBuffer::new(1024);
    ///
    /// if let Some(mut guard) = buffer.try_access() {
    ///     guard[0] = 42;
    ///     guard[1] = 24;
    /// } else {
    ///     println!("Buffer is currently owned by kernel");
    /// }
    /// ```
    pub fn try_access(&self) -> Option<BufferAccessGuard> {
        let mut ownership = self.inner.lock().unwrap();
        match &mut *ownership {
            BufferOwnership::User(ref mut buf) => {
                // Temporarily take ownership for the guard
                let buffer = std::mem::replace(buf, Box::new([]));
                *ownership = BufferOwnership::Returning; // Mark as transitioning
                Some(BufferAccessGuard {
                    buffer,
                    ownership: self.inner.clone(),
                })
            }
            BufferOwnership::Kernel(_, _) => None, // Kernel owns it
            BufferOwnership::Returning => None,    // In transition
        }
    }

    /// Transfer ownership to the kernel for an io_uring operation.
    ///
    /// This method is called internally when submitting operations. It marks the buffer
    /// as owned by the kernel and returns the raw buffer pointer for the operation.
    ///
    /// # Arguments
    ///
    /// * `submission_id` - Unique ID for the operation taking ownership
    ///
    /// # Returns
    ///
    /// Returns the raw buffer pointer and size for the io_uring operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is not currently owned by userspace.
    pub fn give_to_kernel(&self, submission_id: u64) -> Result<(*mut u8, usize)> {
        let mut ownership = self.inner.lock().unwrap();
        match &mut *ownership {
            BufferOwnership::User(buf) => {
                let ptr = buf.as_mut_ptr();
                let len = buf.len();
                // Move buffer to kernel ownership while preserving the actual buffer
                let buffer = std::mem::replace(buf, Box::new([]));
                *ownership = BufferOwnership::Kernel(buffer, submission_id);
                Ok((ptr, len))
            }
            BufferOwnership::Kernel(_, existing_id) => {
                Err(SaferRingError::Io(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    format!("Buffer already owned by kernel (operation {})", existing_id),
                )))
            }
            BufferOwnership::Returning => Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "Buffer ownership is currently transitioning",
            ))),
        }
    }

    /// Return ownership from kernel after operation completion.
    ///
    /// This method is called internally when an io_uring operation completes.
    /// It transfers ownership back to userspace and makes the buffer accessible again.
    ///
    /// # Arguments
    ///
    /// * `submission_id` - ID of the completed operation
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not currently owned by the kernel or if the
    /// submission ID doesn't match.
    pub fn return_from_kernel(&self, submission_id: u64) {
        let mut ownership = self.inner.lock().unwrap();
        match &mut *ownership {
            BufferOwnership::Kernel(buffer, id) if *id == submission_id => {
                // Move buffer back to user ownership
                let buffer = std::mem::replace(buffer, Box::new([]));
                *ownership = BufferOwnership::User(buffer);
            }
            BufferOwnership::Kernel(_, id) => {
                panic!(
                    "Buffer owned by operation {} but tried to return from operation {}",
                    id, submission_id
                );
            }
            _ => {
                panic!("Tried to return buffer that isn't owned by kernel");
            }
        }
    }

    /// Check if the buffer is currently owned by userspace.
    ///
    /// This is useful for debugging and assertions.
    pub fn is_user_owned(&self) -> bool {
        let ownership = self.inner.lock().unwrap();
        matches!(*ownership, BufferOwnership::User(_))
    }

    /// Check if the buffer is currently owned by the kernel.
    ///
    /// Returns the submission ID if owned by kernel, None otherwise.
    pub fn kernel_owner(&self) -> Option<u64> {
        let ownership = self.inner.lock().unwrap();
        match *ownership {
            BufferOwnership::Kernel(_, id) => Some(id),
            _ => None,
        }
    }

    /// Clone the buffer handle.
    ///
    /// Both handles refer to the same underlying buffer with shared ownership tracking.
    /// This is useful for tracking buffers across multiple contexts.
    pub fn clone_handle(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            size: self.size,
            generation: self.generation,
        }
    }

    /// Get raw pointer and length for safe operations.
    ///
    /// This method provides access to the buffer's raw pointer and length
    /// for use in safe operations where ownership is explicitly managed
    /// through the SafeOperation tracking system.
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid while the buffer exists and
    /// should only be used with proper ownership tracking.
    pub fn as_ptr_and_len(&self) -> (*mut u8, usize) {
        let ownership = self.inner.lock().unwrap();
        match &*ownership {
            BufferOwnership::User(buf) => (buf.as_ptr() as *mut u8, buf.len()),
            BufferOwnership::Kernel(_, _) => (std::ptr::null_mut(), 0),
            BufferOwnership::Returning => (std::ptr::null_mut(), 0),
        }
    }
}

/// RAII guard for safe buffer access.
///
/// This guard provides temporary exclusive access to buffer data while ensuring
/// that ownership is properly returned when the guard is dropped.
///
/// The guard implements `Deref` and `DerefMut` for convenient access to the
/// underlying buffer data.
pub struct BufferAccessGuard {
    buffer: Box<[u8]>,
    ownership: Arc<Mutex<BufferOwnership>>,
}

impl BufferAccessGuard {
    /// Get the size of the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get a raw pointer to the buffer data.
    ///
    /// # Safety
    ///
    /// The pointer is valid only while the guard exists and should not be
    /// stored beyond the guard's lifetime.
    pub fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr()
    }

    /// Get a mutable raw pointer to the buffer data.
    ///
    /// # Safety
    ///
    /// The pointer is valid only while the guard exists and should not be
    /// stored beyond the guard's lifetime.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buffer.as_mut_ptr()
    }
}

impl Drop for BufferAccessGuard {
    fn drop(&mut self) {
        // Return the buffer to user ownership
        let buffer = std::mem::replace(&mut self.buffer, Box::new([]));
        let mut ownership = self.ownership.lock().unwrap();
        *ownership = BufferOwnership::User(buffer);
    }
}

impl Deref for BufferAccessGuard {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buffer
    }
}

impl DerefMut for BufferAccessGuard {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
}

/// Marker trait for types that can be safely used as io_uring buffers.
///
/// This trait is automatically implemented for `OwnedBuffer` and ensures that
/// only safe buffer types can be used with io_uring operations.
pub trait SafeBuffer {
    /// Get the size of the buffer in bytes.
    fn size(&self) -> usize;

    /// Transfer ownership to the kernel and get raw buffer info.
    fn give_to_kernel(&self, submission_id: u64) -> Result<(*mut u8, usize)>;

    /// Return ownership from the kernel after operation completion.
    fn return_from_kernel(&self, submission_id: u64);
}

impl SafeBuffer for OwnedBuffer {
    fn size(&self) -> usize {
        self.size()
    }

    fn give_to_kernel(&self, submission_id: u64) -> Result<(*mut u8, usize)> {
        self.give_to_kernel(submission_id)
    }

    fn return_from_kernel(&self, submission_id: u64) {
        self.return_from_kernel(submission_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_owned_buffer_creation() {
        let buffer = OwnedBuffer::new(1024);
        assert_eq!(buffer.size(), 1024);
        assert!(buffer.is_user_owned());
        assert_eq!(buffer.kernel_owner(), None);
    }

    #[test]
    fn test_buffer_access_guard() {
        let buffer = OwnedBuffer::new(1024);

        {
            let mut guard = buffer.try_access().unwrap();
            assert_eq!(guard.len(), 1024);
            guard[0] = 42;
            guard[1] = 24;
        } // Guard dropped here

        // Buffer should be accessible again
        let guard = buffer.try_access().unwrap();
        assert_eq!(guard[0], 42);
        assert_eq!(guard[1], 24);
    }

    #[test]
    fn test_kernel_ownership_transfer() {
        let buffer = OwnedBuffer::new(1024);

        // Give to kernel
        let (ptr, size) = buffer.give_to_kernel(123).unwrap();
        assert!(!ptr.is_null());
        assert_eq!(size, 1024);
        assert!(!buffer.is_user_owned());
        assert_eq!(buffer.kernel_owner(), Some(123));

        // Should not be accessible while kernel owns it
        assert!(buffer.try_access().is_none());

        // Should fail to give to kernel again
        assert!(buffer.give_to_kernel(456).is_err());
    }

    #[test]
    fn test_return_from_kernel() {
        let buffer = OwnedBuffer::new(1024);

        // Give to kernel and then return
        let (_ptr, _size) = buffer.give_to_kernel(123).unwrap();

        // Return from kernel - buffer is preserved internally
        buffer.return_from_kernel(123);

        // Should be accessible again
        assert!(buffer.is_user_owned());
        assert!(buffer.try_access().is_some());
    }

    #[test]
    #[should_panic(
        expected = "Buffer owned by operation 123 but tried to return from operation 456"
    )]
    fn test_mismatched_submission_id_panic() {
        let buffer = OwnedBuffer::new(1024);
        let (_ptr, _size) = buffer.give_to_kernel(123).unwrap();

        // Return from kernel with wrong ID - should panic
        buffer.return_from_kernel(456); // Wrong ID - should panic
    }

    #[test]
    fn test_buffer_from_slice() {
        let data = b"Hello, world!";
        let buffer = OwnedBuffer::from_slice(data);

        assert_eq!(buffer.size(), data.len());
        let guard = buffer.try_access().unwrap();
        assert_eq!(&*guard, data);
    }

    #[test]
    fn test_clone_handle() {
        let buffer = OwnedBuffer::new(1024);
        let cloned = buffer.clone_handle();

        // Both handles should refer to the same buffer
        assert_eq!(buffer.size(), cloned.size());
        assert_eq!(buffer.generation(), cloned.generation());

        // Accessing through one handle should affect the other
        buffer.give_to_kernel(123).unwrap();
        assert!(!cloned.is_user_owned());
        assert_eq!(cloned.kernel_owner(), Some(123));
    }
}
