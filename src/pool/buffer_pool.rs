//! Thread-safe buffer pool implementation.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::buffer::PinnedBuffer;
use crate::error::Result;

use super::{PoolInner, PoolStats, PooledBuffer};

/// Thread-safe pool of pre-allocated pinned buffers.
///
/// Pre-allocates all buffers during construction and pins them in memory,
/// making them immediately ready for io_uring operations. This eliminates
/// allocation overhead and ensures predictable performance.
///
/// # Thread Safety
///
/// [`BufferPool`] is [`Send`] and [`Sync`] - internal synchronization
/// is handled through a [`Mutex`] protecting the pool state.
///
/// # Memory Management
///
/// All buffers are pre-allocated and remain allocated for the pool's lifetime.
/// This provides predictable memory usage but requires careful sizing.
///
/// # Examples
///
/// ```rust
/// use safer_ring::pool::BufferPool;
///
/// let pool = BufferPool::new(10, 4096);
/// if let Some(buffer) = pool.try_get().unwrap() {
///     // Use buffer for I/O operations
///     // Buffer automatically returns to pool on drop
/// }
/// ```
pub struct BufferPool {
    /// Shared pool state protected by mutex
    inner: Arc<Mutex<PoolInner>>,
    /// Immutable capacity - stored outside mutex for lock-free access
    capacity: usize,
    /// Immutable buffer size - stored outside mutex for lock-free access  
    buffer_size: usize,
}

impl BufferPool {
    /// Create a new buffer pool with specified capacity and buffer size.
    ///
    /// All buffers are pre-allocated and pinned during construction.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Number of buffers to pre-allocate (must be > 0)
    /// * `buffer_size` - Size of each buffer in bytes (must be > 0)
    ///
    /// # Panics
    ///
    /// Panics if `capacity` or `buffer_size` is zero.
    pub fn new(capacity: usize, buffer_size: usize) -> Self {
        assert!(capacity > 0, "Pool capacity must be greater than zero");
        assert!(buffer_size > 0, "Buffer size must be greater than zero");

        let mut available = VecDeque::with_capacity(capacity);

        // Pre-allocate all buffers - this is the key optimization
        for _ in 0..capacity {
            available.push_back(PinnedBuffer::with_capacity(buffer_size));
        }

        Self {
            inner: Arc::new(Mutex::new(PoolInner {
                available,
                in_use: 0,
                total_allocations: 0,
                failed_allocations: 0,
            })),
            capacity,
            buffer_size,
        }
    }

    /// Create a buffer pool with custom buffer initialization.
    ///
    /// Allows more control over buffer initialization, such as pre-filling
    /// buffers with specific patterns.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Number of buffers to create
    /// * `buffer_factory` - Closure that creates each buffer
    ///
    /// # Panics
    ///
    /// Panics if buffers have inconsistent sizes.
    pub fn with_factory<F>(capacity: usize, mut buffer_factory: F) -> Self
    where
        F: FnMut() -> PinnedBuffer<[u8]>,
    {
        assert!(capacity > 0, "Pool capacity must be greater than zero");

        let mut available = VecDeque::with_capacity(capacity);
        let mut buffer_size = 0;

        for i in 0..capacity {
            let buffer = buffer_factory();
            if i == 0 {
                buffer_size = buffer.len();
            } else {
                // Ensure consistency - all buffers must be same size
                assert_eq!(
                    buffer.len(),
                    buffer_size,
                    "All buffers must have the same size"
                );
            }
            available.push_back(buffer);
        }

        Self {
            inner: Arc::new(Mutex::new(PoolInner {
                available,
                in_use: 0,
                total_allocations: 0,
                failed_allocations: 0,
            })),
            capacity,
            buffer_size,
        }
    }

    /// Try to get a buffer from the pool.
    ///
    /// Returns `Some(PooledBuffer)` if available, `None` if pool is empty.
    /// Never blocks and never allocates new buffers.
    pub fn try_get(&self) -> Result<Option<PooledBuffer>> {
        let mut inner = PoolInner::lock(&self.inner)?;

        if let Some(buffer) = inner.available.pop_front() {
            inner.in_use += 1;
            inner.total_allocations += 1;

            Ok(Some(PooledBuffer::new(buffer, Arc::clone(&self.inner))))
        } else {
            inner.failed_allocations += 1;
            Ok(None)
        }
    }

    /// Get a buffer from the pool, blocking until one becomes available.
    ///
    /// This method blocks the current thread until a buffer is returned.
    /// Uses exponential backoff to reduce CPU usage while waiting.
    ///
    /// # Performance Note
    ///
    /// For high-performance applications, consider using `try_get()` in a loop
    /// with your own scheduling strategy rather than this blocking method.
    pub fn get_blocking(&self) -> Result<PooledBuffer> {
        let mut backoff_us = 1;
        const MAX_BACKOFF_US: u64 = 1000; // Cap at 1ms

        loop {
            if let Some(buffer) = self.try_get()? {
                return Ok(buffer);
            }

            // Exponential backoff to reduce CPU usage
            std::thread::sleep(std::time::Duration::from_micros(backoff_us));
            backoff_us = (backoff_us * 2).min(MAX_BACKOFF_US);
        }
    }

    /// Get the total capacity of the pool.
    ///
    /// This is a lock-free operation since capacity is immutable.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the number of available buffers.
    pub fn available(&self) -> Result<usize> {
        let inner = PoolInner::lock(&self.inner)?;
        Ok(inner.available.len())
    }

    /// Get the number of buffers currently in use.
    pub fn in_use(&self) -> Result<usize> {
        let inner = PoolInner::lock(&self.inner)?;
        Ok(inner.in_use)
    }

    /// Get the size of each buffer in the pool.
    ///
    /// This is a lock-free operation since buffer size is immutable.
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Get comprehensive statistics about the pool.
    ///
    /// Returns detailed information about pool usage and allocation patterns.
    /// Takes a single lock to ensure all statistics are consistent.
    pub fn stats(&self) -> Result<PoolStats> {
        let inner = PoolInner::lock(&self.inner)?;
        let utilization = inner.in_use as f64 / self.capacity as f64;

        Ok(PoolStats {
            capacity: self.capacity,
            available: inner.available.len(),
            in_use: inner.in_use,
            buffer_size: self.buffer_size,
            total_allocations: inner.total_allocations,
            failed_allocations: inner.failed_allocations,
            utilization,
        })
    }

    /// Check if the pool is empty (no available buffers).
    pub fn is_empty(&self) -> Result<bool> {
        let inner = PoolInner::lock(&self.inner)?;
        Ok(inner.available.is_empty())
    }

    /// Check if the pool is full (all buffers available).
    pub fn is_full(&self) -> Result<bool> {
        let inner = PoolInner::lock(&self.inner)?;
        Ok(inner.available.len() == self.capacity)
    }

    /// Get a snapshot of current pool state in a single lock operation.
    ///
    /// More efficient than calling multiple methods separately when you need
    /// multiple pieces of information about the pool state.
    pub fn snapshot(&self) -> Result<(usize, usize, bool, bool)> {
        let inner = PoolInner::lock(&self.inner)?;
        let available = inner.available.len();
        let in_use = inner.in_use;
        let is_empty = inner.available.is_empty();
        let is_full = available == self.capacity;

        Ok((available, in_use, is_empty, is_full))
    }
}

// SAFETY: BufferPool can be safely sent between threads
unsafe impl Send for BufferPool {}

// SAFETY: BufferPool can be safely shared between threads
unsafe impl Sync for BufferPool {}
