//! Memory leak detection tests to ensure proper resource cleanup.

use safer_ring::{BufferPool, PinnedBuffer, Ring};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Test that pinned buffers are properly deallocated
#[tokio::test]
async fn test_pinned_buffer_deallocation() {
    // Create weak references to track deallocation
    let weak_refs = {
        let mut buffers = Vec::new();
        let mut weak_refs = Vec::new();

        // Create many pinned buffers
        for _ in 0..100 {
            let buffer = Arc::new(PinnedBuffer::with_capacity(4096));
            weak_refs.push(Arc::downgrade(&buffer));
            buffers.push(buffer);
        }

        // Use the buffers
        for (_i, buffer) in buffers.iter().enumerate() {
            let slice = buffer.as_slice();
            // Just access the buffer to ensure it's used
            assert_eq!(slice.len(), 4096);
            assert_eq!(slice[0], 0); // Default initialization
        }

        weak_refs
    }; // buffers dropped here

    // Force garbage collection
    tokio::task::yield_now().await;

    // All weak references should be invalid now
    for weak_ref in weak_refs {
        assert!(weak_ref.upgrade().is_none(), "Buffer was not deallocated");
    }
}

/// Test that buffer pool properly manages memory
#[tokio::test]
async fn test_buffer_pool_memory_management() {
    const POOL_SIZE: usize = 50;
    const BUFFER_SIZE: usize = 8192;

    let pool_weak_ref = {
        let pool = Arc::new(BufferPool::new(POOL_SIZE, BUFFER_SIZE));
        let pool_weak = Arc::downgrade(&pool);

        // Get all buffers from pool
        let mut buffers = Vec::new();
        for _ in 0..POOL_SIZE {
            if let Some(buffer) = pool.get() {
                buffers.push(buffer);
            }
        }

        // Verify pool is exhausted
        assert!(pool.get().is_none());

        // Use the buffers
        for (i, buffer) in buffers.iter_mut().enumerate() {
            let mut slice = buffer.as_mut_slice();
            unsafe {
                let slice_mut = std::pin::Pin::get_unchecked_mut(slice.as_mut());
                slice_mut[0] = (i % 256) as u8;
            }
        }

        // Verify buffer contents
        for (i, buffer) in buffers.iter().enumerate() {
            let slice = buffer.as_slice();
            assert_eq!(slice[0], (i % 256) as u8);
        }

        pool_weak
    }; // pool and all buffers dropped here

    // Force cleanup
    tokio::task::yield_now().await;

    // Pool should be deallocated
    assert!(
        pool_weak_ref.upgrade().is_none(),
        "Buffer pool was not deallocated"
    );
}

/// Test ring cleanup with no operations in flight
#[tokio::test]
async fn test_ring_cleanup_no_leaks() {
    let ring_weak_ref = {
        let ring = Arc::new(Ring::new(32).unwrap());
        let ring_weak = Arc::downgrade(&ring);

        // Verify ring is operational
        assert_eq!(ring.operations_in_flight(), 0);

        ring_weak
    }; // ring dropped here

    // Force cleanup
    tokio::task::yield_now().await;

    // Ring should be deallocated
    assert!(
        ring_weak_ref.upgrade().is_none(),
        "Ring was not deallocated"
    );
}

/// Test that operations properly clean up resources
#[tokio::test]
async fn test_operation_resource_cleanup() {
    const NUM_OPERATIONS: usize = 100;

    let buffer_weak_refs = {
        let mut weak_refs = Vec::new();

        for i in 0..NUM_OPERATIONS {
            let buffer = Arc::new(PinnedBuffer::with_capacity(1024));
            weak_refs.push(Arc::downgrade(&buffer));

            // Simulate operation lifecycle
            {
                let slice = buffer.as_slice();
                assert_eq!(slice.len(), 1024);

                // Simulate operation completion
                tokio::task::yield_now().await;
            }

            // Buffer should still be alive here
            assert!(weak_refs[i].upgrade().is_some());
        }

        weak_refs
    }; // All buffers dropped here

    // Force cleanup
    tokio::task::yield_now().await;

    // All buffers should be deallocated
    for (i, weak_ref) in buffer_weak_refs.iter().enumerate() {
        assert!(
            weak_ref.upgrade().is_none(),
            "Buffer {} was not deallocated",
            i
        );
    }
}

/// Test memory cleanup under concurrent access
#[tokio::test]
async fn test_concurrent_memory_cleanup() {
    const CONCURRENT_TASKS: usize = 10;
    const OPERATIONS_PER_TASK: usize = 50;

    let all_weak_refs = {
        let mut all_weak_refs = Vec::new();
        let mut handles = Vec::new();

        for _task_id in 0..CONCURRENT_TASKS {
            let handle = tokio::spawn(async move {
                let mut weak_refs = Vec::new();

                for i in 0..OPERATIONS_PER_TASK {
                    let buffer = Arc::new(PinnedBuffer::with_capacity(2048));
                    weak_refs.push(Arc::downgrade(&buffer));

                    // Use the buffer
                    let slice = buffer.as_slice();
                    assert_eq!(slice.len(), 2048);

                    // Simulate work
                    if i % 10 == 0 {
                        tokio::task::yield_now().await;
                    }
                }

                weak_refs
            });

            handles.push(handle);
        }

        // Collect all weak references
        let results = futures::future::join_all(handles).await;
        for result in results {
            all_weak_refs.extend(result.unwrap());
        }

        all_weak_refs
    }; // All buffers from all tasks dropped here

    // Force cleanup
    tokio::task::yield_now().await;

    // All buffers should be deallocated
    for (i, weak_ref) in all_weak_refs.iter().enumerate() {
        assert!(
            weak_ref.upgrade().is_none(),
            "Concurrent buffer {} was not deallocated",
            i
        );
    }
}

/// Test that pool buffers are properly recycled without leaks
#[tokio::test]
async fn test_buffer_pool_recycling_no_leaks() {
    const POOL_SIZE: usize = 10;
    const CYCLES: usize = 100;

    let pool = Arc::new(BufferPool::new(POOL_SIZE, 1024));

    // Track initial pool state
    let initial_stats = pool.stats();
    assert_eq!(initial_stats.total_buffers, POOL_SIZE);
    assert_eq!(initial_stats.available_buffers, POOL_SIZE);

    // Perform many allocation/deallocation cycles
    for cycle in 0..CYCLES {
        let mut buffers = Vec::new();

        // Get all buffers
        for _ in 0..POOL_SIZE {
            if let Some(buffer) = pool.get() {
                buffers.push(buffer);
            }
        }

        // Pool should be exhausted
        assert!(pool.get().is_none());

        // Use buffers
        for (i, buffer) in buffers.iter_mut().enumerate() {
            let mut slice = buffer.as_mut_slice();
            unsafe {
                let slice_mut = std::pin::Pin::get_unchecked_mut(slice.as_mut());
                slice_mut[0] = ((cycle + i) % 256) as u8;
            }
        }

        // Verify buffer contents
        for (i, buffer) in buffers.iter().enumerate() {
            let slice = buffer.as_slice();
            assert_eq!(slice[0], ((cycle + i) % 256) as u8);
        }

        // Drop all buffers (return to pool)
        drop(buffers);

        // Yield to allow cleanup
        if cycle % 10 == 0 {
            tokio::task::yield_now().await;
        }
    }

    // Pool should be back to initial state
    let final_stats = pool.stats();
    assert_eq!(final_stats.total_buffers, POOL_SIZE);
    assert_eq!(final_stats.available_buffers, POOL_SIZE);
    assert_eq!(final_stats.in_use_buffers, 0);
}

/// Test cleanup with timeout to detect hanging resources
#[tokio::test]
async fn test_cleanup_with_timeout() {
    const TIMEOUT_DURATION: Duration = Duration::from_secs(5);

    let cleanup_test = async {
        let pool = Arc::new(BufferPool::new(20, 4096));
        let mut buffers = Vec::new();

        // Allocate buffers
        for _ in 0..20 {
            if let Some(buffer) = pool.get() {
                buffers.push(buffer);
            }
        }

        // Use buffers
        for (i, buffer) in buffers.iter_mut().enumerate() {
            let mut slice = buffer.as_mut_slice();
            unsafe {
                let slice_mut = std::pin::Pin::get_unchecked_mut(slice.as_mut());
                let len = slice_mut.len();
                slice_mut[i % len] = (i % 256) as u8;
            }
        }

        // Drop buffers
        drop(buffers);

        // Force cleanup
        tokio::task::yield_now().await;

        // Verify pool state
        let stats = pool.stats();
        assert_eq!(stats.available_buffers, 20);
        assert_eq!(stats.in_use_buffers, 0);
    };

    // Test should complete within timeout
    timeout(TIMEOUT_DURATION, cleanup_test)
        .await
        .expect("Cleanup test timed out - possible resource leak or deadlock");
}
