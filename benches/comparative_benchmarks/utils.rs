//! Utility functions for benchmark setup and execution.

use std::io::Write;
use tempfile::NamedTempFile;

/// Creates a test file of the specified size filled with zeros.
///
/// Uses a simple zero-fill pattern for consistent, reproducible benchmarks
/// without introducing filesystem-dependent performance variations.
/// The zero pattern is optimal because:
/// - Compresses well if the filesystem supports compression
/// - Doesn't trigger special handling for sparse files
/// - Provides predictable I/O patterns for measurement
pub fn setup_test_file(size: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temporary file");
    let data = vec![0u8; size];
    file.write_all(&data).expect("Failed to write test data");
    file.flush().expect("Failed to flush test file");
    file
}
