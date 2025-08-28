//! Generation tracking for buffer lifecycle management.

use std::sync::atomic::{AtomicU64, Ordering};

/// Atomic generation counter for tracking buffer lifecycle events.
///
/// This counter provides thread-safe tracking of buffer state changes throughout
/// its lifecycle. Each buffer operation (allocation, use, reuse) can increment
/// the generation to help with debugging buffer lifecycle issues and detecting
/// use-after-free scenarios in development builds.
///
/// The counter uses relaxed atomic ordering since exact ordering of generation
/// updates across threads is not critical for its debugging purposes.
///
/// # Thread Safety
///
/// All operations on `GenerationCounter` are thread-safe and can be called
/// concurrently from multiple threads.
///
/// # Examples
///
/// ```rust
/// use safer_ring::buffer::GenerationCounter;
///
/// let counter = GenerationCounter::new();
/// assert_eq!(counter.get(), 0);
///
/// counter.increment();
/// assert_eq!(counter.get(), 1);
///
/// counter.set(42);
/// assert_eq!(counter.get(), 42);
/// ```
#[derive(Debug)]
pub struct GenerationCounter {
    /// Atomic counter that tracks buffer lifecycle events
    counter: AtomicU64,
}

impl GenerationCounter {
    /// Creates a new generation counter starting at 0.
    #[inline]
    pub const fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Returns the current generation value.
    #[inline]
    pub fn get(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }

    /// Increments the generation counter atomically.
    #[inline]
    pub fn increment(&self) {
        self.counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Sets the generation counter to a specific value.
    #[inline]
    pub fn set(&self, value: u64) {
        self.counter.store(value, Ordering::Relaxed);
    }
}

impl Default for GenerationCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for GenerationCounter {
    /// Creates a new counter with the same generation value.
    fn clone(&self) -> Self {
        Self {
            counter: AtomicU64::new(self.get()),
        }
    }
}
