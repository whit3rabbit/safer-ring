//! Memory allocation utilities for pinned buffers.

use std::alloc::{alloc_zeroed, Layout};

/// Allocates a zero-initialized buffer with optimal alignment for DMA operations.
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
            panic!("Failed to allocate aligned buffer of size {}", size);
        }

        // Convert raw pointer to boxed slice
        let slice = std::slice::from_raw_parts_mut(ptr, size);
        Box::from_raw(slice)
    }
}

/// Allocates a buffer with specific alignment requirements.
pub fn allocate_with_alignment(size: usize, align: usize) -> Box<[u8]> {
    if size == 0 {
        return Box::new([]);
    }

    let layout = Layout::from_size_align(size, align).expect("Invalid size/alignment combination");

    unsafe {
        let ptr = alloc_zeroed(layout);
        if ptr.is_null() {
            panic!(
                "Failed to allocate buffer of size {} with alignment {}",
                size, align
            );
        }

        let slice = std::slice::from_raw_parts_mut(ptr, size);
        Box::from_raw(slice)
    }
}
