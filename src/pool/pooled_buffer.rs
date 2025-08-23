//! RAII wrapper for pool-managed buffers.

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::buffer::PinnedBuffer;

use super::PoolInner;

/// A buffer borrowed from a pool that automatically returns on drop.
///
/// Provides RAII semantics - the buffer is automatically returned to the pool
/// when dropped, ensuring no buffer leaks even in error conditions.
///
/// # Safety
///
/// Maintains all safety guarantees of [`PinnedBuffer`] while adding
/// automatic pool return semantics.
pub struct PooledBuffer {
    /// The actual buffer (None after being returned)
    buffer: Option<PinnedBuffer<[u8]>>,
    /// Reference to the pool for return on drop
    pool: Arc<Mutex<PoolInner>>,
}

impl PooledBuffer {
    /// Create a new pooled buffer.
    ///
    /// This is an internal constructor used by the pool.
    pub(super) fn new(buffer: PinnedBuffer<[u8]>, pool: Arc<Mutex<PoolInner>>) -> Self {
        Self {
            buffer: Some(buffer),
            pool,
        }
    }

    /// Get a reference to the underlying buffer.
    ///
    /// # Panics
    ///
    /// Panics if the buffer has already been returned (internal bug).
    pub fn buffer(&self) -> &PinnedBuffer<[u8]> {
        self.buffer
            .as_ref()
            .expect("Buffer should be present - this is a bug")
    }

    /// Get a mutable reference to the underlying buffer.
    ///
    /// # Panics
    ///
    /// Panics if the buffer has already been returned (internal bug).
    pub fn buffer_mut(&mut self) -> &mut PinnedBuffer<[u8]> {
        self.buffer
            .as_mut()
            .expect("Buffer should be present - this is a bug")
    }

    /// Get a pinned mutable slice reference to the buffer data.
    ///
    /// This maintains pinning guarantees required for io_uring operations.
    pub fn as_mut_slice(&mut self) -> Pin<&mut [u8]> {
        self.buffer_mut().as_mut_slice()
    }

    /// Get an immutable slice reference to the buffer data.
    pub fn as_slice(&self) -> &[u8] {
        self.buffer().as_slice()
    }

    /// Get the length of the buffer.
    pub fn len(&self) -> usize {
        self.buffer().len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer().is_empty()
    }

    /// Detach the buffer from the pool, taking ownership.
    ///
    /// This removes the buffer from pool management, preventing automatic
    /// return on drop. Use when you need to transfer ownership elsewhere.
    pub fn detach(mut self) -> PinnedBuffer<[u8]> {
        self.buffer
            .take()
            .expect("Buffer should be present - this is a bug")
    }
}

impl Drop for PooledBuffer {
    /// Automatically return the buffer to the pool when dropped.
    ///
    /// Ensures proper resource management even in error conditions.
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            // Return buffer to pool - ignore lock errors during drop
            if let Ok(mut inner) = self.pool.lock() {
                inner.available.push_back(buffer);
                inner.in_use = inner.in_use.saturating_sub(1);
            }
            // If lock fails during drop, we're likely in a panic situation
            // and the buffer will be dropped normally, which is acceptable
        }
    }
}

// Debug implementation that doesn't expose buffer contents
impl std::fmt::Debug for PooledBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledBuffer")
            .field("len", &self.len())
            .field("has_buffer", &self.buffer.is_some())
            .finish()
    }
}

// SAFETY: PooledBuffer can be sent between threads when the underlying buffer is Send
unsafe impl Send for PooledBuffer {}

// SAFETY: PooledBuffer can be shared between threads when the underlying buffer is Sync
// However, since we provide mutable access, it's typically used with exclusive ownership
unsafe impl Sync for PooledBuffer {}
