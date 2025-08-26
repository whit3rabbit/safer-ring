//! Performance profiling and optimization utilities.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Performance counter for tracking operation metrics.
#[derive(Debug)]
pub struct PerfCounter {
    /// Total number of operations
    count: AtomicU64,
    /// Total time spent in operations (nanoseconds)
    total_time_ns: AtomicU64,
    /// Minimum operation time (nanoseconds)
    min_time_ns: AtomicU64,
    /// Maximum operation time (nanoseconds)
    max_time_ns: AtomicU64,
}

impl PerfCounter {
    /// Create a new performance counter.
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            total_time_ns: AtomicU64::new(0),
            min_time_ns: AtomicU64::new(u64::MAX),
            max_time_ns: AtomicU64::new(0),
        }
    }

    /// Record an operation duration.
    pub fn record(&self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;

        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_time_ns.fetch_add(nanos, Ordering::Relaxed);

        // Update min (with compare-and-swap loop)
        let mut current_min = self.min_time_ns.load(Ordering::Relaxed);
        while nanos < current_min {
            match self.min_time_ns.compare_exchange_weak(
                current_min,
                nanos,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }

        // Update max (with compare-and-swap loop)
        let mut current_max = self.max_time_ns.load(Ordering::Relaxed);
        while nanos > current_max {
            match self.max_time_ns.compare_exchange_weak(
                current_max,
                nanos,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
    }

    /// Get performance statistics.
    pub fn stats(&self) -> PerfStats {
        let count = self.count.load(Ordering::Relaxed);
        let total_ns = self.total_time_ns.load(Ordering::Relaxed);
        let min_ns = self.min_time_ns.load(Ordering::Relaxed);
        let max_ns = self.max_time_ns.load(Ordering::Relaxed);

        let avg_ns = if count > 0 { total_ns / count } else { 0 };

        PerfStats {
            count,
            total_time: Duration::from_nanos(total_ns),
            avg_time: Duration::from_nanos(avg_ns),
            min_time: if min_ns == u64::MAX {
                Duration::ZERO
            } else {
                Duration::from_nanos(min_ns)
            },
            max_time: Duration::from_nanos(max_ns),
        }
    }

    /// Reset all counters.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.total_time_ns.store(0, Ordering::Relaxed);
        self.min_time_ns.store(u64::MAX, Ordering::Relaxed);
        self.max_time_ns.store(0, Ordering::Relaxed);
    }
}

impl Default for PerfCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance statistics snapshot.
#[derive(Debug, Clone)]
pub struct PerfStats {
    /// Number of operations recorded
    pub count: u64,
    /// Total time across all operations
    pub total_time: Duration,
    /// Average time per operation
    pub avg_time: Duration,
    /// Minimum operation time
    pub min_time: Duration,
    /// Maximum operation time
    pub max_time: Duration,
}

impl PerfStats {
    /// Calculate operations per second.
    pub fn ops_per_sec(&self) -> f64 {
        if self.total_time.is_zero() {
            0.0
        } else {
            self.count as f64 / self.total_time.as_secs_f64()
        }
    }

    /// Calculate throughput in bytes per second.
    pub fn throughput_bps(&self, bytes_per_op: u64) -> f64 {
        self.ops_per_sec() * bytes_per_op as f64
    }
}

/// RAII timer for automatic performance measurement.
pub struct PerfTimer<'a> {
    counter: &'a PerfCounter,
    start: Instant,
}

impl<'a> PerfTimer<'a> {
    /// Start timing an operation.
    pub fn new(counter: &'a PerfCounter) -> Self {
        Self {
            counter,
            start: Instant::now(),
        }
    }
}

impl<'a> Drop for PerfTimer<'a> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.counter.record(duration);
    }
}

/// Memory usage tracker.
#[derive(Debug)]
pub struct MemoryTracker {
    /// Current allocated bytes
    allocated: AtomicUsize,
    /// Peak allocated bytes
    peak: AtomicUsize,
    /// Total allocations
    total_allocs: AtomicU64,
    /// Total deallocations
    total_deallocs: AtomicU64,
}

impl MemoryTracker {
    /// Create a new memory tracker.
    pub fn new() -> Self {
        Self {
            allocated: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            total_allocs: AtomicU64::new(0),
            total_deallocs: AtomicU64::new(0),
        }
    }

    /// Record an allocation.
    pub fn record_alloc(&self, size: usize) {
        let new_allocated = self.allocated.fetch_add(size, Ordering::Relaxed) + size;
        self.total_allocs.fetch_add(1, Ordering::Relaxed);

        // Update peak
        let mut current_peak = self.peak.load(Ordering::Relaxed);
        while new_allocated > current_peak {
            match self.peak.compare_exchange_weak(
                current_peak,
                new_allocated,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_peak = x,
            }
        }
    }

    /// Record a deallocation.
    pub fn record_dealloc(&self, size: usize) {
        self.allocated.fetch_sub(size, Ordering::Relaxed);
        self.total_deallocs.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current memory usage.
    pub fn current_usage(&self) -> usize {
        self.allocated.load(Ordering::Relaxed)
    }

    /// Get peak memory usage.
    pub fn peak_usage(&self) -> usize {
        self.peak.load(Ordering::Relaxed)
    }

    /// Get memory statistics.
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            current: self.allocated.load(Ordering::Relaxed),
            peak: self.peak.load(Ordering::Relaxed),
            total_allocs: self.total_allocs.load(Ordering::Relaxed),
            total_deallocs: self.total_deallocs.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters.
    pub fn reset(&self) {
        self.allocated.store(0, Ordering::Relaxed);
        self.peak.store(0, Ordering::Relaxed);
        self.total_allocs.store(0, Ordering::Relaxed);
        self.total_deallocs.store(0, Ordering::Relaxed);
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory usage statistics.
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Current allocated bytes
    pub current: usize,
    /// Peak allocated bytes
    pub peak: usize,
    /// Total number of allocations
    pub total_allocs: u64,
    /// Total number of deallocations
    pub total_deallocs: u64,
}

impl MemoryStats {
    /// Calculate allocation rate (allocs per second).
    pub fn alloc_rate(&self, duration: Duration) -> f64 {
        if duration.is_zero() {
            0.0
        } else {
            self.total_allocs as f64 / duration.as_secs_f64()
        }
    }

    /// Calculate average allocation size.
    pub fn avg_alloc_size(&self) -> f64 {
        if self.total_allocs > 0 {
            self.peak as f64 / self.total_allocs as f64
        } else {
            0.0
        }
    }
}

/// Global performance registry for collecting metrics across the library.
pub struct PerfRegistry {
    counters: std::collections::HashMap<String, Arc<PerfCounter>>,
    memory_tracker: Arc<MemoryTracker>,
}

impl PerfRegistry {
    /// Create a new performance registry.
    pub fn new() -> Self {
        Self {
            counters: std::collections::HashMap::new(),
            memory_tracker: Arc::new(MemoryTracker::new()),
        }
    }

    /// Get or create a performance counter.
    pub fn counter(&mut self, name: &str) -> Arc<PerfCounter> {
        self.counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(PerfCounter::new()))
            .clone()
    }

    /// Get the memory tracker.
    pub fn memory_tracker(&self) -> Arc<MemoryTracker> {
        self.memory_tracker.clone()
    }

    /// Get all counter statistics.
    pub fn all_stats(&self) -> std::collections::HashMap<String, PerfStats> {
        self.counters
            .iter()
            .map(|(name, counter)| (name.clone(), counter.stats()))
            .collect()
    }

    /// Reset all counters.
    pub fn reset_all(&self) {
        for counter in self.counters.values() {
            counter.reset();
        }
        self.memory_tracker.reset();
    }
}

impl Default for PerfRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Macro for easy performance timing.
#[macro_export]
macro_rules! time_operation {
    ($counter:expr, $operation:expr) => {{
        let _timer = $crate::perf::PerfTimer::new($counter);
        $operation
    }};
}

/// Macro for timing async operations.
#[macro_export]
macro_rules! time_async_operation {
    ($counter:expr, $operation:expr) => {{
        let start = std::time::Instant::now();
        let result = $operation.await;
        $counter.record(start.elapsed());
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_perf_counter() {
        let counter = PerfCounter::new();

        counter.record(Duration::from_millis(10));
        counter.record(Duration::from_millis(20));
        counter.record(Duration::from_millis(5));

        let stats = counter.stats();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_time, Duration::from_millis(5));
        assert_eq!(stats.max_time, Duration::from_millis(20));
        assert!(stats.avg_time >= Duration::from_millis(11));
        assert!(stats.avg_time <= Duration::from_millis(12));
    }

    #[test]
    fn test_perf_timer() {
        let counter = PerfCounter::new();

        {
            let _timer = PerfTimer::new(&counter);
            thread::sleep(Duration::from_millis(10));
        }

        let stats = counter.stats();
        assert_eq!(stats.count, 1);
        assert!(stats.total_time >= Duration::from_millis(9));
    }

    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();

        tracker.record_alloc(1024);
        assert_eq!(tracker.current_usage(), 1024);
        assert_eq!(tracker.peak_usage(), 1024);

        tracker.record_alloc(2048);
        assert_eq!(tracker.current_usage(), 3072);
        assert_eq!(tracker.peak_usage(), 3072);

        tracker.record_dealloc(1024);
        assert_eq!(tracker.current_usage(), 2048);
        assert_eq!(tracker.peak_usage(), 3072); // Peak doesn't decrease
    }

    #[test]
    fn test_perf_registry() {
        let mut registry = PerfRegistry::new();

        let counter1 = registry.counter("test1");
        let counter2 = registry.counter("test2");

        counter1.record(Duration::from_millis(10));
        counter2.record(Duration::from_millis(20));

        let all_stats = registry.all_stats();
        assert_eq!(all_stats.len(), 2);
        assert!(all_stats.contains_key("test1"));
        assert!(all_stats.contains_key("test2"));
    }

    #[test]
    fn test_concurrent_perf_counter() {
        let counter = Arc::new(PerfCounter::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let counter_clone = counter.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    counter_clone.record(Duration::from_nanos(1000));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = counter.stats();
        assert_eq!(stats.count, 1000);
    }
}
