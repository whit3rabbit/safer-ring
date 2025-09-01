//! Configuration methods for Ring initialization.

#[cfg(target_os = "linux")]
use super::RawFdWrapper;
use super::Ring;
use crate::backend::Backend;
use crate::error::Result;
#[cfg(not(target_os = "linux"))]
use crate::error::SaferRingError;
#[cfg(target_os = "linux")]
use crate::future::WakerRegistry;
#[cfg(target_os = "linux")]
use crate::operation::tracker::OperationTracker;
#[cfg(target_os = "linux")]
use crate::safety::OrphanTracker;
#[cfg(target_os = "linux")]
use std::cell::RefCell;
#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::marker::PhantomData;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use tokio::io::unix::AsyncFd;

impl<'ring> Ring<'ring> {
    /// Create a new Ring with the specified configuration.
    ///
    /// This method allows full customization of the ring behavior including
    /// advanced features, buffer management, and performance optimizations.
    ///
    /// # Arguments
    ///
    /// * `config` - Complete configuration for the ring
    ///
    /// # Returns
    ///
    /// Returns a configured Ring optimized for the specified use case.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration is invalid or ring initialization fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, SaferRingConfig};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a ring optimized for low latency
    /// let config = SaferRingConfig::low_latency();
    /// let ring = Ring::with_config(config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_config(config: crate::config::SaferRingConfig) -> Result<Self> {
        // Validate configuration first
        config.validate()?;

        // Initialize logging if enabled
        if config.logging.enabled {
            let logger = crate::logging::init_logger();
            if let Ok(mut logger) = logger.lock() {
                logger.set_level(config.logging.level);

                if let Some(log_file) = &config.logging.log_file {
                    let file_output = if config.logging.json_format {
                        Box::new(crate::logging::FileOutput::new_json(log_file))
                    } else {
                        Box::new(crate::logging::FileOutput::new(log_file))
                    };
                    logger.add_output(file_output);
                }
            };
        }

        #[cfg(target_os = "linux")]
        {
            // Create io_uring with advanced configuration
            let mut builder: io_uring::Builder<io_uring::squeue::Entry, io_uring::cqueue::Entry> =
                io_uring::IoUring::builder();
            builder.setup_sqpoll(config.ring.sq_entries);

            if config.ring.cq_entries > 0 {
                builder.setup_cqsize(config.ring.cq_entries);
            }

            if config.ring.sq_poll {
                builder.setup_sqpoll(config.ring.sq_entries);
                if let Some(cpu) = config.ring.sq_thread_cpu {
                    builder.setup_sqpoll_cpu(cpu);
                }
                if let Some(idle) = config.ring.sq_thread_idle {
                    builder.setup_sqpoll(idle);
                }
            }

            if config.advanced.coop_taskrun {
                builder.setup_coop_taskrun();
            }

            if config.advanced.defer_taskrun {
                builder.setup_defer_taskrun();
            }

            let _inner = builder.build(config.ring.sq_entries)?;

            crate::logging::log(
                crate::logging::LogLevel::Info,
                "ring",
                &format!(
                    "Created ring with {} SQ entries, {} CQ entries",
                    config.ring.sq_entries,
                    config.ring.cq_entries.max(config.ring.sq_entries)
                ),
            );

            let backend = Box::new(crate::backend::io_uring::IoUringBackend::new(
                config.ring.sq_entries,
            )?);

            // Try to setup AsyncFd integration for Linux only if we're in a tokio context
            #[cfg(target_os = "linux")]
            let async_fd = {
                // Check if we're in a tokio runtime context before creating AsyncFd
                if tokio::runtime::Handle::try_current().is_ok() {
                    // Try to get the io_uring fd if we're using the io_uring backend
                    if let Some(io_uring_backend) = backend
                        .as_any()
                        .downcast_ref::<crate::backend::io_uring::IoUringBackend>(
                    ) {
                        let fd = io_uring_backend.as_raw_fd();
                        AsyncFd::new(RawFdWrapper(fd)).ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            #[cfg(not(target_os = "linux"))]
            let async_fd = None;

            Ok(Self {
                backend: RefCell::new(backend),
                phantom: PhantomData,
                operations: RefCell::new(OperationTracker::new()),
                waker_registry: Arc::new(WakerRegistry::new()),
                orphan_tracker: Arc::new(Mutex::new(OrphanTracker::new())),
                completion_cache: RefCell::new(HashMap::new()),
                async_fd,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _processed = 0;
            crate::logging::log(
                crate::logging::LogLevel::Warn,
                "ring",
                "io_uring not supported on this platform, using stub implementation",
            );

            Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "io_uring is only supported on Linux",
            )))
        }
    }

    /// Create a new Ring with advanced io_uring features.
    ///
    /// This method enables advanced io_uring features like buffer selection,
    /// multi-shot operations, and other performance optimizations based on
    /// kernel support.
    ///
    /// # Arguments
    ///
    /// * `config` - Advanced feature configuration
    ///
    /// # Returns
    ///
    /// Returns a Ring with advanced features enabled where supported.
    ///
    /// # Errors
    ///
    /// Returns an error if ring initialization fails. Features not supported
    /// by the kernel are automatically disabled with a warning.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Ring, AdvancedConfig, FeatureDetector};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Detect available features and create optimal config
    /// let detector = FeatureDetector::new()?;
    /// let config = detector.create_optimal_config();
    /// let ring = Ring::with_advanced_config(config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_advanced_config(config: crate::advanced::AdvancedConfig) -> Result<Self> {
        // Detect available features
        let detector = crate::advanced::FeatureDetector::new()?;

        // Create a SaferRingConfig with advanced features
        let mut safer_config = crate::config::SaferRingConfig {
            advanced: config,
            ..Default::default()
        };

        // Adjust configuration based on available features
        if !detector.has_feature("buffer_selection") && safer_config.advanced.buffer_selection {
            crate::logging::log(
                crate::logging::LogLevel::Warn,
                "ring",
                "Buffer selection not supported on this kernel, disabling feature",
            );
            safer_config.advanced.buffer_selection = false;
        }

        if !detector.has_feature("multi_shot") && safer_config.advanced.multi_shot {
            crate::logging::log(
                crate::logging::LogLevel::Warn,
                "ring",
                "Multi-shot operations not supported on this kernel, disabling feature",
            );
            safer_config.advanced.multi_shot = false;
        }

        Self::with_config(safer_config)
    }
}
