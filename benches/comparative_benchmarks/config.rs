//! Benchmark configuration constants.

use std::time::Duration;

/// Default ring size for io_uring operations.
///
/// Balances memory usage with batching efficiency - small enough to avoid
/// excessive memory overhead while large enough for meaningful batching.
pub const DEFAULT_RING_SIZE: u32 = 32;

/// Chunk size for file operations.
///
/// Optimized for typical page cache behavior (64KB aligns with common
/// readahead sizes and provides good balance between syscall overhead
/// and memory usage).
pub const FILE_CHUNK_SIZE: usize = 64 * 1024;

/// Number of echo operations per network benchmark iteration.
///
/// Provides sufficient work to amortize setup costs while keeping
/// individual benchmark runs reasonably fast.
pub const ECHO_OPERATIONS_PER_ITER: usize = 10;

/// Benchmark measurement time for detailed analysis.
///
/// Longer measurement time provides more stable results for
/// performance analysis at the cost of benchmark duration.
pub const MEASUREMENT_TIME: Duration = Duration::from_secs(10);

/// Sample size for statistical significance.
pub const SAMPLE_SIZE: usize = 50;

/// Detailed analysis sample size for better statistics.
///
/// Higher sample count for detailed analysis provides better
/// statistical confidence in percentile calculations.
pub const DETAILED_SAMPLE_SIZE: usize = 100;
