//! Configuration for batch submission behavior.

/// Configuration for batch submission behavior.
///
/// Controls how batches are processed and submitted to the kernel, allowing
/// fine-tuning of performance and error handling characteristics. Different
/// configurations are suitable for different use cases and performance requirements.
///
/// # Fields
///
/// - `fail_fast`: Controls whether to stop processing when an operation fails
/// - `max_batch_size`: Maximum number of operations in a single batch
/// - `enforce_dependencies`: Whether to respect operation dependency ordering
///
/// # Examples
///
/// ## Default Configuration
/// ```rust
/// # use safer_ring::ring::BatchConfig;
/// let config = BatchConfig::default();
/// assert!(!config.fail_fast);
/// assert_eq!(config.max_batch_size, 256);
/// assert!(config.enforce_dependencies);
/// ```
///
/// ## High-throughput Configuration
/// ```rust
/// # use safer_ring::ring::BatchConfig;
/// let config = BatchConfig {
///     fail_fast: false,           // Continue on errors for maximum throughput
///     max_batch_size: 512,        // Larger batches for efficiency
///     enforce_dependencies: false, // Skip dependency checks for speed
/// };
/// ```
///
/// ## Strict Error Handling Configuration
/// ```rust
/// # use safer_ring::ring::BatchConfig;
/// let config = BatchConfig {
///     fail_fast: true,            // Stop immediately on any error
///     max_batch_size: 64,         // Smaller batches for quick error detection
///     enforce_dependencies: true,  // Strict ordering guarantees
/// };
/// ```
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Whether to stop processing when an operation fails.
    ///
    /// - `true`: Stop immediately when any operation fails (fail-fast mode)
    /// - `false`: Continue processing remaining operations even if some fail
    pub fail_fast: bool,

    /// Maximum number of operations to submit in a single batch.
    ///
    /// Larger batch sizes can improve throughput by reducing syscall overhead,
    /// but may increase latency and memory usage. The optimal size depends on
    /// your specific workload and system characteristics.
    pub max_batch_size: usize,

    /// Whether to enforce dependency ordering between operations.
    ///
    /// - `true`: Operations with dependencies are submitted in the correct order
    /// - `false`: Dependency checking is skipped for maximum performance
    ///
    /// Disabling this can improve performance but may lead to incorrect results
    /// if operations have data dependencies.
    pub enforce_dependencies: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            fail_fast: false,
            max_batch_size: 256, // Reasonable default for most use cases
            enforce_dependencies: true,
        }
    }
}
