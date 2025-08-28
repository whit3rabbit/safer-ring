//! Result types for batch operations.

/// Result of a batch submission.
///
/// Contains the results of all operations in the batch, including both
/// successful completions and errors. Operations are indexed by their
/// position in the original batch, making it easy to correlate results
/// with the original operations.
///
/// # Examples
///
/// ```rust
/// # use safer_ring::ring::{BatchResult, OperationResult};
/// let results = vec![
///     OperationResult::Success(1024),  // Read 1024 bytes
///     OperationResult::Error("Permission denied".to_string()),
///     OperationResult::Success(512),   // Wrote 512 bytes
/// ];
///
/// let batch_result = BatchResult::new(results);
/// assert_eq!(batch_result.successful_count, 2);
/// assert_eq!(batch_result.failed_count, 1);
/// assert!(!batch_result.all_succeeded());
/// assert!(batch_result.any_failed());
///
/// // Iterate through successful operations
/// for (index, bytes) in batch_result.successes() {
///     println!("Operation {} succeeded with {} bytes", index, bytes);
/// }
/// ```
#[derive(Debug)]
pub struct BatchResult {
    /// Results indexed by operation position in the batch
    pub results: Vec<OperationResult>,
    /// Number of operations that completed successfully
    pub successful_count: usize,
    /// Number of operations that failed
    pub failed_count: usize,
}

/// Result of a single operation within a batch.
///
/// Represents the outcome of an individual I/O operation that was part of
/// a batch submission. Operations can succeed with a return value, fail with
/// an error, or be cancelled due to batch-level failures or dependency issues.
///
/// # Variants
///
/// - `Success(i32)`: Operation completed successfully, with the return value
///   (typically bytes transferred for I/O operations)
/// - `Error(String)`: Operation failed with the specified error message
/// - `Cancelled`: Operation was cancelled before execution (usually due to
///   dependency failures or batch-level errors)
///
/// # Examples
///
/// ```rust
/// # use safer_ring::ring::OperationResult;
/// // Successful read operation
/// let success = OperationResult::Success(1024);
/// assert!(success.is_success());
/// assert_eq!(success.success_value(), Some(1024));
///
/// // Failed operation
/// let error = OperationResult::Error("File not found".to_string());
/// assert!(error.is_error());
/// assert_eq!(error.error(), Some(&"File not found".to_string()));
///
/// // Cancelled operation
/// let cancelled = OperationResult::Cancelled;
/// assert!(cancelled.is_cancelled());
/// ```
#[derive(Debug, Clone)]
pub enum OperationResult {
    /// Operation completed successfully with the given result
    Success(i32),
    /// Operation failed with the given error
    Error(String), // Use String instead of std::io::Error for cloneability
    /// Operation was cancelled due to batch failure or dependency failure
    Cancelled,
}

impl BatchResult {
    /// Create a new batch result.
    pub fn new(results: Vec<OperationResult>) -> Self {
        let successful_count = results
            .iter()
            .filter(|r| matches!(r, OperationResult::Success(_)))
            .count();
        let failed_count = results
            .iter()
            .filter(|r| matches!(r, OperationResult::Error(_)))
            .count();

        Self {
            results,
            successful_count,
            failed_count,
        }
    }

    /// Get the result for a specific operation by index.
    pub fn get(&self, index: usize) -> Option<&OperationResult> {
        self.results.get(index)
    }

    /// Check if all operations in the batch succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.failed_count == 0
            && self
                .results
                .iter()
                .all(|r| !matches!(r, OperationResult::Cancelled))
    }

    /// Check if any operations in the batch failed.
    pub fn any_failed(&self) -> bool {
        self.failed_count > 0
    }

    /// Get an iterator over successful results.
    pub fn successes(&self) -> impl Iterator<Item = (usize, i32)> + '_ {
        self.results.iter().enumerate().filter_map(|(i, r)| {
            if let OperationResult::Success(value) = r {
                Some((i, *value))
            } else {
                None
            }
        })
    }

    /// Get an iterator over failed results.
    pub fn failures(&self) -> impl Iterator<Item = (usize, &String)> + '_ {
        self.results.iter().enumerate().filter_map(|(i, r)| {
            if let OperationResult::Error(error) = r {
                Some((i, error))
            } else {
                None
            }
        })
    }
}

impl OperationResult {
    /// Check if this result represents a successful operation.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Check if this result represents a failed operation.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Check if this result represents a cancelled operation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Get the success value if this result is successful.
    pub fn success_value(&self) -> Option<i32> {
        if let Self::Success(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Get the error if this result is an error.
    pub fn error(&self) -> Option<&String> {
        if let Self::Error(error) = self {
            Some(error)
        } else {
            None
        }
    }
}
