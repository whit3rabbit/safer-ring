//! Memory allocation utilities for pinned buffers.
//!
//! This module provides specialized buffer allocation functions optimized for
//! Direct Memory Access (DMA) operations and io_uring. The allocators ensure
//! proper memory alignment and zero-initialization for safe kernel interactions.
//!
//! # Key Features
//!
//! - Page-aligned allocation for optimal DMA performance
//! - Zero-initialized memory for security
//! - Custom alignment support for specialized use cases
//! - Fallback strategies for allocation failures
//!
//! # Examples
//!
//! ```
//! use safer_ring::buffer::allocation::{allocate_aligned_buffer, allocate_with_alignment};
//!
//! // Allocate a page-aligned buffer for DMA operations
//! let buffer = allocate_aligned_buffer(8192);
//! assert_eq!(buffer.len(), 8192);
//!
//! // Allocate with custom alignment
//! let aligned_buffer = allocate_with_alignment(1024, 64);
//! assert_eq!(aligned_buffer.len(), 1024);
//! ```

use std::alloc::{alloc_zeroed, Layout};

/// Allocates a zero-initialized buffer with optimal alignment for DMA operations.
///
/// This function attempts to allocate memory with page-aligned (4096 byte) alignment
/// for optimal performance with Direct Memory Access operations. If page alignment
/// fails, it falls back to natural byte alignment. The allocated memory is
/// zero-initialized for security.
///
/// # Parameters
///
/// * `size` - The size in bytes of the buffer to allocate. If 0, returns an empty slice.
///
/// # Returns
///
/// Returns a `Box<[u8]>` containing the allocated zero-initialized buffer.
/// The buffer will be page-aligned (4096 bytes) when possible, or naturally
/// aligned as a fallback.
///
/// # Panics
///
/// Panics if:
/// - Memory allocation fails (out of memory)
/// - The size parameter is too large for the system to handle
///
/// # Examples
///
/// ```
/// use safer_ring::buffer::allocation::allocate_aligned_buffer;
///
/// // Allocate an 8KB buffer
/// let buffer = allocate_aligned_buffer(8192);
/// assert_eq!(buffer.len(), 8192);
/// assert!(buffer.iter().all(|&b| b == 0)); // All zeros
///
/// // Empty buffer case
/// let empty = allocate_aligned_buffer(0);
/// assert_eq!(empty.len(), 0);
/// ```
pub fn allocate_aligned_buffer(size: usize) -> Box<[u8]> {
    if size == 0 {
        return Box::new([]);
    }

    // Try page-aligned allocation first for better DMA performance
    let layout = Layout::from_size_align(size, 4096).unwrap_or_else(|_| {
        // Fall back to natural alignment if page alignment fails
        Layout::from_size_align(size, std::mem::align_of::<u8>())
            .expect("Failed to create layout for buffer allocation")
    });

    unsafe {
        let ptr = alloc_zeroed(layout);
        if ptr.is_null() {
            panic!("Failed to allocate aligned buffer of size {size}");
        }

        // Convert raw pointer to boxed slice
        let slice = std::slice::from_raw_parts_mut(ptr, size);
        Box::from_raw(slice)
    }
}

/// Allocates a buffer with specific alignment requirements.
///
/// This function allocates zero-initialized memory with a custom alignment
/// requirement. Unlike `allocate_aligned_buffer`, this function allows you to
/// specify the exact alignment needed, which is useful for specialized hardware
/// requirements or performance optimizations.
///
/// # Parameters
///
/// * `size` - The size in bytes of the buffer to allocate. If 0, returns an empty slice.
/// * `align` - The required alignment in bytes. Must be a power of 2.
///
/// # Returns
///
/// Returns a `Box<[u8]>` containing the allocated zero-initialized buffer
/// aligned to the specified boundary.
///
/// # Panics
///
/// Panics if:
/// - The `align` parameter is not a power of 2
/// - The `size` and `align` combination is invalid
/// - Memory allocation fails (out of memory)
/// - The alignment requirement cannot be satisfied
///
/// # Examples
///
/// ```
/// use safer_ring::buffer::allocation::allocate_with_alignment;
///
/// // Allocate 1KB buffer aligned to 64-byte boundary (for cache line alignment)
/// let buffer = allocate_with_alignment(1024, 64);
/// assert_eq!(buffer.len(), 1024);
/// assert!(buffer.iter().all(|&b| b == 0)); // All zeros
///
/// // Allocate with 16-byte alignment
/// let aligned = allocate_with_alignment(256, 16);
/// assert_eq!(aligned.len(), 256);
///
/// // Empty buffer case
/// let empty = allocate_with_alignment(0, 32);
/// assert_eq!(empty.len(), 0);
/// ```
pub fn allocate_with_alignment(size: usize, align: usize) -> Box<[u8]> {
    if size == 0 {
        return Box::new([]);
    }

    let layout = Layout::from_size_align(size, align).expect("Invalid size/alignment combination");

    unsafe {
        let ptr = alloc_zeroed(layout);
        if ptr.is_null() {
            panic!("Failed to allocate buffer of size {size} with alignment {align}");
        }

        let slice = std::slice::from_raw_parts_mut(ptr, size);
        Box::from_raw(slice)
    }
}
