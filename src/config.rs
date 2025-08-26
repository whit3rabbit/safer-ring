//! Configuration options for different safer-ring use cases.
//!
//! This module provides comprehensive configuration options that allow users
//! to optimize safer-ring for their specific use cases, from low-latency
//! applications to high-throughput batch processing.

use crate::advanced::{AdvancedConfig, FeatureDetector};
use crate::error::{Result, SaferRingError};
use crate::logging::LogLevel;
use std::time::Duration;

/// Comprehensive configuration for safer-ring operations.
///
/// This struct provides all configuration options needed to optimize
/// safer-ring for different use cases and environments.
#[derive(Debug, Clone)]
pub struct SaferRingConfig {
    /// Ring configuration
    pub ring: RingConfig,
    /// Buffer management configuration
    pub buffer: BufferConfig,
    /// Performance optimization configuration
    pub performance: PerformanceConfig,
    /// Logging and debugging configuration
    pub logging: LoggingConfig,
    /// Advanced io_uring features configuration
    pub advanced: AdvancedConfig,
    /// Error handling configuration
    pub error_handling: ErrorHandlingConfig,
}

/// Ring-specific configuration options.
#[derive(Debug, Clone)]
pub struct RingConfig {
    /// Number of submission queue entries
    pub sq_entries: u32,
    /// Number of completion queue entries (0 = same as sq_entries)
    pub cq_entries: u32,
    /// Enable submission queue polling
    pub sq_poll: bool,
    /// CPU for SQ polling thread
    pub sq_thread_cpu: Option<u32>,
    /// SQ polling idle timeout in milliseconds
    pub sq_thread_idle: Option<u32>,
    /// Enable completion queue overflow handling
    pub cq_overflow: bool,
    /// Enable kernel submission queue thread
    pub kernel_sq_thread: bool,
}

/// Buffer management configuration.
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Default buffer size for operations
    pub default_size: usize,
    /// Buffer alignment (0 = system default)
    pub alignment: usize,
    /// Enable buffer pooling
    pub enable_pooling: bool,
    /// Buffer pool size per thread
    pub pool_size: usize,
    /// Maximum buffer size for pooling
    pub max_pooled_size: usize,
    /// Enable NUMA-aware allocation
    pub numa_aware: bool,
    /// Preferred NUMA node (-1 = no preference)
    pub numa_node: i32,
    /// Enable buffer pre-registration
    pub pre_register: bool,
}

/// Performance optimization configuration.
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Enable batch submission optimization
    pub batch_submission: bool,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Batch timeout for partial batches
    pub batch_timeout: Duration,
    /// Enable zero-copy optimizations
    pub zero_copy: bool,
    /// Enable vectored I/O optimization
    pub vectored_io: bool,
    /// Enable operation coalescing
    pub operation_coalescing: bool,
    /// Coalescing timeout
    pub coalescing_timeout: Duration,
    /// Enable adaptive polling
    pub adaptive_polling: bool,
    /// Polling timeout
    pub poll_timeout: Duration,
}

/// Logging and debugging configuration.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Enable logging
    pub enabled: bool,
    /// Minimum log level
    pub level: LogLevel,
    /// Enable performance metrics collection
    pub metrics: bool,
    /// Enable operation tracing
    pub tracing: bool,
    /// Log file path (None = console only)
    pub log_file: Option<std::path::PathBuf>,
    /// Use JSON format for logs
    pub json_format: bool,
    /// Enable debug assertions
    pub debug_assertions: bool,
}

/// Error handling configuration.
#[derive(Debug, Clone)]
pub struct ErrorHandlingConfig {
    /// Retry failed operations automatically
    pub auto_retry: bool,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
    /// Enable graceful degradation
    pub graceful_degradation: bool,
    /// Fallback to synchronous I/O on errors
    pub sync_fallback: bool,
    /// Panic on unrecoverable errors
    pub panic_on_fatal: bool,
}

#[allow(clippy::derivable_impls)] // Custom defaults are intentional
impl Default for SaferRingConfig {
    fn default() -> Self {
        Self {
            ring: RingConfig::default(),
            buffer: BufferConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
            advanced: AdvancedConfig::default(),
            error_handling: ErrorHandlingConfig::default(),
        }
    }
}

impl Default for RingConfig {
    fn default() -> Self {
        Self {
            sq_entries: 128,
            cq_entries: 0, // Same as sq_entries
            sq_poll: false,
            sq_thread_cpu: None,
            sq_thread_idle: None,
            cq_overflow: true,
            kernel_sq_thread: false,
        }
    }
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            default_size: 4096,
            alignment: 0, // System default
            enable_pooling: true,
            pool_size: 64,
            max_pooled_size: 1024 * 1024, // 1MB
            numa_aware: false,
            numa_node: -1,
            pre_register: false,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            batch_submission: true,
            max_batch_size: 32,
            batch_timeout: Duration::from_millis(1),
            zero_copy: true,
            vectored_io: true,
            operation_coalescing: false,
            coalescing_timeout: Duration::from_micros(100),
            adaptive_polling: false,
            poll_timeout: Duration::from_micros(10),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            level: LogLevel::Info,
            metrics: false,
            tracing: false,
            log_file: None,
            json_format: false,
            debug_assertions: cfg!(debug_assertions),
        }
    }
}

impl Default for ErrorHandlingConfig {
    fn default() -> Self {
        Self {
            auto_retry: false,
            max_retries: 3,
            retry_delay: Duration::from_millis(10),
            graceful_degradation: true,
            sync_fallback: false,
            panic_on_fatal: true,
        }
    }
}

impl SaferRingConfig {
    /// Create a configuration optimized for low latency applications.
    ///
    /// This configuration prioritizes minimal latency over throughput,
    /// suitable for real-time applications and interactive workloads.
    pub fn low_latency() -> Self {
        Self {
            ring: RingConfig {
                sq_entries: 32,
                cq_entries: 0,
                sq_poll: true,
                sq_thread_cpu: Some(0),
                sq_thread_idle: Some(1),
                cq_overflow: true,
                kernel_sq_thread: true,
            },
            buffer: BufferConfig {
                default_size: 1024,
                alignment: 64, // Cache line aligned
                enable_pooling: true,
                pool_size: 32,
                max_pooled_size: 64 * 1024, // 64KB
                numa_aware: true,
                numa_node: -1,
                pre_register: true,
            },
            performance: PerformanceConfig {
                batch_submission: false, // Immediate submission
                max_batch_size: 1,
                batch_timeout: Duration::from_micros(1),
                zero_copy: true,
                vectored_io: false,
                operation_coalescing: false,
                coalescing_timeout: Duration::ZERO,
                adaptive_polling: true,
                poll_timeout: Duration::from_nanos(100),
            },
            logging: LoggingConfig {
                enabled: false, // Minimal overhead
                level: LogLevel::Error,
                metrics: false,
                tracing: false,
                log_file: None,
                json_format: false,
                debug_assertions: false,
            },
            advanced: AdvancedConfig {
                buffer_selection: true,
                multi_shot: false,
                provided_buffers: true,
                fast_poll: true,
                sq_poll: true,
                sq_thread_cpu: Some(0),
                coop_taskrun: true,
                defer_taskrun: true,
            },
            error_handling: ErrorHandlingConfig {
                auto_retry: false,
                max_retries: 0,
                retry_delay: Duration::ZERO,
                graceful_degradation: false,
                sync_fallback: false,
                panic_on_fatal: true,
            },
        }
    }

    /// Create a configuration optimized for high throughput applications.
    ///
    /// This configuration prioritizes maximum throughput over latency,
    /// suitable for batch processing and data-intensive workloads.
    pub fn high_throughput() -> Self {
        Self {
            ring: RingConfig {
                sq_entries: 1024,
                cq_entries: 2048,
                sq_poll: false,
                sq_thread_cpu: None,
                sq_thread_idle: None,
                cq_overflow: true,
                kernel_sq_thread: false,
            },
            buffer: BufferConfig {
                default_size: 64 * 1024, // 64KB
                alignment: 4096,         // Page aligned
                enable_pooling: true,
                pool_size: 256,
                max_pooled_size: 16 * 1024 * 1024, // 16MB
                numa_aware: true,
                numa_node: -1,
                pre_register: true,
            },
            performance: PerformanceConfig {
                batch_submission: true,
                max_batch_size: 256,
                batch_timeout: Duration::from_millis(10),
                zero_copy: true,
                vectored_io: true,
                operation_coalescing: true,
                coalescing_timeout: Duration::from_millis(1),
                adaptive_polling: false,
                poll_timeout: Duration::from_millis(1),
            },
            logging: LoggingConfig {
                enabled: true,
                level: LogLevel::Info,
                metrics: true,
                tracing: false,
                log_file: None,
                json_format: false,
                debug_assertions: false,
            },
            advanced: AdvancedConfig {
                buffer_selection: true,
                multi_shot: true,
                provided_buffers: true,
                fast_poll: false,
                sq_poll: false,
                sq_thread_cpu: None,
                coop_taskrun: true,
                defer_taskrun: true,
            },
            error_handling: ErrorHandlingConfig {
                auto_retry: true,
                max_retries: 3,
                retry_delay: Duration::from_millis(1),
                graceful_degradation: true,
                sync_fallback: true,
                panic_on_fatal: false,
            },
        }
    }

    /// Create a configuration optimized for development and debugging.
    ///
    /// This configuration enables comprehensive logging and debugging features
    /// at the cost of performance, suitable for development and testing.
    pub fn development() -> Self {
        Self {
            ring: RingConfig {
                sq_entries: 64,
                cq_entries: 0,
                sq_poll: false,
                sq_thread_cpu: None,
                sq_thread_idle: None,
                cq_overflow: true,
                kernel_sq_thread: false,
            },
            buffer: BufferConfig {
                default_size: 4096,
                alignment: 0,
                enable_pooling: true,
                pool_size: 32,
                max_pooled_size: 1024 * 1024,
                numa_aware: false,
                numa_node: -1,
                pre_register: false,
            },
            performance: PerformanceConfig {
                batch_submission: true,
                max_batch_size: 16,
                batch_timeout: Duration::from_millis(1),
                zero_copy: true,
                vectored_io: true,
                operation_coalescing: false,
                coalescing_timeout: Duration::from_millis(1),
                adaptive_polling: false,
                poll_timeout: Duration::from_millis(1),
            },
            logging: LoggingConfig {
                enabled: true,
                level: LogLevel::Debug,
                metrics: true,
                tracing: true,
                log_file: Some("safer-ring.log".into()),
                json_format: false,
                debug_assertions: true,
            },
            advanced: AdvancedConfig::default(),
            error_handling: ErrorHandlingConfig {
                auto_retry: true,
                max_retries: 1,
                retry_delay: Duration::from_millis(10),
                graceful_degradation: true,
                sync_fallback: true,
                panic_on_fatal: false,
            },
        }
    }

    /// Create a configuration optimized for production environments.
    ///
    /// This configuration balances performance and reliability for production
    /// deployments with appropriate logging and error handling.
    pub fn production() -> Self {
        Self {
            ring: RingConfig {
                sq_entries: 256,
                cq_entries: 0,
                sq_poll: false,
                sq_thread_cpu: None,
                sq_thread_idle: None,
                cq_overflow: true,
                kernel_sq_thread: false,
            },
            buffer: BufferConfig {
                default_size: 8192,
                alignment: 0,
                enable_pooling: true,
                pool_size: 128,
                max_pooled_size: 4 * 1024 * 1024, // 4MB
                numa_aware: true,
                numa_node: -1,
                pre_register: false,
            },
            performance: PerformanceConfig {
                batch_submission: true,
                max_batch_size: 64,
                batch_timeout: Duration::from_millis(5),
                zero_copy: true,
                vectored_io: true,
                operation_coalescing: false,
                coalescing_timeout: Duration::from_millis(1),
                adaptive_polling: false,
                poll_timeout: Duration::from_millis(1),
            },
            logging: LoggingConfig {
                enabled: true,
                level: LogLevel::Warn,
                metrics: true,
                tracing: false,
                log_file: Some("/var/log/safer-ring.log".into()),
                json_format: true,
                debug_assertions: false,
            },
            advanced: AdvancedConfig::default(),
            error_handling: ErrorHandlingConfig {
                auto_retry: true,
                max_retries: 3,
                retry_delay: Duration::from_millis(100),
                graceful_degradation: true,
                sync_fallback: true,
                panic_on_fatal: false,
            },
        }
    }

    /// Create an optimal configuration based on the current system.
    ///
    /// This method detects system capabilities and creates a configuration
    /// that takes advantage of available features while maintaining compatibility.
    pub fn auto_detect() -> Result<Self> {
        let detector = FeatureDetector::new()?;
        let cpu_count = num_cpus::get();

        // Adjust buffer pool size based on available memory
        let (pool_size, max_pooled_size) = if let Ok(memory_info) = Self::get_memory_info() {
            let available_mb = memory_info.available / (1024 * 1024);
            if available_mb > 1024 {
                (256, 16 * 1024 * 1024)
            } else if available_mb > 512 {
                (128, 8 * 1024 * 1024)
            } else {
                (64, 1024 * 1024) // Default values
            }
        } else {
            (64, 1024 * 1024) // Default values
        };

        Ok(Self {
            advanced: detector.create_optimal_config(),
            ring: RingConfig {
                sq_entries: (cpu_count * 32).min(1024) as u32,
                ..Default::default()
            },
            buffer: BufferConfig {
                numa_aware: cpu_count > 16,
                pool_size,
                max_pooled_size,
                ..Default::default()
            },
            ..Default::default()
        })
    }

    /// Validate the configuration for consistency and compatibility.
    pub fn validate(&self) -> Result<()> {
        // Validate ring configuration
        if self.ring.sq_entries == 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "SQ entries must be greater than 0",
            )));
        }

        if self.ring.sq_entries > 4096 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "SQ entries should not exceed 4096",
            )));
        }

        // Validate buffer configuration
        if self.buffer.default_size == 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Default buffer size must be greater than 0",
            )));
        }

        if self.buffer.pool_size == 0 && self.buffer.enable_pooling {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Pool size must be greater than 0 when pooling is enabled",
            )));
        }

        // Validate performance configuration
        if self.performance.max_batch_size == 0 && self.performance.batch_submission {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Max batch size must be greater than 0 when batch submission is enabled",
            )));
        }

        // Validate error handling configuration
        if self.error_handling.auto_retry && self.error_handling.max_retries == 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Max retries must be greater than 0 when auto retry is enabled",
            )));
        }

        Ok(())
    }

    /// Get system memory information.
    fn get_memory_info() -> Result<MemoryInfo> {
        #[cfg(target_os = "linux")]
        {
            let meminfo = std::fs::read_to_string("/proc/meminfo").map_err(SaferRingError::Io)?;

            let mut total = 0;
            let mut available = 0;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        total = value.parse::<u64>().unwrap_or(0) * 1024; // Convert KB to bytes
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        available = value.parse::<u64>().unwrap_or(0) * 1024; // Convert KB to bytes
                    }
                }
            }

            Ok(MemoryInfo { total, available })
        }

        #[cfg(not(target_os = "linux"))]
        {
            // Fallback for non-Linux systems
            Ok(MemoryInfo {
                total: 8 * 1024 * 1024 * 1024,     // 8GB default
                available: 4 * 1024 * 1024 * 1024, // 4GB default
            })
        }
    }
}

/// System memory information.
#[derive(Debug, Clone)]
struct MemoryInfo {
    /// Total system memory in bytes
    #[allow(dead_code)]
    total: u64,
    /// Available system memory in bytes
    available: u64,
}

/// Configuration builder for fluent configuration creation.
#[derive(Debug)]
pub struct ConfigBuilder {
    config: SaferRingConfig,
}

impl ConfigBuilder {
    /// Create a new configuration builder.
    pub fn new() -> Self {
        Self {
            config: SaferRingConfig::default(),
        }
    }

    /// Set ring configuration.
    pub fn ring(mut self, ring: RingConfig) -> Self {
        self.config.ring = ring;
        self
    }

    /// Set buffer configuration.
    pub fn buffer(mut self, buffer: BufferConfig) -> Self {
        self.config.buffer = buffer;
        self
    }

    /// Set performance configuration.
    pub fn performance(mut self, performance: PerformanceConfig) -> Self {
        self.config.performance = performance;
        self
    }

    /// Set logging configuration.
    pub fn logging(mut self, logging: LoggingConfig) -> Self {
        self.config.logging = logging;
        self
    }

    /// Set advanced configuration.
    pub fn advanced(mut self, advanced: AdvancedConfig) -> Self {
        self.config.advanced = advanced;
        self
    }

    /// Set error handling configuration.
    pub fn error_handling(mut self, error_handling: ErrorHandlingConfig) -> Self {
        self.config.error_handling = error_handling;
        self
    }

    /// Build the final configuration.
    pub fn build(self) -> Result<SaferRingConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SaferRingConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_low_latency_config() {
        let config = SaferRingConfig::low_latency();
        assert!(config.validate().is_ok());
        assert_eq!(config.ring.sq_entries, 32);
        assert!(config.ring.sq_poll);
        assert!(!config.performance.batch_submission);
    }

    #[test]
    fn test_high_throughput_config() {
        let config = SaferRingConfig::high_throughput();
        assert!(config.validate().is_ok());
        assert_eq!(config.ring.sq_entries, 1024);
        assert!(config.performance.batch_submission);
        assert_eq!(config.performance.max_batch_size, 256);
    }

    #[test]
    fn test_development_config() {
        let config = SaferRingConfig::development();
        assert!(config.validate().is_ok());
        assert!(config.logging.enabled);
        assert_eq!(config.logging.level, LogLevel::Debug);
        assert!(config.logging.tracing);
    }

    #[test]
    fn test_production_config() {
        let config = SaferRingConfig::production();
        assert!(config.validate().is_ok());
        assert!(config.logging.enabled);
        assert_eq!(config.logging.level, LogLevel::Warn);
        assert!(config.logging.json_format);
    }

    #[test]
    fn test_config_validation() {
        let mut config = SaferRingConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid SQ entries should fail
        config.ring.sq_entries = 0;
        assert!(config.validate().is_err());

        config.ring.sq_entries = 128;
        config.buffer.default_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .ring(RingConfig {
                sq_entries: 64,
                ..Default::default()
            })
            .buffer(BufferConfig {
                default_size: 8192,
                ..Default::default()
            })
            .build()
            .unwrap();

        assert_eq!(config.ring.sq_entries, 64);
        assert_eq!(config.buffer.default_size, 8192);
    }

    #[test]
    fn test_auto_detect_config() {
        // This test may fail on systems without io_uring support
        if let Ok(config) = SaferRingConfig::auto_detect() {
            assert!(config.validate().is_ok());
            assert!(config.ring.sq_entries > 0);
        }
    }
}
