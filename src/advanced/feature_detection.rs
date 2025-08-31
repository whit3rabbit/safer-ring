//! Kernel feature detection and capability probing.

use super::AdvancedConfig;
use crate::error::Result;
#[cfg(target_os = "linux")]
use crate::error::SaferRingError;

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
    /// miss backported features.
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
            "buffer_selection" => self.features.buffer_selection,
            "multi_shot" => self.features.multi_shot,
            "provided_buffers" => self.features.provided_buffers,
            "fast_poll" => self.features.fast_poll,
            "sq_poll" => self.features.sq_poll,
            "coop_taskrun" => self.features.coop_taskrun,
            "defer_taskrun" => self.features.defer_taskrun,
            "fixed_files" => self.features.fixed_files,
            "fixed_buffers" => self.features.fixed_buffers,
            _ => false,
        }
    }

    /// Get a list of all available features.
    pub fn available_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();

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

        features
    }

    /// Create an AdvancedConfig with features enabled based on availability.
    ///
    /// SQ polling is disabled by default due to CPU usage concerns.
    pub fn create_optimal_config(&self) -> AdvancedConfig {
        AdvancedConfig {
            buffer_selection: self.features.buffer_selection,
            multi_shot: self.features.multi_shot,
            provided_buffers: self.features.provided_buffers,
            fast_poll: self.features.fast_poll,
            sq_poll: false, // Disabled by default due to CPU usage
            sq_thread_cpu: None,
            coop_taskrun: self.features.coop_taskrun,
            defer_taskrun: self.features.defer_taskrun,
        }
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
}
