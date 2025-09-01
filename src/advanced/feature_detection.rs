//! Kernel feature detection and capability probing.

use super::AdvancedConfig;
use crate::error::Result;
#[cfg(target_os = "linux")]
use crate::error::SaferRingError;

#[cfg(target_os = "linux")]
use io_uring::{register::Probe, IoUring};

/// Kernel feature detection and capability probing.
///
/// Provides methods to detect which advanced io_uring features
/// are available on the current kernel version.
#[derive(Debug, Clone)]
pub struct FeatureDetector {
    /// Detected kernel version
    pub kernel_version: KernelVersion,
    /// Available features
    pub features: AvailableFeatures,
}

/// Kernel version information.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KernelVersion {
    /// Major version number
    pub major: u32,
    /// Minor version number
    pub minor: u32,
    /// Patch version number
    pub patch: u32,
}

/// Available io_uring features on this kernel.
#[derive(Debug, Clone)]
pub struct AvailableFeatures {
    /// Buffer selection is supported
    pub buffer_selection: bool,
    /// Multi-shot operations are supported
    pub multi_shot: bool,
    /// Provided buffers are supported
    pub provided_buffers: bool,
    /// Fast poll is supported
    pub fast_poll: bool,
    /// SQ polling is supported
    pub sq_poll: bool,
    /// Cooperative task running is supported
    pub coop_taskrun: bool,
    /// Task work defer is supported
    pub defer_taskrun: bool,
    /// Fixed files are supported
    pub fixed_files: bool,
    /// Fixed buffers are supported
    pub fixed_buffers: bool,

    // Kernel 6.1+ features
    /// Advanced task work management (6.1+)
    pub advanced_task_work: bool,
    /// Deferred async work until GETEVENTS (6.1+)
    pub defer_async_work: bool,

    // Kernel 6.2+ features
    /// Zero-copy send reporting with SEND_ZC_REPORT_USAGE (6.2+)
    pub send_zc_reporting: bool,
    /// Completion batching for multishot operations (6.2+)
    pub multishot_completion_batching: bool,
    /// EPOLL_URING_WAKE support (6.2+)
    pub epoll_uring_wake: bool,

    // Kernel 6.3+ features
    /// io_uring_register() with registered ring fd (6.3+)
    pub registered_ring_fd: bool,

    // Kernel 6.4+ features
    /// Multishot timeout operations (6.4+)
    pub multishot_timeouts: bool,

    // Kernel 6.5+ features
    /// User-allocated ring memory (6.5+)
    pub user_allocated_ring_memory: bool,

    // Kernel 6.6+ features
    /// Async operation cancellation with IORING_ASYNC_CANCEL_OP (6.6+)
    pub async_cancel_op: bool,
    /// io_uring command support for sockets (6.6+)
    pub socket_command_support: bool,
    /// System-wide io_uring disable sysctl (6.6+)
    pub sysctl_disable_support: bool,
    /// Direct I/O performance optimizations (6.6+)
    pub direct_io_optimizations: bool,

    // Kernel 6.11+ features
    /// MSG_RING performance improvements (6.11+)
    pub msg_ring_speedup: bool,
    /// Native bind/listen operations (6.11+)
    pub native_bind_listen: bool,

    // Kernel 6.12+ features
    /// Improved huge page segment handling (6.12+)
    pub improved_huge_pages: bool,
    /// Fully asynchronous discard operations (6.12+)
    pub async_discard: bool,
    /// Minimum timeout waits with dual conditions (6.12+)
    pub minimum_timeout_waits: bool,
    /// Absolute timeouts with multiple clock sources (6.12+)
    pub absolute_timeouts: bool,
    /// Incremental provided buffer consumption (6.12+)
    pub incremental_buffer_consumption: bool,
    /// Registered buffer cloning across threads (6.12+)
    pub registered_buffer_cloning: bool,
}

impl FeatureDetector {
    /// Create a new feature detector and probe the kernel.
    ///
    /// Detects the kernel version and probes for available features.
    ///
    /// # Errors
    ///
    /// Returns an error if kernel version detection fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use safer_ring::advanced::FeatureDetector;
    ///
    /// let detector = FeatureDetector::new()?;
    /// println!("Kernel: {}.{}.{}",
    ///     detector.kernel_version.major,
    ///     detector.kernel_version.minor,
    ///     detector.kernel_version.patch);
    /// # Ok::<(), safer_ring::error::SaferRingError>(())
    /// ```
    pub fn new() -> Result<Self> {
        let kernel_version = Self::detect_kernel_version()?;
        let features = Self::probe_features(&kernel_version);

        Ok(Self {
            kernel_version,
            features,
        })
    }

    /// Detect the current kernel version.
    ///
    /// Uses the `uname` syscall on Linux to retrieve kernel version information.
    /// On non-Linux platforms, returns a minimal version (0.0.0).
    fn detect_kernel_version() -> Result<KernelVersion> {
        #[cfg(target_os = "linux")]
        {
            // Use libc to get kernel version via uname syscall
            let mut utsname = unsafe { std::mem::zeroed::<libc::utsname>() };
            let result = unsafe { libc::uname(&mut utsname) };

            if result != 0 {
                return Err(SaferRingError::Io(std::io::Error::last_os_error()));
            }

            let release =
                unsafe { std::ffi::CStr::from_ptr(utsname.release.as_ptr()).to_string_lossy() };

            let (major, minor, patch) = Self::parse_kernel_version(&release).map_err(|e| {
                SaferRingError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse kernel version: {e}"),
                ))
            })?;

            Ok(KernelVersion {
                major,
                minor,
                patch,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            // Return minimal version for non-Linux platforms
            Ok(KernelVersion {
                major: 0,
                minor: 0,
                patch: 0,
            })
        }
    }

    /// Parse kernel version string into components.
    ///
    /// Handles various kernel version formats including those with
    /// additional suffixes like "-arch1" or "-generic".
    #[allow(dead_code)]
    fn parse_kernel_version(version_str: &str) -> std::result::Result<(u32, u32, u32), String> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() < 2 {
            return Err("Invalid kernel version format".to_string());
        }

        let major = parts[0]
            .parse()
            .map_err(|_| "Invalid major version number".to_string())?;

        let minor = parts[1]
            .parse()
            .map_err(|_| "Invalid minor version number".to_string())?;

        let patch = if parts.len() > 2 {
            // Extract numeric part before any non-numeric characters
            // This handles cases like "12-arch1" -> 12
            let patch_str = parts[2]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>();
            patch_str.parse().unwrap_or(0)
        } else {
            0
        };

        Ok((major, minor, patch))
    }

    /// Probe for available features based on kernel version.
    ///
    /// Feature availability is determined by when they were introduced
    /// in the Linux kernel. This is a conservative approach that may
    /// miss backported features but ensures compatibility.
    ///
    /// Uses kernel version comparison to determine which features should
    /// be available based on historical kernel development.
    fn probe_features(kernel_version: &KernelVersion) -> AvailableFeatures {
        AvailableFeatures {
            // Basic io_uring support (5.1+)
            fixed_files: kernel_version
                >= &KernelVersion {
                    major: 5,
                    minor: 1,
                    patch: 0,
                },
            fixed_buffers: kernel_version
                >= &KernelVersion {
                    major: 5,
                    minor: 1,
                    patch: 0,
                },

            // Fast poll (5.7+)
            fast_poll: kernel_version
                >= &KernelVersion {
                    major: 5,
                    minor: 7,
                    patch: 0,
                },

            // SQ polling (5.11+)
            sq_poll: kernel_version
                >= &KernelVersion {
                    major: 5,
                    minor: 11,
                    patch: 0,
                },

            // Buffer selection (5.13+)
            buffer_selection: kernel_version
                >= &KernelVersion {
                    major: 5,
                    minor: 13,
                    patch: 0,
                },
            provided_buffers: kernel_version
                >= &KernelVersion {
                    major: 5,
                    minor: 13,
                    patch: 0,
                },

            // Multi-shot operations (5.19+)
            multi_shot: kernel_version
                >= &KernelVersion {
                    major: 5,
                    minor: 19,
                    patch: 0,
                },

            // Cooperative task running (6.0+)
            coop_taskrun: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 0,
                    patch: 0,
                },
            defer_taskrun: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 0,
                    patch: 0,
                },

            // Kernel 6.1+ features
            advanced_task_work: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 1,
                    patch: 0,
                },
            defer_async_work: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 1,
                    patch: 0,
                },

            // Kernel 6.2+ features
            send_zc_reporting: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 2,
                    patch: 0,
                },
            multishot_completion_batching: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 2,
                    patch: 0,
                },
            epoll_uring_wake: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 2,
                    patch: 0,
                },

            // Kernel 6.3+ features
            registered_ring_fd: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 3,
                    patch: 0,
                },

            // Kernel 6.4+ features
            multishot_timeouts: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 4,
                    patch: 0,
                },

            // Kernel 6.5+ features
            user_allocated_ring_memory: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 5,
                    patch: 0,
                },

            // Kernel 6.6+ features
            async_cancel_op: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 6,
                    patch: 0,
                },
            socket_command_support: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 6,
                    patch: 0,
                },
            sysctl_disable_support: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 6,
                    patch: 0,
                },
            direct_io_optimizations: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 6,
                    patch: 0,
                },

            // Kernel 6.11+ features
            msg_ring_speedup: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 11,
                    patch: 0,
                },
            native_bind_listen: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 11,
                    patch: 0,
                },

            // Kernel 6.12+ features
            improved_huge_pages: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 12,
                    patch: 0,
                },
            async_discard: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 12,
                    patch: 0,
                },
            minimum_timeout_waits: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 12,
                    patch: 0,
                },
            absolute_timeouts: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 12,
                    patch: 0,
                },
            incremental_buffer_consumption: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 12,
                    patch: 0,
                },
            registered_buffer_cloning: kernel_version
                >= &KernelVersion {
                    major: 6,
                    minor: 12,
                    patch: 0,
                },
        }
    }

    /// Check if a specific feature is available.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use safer_ring::advanced::FeatureDetector;
    /// let detector = FeatureDetector::new()?;
    ///
    /// if detector.has_feature("multi_shot") {
    ///     println!("Multi-shot operations are supported");
    /// }
    /// # Ok::<(), safer_ring::error::SaferRingError>(())
    /// ```
    pub fn has_feature(&self, feature: &str) -> bool {
        match feature {
            // Legacy features
            "buffer_selection" => self.features.buffer_selection,
            "multi_shot" => self.features.multi_shot,
            "provided_buffers" => self.features.provided_buffers,
            "fast_poll" => self.features.fast_poll,
            "sq_poll" => self.features.sq_poll,
            "coop_taskrun" => self.features.coop_taskrun,
            "defer_taskrun" => self.features.defer_taskrun,
            "fixed_files" => self.features.fixed_files,
            "fixed_buffers" => self.features.fixed_buffers,

            // Kernel 6.1+ features
            "advanced_task_work" => self.features.advanced_task_work,
            "defer_async_work" => self.features.defer_async_work,

            // Kernel 6.2+ features
            "send_zc_reporting" => self.features.send_zc_reporting,
            "multishot_completion_batching" => self.features.multishot_completion_batching,
            "epoll_uring_wake" => self.features.epoll_uring_wake,

            // Kernel 6.3+ features
            "registered_ring_fd" => self.features.registered_ring_fd,

            // Kernel 6.4+ features
            "multishot_timeouts" => self.features.multishot_timeouts,

            // Kernel 6.5+ features
            "user_allocated_ring_memory" => self.features.user_allocated_ring_memory,

            // Kernel 6.6+ features
            "async_cancel_op" => self.features.async_cancel_op,
            "socket_command_support" => self.features.socket_command_support,
            "sysctl_disable_support" => self.features.sysctl_disable_support,
            "direct_io_optimizations" => self.features.direct_io_optimizations,

            // Kernel 6.11+ features
            "msg_ring_speedup" => self.features.msg_ring_speedup,
            "native_bind_listen" => self.features.native_bind_listen,

            // Kernel 6.12+ features
            "improved_huge_pages" => self.features.improved_huge_pages,
            "async_discard" => self.features.async_discard,
            "minimum_timeout_waits" => self.features.minimum_timeout_waits,
            "absolute_timeouts" => self.features.absolute_timeouts,
            "incremental_buffer_consumption" => self.features.incremental_buffer_consumption,
            "registered_buffer_cloning" => self.features.registered_buffer_cloning,

            _ => false,
        }
    }

    /// Get a list of all available features.
    ///
    /// Returns a vector of feature names that are available on the current kernel.
    /// This can be useful for logging, debugging, or conditional feature usage.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use safer_ring::advanced::FeatureDetector;
    ///
    /// let detector = FeatureDetector::new()?;
    /// let features = detector.available_features();
    ///
    /// println!("Available features: {:?}", features);
    /// for feature in features {
    ///     println!("- {}", feature);
    /// }
    /// # Ok::<(), safer_ring::error::SaferRingError>(())
    /// ```
    pub fn available_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();

        // Legacy features
        if self.features.fixed_files {
            features.push("fixed_files");
        }
        if self.features.fixed_buffers {
            features.push("fixed_buffers");
        }
        if self.features.fast_poll {
            features.push("fast_poll");
        }
        if self.features.sq_poll {
            features.push("sq_poll");
        }
        if self.features.buffer_selection {
            features.push("buffer_selection");
        }
        if self.features.provided_buffers {
            features.push("provided_buffers");
        }
        if self.features.multi_shot {
            features.push("multi_shot");
        }
        if self.features.coop_taskrun {
            features.push("coop_taskrun");
        }
        if self.features.defer_taskrun {
            features.push("defer_taskrun");
        }

        // Kernel 6.1+ features
        if self.features.advanced_task_work {
            features.push("advanced_task_work");
        }
        if self.features.defer_async_work {
            features.push("defer_async_work");
        }

        // Kernel 6.2+ features
        if self.features.send_zc_reporting {
            features.push("send_zc_reporting");
        }
        if self.features.multishot_completion_batching {
            features.push("multishot_completion_batching");
        }
        if self.features.epoll_uring_wake {
            features.push("epoll_uring_wake");
        }

        // Kernel 6.3+ features
        if self.features.registered_ring_fd {
            features.push("registered_ring_fd");
        }

        // Kernel 6.4+ features
        if self.features.multishot_timeouts {
            features.push("multishot_timeouts");
        }

        // Kernel 6.5+ features
        if self.features.user_allocated_ring_memory {
            features.push("user_allocated_ring_memory");
        }

        // Kernel 6.6+ features
        if self.features.async_cancel_op {
            features.push("async_cancel_op");
        }
        if self.features.socket_command_support {
            features.push("socket_command_support");
        }
        if self.features.sysctl_disable_support {
            features.push("sysctl_disable_support");
        }
        if self.features.direct_io_optimizations {
            features.push("direct_io_optimizations");
        }

        // Kernel 6.11+ features
        if self.features.msg_ring_speedup {
            features.push("msg_ring_speedup");
        }
        if self.features.native_bind_listen {
            features.push("native_bind_listen");
        }

        // Kernel 6.12+ features
        if self.features.improved_huge_pages {
            features.push("improved_huge_pages");
        }
        if self.features.async_discard {
            features.push("async_discard");
        }
        if self.features.minimum_timeout_waits {
            features.push("minimum_timeout_waits");
        }
        if self.features.absolute_timeouts {
            features.push("absolute_timeouts");
        }
        if self.features.incremental_buffer_consumption {
            features.push("incremental_buffer_consumption");
        }
        if self.features.registered_buffer_cloning {
            features.push("registered_buffer_cloning");
        }

        features
    }

    /// Create an AdvancedConfig with features enabled based on availability.
    ///
    /// This method creates a configuration that balances performance and stability
    /// by enabling safe features while keeping potentially problematic ones disabled.
    /// SQ polling is disabled by default due to CPU usage concerns.
    /// Some advanced features are disabled by default for stability.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use safer_ring::advanced::FeatureDetector;
    ///
    /// let detector = FeatureDetector::new()?;
    /// let config = detector.create_optimal_config();
    ///
    /// // Use the config to create a ring with optimal settings
    /// // let ring = Ring::with_advanced_config(config)?;
    /// # Ok::<(), safer_ring::error::SaferRingError>(())
    /// ```
    pub fn create_optimal_config(&self) -> AdvancedConfig {
        AdvancedConfig {
            // Legacy features
            buffer_selection: self.features.buffer_selection,
            multi_shot: self.features.multi_shot,
            provided_buffers: self.features.provided_buffers,
            fast_poll: self.features.fast_poll,
            sq_poll: false, // Disabled by default due to CPU usage
            sq_thread_cpu: None,
            coop_taskrun: self.features.coop_taskrun,
            defer_taskrun: self.features.defer_taskrun,

            // Kernel 6.1+ features
            advanced_task_work: self.features.advanced_task_work,
            defer_async_work: false, // Disabled by default - requires careful application integration

            // Kernel 6.2+ features
            send_zc_reporting: self.features.send_zc_reporting,
            multishot_completion_batching: self.features.multishot_completion_batching,
            epoll_uring_wake: self.features.epoll_uring_wake,

            // Kernel 6.3+ features
            registered_ring_fd: self.features.registered_ring_fd,

            // Kernel 6.4+ features
            multishot_timeouts: self.features.multishot_timeouts,

            // Kernel 6.5+ features
            user_allocated_ring_memory: false, // Disabled by default - requires custom memory management

            // Kernel 6.6+ features
            async_cancel_op: self.features.async_cancel_op,
            socket_command_support: self.features.socket_command_support,
            direct_io_optimizations: self.features.direct_io_optimizations,

            // Kernel 6.11+ features
            msg_ring_speedup: self.features.msg_ring_speedup,
            native_bind_listen: self.features.native_bind_listen,

            // Kernel 6.12+ features
            improved_huge_pages: self.features.improved_huge_pages,
            async_discard: self.features.async_discard,
            minimum_timeout_waits: self.features.minimum_timeout_waits,
            absolute_timeouts: self.features.absolute_timeouts,
            incremental_buffer_consumption: self.features.incremental_buffer_consumption,
            registered_buffer_cloning: self.features.registered_buffer_cloning,
        }
    }

    /// Create a new feature detector with hybrid detection strategy.
    ///
    /// This method combines kernel version detection with direct kernel probing
    /// for more accurate feature detection, especially useful in environments
    /// with backported features.
    ///
    /// # Errors
    ///
    /// Returns an error if kernel version detection fails or if direct probing
    /// cannot be initialized.
    pub fn new_with_probing() -> Result<Self> {
        let kernel_version = Self::detect_kernel_version()?;
        let version_features = Self::probe_features(&kernel_version);

        #[cfg(target_os = "linux")]
        {
            // Try to enhance feature detection with direct probing
            match DirectProbeDetector::new() {
                Ok(probe_detector) => {
                    let probe_features = probe_detector.probe_kernel_features();
                    let features = Self::merge_feature_detection(version_features, probe_features);

                    Ok(Self {
                        kernel_version,
                        features,
                    })
                }
                Err(_) => {
                    // Fall back to version-based detection if probing fails
                    Ok(Self {
                        kernel_version,
                        features: version_features,
                    })
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self {
                kernel_version,
                features: version_features,
            })
        }
    }

    /// Merge version-based and probe-based feature detection results.
    ///
    /// This method combines the conservative version-based approach with
    /// the more accurate probe-based detection, preferring probe results
    /// when they indicate a feature is available.
    #[cfg(target_os = "linux")]
    fn merge_feature_detection(
        version_features: AvailableFeatures,
        probe_features: AvailableFeatures,
    ) -> AvailableFeatures {
        AvailableFeatures {
            // Use OR logic: if either method detects a feature, consider it available
            fixed_files: version_features.fixed_files || probe_features.fixed_files,
            fixed_buffers: version_features.fixed_buffers || probe_features.fixed_buffers,
            fast_poll: version_features.fast_poll || probe_features.fast_poll,
            sq_poll: version_features.sq_poll || probe_features.sq_poll,
            buffer_selection: version_features.buffer_selection || probe_features.buffer_selection,
            provided_buffers: version_features.provided_buffers || probe_features.provided_buffers,
            multi_shot: version_features.multi_shot || probe_features.multi_shot,
            coop_taskrun: version_features.coop_taskrun || probe_features.coop_taskrun,
            defer_taskrun: version_features.defer_taskrun || probe_features.defer_taskrun,

            // For newer features, prefer version-based detection as primary source
            // since opcode availability doesn't always indicate full feature support
            advanced_task_work: version_features.advanced_task_work,
            defer_async_work: version_features.defer_async_work,
            send_zc_reporting: version_features.send_zc_reporting,
            multishot_completion_batching: version_features.multishot_completion_batching,
            epoll_uring_wake: version_features.epoll_uring_wake,
            registered_ring_fd: version_features.registered_ring_fd,
            multishot_timeouts: version_features.multishot_timeouts,
            user_allocated_ring_memory: version_features.user_allocated_ring_memory,
            async_cancel_op: version_features.async_cancel_op,
            socket_command_support: version_features.socket_command_support,
            sysctl_disable_support: version_features.sysctl_disable_support,
            direct_io_optimizations: version_features.direct_io_optimizations,
            msg_ring_speedup: version_features.msg_ring_speedup,
            native_bind_listen: version_features.native_bind_listen,
            improved_huge_pages: version_features.improved_huge_pages,
            async_discard: version_features.async_discard,
            minimum_timeout_waits: version_features.minimum_timeout_waits,
            absolute_timeouts: version_features.absolute_timeouts,
            incremental_buffer_consumption: version_features.incremental_buffer_consumption,
            registered_buffer_cloning: version_features.registered_buffer_cloning,
        }
    }
}

/// Direct kernel probing for io_uring features.
///
/// This detector uses the io_uring kernel probing mechanism to directly
/// query the kernel for supported operations and features, providing
/// more accurate detection than version-based approaches.
#[cfg(target_os = "linux")]
pub struct DirectProbeDetector {
    probe: Probe,
}

#[cfg(target_os = "linux")]
impl DirectProbeDetector {
    /// Create a new direct probe detector.
    ///
    /// Initializes a minimal io_uring instance for probing kernel capabilities.
    /// This requires io_uring to be available on the system and sufficient
    /// permissions to create io_uring instances.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(target_os = "linux")]
    /// # {
    /// use safer_ring::advanced::DirectProbeDetector;
    ///
    /// match DirectProbeDetector::new() {
    ///     Ok(detector) => {
    ///         let features = detector.probe_kernel_features();
    ///         println!("Direct probe completed successfully");
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Direct probing failed: {}", e);
    ///         // Fall back to version-based detection
    ///     }
    /// }
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - io_uring initialization fails (insufficient permissions, kernel too old)
    /// - Probe registration fails (kernel doesn't support probing)
    /// - System resources are insufficient for creating the temporary ring
    pub fn new() -> Result<Self> {
        let ring = IoUring::new(1)?;
        let mut probe = Probe::new();

        // Register the probe with the kernel to get supported features
        ring.submitter().register_probe(&mut probe)?;

        // Ring is dropped here, only keeping the probe results
        Ok(Self { probe })
    }

    /// Probe the kernel for available io_uring features.
    ///
    /// This method directly queries the kernel to determine which
    /// io_uring operations and features are supported, providing
    /// the most accurate feature detection possible.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(target_os = "linux")]
    /// # {
    /// use safer_ring::advanced::DirectProbeDetector;
    ///
    /// let detector = DirectProbeDetector::new()?;
    /// let features = detector.probe_kernel_features();
    ///
    /// if features.multi_shot {
    ///     println!("Multi-shot operations are supported");
    /// }
    /// if features.buffer_selection {
    ///     println!("Buffer selection is supported");
    /// }
    /// # Ok::<(), safer_ring::error::SaferRingError>(())
    /// # }
    /// ```
    pub fn probe_kernel_features(&self) -> AvailableFeatures {
        AvailableFeatures {
            // Check for basic operation support via opcodes
            fixed_files: self.is_opcode_supported(io_uring::opcode::Read::CODE)
                && self.is_opcode_supported(io_uring::opcode::Write::CODE),
            fixed_buffers: self.is_opcode_supported(io_uring::opcode::Read::CODE),
            fast_poll: self.is_opcode_supported(io_uring::opcode::PollAdd::CODE),
            sq_poll: true, // SQ polling is a setup feature, assume available if io_uring works
            buffer_selection: self.is_opcode_supported(io_uring::opcode::Recv::CODE), // Proxy check
            provided_buffers: self.is_opcode_supported(io_uring::opcode::Recv::CODE), // Proxy check
            multi_shot: self.is_opcode_supported(io_uring::opcode::Accept::CODE), // Proxy for multishot support

            // These are runtime/setup features that can't be directly probed via opcodes
            // so we conservatively set them to false and rely on version detection
            coop_taskrun: false,
            defer_taskrun: false,

            // Newer features - use conservative approach for direct probing
            advanced_task_work: false,
            defer_async_work: false,
            send_zc_reporting: self.is_opcode_supported(io_uring::opcode::SendZc::CODE),
            multishot_completion_batching: false,
            epoll_uring_wake: false,
            registered_ring_fd: false,
            multishot_timeouts: self.is_opcode_supported(io_uring::opcode::Timeout::CODE),
            user_allocated_ring_memory: false,
            async_cancel_op: self.is_opcode_supported(io_uring::opcode::AsyncCancel::CODE),
            socket_command_support: false,
            sysctl_disable_support: false,
            direct_io_optimizations: false,
            msg_ring_speedup: false,
            native_bind_listen: false,
            improved_huge_pages: false,
            async_discard: false,
            minimum_timeout_waits: false,
            absolute_timeouts: false,
            incremental_buffer_consumption: false,
            registered_buffer_cloning: false,
        }
    }

    /// Check if a specific opcode is supported by the kernel.
    ///
    /// # Arguments
    ///
    /// * `opcode` - The operation code to check for support
    ///
    /// # Returns
    ///
    /// `true` if the opcode is supported, `false` otherwise
    fn is_opcode_supported(&self, opcode: u8) -> bool {
        self.probe.is_supported(opcode)
    }
}

/// Stub implementation for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub struct DirectProbeDetector;

#[cfg(not(target_os = "linux"))]
impl DirectProbeDetector {
    /// Stub implementation for non-Linux platforms.
    pub fn new() -> Result<Self> {
        use crate::error::SaferRingError;
        Err(SaferRingError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Direct probing is only available on Linux",
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_version_parsing() {
        let (major, minor, patch) = FeatureDetector::parse_kernel_version("5.19.0").unwrap();
        assert_eq!(major, 5);
        assert_eq!(minor, 19);
        assert_eq!(patch, 0);

        let (major, minor, patch) = FeatureDetector::parse_kernel_version("6.1.12-arch1").unwrap();
        assert_eq!(major, 6);
        assert_eq!(minor, 1);
        assert_eq!(patch, 12);
    }

    #[test]
    fn test_kernel_version_comparison() {
        let v1 = KernelVersion {
            major: 5,
            minor: 19,
            patch: 0,
        };
        let v2 = KernelVersion {
            major: 6,
            minor: 0,
            patch: 0,
        };

        assert!(v1 < v2);
        assert!(v2 > v1);
        assert_eq!(v1, v1);
    }

    #[test]
    fn test_feature_detection() {
        let detector = FeatureDetector::new().unwrap();

        // Basic sanity checks
        assert!(detector.kernel_version.major > 0 || cfg!(not(target_os = "linux")));

        // Test feature querying
        let _has_buffer_selection = detector.has_feature("buffer_selection");
        let _available_features = detector.available_features();

        // Test unknown feature
        assert!(!detector.has_feature("unknown_feature"));
    }

    #[test]
    fn test_feature_version_requirements() {
        let old_kernel = KernelVersion {
            major: 5,
            minor: 0,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&old_kernel);

        // Old kernel should not have advanced features
        assert!(!features.fixed_files);
        assert!(!features.multi_shot);
        assert!(!features.buffer_selection);

        let new_kernel = KernelVersion {
            major: 6,
            minor: 5,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&new_kernel);

        // New kernel should have all features
        assert!(features.fixed_files);
        assert!(features.multi_shot);
        assert!(features.buffer_selection);
        assert!(features.coop_taskrun);
    }

    #[test]
    fn test_kernel_61_features() {
        let kernel_61 = KernelVersion {
            major: 6,
            minor: 1,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&kernel_61);

        // Should have 6.1+ features
        assert!(features.advanced_task_work);
        assert!(features.defer_async_work);

        // Should not have later features
        assert!(!features.send_zc_reporting);
        assert!(!features.multishot_completion_batching);
    }

    #[test]
    fn test_kernel_62_features() {
        let kernel_62 = KernelVersion {
            major: 6,
            minor: 2,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&kernel_62);

        // Should have 6.1+ features
        assert!(features.advanced_task_work);
        assert!(features.defer_async_work);

        // Should have 6.2+ features
        assert!(features.send_zc_reporting);
        assert!(features.multishot_completion_batching);
        assert!(features.epoll_uring_wake);

        // Should not have later features
        assert!(!features.registered_ring_fd);
    }

    #[test]
    fn test_kernel_66_features() {
        let kernel_66 = KernelVersion {
            major: 6,
            minor: 6,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&kernel_66);

        // Should have all 6.6+ features
        assert!(features.async_cancel_op);
        assert!(features.socket_command_support);
        assert!(features.sysctl_disable_support);
        assert!(features.direct_io_optimizations);

        // Should not have later features
        assert!(!features.msg_ring_speedup);
        assert!(!features.native_bind_listen);
    }

    #[test]
    fn test_kernel_612_features() {
        let kernel_612 = KernelVersion {
            major: 6,
            minor: 12,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&kernel_612);

        // Should have all latest features
        assert!(features.improved_huge_pages);
        assert!(features.async_discard);
        assert!(features.minimum_timeout_waits);
        assert!(features.absolute_timeouts);
        assert!(features.incremental_buffer_consumption);
        assert!(features.registered_buffer_cloning);

        // Should also have earlier features
        assert!(features.async_cancel_op);
        assert!(features.msg_ring_speedup);
        assert!(features.native_bind_listen);
    }

    #[test]
    fn test_has_feature_new_features() {
        let detector = FeatureDetector::new().unwrap();

        // Test that all new feature strings are recognized
        let new_features = vec![
            "advanced_task_work",
            "defer_async_work",
            "send_zc_reporting",
            "multishot_completion_batching",
            "epoll_uring_wake",
            "registered_ring_fd",
            "multishot_timeouts",
            "user_allocated_ring_memory",
            "async_cancel_op",
            "socket_command_support",
            "sysctl_disable_support",
            "direct_io_optimizations",
            "msg_ring_speedup",
            "native_bind_listen",
            "improved_huge_pages",
            "async_discard",
            "minimum_timeout_waits",
            "absolute_timeouts",
            "incremental_buffer_consumption",
            "registered_buffer_cloning",
        ];

        for feature in new_features {
            // Should not panic and should return a boolean
            let _ = detector.has_feature(feature);
        }

        // Test unknown feature
        assert!(!detector.has_feature("unknown_new_feature"));
    }

    #[test]
    fn test_available_features_includes_new_features() {
        let new_kernel = KernelVersion {
            major: 6,
            minor: 12,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&new_kernel);
        let detector = FeatureDetector {
            kernel_version: new_kernel,
            features,
        };

        let available = detector.available_features();

        // Should include some of the new features
        assert!(available.contains(&"advanced_task_work"));
        assert!(available.contains(&"async_cancel_op"));
        assert!(available.contains(&"improved_huge_pages"));

        // Should be a significant number of features
        assert!(available.len() > 10);
    }

    #[test]
    fn test_create_optimal_config_new_features() {
        let new_kernel = KernelVersion {
            major: 6,
            minor: 12,
            patch: 0,
        };
        let features = FeatureDetector::probe_features(&new_kernel);
        let detector = FeatureDetector {
            kernel_version: new_kernel,
            features,
        };

        let config = detector.create_optimal_config();

        // Should enable safe new features
        assert!(config.advanced_task_work);
        assert!(config.async_cancel_op);
        assert!(config.improved_huge_pages);

        // Should keep some features disabled by default for safety
        assert!(!config.defer_async_work);
        assert!(!config.user_allocated_ring_memory);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_new_with_probing_fallback() {
        // This test should not panic even if direct probing fails
        let detector = FeatureDetector::new_with_probing().unwrap();

        // Should have detected some kernel version
        assert!(detector.kernel_version.major > 0);

        // Should have some features available
        let available = detector.available_features();
        assert!(!available.is_empty());
    }

    #[test]
    fn test_feature_detection_consistency() {
        // Verify that newer kernels have all features of older kernels
        let kernels = vec![
            KernelVersion {
                major: 6,
                minor: 0,
                patch: 0,
            },
            KernelVersion {
                major: 6,
                minor: 1,
                patch: 0,
            },
            KernelVersion {
                major: 6,
                minor: 6,
                patch: 0,
            },
            KernelVersion {
                major: 6,
                minor: 12,
                patch: 0,
            },
        ];

        let mut previous_feature_count = 0;

        for kernel in kernels {
            let features = FeatureDetector::probe_features(&kernel);
            let detector = FeatureDetector {
                kernel_version: kernel.clone(),
                features,
            };
            let available = detector.available_features();

            // Each newer kernel should have at least as many features as the previous
            assert!(
                available.len() >= previous_feature_count,
                "Kernel {}.{}.{} has fewer features than previous kernel",
                kernel.major,
                kernel.minor,
                kernel.patch
            );

            previous_feature_count = available.len();
        }
    }
}
