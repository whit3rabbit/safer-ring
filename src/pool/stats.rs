//! Pool statistics and monitoring.

/// Statistics about buffer pool usage.
///
/// Provides insights into pool performance and utilization patterns.
/// All statistics are captured at a specific point in time and may
/// become stale as the pool state changes.
///
/// # Examples
///
/// ```rust
/// use safer_ring::pool::BufferPool;
///
/// let pool = BufferPool::new(10, 4096);
/// let stats = pool.stats();
///
/// println!("Pool utilization: {:.1}%", stats.utilization_percent());
/// println!("Success rate: {:.1}%", stats.success_rate_percent());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolStats {
    /// Total capacity of the pool
    pub capacity: usize,
    /// Number of buffers currently available
    pub available: usize,
    /// Number of buffers currently in use
    pub in_use: usize,
    /// Size of each buffer in bytes
    pub buffer_size: usize,
    /// Total successful allocations since pool creation
    pub total_allocations: u64,
    /// Total failed allocation attempts since pool creation
    pub failed_allocations: u64,
    /// Current utilization as a ratio (0.0 to 1.0)
    pub utilization: f64,
}

impl PoolStats {
    /// Get utilization as a percentage (0.0 to 100.0).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use safer_ring::pool::PoolStats;
    /// let stats = PoolStats {
    ///     capacity: 10,
    ///     available: 3,
    ///     in_use: 7,
    ///     buffer_size: 4096,
    ///     total_allocations: 100,
    ///     failed_allocations: 5,
    ///     utilization: 0.7,
    /// };
    /// assert_eq!(stats.utilization_percent(), 70.0);
    /// ```
    pub fn utilization_percent(&self) -> f64 {
        self.utilization * 100.0
    }

    /// Get the success rate of allocations as a percentage (0.0 to 100.0).
    ///
    /// Returns 100.0 if no allocation attempts have been made.
    pub fn success_rate_percent(&self) -> f64 {
        let total_attempts = self.total_allocations + self.failed_allocations;
        if total_attempts == 0 {
            100.0 // No attempts means perfect success rate
        } else {
            (self.total_allocations as f64 / total_attempts as f64) * 100.0
        }
    }

    /// Get the total memory allocated by the pool in bytes.
    pub fn total_memory_bytes(&self) -> usize {
        self.capacity * self.buffer_size
    }

    /// Get the memory currently in use in bytes.
    pub fn memory_in_use_bytes(&self) -> usize {
        self.in_use * self.buffer_size
    }

    /// Check if the pool is under high pressure (utilization > 80%).
    ///
    /// This can be used to trigger alerts or scaling decisions.
    pub fn is_under_pressure(&self) -> bool {
        self.utilization > 0.8
    }

    /// Check if the pool has experienced allocation failures.
    pub fn has_allocation_failures(&self) -> bool {
        self.failed_allocations > 0
    }
}
