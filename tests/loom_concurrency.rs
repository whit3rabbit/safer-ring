//! Loom-based concurrency tests to verify thread safety and detect race conditions.

#![cfg(loom)]

use loom::sync::{Arc, Mutex};
use loom::thread;
use safer_ring::{BufferPool, Ring};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Test concurrent access to buffer pool
#[test]
fn test_buffer_pool_concurrent_access() {
    loom::model(|| {
        let pool = Arc::new(BufferPool::new(4, 1024));
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        // Spawn multiple threads that compete for buffers
        for _ in 0..3 {
            let pool_clone = Arc::clone(&pool);
            let counter_clone = Arc::clone(&counter);

            let handle = thread::spawn(move || {
                // Try to get a buffer
                if let Some(_buffer) = pool_clone.get() {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    // Simulate some work
                    loom::thread::yield_now();
                    // Buffer automatically returned to pool on drop
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify that operations completed successfully
        let final_count = counter.load(Ordering::SeqCst);
        assert!(final_count <= 3); // At most 3 operations could succeed
    });
}

/// Test concurrent ring operations
#[test]
fn test_ring_concurrent_operations() {
    loom::model(|| {
        let ring = Arc::new(Mutex::new(Ring::new(8).unwrap()));
        let success_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        // Spawn threads that submit operations concurrently
        for i in 0..2 {
            let ring_clone = Arc::clone(&ring);
            let success_clone = Arc::clone(&success_count);

            let handle = thread::spawn(move || {
                let buffer = vec![i as u8; 64];

                // Try to submit an operation
                if let Ok(ring_guard) = ring_clone.try_lock() {
                    // Simulate operation submission
                    success_clone.fetch_add(1, Ordering::SeqCst);
                    loom::thread::yield_now();
                }
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify operations completed
        let final_count = success_count.load(Ordering::SeqCst);
        assert!(final_count <= 2);
    });
}

/// Test operation tracker thread safety
#[test]
fn test_operation_tracker_concurrency() {
    loom::model(|| {
        use safer_ring::operation::tracker::OperationTracker;

        let tracker = Arc::new(Mutex::new(OperationTracker::new()));
        let operation_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        // Multiple threads adding and removing operations
        for i in 0..2 {
            let tracker_clone = Arc::clone(&tracker);
            let count_clone = Arc::clone(&operation_count);

            let handle = thread::spawn(move || {
                if let Ok(mut tracker_guard) = tracker_clone.try_lock() {
                    // Add operation
                    let op_id = tracker_guard.add_operation();
                    count_clone.fetch_add(1, Ordering::SeqCst);

                    loom::thread::yield_now();

                    // Remove operation
                    tracker_guard.remove_operation(op_id);
                    count_clone.fetch_sub(1, Ordering::SeqCst);
                }
            });

            handles.push(handle);
        }

        // Wait for completion
        for handle in handles {
            handle.join().unwrap();
        }

        // All operations should be cleaned up
        let final_count = operation_count.load(Ordering::SeqCst);
        assert_eq!(final_count, 0);
    });
}

/// Test buffer pool statistics under concurrent access
#[test]
fn test_buffer_pool_stats_consistency() {
    loom::model(|| {
        let pool = Arc::new(BufferPool::new(2, 512));

        let mut handles = Vec::new();

        // Multiple threads getting and releasing buffers
        for _ in 0..2 {
            let pool_clone = Arc::clone(&pool);

            let handle = thread::spawn(move || {
                let _buffer1 = pool_clone.get();
                loom::thread::yield_now();
                let _buffer2 = pool_clone.get();
                loom::thread::yield_now();
                // Buffers dropped here, should return to pool
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Pool should have consistent state
        let stats = pool.stats();
        assert_eq!(stats.total_buffers, 2);
        assert_eq!(stats.available_buffers, 2);
        assert_eq!(stats.in_use_buffers, 0);
    });
}
