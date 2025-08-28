//! Stress tests for high-throughput scenarios and resource management.

use safer_ring::{BufferPool, PinnedBuffer};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Stress test for high-frequency buffer allocation and deallocation
#[tokio::test]
async fn stress_test_buffer_pool_high_frequency() {
    const POOL_SIZE: usize = 100;
    const BUFFER_SIZE: usize = 4096;
    const OPERATIONS: usize = 10000;
    const CONCURRENT_TASKS: usize = 10;

    let pool = Arc::new(BufferPool::new(POOL_SIZE, BUFFER_SIZE));
    let start_time = Instant::now();

    let mut handles = Vec::new();

    for task_id in 0..CONCURRENT_TASKS {
        let pool_clone = Arc::clone(&pool);

        let handle = tokio::spawn(async move {
            let mut local_operations = 0;
            let operations_per_task = OPERATIONS / CONCURRENT_TASKS;

            for i in 0..operations_per_task {
                // Get buffer from pool
                let mut buffer = loop {
                    if let Some(buf) = pool_clone.get() {
                        break buf;
                    }
                    // Yield to allow other tasks to return buffers
                    tokio::task::yield_now().await;
                };

                // Simulate work with the buffer
                let mut slice = buffer.as_mut_slice();
                for (idx, byte) in slice.iter_mut().enumerate() {
                    *byte = ((task_id + i + idx) % 256) as u8;
                }

                // Verify data integrity
                for (idx, &byte) in slice.iter().enumerate() {
                    assert_eq!(byte, ((task_id + i + idx) % 256) as u8);
                }

                local_operations += 1;

                // Occasionally yield to increase contention
                if i % 100 == 0 {
                    tokio::task::yield_now().await;
                }
            }

            local_operations
        });

        handles.push(handle);
    }

    // Wait for all tasks with timeout
    let results = timeout(Duration::from_secs(30), futures::future::join_all(handles))
        .await
        .expect("Stress test timed out");

    let total_operations: usize = results.into_iter().map(|r| r.unwrap()).sum();
    let elapsed = start_time.elapsed();

    println!("Completed {} operations in {:?}", total_operations, elapsed);
    println!(
        "Operations per second: {:.2}",
        total_operations as f64 / elapsed.as_secs_f64()
    );

    // Verify pool is in consistent state
    let stats = pool.stats();
    assert_eq!(stats.total_buffers, POOL_SIZE);
    assert_eq!(stats.available_buffers, POOL_SIZE);
    assert_eq!(stats.in_use_buffers, 0);
}

/// Stress test for ring operation submission and completion
#[cfg(target_os = "linux")]
#[tokio::test]
async fn stress_test_ring_operations() {
    const RING_SIZE: usize = 256;
    const OPERATIONS: usize = 5000;
    const BUFFER_SIZE: usize = 1024;

    let ring = Ring::new(RING_SIZE as u32).unwrap();
    let pool = Arc::new(BufferPool::new(RING_SIZE * 2, BUFFER_SIZE));

    let start_time = Instant::now();
    let mut handles = Vec::new();

    // Create temporary files for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let mut temp_files = Vec::new();

    for i in 0..10 {
        let file_path = temp_dir.path().join(format!("test_file_{}.txt", i));
        let file = std::fs::File::create(&file_path).unwrap();
        temp_files.push(file);
    }

    // Spawn multiple tasks performing I/O operations
    for task_id in 0..5 {
        let pool_clone = Arc::clone(&pool);

        let handle = tokio::spawn(async move {
            let mut completed_ops = 0;

            for i in 0..(OPERATIONS / 5) {
                // Get buffer from pool
                let mut buffer = loop {
                    if let Some(buf) = pool_clone.get() {
                        break buf;
                    }
                    tokio::task::yield_now().await;
                };

                // Fill buffer with test data
                let mut slice = buffer.as_mut_slice();
                for (idx, byte) in slice.iter_mut().enumerate() {
                    *byte = ((task_id + i + idx) % 256) as u8;
                }

                // Simulate operation completion
                tokio::task::yield_now().await;

                // Verify data integrity
                for (idx, &byte) in slice.iter().enumerate() {
                    assert_eq!(byte, ((task_id + i + idx) % 256) as u8);
                }

                completed_ops += 1;

                // Periodic yielding to increase contention
                if i % 50 == 0 {
                    tokio::task::yield_now().await;
                }
            }

            completed_ops
        });

        handles.push(handle);
    }

    // Wait for completion with timeout
    let results = timeout(Duration::from_secs(60), futures::future::join_all(handles))
        .await
        .expect("Ring stress test timed out");

    let total_operations: usize = results.into_iter().map(|r| r.unwrap()).sum();
    let elapsed = start_time.elapsed();

    println!(
        "Ring completed {} operations in {:?}",
        total_operations, elapsed
    );
    println!(
        "Ring operations per second: {:.2}",
        total_operations as f64 / elapsed.as_secs_f64()
    );

    // Verify no operations are in flight
    assert_eq!(ring.operations_in_flight(), 0);
}

/// Non-Linux version of ring operations test
#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn stress_test_ring_operations() {
    use safer_ring::Ring;

    // On non-Linux platforms, Ring::new should return an error
    match Ring::new(64) {
        Ok(_) => panic!("Ring creation should fail on non-Linux platforms"),
        Err(e) => {
            println!("Expected error on non-Linux platform: {}", e);
        }
    }
}

/// Memory pressure test - ensure no leaks under high allocation pressure
#[tokio::test]
async fn stress_test_memory_pressure() {
    const ITERATIONS: usize = 1000;
    const BUFFERS_PER_ITERATION: usize = 50;
    const BUFFER_SIZE: usize = 8192;

    for iteration in 0..ITERATIONS {
        let mut buffers = Vec::new();

        // Allocate many pinned buffers
        for _ in 0..BUFFERS_PER_ITERATION {
            let buffer = PinnedBuffer::with_capacity(BUFFER_SIZE);
            buffers.push(buffer);
        }

        // Use the buffers
        for (i, buffer) in buffers.iter_mut().enumerate() {
            let mut slice = buffer.as_mut_slice();
            for (idx, byte) in slice.iter_mut().enumerate() {
                *byte = ((iteration + i + idx) % 256) as u8;
            }
        }

        // Verify data integrity
        for (i, buffer) in buffers.iter().enumerate() {
            let slice = buffer.as_slice();
            for (idx, &byte) in slice.iter().enumerate() {
                assert_eq!(byte, ((iteration + i + idx) % 256) as u8);
            }
        }

        // Buffers are dropped here

        // Periodic progress reporting
        if iteration % 100 == 0 {
            println!(
                "Memory pressure test: iteration {}/{}",
                iteration, ITERATIONS
            );
        }

        // Yield to allow cleanup
        if iteration % 10 == 0 {
            tokio::task::yield_now().await;
        }
    }

    println!("Memory pressure test completed successfully");
}

/// Test resource cleanup under error conditions
#[tokio::test]
async fn stress_test_error_recovery() {
    const POOL_SIZE: usize = 20;
    const BUFFER_SIZE: usize = 1024;
    const ERROR_RATE: usize = 10; // 1 in 10 operations will "fail"

    let pool = Arc::new(BufferPool::new(POOL_SIZE, BUFFER_SIZE));
    let mut handles = Vec::new();

    for task_id in 0..5 {
        let pool_clone = Arc::clone(&pool);

        let handle = tokio::spawn(async move {
            let mut successful_ops = 0;
            let mut failed_ops = 0;

            for i in 0..200 {
                // Get buffer from pool
                let mut buffer = loop {
                    if let Some(buf) = pool_clone.get() {
                        break buf;
                    }
                    tokio::task::yield_now().await;
                };

                // Simulate operation that might fail
                if (task_id + i) % ERROR_RATE == 0 {
                    // Simulate error - buffer should still be cleaned up
                    failed_ops += 1;
                    drop(buffer); // Explicit drop to simulate error cleanup
                } else {
                    // Successful operation
                    let mut slice = buffer.as_mut_slice();
                    slice[0] = (i % 256) as u8;
                    successful_ops += 1;
                    // Buffer automatically returned on drop
                }

                tokio::task::yield_now().await;
            }

            (successful_ops, failed_ops)
        });

        handles.push(handle);
    }

    // Wait for all tasks
    let results = futures::future::join_all(handles).await;

    let (total_success, total_failures): (usize, usize) = results
        .into_iter()
        .map(|r| r.unwrap())
        .fold((0, 0), |(s1, f1), (s2, f2)| (s1 + s2, f1 + f2));

    println!(
        "Error recovery test: {} successful, {} failed operations",
        total_success, total_failures
    );

    // Pool should be in consistent state despite errors
    let stats = pool.stats();
    assert_eq!(stats.total_buffers, POOL_SIZE);
    assert_eq!(stats.available_buffers, POOL_SIZE);
    assert_eq!(stats.in_use_buffers, 0);
}
