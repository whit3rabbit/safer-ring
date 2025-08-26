//! Generation tracking for buffer lifecycle management.

use std::sync::atomic::{AtomicU64, Ordering};

/// Atomic generation counter for tracking buffer lifecycle events.
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
