//! Runtime detection and fallback system for safer-ring.
//!
//! This module provides runtime detection of io_uring availability and automatic
//! fallback to alternative implementations when io_uring is not available. This is
//! particularly important in cloud environments like Docker, Kubernetes (GKE, EKS),
//! and serverless platforms where io_uring is often disabled by default.
//!
//! # Architecture
//!
//! The runtime uses a backend abstraction that can switch between:
//! - **IoUringBackend**: Full io_uring implementation (Linux 5.1+)
//! - **EpollBackend**: Traditional epoll-based fallback (all Linux)
//! - **StubBackend**: No-op implementation for non-Linux platforms
//!
//! # Example
//!
//! ```rust,no_run
//! use safer_ring::runtime::{Runtime, Backend};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Automatically detect best available backend
//! let runtime = Runtime::auto_detect()?;
//!
//! match runtime.backend() {
//!     Backend::IoUring => println!("Using high-performance io_uring"),
//!     Backend::Epoll => println!("Using epoll fallback"),
//!     Backend::Stub => println!("Using stub implementation"),
//! }
//!
//! // Check if we're in a restricted environment
//! if runtime.is_cloud_environment() {
//!     println!("Detected cloud environment, see docs for optimization tips");
//! }
//! # Ok(())
//! # }
//! ```

use std::fs;
use std::io;
use std::path::Path;

use crate::error::{Result, SaferRingError};

/// Available runtime backends for IO operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// High-performance io_uring backend (Linux 5.1+)
    IoUring,
    /// Traditional epoll-based backend (all Linux)
    Epoll,
    /// Stub implementation for non-Linux platforms
    Stub,
}

impl Backend {
    /// Get a human-readable description of the backend.
    pub fn description(&self) -> &'static str {
        match self {
            Backend::IoUring => "High-performance io_uring backend",
            Backend::Epoll => "Traditional epoll-based backend",
            Backend::Stub => "Stub implementation for non-Linux platforms",
        }
    }

    /// Check if this backend supports advanced features.
    pub fn supports_advanced_features(&self) -> bool {
        matches!(self, Backend::IoUring)
    }

    /// Get expected performance multiplier compared to epoll baseline.
    pub fn performance_multiplier(&self) -> f32 {
        match self {
            Backend::IoUring => 3.0, // 3x performance improvement
            Backend::Epoll => 1.0,   // Baseline
            Backend::Stub => 0.1,    // Minimal performance
        }
    }
}

/// Runtime environment with automatic backend detection and fallback.
///
/// The runtime automatically detects the best available backend and provides
/// information about the current environment for optimization guidance.
#[derive(Debug)]
pub struct Runtime {
    backend: Backend,
    environment: EnvironmentInfo,
}

impl Runtime {
    /// Create a new runtime with automatic backend detection.
    ///
    /// This method probes the system for io_uring availability and selects
    /// the best available backend. If io_uring is not available, it falls
    /// back to epoll (on Linux) or stub implementation (on other platforms).
    ///
    /// # Returns
    ///
    /// A runtime configured with the optimal backend for the current environment.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use safer_ring::runtime::Runtime;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let runtime = Runtime::auto_detect()?;
    /// println!("Using backend: {}", runtime.backend().description());
    /// # Ok(())
    /// # }
    /// ```
    pub fn auto_detect() -> Result<Self> {
        let environment = EnvironmentInfo::detect();
        let backend = Self::select_best_backend(&environment)?;

        // Log warnings for restricted environments
        if environment.is_cloud_environment() && backend != Backend::IoUring {
            eprintln!("WARNING: io_uring not available in cloud environment");
            eprintln!("Performance may be degraded. See documentation for configuration tips:");
            eprintln!("- Docker: Add --cap-add SYS_ADMIN or use --privileged");
            eprintln!("- Kubernetes: Set privileged: true or configure seccomp/apparmor");
            eprintln!("- Cloud Run/Lambda: Consider using Cloud Functions with custom runtimes");
        }

        Ok(Self {
            backend,
            environment,
        })
    }

    /// Create a runtime with a specific backend.
    ///
    /// This bypasses automatic detection and forces the use of a specific backend.
    /// Use this when you want to test fallback behavior or have specific requirements.
    ///
    /// # Arguments
    ///
    /// * `backend` - The backend to use
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use safer_ring::runtime::{Runtime, Backend};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Force epoll backend for testing
    /// let runtime = Runtime::with_backend(Backend::Epoll)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_backend(backend: Backend) -> Result<Self> {
        let environment = EnvironmentInfo::detect();

        // Validate that the requested backend is available
        match backend {
            Backend::IoUring => {
                if !Self::is_io_uring_available() {
                    return Err(SaferRingError::Io(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "io_uring backend requested but not available on this system",
                    )));
                }
            }
            Backend::Epoll => {
                #[cfg(not(target_os = "linux"))]
                {
                    return Err(SaferRingError::Io(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "epoll backend only available on Linux",
                    )));
                }
            }
            Backend::Stub => {
                // Stub backend is always available
            }
        }

        Ok(Self {
            backend,
            environment,
        })
    }

    /// Get the active backend.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// Get environment information.
    pub fn environment(&self) -> &EnvironmentInfo {
        &self.environment
    }

    /// Check if running in a cloud environment.
    pub fn is_cloud_environment(&self) -> bool {
        self.environment.is_cloud_environment()
    }

    /// Get performance guidance for the current environment.
    pub fn performance_guidance(&self) -> Vec<&'static str> {
        let mut guidance = Vec::new();

        match self.backend {
            Backend::IoUring => {
                guidance.push("✓ Using high-performance io_uring backend");
                if self.environment.container_runtime.is_some() {
                    guidance
                        .push("Consider tuning container security settings for better performance");
                }
            }
            Backend::Epoll => {
                guidance.push("⚠ Using epoll fallback - performance may be limited");
                if self.is_cloud_environment() {
                    guidance
                        .push("Check cloud platform documentation for enabling io_uring support");
                }
            }
            Backend::Stub => {
                guidance.push("⚠ Using stub implementation - limited functionality");
                guidance.push("Consider running on Linux for better performance");
            }
        }

        if let Some(container) = &self.environment.container_runtime {
            match container.as_str() {
                "docker" => {
                    guidance.push("Docker detected: Add --cap-add SYS_ADMIN for io_uring support");
                }
                "containerd" | "cri-o" => {
                    guidance.push(
                        "Kubernetes detected: Configure privileged pods or custom seccomp profiles",
                    );
                }
                _ => {}
            }
        }

        guidance
    }

    /// Select the best available backend for the current environment.
    fn select_best_backend(_environment: &EnvironmentInfo) -> Result<Backend> {
        // Check platform first
        #[cfg(not(target_os = "linux"))]
        {
            Ok(Backend::Stub)
        }

        #[cfg(target_os = "linux")]
        {
            // Try io_uring first
            if Self::is_io_uring_available() {
                // Additional checks for restricted environments
                if _environment.is_cloud_environment() {
                    // In cloud environments, io_uring might be restricted
                    if let Some(restriction) = Self::check_io_uring_restrictions() {
                        eprintln!("io_uring restricted: {}", restriction);
                        return Ok(Backend::Epoll);
                    }
                }
                return Ok(Backend::IoUring);
            }

            // Fall back to epoll
            Ok(Backend::Epoll)
        }
    }

    /// Check if io_uring is available on the system.
    #[cfg(target_os = "linux")]
    fn is_io_uring_available() -> bool {
        // Try to create a minimal io_uring instance
        match io_uring::IoUring::new(1) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Check if io_uring is available (always false on non-Linux).
    #[cfg(not(target_os = "linux"))]
    fn is_io_uring_available() -> bool {
        false
    }

    /// Check for io_uring restrictions in the current environment.
    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    fn check_io_uring_restrictions() -> Option<String> {
        // Check seccomp restrictions
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            if status.contains("Seccomp:") && !status.contains("Seccomp:\t0") {
                return Some("seccomp profile may restrict io_uring system calls".to_string());
            }
        }

        // Check AppArmor restrictions
        if Path::new("/proc/self/attr/current").exists() {
            if let Ok(apparmor) = fs::read_to_string("/proc/self/attr/current") {
                if !apparmor.trim().is_empty() && apparmor.trim() != "unconfined" {
                    return Some("AppArmor profile may restrict io_uring".to_string());
                }
            }
        }

        // Check for container indicators that might restrict io_uring
        if Path::new("/.dockerenv").exists() {
            return Some("Docker environment detected - io_uring may be restricted".to_string());
        }

        None
    }

    /// Check for io_uring restrictions (no-op on non-Linux).
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    fn check_io_uring_restrictions() -> Option<String> {
        None
    }
}

/// Information about the runtime environment.
///
/// Collects information about the system environment to help guide
/// performance optimization and provide helpful warnings.
#[derive(Debug)]
pub struct EnvironmentInfo {
    /// Detected container runtime (docker, containerd, etc.)
    pub container_runtime: Option<String>,
    /// Whether running in Kubernetes
    pub kubernetes: bool,
    /// Whether running in a cloud serverless environment
    pub serverless: bool,
    /// Kernel version for Linux systems
    pub kernel_version: Option<String>,
    /// Available CPU count
    pub cpu_count: usize,
}

impl EnvironmentInfo {
    /// Detect current environment information.
    pub fn detect() -> Self {
        Self {
            container_runtime: Self::detect_container_runtime(),
            kubernetes: Self::detect_kubernetes(),
            serverless: Self::detect_serverless(),
            kernel_version: Self::detect_kernel_version(),
            cpu_count: Self::detect_cpu_count(),
        }
    }

    /// Check if running in any kind of cloud environment.
    pub fn is_cloud_environment(&self) -> bool {
        self.container_runtime.is_some() || self.kubernetes || self.serverless
    }

    /// Detect container runtime.
    fn detect_container_runtime() -> Option<String> {
        // Check for Docker
        if Path::new("/.dockerenv").exists() {
            return Some("docker".to_string());
        }

        // Check for other container indicators
        if let Ok(cgroup) = fs::read_to_string("/proc/1/cgroup") {
            if cgroup.contains("docker") {
                return Some("docker".to_string());
            }
            if cgroup.contains("containerd") {
                return Some("containerd".to_string());
            }
            if cgroup.contains("cri-o") {
                return Some("cri-o".to_string());
            }
        }

        None
    }

    /// Detect Kubernetes environment.
    fn detect_kubernetes() -> bool {
        std::env::var("KUBERNETES_SERVICE_HOST").is_ok()
            || Path::new("/var/run/secrets/kubernetes.io").exists()
    }

    /// Detect serverless environment.
    fn detect_serverless() -> bool {
        // AWS Lambda
        std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok() ||
        // Google Cloud Functions
        std::env::var("FUNCTION_NAME").is_ok() ||
        // Azure Functions
        std::env::var("AZURE_FUNCTIONS_ENVIRONMENT").is_ok() ||
        // Cloud Run
        std::env::var("K_SERVICE").is_ok()
    }

    /// Detect kernel version on Linux.
    #[cfg(target_os = "linux")]
    fn detect_kernel_version() -> Option<String> {
        fs::read_to_string("/proc/version")
            .ok()
            .and_then(|v| v.split_whitespace().nth(2).map(|s| s.to_string()))
    }

    /// Detect kernel version (none on non-Linux).
    #[cfg(not(target_os = "linux"))]
    fn detect_kernel_version() -> Option<String> {
        None
    }

    /// Detect CPU count.
    fn detect_cpu_count() -> usize {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
    }
}

/// Check if io_uring is available in the current environment.
///
/// This is a convenience function that creates a minimal runtime to test availability.
///
/// # Example
///
/// ```rust,no_run
/// use safer_ring::runtime::is_io_uring_available;
///
/// if is_io_uring_available() {
///     println!("io_uring is available!");
/// } else {
///     println!("io_uring is not available, will use fallback");
/// }
/// ```
pub fn is_io_uring_available() -> bool {
    Runtime::is_io_uring_available()
}

/// Get environment information for the current system.
///
/// # Example
///
/// ```rust,no_run
/// use safer_ring::runtime::get_environment_info;
///
/// let env = get_environment_info();
/// if env.is_cloud_environment() {
///     println!("Running in cloud environment");
/// }
/// println!("CPU count: {}", env.cpu_count);
/// ```
pub fn get_environment_info() -> EnvironmentInfo {
    EnvironmentInfo::detect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_properties() {
        assert_eq!(
            Backend::IoUring.description(),
            "High-performance io_uring backend"
        );
        assert_eq!(
            Backend::Epoll.description(),
            "Traditional epoll-based backend"
        );
        assert_eq!(
            Backend::Stub.description(),
            "Stub implementation for non-Linux platforms"
        );

        assert!(Backend::IoUring.supports_advanced_features());
        assert!(!Backend::Epoll.supports_advanced_features());
        assert!(!Backend::Stub.supports_advanced_features());

        assert_eq!(Backend::IoUring.performance_multiplier(), 3.0);
        assert_eq!(Backend::Epoll.performance_multiplier(), 1.0);
        assert_eq!(Backend::Stub.performance_multiplier(), 0.1);
    }

    #[test]
    fn test_runtime_auto_detect() {
        let runtime = Runtime::auto_detect().unwrap();

        // Should have detected some backend
        match runtime.backend() {
            Backend::IoUring | Backend::Epoll | Backend::Stub => {}
        }

        // Environment should be detected
        let env = runtime.environment();
        assert!(env.cpu_count > 0);
    }

    #[test]
    fn test_environment_detection() {
        let env = EnvironmentInfo::detect();

        // Should detect CPU count
        assert!(env.cpu_count > 0);

        // Cloud environment detection should not panic
        let _ = env.is_cloud_environment();
    }

    #[test]
    fn test_performance_guidance() {
        let runtime = Runtime::auto_detect().unwrap();
        let guidance = runtime.performance_guidance();

        // Should provide at least one guidance point
        assert!(!guidance.is_empty());

        // Each guidance should be non-empty
        for guide in guidance {
            assert!(!guide.is_empty());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_stub_backend_on_non_linux_request() {
        // On Linux, requesting stub should work
        let runtime = Runtime::with_backend(Backend::Stub).unwrap();
        assert_eq!(runtime.backend(), Backend::Stub);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_epoll_backend_fails_on_non_linux() {
        // On non-Linux, requesting epoll should fail
        let result = Runtime::with_backend(Backend::Epoll);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_io_uring_available() {
        // Should not panic regardless of availability
        let _ = is_io_uring_available();
    }

    #[test]
    fn test_get_environment_info() {
        let env = get_environment_info();
        assert!(env.cpu_count > 0);
    }
}
