//! Buffer pool for efficient buffer reuse.
//!
//! This module provides thread-safe buffer pooling for io_uring operations,
//! eliminating allocation overhead in hot paths while maintaining memory safety.

mod buffer_pool;
mod pooled_buffer;
mod stats;

#[cfg(test)]
mod tests;

pub use buffer_pool::BufferPool;
pub use pooled_buffer::PooledBuffer;
pub use stats::PoolStats;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::buffer::PinnedBuffer;
use crate::error::{Result, SaferRingError};

/// Internal pool state protected by mutex for thread safety.
///
/// Only contains mutable state - immutable fields like capacity and buffer_size
/// are stored directly in BufferPool for lock-free access.
pub(crate) struct PoolInner {
    /// Available buffers - FIFO queue ensures fair allocation
    pub(crate) available: VecDeque<PinnedBuffer<[u8]>>,
    /// Number of buffers currently in use
    pub(crate) in_use: usize,
    /// Total successful allocations (for monitoring)
    pub(crate) total_allocations: u64,
    /// Total failed allocation attempts (for monitoring)
    pub(crate) failed_allocations: u64,
}

impl PoolInner {
    /// Helper to lock pool state with proper error handling.
    pub(crate) fn lock(inner: &Arc<Mutex<PoolInner>>) -> Result<MutexGuard<'_, PoolInner>> {
        inner.lock().map_err(|_| SaferRingError::PoolPoisoned)
    }
}
