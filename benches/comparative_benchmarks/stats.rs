//! Performance statistics collection and analysis.

use std::time::Duration;

/// Comprehensive performance statistics for benchmark analysis.
#[derive(Debug, Default, Clone)]
pub struct PerformanceStats {
    operations: u64,
    bytes_processed: u64,
    total_time: Duration,
    min_time: Option<Duration>,
    max_time: Option<Duration>,
    times: Vec<Duration>,
}

impl PerformanceStats {
    /// Creates a new statistics collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a single operation's performance metrics.
    ///
    /// Tracks both throughput (bytes/time) and latency (operation duration)
    /// for comprehensive performance characterization.
    pub fn record_operation(&mut self, bytes: u64, duration: Duration) {
        self.operations += 1;
        self.bytes_processed += bytes;
        self.total_time += duration;

        // Update min/max with efficient single-pass logic
        self.min_time = Some(self.min_time.map_or(duration, |min| min.min(duration)));
        self.max_time = Some(self.max_time.map_or(duration, |max| max.max(duration)));
        self.times.push(duration);
    }

    /// Calculates throughput in megabytes per second.
    pub fn throughput_mbps(&self) -> f64 {
        if self.total_time.as_secs_f64() > 0.0 {
            (self.bytes_processed as f64) / (1024.0 * 1024.0) / self.total_time.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Calculates average latency in microseconds.
    pub fn avg_latency_us(&self) -> f64 {
        if self.operations > 0 {
            self.total_time.as_micros() as f64 / self.operations as f64
        } else {
            0.0
        }
    }

    /// Calculates latency percentiles for distribution analysis.
    ///
    /// Returns (P50, P95, P99) percentiles to understand
    /// both typical and tail latency characteristics.
    pub fn calculate_percentiles(&self) -> (Duration, Duration, Duration) {
        let mut sorted_times = self.times.clone();
        sorted_times.sort();

        let len = sorted_times.len();
        if len == 0 {
            return (Duration::ZERO, Duration::ZERO, Duration::ZERO);
        }

        // Use proper percentile calculation with bounds checking
        let p50_idx = len / 2;
        let p95_idx = (len * 95) / 100;
        let p99_idx = (len * 99) / 100;

        let p50 = sorted_times[p50_idx.min(len - 1)];
        let p95 = sorted_times[p95_idx.min(len - 1)];
        let p99 = sorted_times[p99_idx.min(len - 1)];

        (p50, p95, p99)
    }

    /// Returns the total number of operations recorded.
    pub fn operations(&self) -> u64 {
        self.operations
    }

    /// Returns the minimum operation time, if any operations were recorded.
    pub fn min_time(&self) -> Option<Duration> {
        self.min_time
    }

    /// Returns the maximum operation time, if any operations were recorded.
    pub fn max_time(&self) -> Option<Duration> {
        self.max_time
    }

    /// Prints detailed statistics in a formatted report.
    pub fn print_detailed_report(&self, label: &str) {
        let (p50, p95, p99) = self.calculate_percentiles();

        println!("\n📈 {} Detailed Statistics:", label);
        println!("   Operations: {}", self.operations);
        println!("   Throughput: {:.2} MB/s", self.throughput_mbps());
        println!("   Avg Latency: {:.2} μs", self.avg_latency_us());
        println!("   P50 Latency: {:?}", p50);
        println!("   P95 Latency: {:?}", p95);
        println!("   P99 Latency: {:?}", p99);

        if let Some(min) = self.min_time {
            println!("   Min Operation: {:?}", min);
        }
        if let Some(max) = self.max_time {
            println!("   Max Operation: {:?}", max);
        }
    }
}
