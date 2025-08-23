//! Error types and handling for safer-ring operations.
//!
//! This module provides a comprehensive error type that covers all possible
//! failure modes in safer-ring operations, with proper error chaining and
//! platform-specific handling.

use static_assertions;
use thiserror::Error;

/// Result type alias for safer-ring operations.
///
/// This type alias simplifies function signatures throughout the crate by
/// providing a consistent error type while allowing different success types.
pub type Result<T> = std::result::Result<T, SaferRingError>;

/// Comprehensive error type for safer-ring operations.
///
/// This enum covers all possible error conditions that can occur during
/// safer-ring operations, from memory safety violations to underlying
/// system errors. Each variant provides specific context about the failure.
///
/// # Design Notes
///
/// - Uses `thiserror` for automatic `Error` trait implementation
/// - Provides automatic conversion from common error types via `#[from]`
/// - Platform-specific variants are conditionally compiled
/// - All variants are `Send + Sync` for use in async contexts
#[derive(Debug, Error)]
pub enum SaferRingError {
    /// Buffer is still in flight and cannot be accessed.
    ///
    /// This error occurs when attempting to access or drop a buffer
    /// that is currently being used by an in-flight io_uring operation.
    /// The buffer must remain pinned until the operation completes.
    #[error("Buffer still in flight")]
    BufferInFlight,

    /// Operation is not yet completed.
    ///
    /// This error occurs when attempting to extract results from an
    /// operation that hasn't finished yet. Use polling or await the
    /// operation's future to wait for completion.
    #[error("Operation not completed")]
    OperationPending,

    /// Ring has operations in flight and cannot be dropped.
    ///
    /// This error occurs when attempting to drop a Ring that still has
    /// pending operations. All operations must complete before the ring
    /// can be safely destroyed to prevent use-after-free bugs.
    #[error("Ring has {count} operations in flight")]
    OperationsInFlight {
        /// Number of operations still in flight
        count: usize,
    },

    /// Invalid operation state transition.
    ///
    /// This error occurs when attempting to transition an operation to
    /// an invalid state (e.g., submitting an already-submitted operation).
    /// The type system should prevent most of these at compile time.
    #[error("Invalid operation state transition")]
    InvalidStateTransition,

    /// Resource is not registered.
    ///
    /// This error occurs when attempting to use a file descriptor or
    /// buffer that hasn't been registered with the ring, when registration
    /// is required for the operation.
    #[error("Resource not registered")]
    NotRegistered,

    /// Buffer pool is empty.
    ///
    /// This error occurs when requesting a buffer from an empty pool.
    /// Consider increasing pool size or implementing fallback allocation.
    #[error("Buffer pool is empty")]
    PoolEmpty,

    /// Buffer pool mutex is poisoned.
    ///
    /// This error occurs when a thread panics while holding the pool's mutex,
    /// leaving it in a poisoned state. The pool cannot be safely used after this.
    #[error("Buffer pool mutex is poisoned")]
    PoolPoisoned,

    /// Underlying io_uring error (Linux only).
    ///
    /// This wraps errors from the underlying io_uring system calls,
    /// providing context while preserving the original error information.
    #[cfg(target_os = "linux")]
    #[error("io_uring error: {0}")]
    IoUring(#[from] io_uring::Error),

    /// Standard I/O error.
    ///
    /// This wraps standard library I/O errors, which can occur during
    /// file operations or when io_uring falls back to synchronous I/O.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// Ensure our error type can be safely sent between threads
// This is important for async runtimes that may move futures between threads
static_assertions::assert_impl_all!(SaferRingError: Send, Sync);

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io::{Error as IoError, ErrorKind};

    /// Test error message formatting for all variants
    mod error_messages {
        use super::*;

        #[test]
        fn buffer_in_flight() {
            let error = SaferRingError::BufferInFlight;
            assert_eq!(error.to_string(), "Buffer still in flight");
        }

        #[test]
        fn operation_pending() {
            let error = SaferRingError::OperationPending;
            assert_eq!(error.to_string(), "Operation not completed");
        }

        #[test]
        fn operations_in_flight() {
            let error = SaferRingError::OperationsInFlight { count: 5 };
            assert_eq!(error.to_string(), "Ring has 5 operations in flight");
        }

        #[test]
        fn invalid_state_transition() {
            let error = SaferRingError::InvalidStateTransition;
            assert_eq!(error.to_string(), "Invalid operation state transition");
        }

        #[test]
        fn not_registered() {
            let error = SaferRingError::NotRegistered;
            assert_eq!(error.to_string(), "Resource not registered");
        }

        #[test]
        fn pool_empty() {
            let error = SaferRingError::PoolEmpty;
            assert_eq!(error.to_string(), "Buffer pool is empty");
        }
    }

    /// Test error conversion and chaining
    mod error_conversion {
        use super::*;

        #[test]
        fn io_error_conversion() {
            let io_error = IoError::new(ErrorKind::PermissionDenied, "Access denied");
            let safer_ring_error = SaferRingError::from(io_error);

            // Verify the conversion worked correctly
            let SaferRingError::Io(ref e) = safer_ring_error else {
                panic!("Expected Io error variant");
            };

            assert_eq!(e.kind(), ErrorKind::PermissionDenied);
            assert!(e.to_string().contains("Access denied"));
            assert!(safer_ring_error.to_string().contains("I/O error"));
        }

        #[cfg(target_os = "linux")]
        #[test]
        fn io_uring_error_conversion() {
            // Create an io_uring error from an I/O error for testing
            let io_error = IoError::new(ErrorKind::InvalidInput, "Invalid argument");
            let io_uring_error = io_uring::Error::from(io_error);
            let safer_ring_error = SaferRingError::from(io_uring_error);

            let SaferRingError::IoUring(_) = safer_ring_error else {
                panic!("Expected IoUring error variant");
            };

            assert!(safer_ring_error.to_string().contains("io_uring error"));
        }
    }

    /// Test error trait implementations
    mod error_traits {
        use super::*;

        #[test]
        fn implements_error_trait() {
            let error = SaferRingError::BufferInFlight;

            // Verify it implements std::error::Error
            let _: &dyn std::error::Error = &error;

            // Simple errors should have no source
            assert!(error.source().is_none());
        }

        #[test]
        fn preserves_error_source() {
            let io_error = IoError::new(ErrorKind::NotFound, "File not found");
            let safer_ring_error = SaferRingError::from(io_error);

            // Verify the source is preserved
            assert!(safer_ring_error.source().is_some());

            let source = safer_ring_error.source().unwrap();
            let io_err = source.downcast_ref::<IoError>().unwrap();
            assert_eq!(io_err.kind(), ErrorKind::NotFound);
        }

        #[test]
        fn debug_formatting() {
            let error = SaferRingError::OperationsInFlight { count: 3 };
            let debug_str = format!("{:?}", error);

            assert!(debug_str.contains("OperationsInFlight"));
            assert!(debug_str.contains("count: 3"));
        }
    }

    /// Test the Result type alias
    mod result_alias {
        use super::*;

        #[test]
        fn success_case() {
            fn returns_success() -> Result<i32> {
                Ok(42)
            }

            assert_eq!(returns_success().unwrap(), 42);
        }

        #[test]
        fn error_case() {
            fn returns_error() -> Result<i32> {
                Err(SaferRingError::BufferInFlight)
            }

            assert!(returns_error().is_err());
            match returns_error() {
                Err(SaferRingError::BufferInFlight) => {}
                _ => panic!("Expected BufferInFlight error"),
            }
        }
    }
}
