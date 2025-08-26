//! Configuration for batch submission behavior.

/// Configuration for batch submission behavior.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Whether to continue submitting operations if one fails
    pub fail_fast: bool,
    /// Maximum number of operations to submit in a single batch
    pub max_batch_size: usize,
    /// Whether to enforce dependency ordering
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
