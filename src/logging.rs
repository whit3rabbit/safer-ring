//! Comprehensive logging and debugging support for safer-ring.
//!
//! This module provides structured logging, performance metrics, and debugging
//! utilities to help diagnose issues and optimize performance in safer-ring applications.

use crate::error::{Result, SaferRingError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Log level for safer-ring operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Trace-level logging (very verbose)
    Trace = 0,
    /// Debug-level logging
    Debug = 1,
    /// Info-level logging
    Info = 2,
    /// Warning-level logging
    Warn = 3,
    /// Error-level logging
    Error = 4,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Log entry containing structured information about safer-ring operations.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp when the log entry was created
    pub timestamp: SystemTime,
    /// Log level
    pub level: LogLevel,
    /// Component that generated the log
    pub component: String,
    /// Operation ID if applicable
    pub operation_id: Option<u64>,
    /// File descriptor if applicable
    pub fd: Option<i32>,
    /// Message content
    pub message: String,
    /// Additional structured data
    pub metadata: HashMap<String, String>,
    /// Duration if this is a timing log
    pub duration: Option<Duration>,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(level: LogLevel, component: &str, message: &str) -> Self {
        Self {
            timestamp: SystemTime::now(),
            level,
            component: component.to_string(),
            operation_id: None,
            fd: None,
            message: message.to_string(),
            metadata: HashMap::new(),
            duration: None,
        }
    }

    /// Add operation ID to the log entry.
    pub fn with_operation_id(mut self, operation_id: u64) -> Self {
        self.operation_id = Some(operation_id);
        self
    }

    /// Add file descriptor to the log entry.
    pub fn with_fd(mut self, fd: i32) -> Self {
        self.fd = Some(fd);
        self
    }

    /// Add metadata to the log entry.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Add duration to the log entry.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Format the log entry as a human-readable string.
    pub fn format(&self) -> String {
        let timestamp = self
            .timestamp
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let mut parts = vec![
            format!("[{}]", timestamp),
            format!("{}", self.level),
            format!("{}", self.component),
        ];

        if let Some(op_id) = self.operation_id {
            parts.push(format!("op:{}", op_id));
        }

        if let Some(fd) = self.fd {
            parts.push(format!("fd:{}", fd));
        }

        parts.push(self.message.clone());

        if let Some(duration) = self.duration {
            parts.push(format!("duration:{}μs", duration.as_micros()));
        }

        if !self.metadata.is_empty() {
            let metadata_str = self
                .metadata
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect::<Vec<_>>()
                .join(",");
            parts.push(format!("metadata:{{{}}}", metadata_str));
        }

        parts.join(" ")
    }

    /// Format the log entry as JSON.
    pub fn format_json(&self) -> String {
        let timestamp = self
            .timestamp
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let mut json_parts = vec![
            format!("\"timestamp\":{}", timestamp),
            format!("\"level\":\"{}\"", self.level),
            format!("\"component\":\"{}\"", self.component),
            format!("\"message\":\"{}\"", self.message.replace('"', "\\\"")),
        ];

        if let Some(op_id) = self.operation_id {
            json_parts.push(format!("\"operation_id\":{}", op_id));
        }

        if let Some(fd) = self.fd {
            json_parts.push(format!("\"fd\":{}", fd));
        }

        if let Some(duration) = self.duration {
            json_parts.push(format!("\"duration_us\":{}", duration.as_micros()));
        }

        if !self.metadata.is_empty() {
            let metadata_json = self
                .metadata
                .iter()
                .map(|(k, v)| format!("\"{}\":\"{}\"", k, v.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(",");
            json_parts.push(format!("\"metadata\":{{{}}}", metadata_json));
        }

        format!("{{{}}}", json_parts.join(","))
    }
}

/// Trait for log output destinations.
pub trait LogOutput: Send + Sync {
    /// Write a log entry to the output.
    fn write(&self, entry: &LogEntry) -> Result<()>;

    /// Flush any buffered output.
    fn flush(&self) -> Result<()>;
}

/// Console log output that writes to stderr.
#[derive(Debug)]
pub struct ConsoleOutput {
    /// Whether to use JSON format
    json_format: bool,
}

impl ConsoleOutput {
    /// Create a new console output with text format.
    pub fn new() -> Self {
        Self { json_format: false }
    }

    /// Create a new console output with JSON format.
    pub fn new_json() -> Self {
        Self { json_format: true }
    }
}

impl Default for ConsoleOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl LogOutput for ConsoleOutput {
    fn write(&self, entry: &LogEntry) -> Result<()> {
        let formatted = if self.json_format {
            entry.format_json()
        } else {
            entry.format()
        };

        eprintln!("{}", formatted);
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        use std::io::Write;
        std::io::stderr().flush().map_err(SaferRingError::Io)?;
        Ok(())
    }
}

/// File log output that writes to a file.
#[derive(Debug)]
pub struct FileOutput {
    /// Path to the log file
    path: std::path::PathBuf,
    /// Whether to use JSON format
    json_format: bool,
}

impl FileOutput {
    /// Create a new file output.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            json_format: false,
        }
    }

    /// Create a new file output with JSON format.
    pub fn new_json<P: AsRef<std::path::Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            json_format: true,
        }
    }
}

impl LogOutput for FileOutput {
    fn write(&self, entry: &LogEntry) -> Result<()> {
        use std::io::Write;

        let formatted = if self.json_format {
            entry.format_json()
        } else {
            entry.format()
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(SaferRingError::Io)?;

        writeln!(file, "{}", formatted).map_err(SaferRingError::Io)?;
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        // File is opened and closed for each write, so no explicit flush needed
        Ok(())
    }
}

/// Central logger for safer-ring operations.
pub struct Logger {
    /// Minimum log level to output
    min_level: LogLevel,
    /// Output destinations
    outputs: Vec<Box<dyn LogOutput>>,
    /// Performance metrics
    metrics: Arc<Mutex<PerformanceMetrics>>,
}

impl Logger {
    /// Create a new logger with console output.
    pub fn new() -> Self {
        Self {
            min_level: LogLevel::Info,
            outputs: vec![Box::new(ConsoleOutput::new())],
            metrics: Arc::new(Mutex::new(PerformanceMetrics::new())),
        }
    }

    /// Set the minimum log level.
    pub fn set_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    /// Add an output destination.
    pub fn add_output(&mut self, output: Box<dyn LogOutput>) {
        self.outputs.push(output);
    }

    /// Log a message at the specified level.
    pub fn log(&self, level: LogLevel, component: &str, message: &str) {
        if level >= self.min_level {
            let entry = LogEntry::new(level, component, message);
            self.write_entry(&entry);
        }
    }

    /// Log a message with operation context.
    pub fn log_operation(
        &self,
        level: LogLevel,
        component: &str,
        operation_id: u64,
        fd: Option<i32>,
        message: &str,
    ) {
        if level >= self.min_level {
            let mut entry =
                LogEntry::new(level, component, message).with_operation_id(operation_id);

            if let Some(fd) = fd {
                entry = entry.with_fd(fd);
            }

            self.write_entry(&entry);
        }
    }

    /// Log a timing measurement.
    pub fn log_timing(&self, component: &str, operation: &str, duration: Duration) {
        if LogLevel::Debug >= self.min_level {
            let entry = LogEntry::new(
                LogLevel::Debug,
                component,
                &format!("{} completed", operation),
            )
            .with_duration(duration);
            self.write_entry(&entry);

            // Update performance metrics
            if let Ok(mut metrics) = self.metrics.lock() {
                metrics.record_operation(operation, duration);
            }
        }
    }

    /// Log an error with context.
    pub fn log_error(&self, component: &str, error: &SaferRingError, context: &str) {
        let message = format!("{}: {}", context, error);
        let entry = LogEntry::new(LogLevel::Error, component, &message);
        self.write_entry(&entry);
    }

    /// Write a log entry to all outputs.
    fn write_entry(&self, entry: &LogEntry) {
        for output in &self.outputs {
            if let Err(e) = output.write(entry) {
                eprintln!("Failed to write log entry: {}", e);
            }
        }
    }

    /// Flush all outputs.
    pub fn flush(&self) {
        for output in &self.outputs {
            if let Err(e) = output.flush() {
                eprintln!("Failed to flush log output: {}", e);
            }
        }
    }

    /// Get performance metrics.
    pub fn get_metrics(&self) -> Result<PerformanceMetrics> {
        self.metrics.lock().map(|m| m.clone()).map_err(|_| {
            SaferRingError::Io(std::io::Error::other("Failed to acquire metrics lock"))
        })
    }

    /// Reset performance metrics.
    pub fn reset_metrics(&self) -> Result<()> {
        self.metrics.lock().map(|mut m| m.reset()).map_err(|_| {
            SaferRingError::Io(std::io::Error::other("Failed to acquire metrics lock"))
        })
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance metrics for safer-ring operations.
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Operation counts by type
    operation_counts: HashMap<String, u64>,
    /// Total duration by operation type
    operation_durations: HashMap<String, Duration>,
    /// Minimum duration by operation type
    min_durations: HashMap<String, Duration>,
    /// Maximum duration by operation type
    max_durations: HashMap<String, Duration>,
    /// Start time for metrics collection
    start_time: Instant,
}

impl PerformanceMetrics {
    /// Create new performance metrics.
    pub fn new() -> Self {
        Self {
            operation_counts: HashMap::new(),
            operation_durations: HashMap::new(),
            min_durations: HashMap::new(),
            max_durations: HashMap::new(),
            start_time: Instant::now(),
        }
    }

    /// Record an operation timing.
    pub fn record_operation(&mut self, operation: &str, duration: Duration) {
        let count = self
            .operation_counts
            .entry(operation.to_string())
            .or_insert(0);
        *count += 1;

        let total_duration = self
            .operation_durations
            .entry(operation.to_string())
            .or_insert(Duration::ZERO);
        *total_duration += duration;

        let min_duration = self
            .min_durations
            .entry(operation.to_string())
            .or_insert(duration);
        if duration < *min_duration {
            *min_duration = duration;
        }

        let max_duration = self
            .max_durations
            .entry(operation.to_string())
            .or_insert(duration);
        if duration > *max_duration {
            *max_duration = duration;
        }
    }

    /// Get the count for a specific operation type.
    pub fn get_count(&self, operation: &str) -> u64 {
        self.operation_counts.get(operation).copied().unwrap_or(0)
    }

    /// Get the average duration for a specific operation type.
    pub fn get_average_duration(&self, operation: &str) -> Option<Duration> {
        let count = self.get_count(operation);
        if count == 0 {
            return None;
        }

        self.operation_durations
            .get(operation)
            .map(|total| *total / count as u32)
    }

    /// Get the minimum duration for a specific operation type.
    pub fn get_min_duration(&self, operation: &str) -> Option<Duration> {
        self.min_durations.get(operation).copied()
    }

    /// Get the maximum duration for a specific operation type.
    pub fn get_max_duration(&self, operation: &str) -> Option<Duration> {
        self.max_durations.get(operation).copied()
    }

    /// Get all operation types.
    pub fn get_operation_types(&self) -> Vec<String> {
        self.operation_counts.keys().cloned().collect()
    }

    /// Get total operations across all types.
    pub fn get_total_operations(&self) -> u64 {
        self.operation_counts.values().sum()
    }

    /// Get the duration since metrics collection started.
    pub fn get_collection_duration(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Reset all metrics.
    pub fn reset(&mut self) {
        self.operation_counts.clear();
        self.operation_durations.clear();
        self.min_durations.clear();
        self.max_durations.clear();
        self.start_time = Instant::now();
    }

    /// Generate a summary report.
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Safer-Ring Performance Metrics ===\n");
        report.push_str(&format!(
            "Collection Duration: {:?}\n",
            self.get_collection_duration()
        ));
        report.push_str(&format!(
            "Total Operations: {}\n\n",
            self.get_total_operations()
        ));

        for operation in self.get_operation_types() {
            report.push_str(&format!("Operation: {}\n", operation));
            report.push_str(&format!("  Count: {}\n", self.get_count(&operation)));

            if let Some(avg) = self.get_average_duration(&operation) {
                report.push_str(&format!("  Average Duration: {:?}\n", avg));
            }

            if let Some(min) = self.get_min_duration(&operation) {
                report.push_str(&format!("  Min Duration: {:?}\n", min));
            }

            if let Some(max) = self.get_max_duration(&operation) {
                report.push_str(&format!("  Max Duration: {:?}\n", max));
            }

            report.push('\n');
        }

        report
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Global logger instance.
static GLOBAL_LOGGER: std::sync::OnceLock<Arc<Mutex<Logger>>> = std::sync::OnceLock::new();

/// Initialize the global logger.
pub fn init_logger() -> Arc<Mutex<Logger>> {
    GLOBAL_LOGGER
        .get_or_init(|| Arc::new(Mutex::new(Logger::new())))
        .clone()
}

/// Log a message using the global logger.
pub fn log(level: LogLevel, component: &str, message: &str) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        if let Ok(logger) = logger.lock() {
            logger.log(level, component, message);
        }
    }
}

/// Log an operation using the global logger.
pub fn log_operation(
    level: LogLevel,
    component: &str,
    operation_id: u64,
    fd: Option<i32>,
    message: &str,
) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        if let Ok(logger) = logger.lock() {
            logger.log_operation(level, component, operation_id, fd, message);
        }
    }
}

/// Log timing using the global logger.
pub fn log_timing(component: &str, operation: &str, duration: Duration) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        if let Ok(logger) = logger.lock() {
            logger.log_timing(component, operation, duration);
        }
    }
}

/// Convenience macros for logging.
///
/// Log a trace-level message using the global logger.
#[macro_export]
macro_rules! log_trace {
    ($component:expr, $($arg:tt)*) => {
        $crate::logging::log($crate::logging::LogLevel::Trace, $component, &format!($($arg)*))
    };
}

/// Log a debug-level message using the global logger.
#[macro_export]
macro_rules! log_debug {
    ($component:expr, $($arg:tt)*) => {
        $crate::logging::log($crate::logging::LogLevel::Debug, $component, &format!($($arg)*))
    };
}

/// Log an info-level message using the global logger.
#[macro_export]
macro_rules! log_info {
    ($component:expr, $($arg:tt)*) => {
        $crate::logging::log($crate::logging::LogLevel::Info, $component, &format!($($arg)*))
    };
}

/// Log a warning-level message using the global logger.
#[macro_export]
macro_rules! log_warn {
    ($component:expr, $($arg:tt)*) => {
        $crate::logging::log($crate::logging::LogLevel::Warn, $component, &format!($($arg)*))
    };
}

/// Log an error-level message using the global logger.
#[macro_export]
macro_rules! log_error {
    ($component:expr, $($arg:tt)*) => {
        $crate::logging::log($crate::logging::LogLevel::Error, $component, &format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    // Arc import removed as unused

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(LogLevel::Info, "test", "test message")
            .with_operation_id(123)
            .with_fd(4)
            .with_metadata("key", "value")
            .with_duration(Duration::from_millis(10));

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.component, "test");
        assert_eq!(entry.message, "test message");
        assert_eq!(entry.operation_id, Some(123));
        assert_eq!(entry.fd, Some(4));
        assert_eq!(entry.metadata.get("key"), Some(&"value".to_string()));
        assert_eq!(entry.duration, Some(Duration::from_millis(10)));
    }

    #[test]
    fn test_log_entry_formatting() {
        let entry = LogEntry::new(LogLevel::Info, "test", "test message");
        let formatted = entry.format();

        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("test"));
        assert!(formatted.contains("test message"));
    }

    #[test]
    fn test_log_entry_json_formatting() {
        let entry = LogEntry::new(LogLevel::Info, "test", "test message");
        let json = entry.format_json();

        assert!(json.contains("\"level\":\"INFO\""));
        assert!(json.contains("\"component\":\"test\""));
        assert!(json.contains("\"message\":\"test message\""));
    }

    #[test]
    fn test_logger_creation() {
        let logger = Logger::new();
        assert_eq!(logger.min_level, LogLevel::Info);
        assert_eq!(logger.outputs.len(), 1);
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::new();

        metrics.record_operation("read", Duration::from_millis(10));
        metrics.record_operation("read", Duration::from_millis(20));
        metrics.record_operation("write", Duration::from_millis(15));

        assert_eq!(metrics.get_count("read"), 2);
        assert_eq!(metrics.get_count("write"), 1);
        assert_eq!(metrics.get_total_operations(), 3);

        assert_eq!(
            metrics.get_average_duration("read"),
            Some(Duration::from_millis(15))
        );
        assert_eq!(
            metrics.get_min_duration("read"),
            Some(Duration::from_millis(10))
        );
        assert_eq!(
            metrics.get_max_duration("read"),
            Some(Duration::from_millis(20))
        );
    }

    #[test]
    fn test_global_logger() {
        let _logger = init_logger();

        log(LogLevel::Info, "test", "test message");
        log_operation(LogLevel::Debug, "test", 123, Some(4), "operation message");
        log_timing("test", "read", Duration::from_millis(10));
    }
}
