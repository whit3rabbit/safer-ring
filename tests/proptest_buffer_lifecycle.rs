//! Property-based tests for buffer lifecycle management using proptest.

use proptest::prelude::*;
use safer_ring::{BufferPool, PinnedBuffer};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Test that buffer pools maintain correct allocation/deallocation invariants
#[test]
fn test_buffer_pool_invariants() {
    let rt = Runtime::new().unwrap();

    proptest!(|(
        pool_size in 1usize..=100,
        buffer_size in 64usize..=4096,
        _operations in prop::collection::vec(0usize..10, 1..50)
    )| {
        rt.block_on(async {
            let pool = Arc::new(BufferPool::new(pool_size, buffer_size));
            let mut buffers = Vec::new();

            // Test allocation up to pool capacity
            for _ in 0..pool_size {
                if let Some(buffer) = pool.get() {
                    prop_assert_eq!(buffer.len(), buffer_size);
                    buffers.push(buffer);
                }
            }

            // Pool should be exhausted
            prop_assert!(pool.get().is_none());

            // Drop half the buffers
            let half = buffers.len() / 2;
            buffers.truncate(half);

            // Should be able to allocate again
            for _ in 0..(pool_size - half) {
                if let Some(buffer) = pool.get() {
                    prop_assert_eq!(buffer.len(), buffer_size);
                    buffers.push(buffer);
                }
            }

            // Pool should be exhausted again
            prop_assert!(pool.get().is_none());

            Ok(())
        })?
    });
}

/// Test that pinned buffers maintain their memory layout
#[test]
fn test_pinned_buffer_stability() {
    proptest!(|(
        buffer_size in 64usize..=4096,
        data in prop::collection::vec(0u8..=255, 64..4096)
    )| {
        let mut buffer = PinnedBuffer::with_capacity(buffer_size);
        let mut slice = buffer.as_mut_slice();

        // Store the original pointer
        let original_ptr = slice.as_ptr();

        // Fill with test data
        let copy_len = std::cmp::min(data.len(), slice.len());
        unsafe {
            let slice_mut = std::pin::Pin::get_unchecked_mut(slice.as_mut());
            slice_mut[..copy_len].copy_from_slice(&data[..copy_len]);
        }

        // Pointer should remain stable
        prop_assert_eq!(slice.as_ptr(), original_ptr);

        // Data should be preserved
        prop_assert_eq!(&slice[..copy_len], &data[..copy_len]);
    });
}

/// Test buffer lifecycle with concurrent operations
#[test]
fn test_concurrent_buffer_operations() {
    let rt = Runtime::new().unwrap();

    proptest!(|(
        num_operations in 1usize..=20,
        buffer_sizes in prop::collection::vec(64usize..=1024, 1..20)
    )| {
        rt.block_on(async {
            let pool = Arc::new(BufferPool::new(num_operations * 2, 1024));
            let mut handles = Vec::new();

            for &_size in &buffer_sizes[..std::cmp::min(buffer_sizes.len(), num_operations)] {
                let pool_clone = Arc::clone(&pool);
                let handle = tokio::spawn(async move {
                    // Get buffer from pool
                    let buffer = pool_clone.get();

                    if let Some(mut buf) = buffer {
                        // Simulate some work with the buffer
                        let mut slice = buf.as_mut_slice();
                        unsafe {
                            let slice_mut = std::pin::Pin::get_unchecked_mut(slice.as_mut());
                            for (i, byte) in slice_mut.iter_mut().enumerate() {
                                *byte = (i % 256) as u8;
                            }
                        }

                        // Verify data integrity
                        for (i, &byte) in slice.iter().enumerate() {
                            assert_eq!(byte, (i % 256) as u8);
                        }

                        true
                    } else {
                        false
                    }
                });
                handles.push(handle);
            }

            // Wait for all operations to complete
            let results: Vec<_> = futures::future::join_all(handles).await;

            // At least some operations should have succeeded
            let successful = results.iter().filter(|r| *r.as_ref().unwrap_or(&false)).count();
            prop_assert!(successful > 0);

            Ok(())
        })?;
    });
}

/// Test that buffer generation tracking prevents use-after-free
#[test]
fn test_buffer_generation_tracking() {
    proptest!(|(
        operations in prop::collection::vec(1usize..=10, 1..20)
    )| {
        let mut buffer = PinnedBuffer::with_capacity(1024);
        let initial_generation = buffer.generation();

        // Simulate multiple operation cycles
        for &op_count in &operations {
            for _ in 0..op_count {
                // Simulate operation start
                buffer.mark_in_use();

                // Generation should increment
                prop_assert!(buffer.generation() > initial_generation);

                // Simulate operation completion
                buffer.mark_available();
            }
        }

        // Buffer should be available after all operations
        prop_assert!(buffer.is_available());
    });
}
