//! Advanced configuration for io_uring operations.

/// Advanced features configuration for io_uring operations.
///
/// Enables configuration of advanced io_uring features that may not be
/// available on all kernel versions. Features are disabled by default
/// to ensure compatibility. Use the provided constructors and methods
/// to create configurations suitable for your use case.
///
/// # Examples
///
/// ```rust
/// use safer_ring::advanced::AdvancedConfig;
///
/// // Create a conservative configuration (all features disabled)
/// let conservative = AdvancedConfig::default();
///
/// // Create a configuration with basic stable features
/// let basic = AdvancedConfig::stable();
///
/// // Create a performance-optimized configuration
/// let performance = AdvancedConfig::performance();
///
/// // Enable specific features
/// let custom = AdvancedConfig::default()
///     .with_buffer_selection(true)
///     .with_multi_shot(true)
///     .with_fast_poll(true);
/// ```
///
/// # Feature Categories
///
/// Features are grouped by kernel version:
/// - **Legacy** (5.1-5.19): Basic io_uring features
/// - **6.1+**: Advanced task management
/// - **6.2+**: Zero-copy optimizations and batching
/// - **6.3-6.5**: Enhanced resource management
/// - **6.6+**: Performance and cancellation improvements
/// - **6.11-6.12**: Latest optimizations
///
/// # Safety and Compatibility
///
/// Some features may have stability or performance implications:
/// - `sq_poll`: High CPU usage, use with caution
/// - `defer_async_work`: Requires careful application integration
/// - `user_allocated_ring_memory`: Requires custom memory management
///
/// Use `validate()` to check feature compatibility before use.
#[derive(Debug, Clone, Default)]
pub struct AdvancedConfig {
    /// Enable buffer selection for read operations
    pub buffer_selection: bool,
    /// Enable multi-shot operations where supported
    pub multi_shot: bool,
    /// Enable provided buffers for zero-copy operations
    pub provided_buffers: bool,
    /// Enable fast poll for network operations
    pub fast_poll: bool,
    /// Enable submission queue polling
    pub sq_poll: bool,
    /// Enable io_uring kernel thread affinity
    pub sq_thread_cpu: Option<u32>,
    /// Enable cooperative task running
    pub coop_taskrun: bool,
    /// Enable task work defer
    pub defer_taskrun: bool,

    // Kernel 6.1+ features
    /// Enable advanced task work management (6.1+)
    pub advanced_task_work: bool,
    /// Enable deferred async work until GETEVENTS (6.1+)
    pub defer_async_work: bool,

    // Kernel 6.2+ features
    /// Enable zero-copy send reporting (6.2+)
    pub send_zc_reporting: bool,
    /// Enable completion batching for multishot operations (6.2+)
    pub multishot_completion_batching: bool,
    /// Enable EPOLL_URING_WAKE support (6.2+)
    pub epoll_uring_wake: bool,

    // Kernel 6.3+ features
    /// Enable io_uring_register() with registered ring fd (6.3+)
    pub registered_ring_fd: bool,

    // Kernel 6.4+ features
    /// Enable multishot timeout operations (6.4+)
    pub multishot_timeouts: bool,

    // Kernel 6.5+ features
    /// Enable user-allocated ring memory (6.5+)
    pub user_allocated_ring_memory: bool,

    // Kernel 6.6+ features
    /// Enable async operation cancellation (6.6+)
    pub async_cancel_op: bool,
    /// Enable socket command support (6.6+)
    pub socket_command_support: bool,
    /// Enable Direct I/O performance optimizations (6.6+)
    pub direct_io_optimizations: bool,

    // Kernel 6.11+ features
    /// Enable MSG_RING performance improvements (6.11+)
    pub msg_ring_speedup: bool,
    /// Enable native bind/listen operations (6.11+)
    pub native_bind_listen: bool,

    // Kernel 6.12+ features
    /// Enable improved huge page handling (6.12+)
    pub improved_huge_pages: bool,
    /// Enable async discard operations (6.12+)
    pub async_discard: bool,
    /// Enable minimum timeout waits (6.12+)
    pub minimum_timeout_waits: bool,
    /// Enable absolute timeouts with multiple clock sources (6.12+)
    pub absolute_timeouts: bool,
    /// Enable incremental buffer consumption (6.12+)
    pub incremental_buffer_consumption: bool,
    /// Enable registered buffer cloning (6.12+)
    pub registered_buffer_cloning: bool,
}

impl AdvancedConfig {
    /// Create a new AdvancedConfig with all features disabled.
    ///
    /// This is equivalent to `AdvancedConfig::default()` but more explicit.
    /// Provides maximum compatibility across all kernel versions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::new();
    /// assert!(!config.buffer_selection);
    /// assert!(!config.multi_shot);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with stable, well-tested features enabled.
    ///
    /// Enables features that are considered stable and have minimal
    /// compatibility or performance risks. Suitable for production use
    /// on kernels 5.13+.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::stable();
    /// assert!(config.buffer_selection);
    /// assert!(config.fast_poll);
    /// assert!(!config.sq_poll); // Disabled due to CPU usage concerns
    /// ```
    pub fn stable() -> Self {
        Self {
            // Core stable features (5.13+)
            buffer_selection: true,
            fast_poll: true,
            provided_buffers: true,

            // Task management features (6.0+)
            coop_taskrun: true,
            defer_taskrun: true,

            // Keep potentially problematic features disabled
            sq_poll: false,
            defer_async_work: false,
            user_allocated_ring_memory: false,

            ..Self::default()
        }
    }

    /// Create a performance-optimized configuration.
    ///
    /// Enables features that can improve performance, including some
    /// that may have higher resource usage. Suitable for high-performance
    /// applications on modern kernels (6.2+).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::performance();
    /// assert!(config.multi_shot);
    /// assert!(config.advanced_task_work);
    /// assert!(config.multishot_completion_batching);
    /// ```
    pub fn performance() -> Self {
        Self {
            // All stable features
            ..Self::stable()
        }
        .with_multi_shot(true)
        .with_advanced_task_work(true)
        .with_send_zc_reporting(true)
        .with_multishot_completion_batching(true)
        .with_async_cancel_op(true)
        .with_direct_io_optimizations(true)
    }

    /// Create a development-friendly configuration.
    ///
    /// Enables features that are helpful for development and testing,
    /// prioritizing debuggability over maximum performance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::development();
    /// assert!(config.buffer_selection);
    /// assert!(!config.sq_poll); // Disabled for easier debugging
    /// ```
    pub fn development() -> Self {
        Self {
            // Enable features useful for development
            buffer_selection: true,
            multi_shot: true,
            provided_buffers: true,
            fast_poll: true,
            coop_taskrun: true,
            defer_taskrun: true,

            // Disable features that complicate debugging
            sq_poll: false,
            defer_async_work: false,

            ..Self::default()
        }
    }

    /// Enable buffer selection feature.
    ///
    /// Buffer selection allows the kernel to choose buffers for read operations,
    /// improving efficiency for workloads with varying buffer sizes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::new().with_buffer_selection(true);
    /// assert!(config.buffer_selection);
    /// ```
    pub fn with_buffer_selection(mut self, enabled: bool) -> Self {
        self.buffer_selection = enabled;
        self
    }

    /// Enable multi-shot operations.
    ///
    /// Multi-shot operations can handle multiple completions from a single
    /// submission, reducing system call overhead for operations like accept().
    pub fn with_multi_shot(mut self, enabled: bool) -> Self {
        self.multi_shot = enabled;
        self
    }

    /// Enable provided buffers for zero-copy operations.
    pub fn with_provided_buffers(mut self, enabled: bool) -> Self {
        self.provided_buffers = enabled;
        self
    }

    /// Enable fast poll for network operations.
    pub fn with_fast_poll(mut self, enabled: bool) -> Self {
        self.fast_poll = enabled;
        self
    }

    /// Enable submission queue polling with optional CPU affinity.
    ///
    /// **Warning**: SQ polling uses dedicated CPU resources and should be
    /// used carefully in production environments.
    pub fn with_sq_poll(mut self, enabled: bool, cpu: Option<u32>) -> Self {
        self.sq_poll = enabled;
        self.sq_thread_cpu = if enabled { cpu } else { None };
        self
    }

    /// Enable cooperative task running.
    pub fn with_coop_taskrun(mut self, enabled: bool) -> Self {
        self.coop_taskrun = enabled;
        self
    }

    /// Enable task work defer.
    pub fn with_defer_taskrun(mut self, enabled: bool) -> Self {
        self.defer_taskrun = enabled;
        self
    }

    /// Enable advanced task work management (6.1+).
    pub fn with_advanced_task_work(mut self, enabled: bool) -> Self {
        self.advanced_task_work = enabled;
        self
    }

    /// Enable zero-copy send reporting (6.2+).
    pub fn with_send_zc_reporting(mut self, enabled: bool) -> Self {
        self.send_zc_reporting = enabled;
        self
    }

    /// Enable completion batching for multishot operations (6.2+).
    pub fn with_multishot_completion_batching(mut self, enabled: bool) -> Self {
        self.multishot_completion_batching = enabled;
        self
    }

    /// Enable async operation cancellation (6.6+).
    pub fn with_async_cancel_op(mut self, enabled: bool) -> Self {
        self.async_cancel_op = enabled;
        self
    }

    /// Enable Direct I/O performance optimizations (6.6+).
    pub fn with_direct_io_optimizations(mut self, enabled: bool) -> Self {
        self.direct_io_optimizations = enabled;
        self
    }

    /// Get a list of all enabled features.
    ///
    /// Returns a vector of feature names that are currently enabled.
    /// Useful for logging, debugging, or feature validation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::stable();
    /// let enabled = config.enabled_features();
    /// println!("Enabled features: {:?}", enabled);
    /// ```
    pub fn enabled_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();

        // Legacy features
        if self.buffer_selection {
            features.push("buffer_selection");
        }
        if self.multi_shot {
            features.push("multi_shot");
        }
        if self.provided_buffers {
            features.push("provided_buffers");
        }
        if self.fast_poll {
            features.push("fast_poll");
        }
        if self.sq_poll {
            features.push("sq_poll");
        }
        if self.coop_taskrun {
            features.push("coop_taskrun");
        }
        if self.defer_taskrun {
            features.push("defer_taskrun");
        }

        // 6.1+ features
        if self.advanced_task_work {
            features.push("advanced_task_work");
        }
        if self.defer_async_work {
            features.push("defer_async_work");
        }

        // 6.2+ features
        if self.send_zc_reporting {
            features.push("send_zc_reporting");
        }
        if self.multishot_completion_batching {
            features.push("multishot_completion_batching");
        }
        if self.epoll_uring_wake {
            features.push("epoll_uring_wake");
        }

        // 6.3+ features
        if self.registered_ring_fd {
            features.push("registered_ring_fd");
        }

        // 6.4+ features
        if self.multishot_timeouts {
            features.push("multishot_timeouts");
        }

        // 6.5+ features
        if self.user_allocated_ring_memory {
            features.push("user_allocated_ring_memory");
        }

        // 6.6+ features
        if self.async_cancel_op {
            features.push("async_cancel_op");
        }
        if self.socket_command_support {
            features.push("socket_command_support");
        }
        if self.direct_io_optimizations {
            features.push("direct_io_optimizations");
        }

        // 6.11+ features
        if self.msg_ring_speedup {
            features.push("msg_ring_speedup");
        }
        if self.native_bind_listen {
            features.push("native_bind_listen");
        }

        // 6.12+ features
        if self.improved_huge_pages {
            features.push("improved_huge_pages");
        }
        if self.async_discard {
            features.push("async_discard");
        }
        if self.minimum_timeout_waits {
            features.push("minimum_timeout_waits");
        }
        if self.absolute_timeouts {
            features.push("absolute_timeouts");
        }
        if self.incremental_buffer_consumption {
            features.push("incremental_buffer_consumption");
        }
        if self.registered_buffer_cloning {
            features.push("registered_buffer_cloning");
        }

        features
    }

    /// Count the number of enabled features.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::stable();
    /// let count = config.feature_count();
    /// println!("Enabled {} features", count);
    /// ```
    pub fn feature_count(&self) -> usize {
        self.enabled_features().len()
    }

    /// Validate the configuration for internal consistency.
    ///
    /// Checks for feature combinations that are known to be problematic
    /// or mutually exclusive. Returns a list of warnings about the current
    /// configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::AdvancedConfig;
    ///
    /// let config = AdvancedConfig::performance();
    /// let warnings = config.validate();
    /// if !warnings.is_empty() {
    ///     println!("Configuration warnings: {:?}", warnings);
    /// }
    /// ```
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check for problematic feature combinations
        if self.sq_poll && self.defer_taskrun {
            warnings.push("sq_poll and defer_taskrun may conflict on some kernels".to_string());
        }

        if self.defer_async_work && !self.advanced_task_work {
            warnings.push("defer_async_work requires advanced_task_work for stability".to_string());
        }

        if self.user_allocated_ring_memory {
            warnings
                .push("user_allocated_ring_memory requires custom memory management".to_string());
        }

        if self.sq_poll && self.sq_thread_cpu.is_none() {
            warnings.push("sq_poll without CPU affinity may cause performance issues".to_string());
        }

        // Check for resource-intensive combinations
        let intensive_features = [
            self.sq_poll,
            self.defer_async_work,
            self.user_allocated_ring_memory,
        ];
        if intensive_features.iter().filter(|&&x| x).count() > 1 {
            warnings.push("Multiple resource-intensive features enabled".to_string());
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AdvancedConfig::default();

        // Legacy features
        assert!(!config.buffer_selection);
        assert!(!config.multi_shot);
        assert!(!config.provided_buffers);
        assert!(!config.fast_poll);
        assert!(!config.sq_poll);
        assert!(config.sq_thread_cpu.is_none());
        assert!(!config.coop_taskrun);
        assert!(!config.defer_taskrun);

        // Kernel 6.1+ features
        assert!(!config.advanced_task_work);
        assert!(!config.defer_async_work);

        // Kernel 6.2+ features
        assert!(!config.send_zc_reporting);
        assert!(!config.multishot_completion_batching);
        assert!(!config.epoll_uring_wake);

        // Kernel 6.3+ features
        assert!(!config.registered_ring_fd);

        // Kernel 6.4+ features
        assert!(!config.multishot_timeouts);

        // Kernel 6.5+ features
        assert!(!config.user_allocated_ring_memory);

        // Kernel 6.6+ features
        assert!(!config.async_cancel_op);
        assert!(!config.socket_command_support);
        assert!(!config.direct_io_optimizations);

        // Kernel 6.11+ features
        assert!(!config.msg_ring_speedup);
        assert!(!config.native_bind_listen);

        // Kernel 6.12+ features
        assert!(!config.improved_huge_pages);
        assert!(!config.async_discard);
        assert!(!config.minimum_timeout_waits);
        assert!(!config.absolute_timeouts);
        assert!(!config.incremental_buffer_consumption);
        assert!(!config.registered_buffer_cloning);
    }

    #[test]
    fn test_new_config() {
        let config = AdvancedConfig::new();
        // Should be identical to default
        assert_eq!(
            format!("{config:?}"),
            format!("{:?}", AdvancedConfig::default())
        );
    }

    #[test]
    fn test_stable_config() {
        let config = AdvancedConfig::stable();

        // Should enable stable features
        assert!(config.buffer_selection);
        assert!(config.fast_poll);
        assert!(config.provided_buffers);
        assert!(config.coop_taskrun);
        assert!(config.defer_taskrun);

        // Should keep problematic features disabled
        assert!(!config.sq_poll);
        assert!(!config.defer_async_work);
        assert!(!config.user_allocated_ring_memory);
    }

    #[test]
    fn test_performance_config() {
        let config = AdvancedConfig::performance();

        // Should have stable features
        assert!(config.buffer_selection);
        assert!(config.fast_poll);

        // Should have performance features
        assert!(config.multi_shot);
        assert!(config.advanced_task_work);
        assert!(config.send_zc_reporting);
        assert!(config.multishot_completion_batching);
    }

    #[test]
    fn test_development_config() {
        let config = AdvancedConfig::development();

        // Should enable useful development features
        assert!(config.buffer_selection);
        assert!(config.multi_shot);
        assert!(config.provided_buffers);

        // Should disable debugging-unfriendly features
        assert!(!config.sq_poll);
        assert!(!config.defer_async_work);
    }

    #[test]
    fn test_builder_pattern() {
        let config = AdvancedConfig::new()
            .with_buffer_selection(true)
            .with_multi_shot(true)
            .with_fast_poll(true);

        assert!(config.buffer_selection);
        assert!(config.multi_shot);
        assert!(config.fast_poll);
        assert!(!config.provided_buffers); // Should remain false
    }

    #[test]
    fn test_sq_poll_configuration() {
        let config = AdvancedConfig::new().with_sq_poll(true, Some(2));
        assert!(config.sq_poll);
        assert_eq!(config.sq_thread_cpu, Some(2));

        let config = AdvancedConfig::new().with_sq_poll(false, Some(2));
        assert!(!config.sq_poll);
        assert_eq!(config.sq_thread_cpu, None); // Should be reset when disabled
    }

    #[test]
    fn test_enabled_features() {
        let config = AdvancedConfig::stable();
        let features = config.enabled_features();

        assert!(features.contains(&"buffer_selection"));
        assert!(features.contains(&"fast_poll"));
        assert!(!features.is_empty());

        let default_config = AdvancedConfig::default();
        assert!(default_config.enabled_features().is_empty());
    }

    #[test]
    fn test_feature_count() {
        let default_config = AdvancedConfig::default();
        assert_eq!(default_config.feature_count(), 0);

        let stable_config = AdvancedConfig::stable();
        assert!(stable_config.feature_count() > 0);

        let performance_config = AdvancedConfig::performance();
        assert!(performance_config.feature_count() >= stable_config.feature_count());
    }

    #[test]
    fn test_validation() {
        let good_config = AdvancedConfig::stable();
        let warnings = good_config.validate();
        // Stable config should have minimal warnings
        assert!(warnings.len() <= 1);

        let problematic_config = AdvancedConfig::new()
            .with_sq_poll(true, None)
            .with_defer_taskrun(true);
        let warnings = problematic_config.validate();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_resource_intensive_validation() {
        let intensive_config = AdvancedConfig {
            sq_poll: true,
            defer_async_work: true,
            user_allocated_ring_memory: true,
            sq_thread_cpu: Some(0),
            advanced_task_work: true, // Required for defer_async_work
            ..AdvancedConfig::default()
        };

        let warnings = intensive_config.validate();
        assert!(warnings.iter().any(|w| w.contains("resource-intensive")));
    }
}
