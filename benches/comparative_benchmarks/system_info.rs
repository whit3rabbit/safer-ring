//! System information collection for benchmark context.

use std::fs;

/// System information collected at benchmark runtime.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub kernel_version: String,
    pub io_uring_version: Option<String>,
    pub cpu_info: String,
    pub memory_info: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl SystemInfo {
    /// Collects comprehensive system information for benchmark context.
    pub fn collect() -> Self {
        Self {
            kernel_version: get_kernel_version(),
            io_uring_version: get_io_uring_version(),
            cpu_info: get_cpu_info(),
            memory_info: get_memory_info(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Logs system information in a formatted, human-readable way.
    pub fn log_system_info(&self) {
        println!("🖥️  System Information");
        println!("=====================");
        println!(
            "📅 Timestamp: {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("🐧 Kernel: {}", self.kernel_version);
        if let Some(ref uring_ver) = self.io_uring_version {
            println!("⚡ io_uring: {}", uring_ver);
        } else {
            println!("⚡ io_uring: Not available or version unknown");
        }
        println!("💻 CPU: {}", self.cpu_info);
        println!("💾 Memory: {}", self.memory_info);
        println!();
    }
}

/// Extracts kernel version from /proc/version.
fn get_kernel_version() -> String {
    fs::read_to_string("/proc/version")
        .unwrap_or_else(|_| "Unknown".to_string())
        .lines()
        .next()
        .unwrap_or("Unknown")
        .to_string()
}

/// Detects io_uring availability and status.
///
/// Checks both the kernel control file and version heuristics
/// to provide comprehensive io_uring support information.
fn get_io_uring_version() -> Option<String> {
    // Primary check: kernel control file
    if fs::metadata("/proc/sys/kernel/io_uring_disabled").is_ok() {
        let disabled = fs::read_to_string("/proc/sys/kernel/io_uring_disabled")
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            .to_string();

        return Some(if disabled == "0" {
            "Available (enabled)".to_string()
        } else {
            "Available (disabled)".to_string()
        });
    }

    // Fallback: version-based detection
    let version = get_kernel_version();
    if version.contains("5.1") || (version.contains("5.") && !version.contains("5.0")) {
        Some("Supported (kernel 5.1+)".to_string())
    } else {
        None
    }
}

/// Extracts CPU model information from /proc/cpuinfo.
fn get_cpu_info() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("model name"))
                .map(|line| {
                    line.split(':')
                        .nth(1)
                        .unwrap_or("Unknown")
                        .trim()
                        .to_string()
                })
        })
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Extracts and formats memory information from /proc/meminfo.
///
/// Converts KB values to GB for better readability in benchmark reports.
fn get_memory_info() -> String {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("MemTotal"))
                .map(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .unwrap_or("Unknown")
                        .to_string()
                })
                .map(|kb| {
                    if let Ok(kb_num) = kb.parse::<u64>() {
                        let gb = kb_num as f64 / 1024.0 / 1024.0;
                        format!("{:.1} GB", gb)
                    } else {
                        kb
                    }
                })
        })
        .unwrap_or_else(|| "Unknown".to_string())
}
