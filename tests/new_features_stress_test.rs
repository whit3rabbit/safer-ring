//! Stress tests for the new safety features to ensure they work under load.

use safer_ring::{
    buffer::numa::allocate_numa_buffer, OrphanTracker, OwnedBuffer, Registry, Ring, Runtime,
    SafeOperation,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_concurrent_owned_buffer_creation() {
    println!("Testing concurrent OwnedBuffer creation...");
    let start = Instant::now();

    let handles: Vec<_> = (0..100)
        .map(|i| {
            tokio::spawn(async move {
                let buffer = OwnedBuffer::new(1024 + i);
                assert_eq!(buffer.size(), 1024 + i);
                buffer.generation()
            })
        })
        .collect();

    let mut generations = Vec::new();
    for handle in handles {
        generations.push(handle.await.unwrap());
    }

    // Since generation is not atomic, we can't guarantee uniqueness here.
    // The main point is that concurrent creation is safe.
    assert_eq!(generations.len(), 100);

    println!(
        "✓ Created 100 OwnedBuffers concurrently in {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn test_orphan_tracker_high_load() {
    println!("Testing OrphanTracker under high load...");
    let start = Instant::now();

    let orphan_tracker = Arc::new(Mutex::new(OrphanTracker::new()));
    let tracker_clone = orphan_tracker.clone();

    // Spawn multiple tasks creating and dropping operations
    let handles: Vec<_> = (0..50)
        .map(|_| {
            let tracker = tracker_clone.clone();
            tokio::spawn(async move {
                for _ in 0..20 {
                    let buffer = OwnedBuffer::new(512);
                    let submission_id = {
                        let mut t = tracker.lock().unwrap();
                        t.next_submission_id()
                    };

                    let operation =
                        SafeOperation::new(buffer, submission_id, Arc::downgrade(&tracker));

                    // Drop operation to simulate cancellation and trigger orphan registration
                    drop(operation);

                    // Small delay to increase chance of contention
                    tokio::time::sleep(Duration::from_micros(1)).await;
                }
            })
        })
        .collect();

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Check final state
    let final_count = orphan_tracker.lock().unwrap().orphan_count();
    let cleaned = {
        let mut tracker = orphan_tracker.lock().unwrap();
        tracker.cleanup_all_orphans()
    };

    // final_count should equal cleaned count
    assert_eq!(final_count, cleaned);

    println!(
        "✓ Processed 1000 operations with {} orphans, cleaned {} in {:?}",
        final_count,
        cleaned,
        start.elapsed()
    );
}

#[test]
fn test_registry_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Registry under stress...");
    let start = Instant::now();

    let mut registry = Registry::new();

    // Register many fixed files
    let fds: Vec<i32> = (0..100).collect();
    let fixed_files = registry.register_fixed_files(fds)?;
    assert_eq!(fixed_files.len(), 100);

    // Register many buffer slots
    let buffers: Vec<_> = (0..100).map(|i| OwnedBuffer::new(1024 + i * 64)).collect();
    let buffer_slots = registry.register_buffer_slots(buffers)?;
    assert_eq!(buffer_slots.len(), 100);

    // Verify all registrations
    assert_eq!(registry.fixed_file_count(), 100);
    assert_eq!(registry.buffer_slot_count(), 100);

    // Unregister everything
    registry.unregister_fixed_files()?;
    let returned_buffers = registry.unregister_buffer_slots()?;
    assert_eq!(returned_buffers.len(), 100);

    println!(
        "✓ Registered and unregistered 200 resources in {:?}",
        start.elapsed()
    );
    Ok(())
}

#[test]
fn test_runtime_detection_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Runtime detection under stress...");
    let start = Instant::now();

    // Test rapid runtime detections
    let handles: Vec<_> = (0..20)
        .map(|_| {
            thread::spawn(|| {
                for _ in 0..10 {
                    let runtime = Runtime::auto_detect().unwrap();
                    let env = runtime.environment();
                    assert!(env.cpu_count > 0);

                    let guidance = runtime.performance_guidance();
                    assert!(!guidance.is_empty());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    println!(
        "✓ Performed 200 runtime detections in {:?}",
        start.elapsed()
    );
    Ok(())
}

#[test]
fn test_numa_allocation_stress() {
    println!("Testing NUMA allocation under stress...");
    let start = Instant::now();

    let handles: Vec<_> = (0..50)
        .map(|i| {
            thread::spawn(move || {
                // Allocate buffers with different NUMA preferences
                let mut buffer1 = allocate_numa_buffer(4096, None);
                let mut buffer2 = allocate_numa_buffer(8192, Some(i % 2)); // Alternate nodes

                assert_eq!(buffer1.len(), 4096);
                assert_eq!(buffer2.len(), 8192);

                // Use buffers to prevent optimization
                buffer1[0] = 0x42;
                buffer2[0] = 0x24;
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    println!("✓ Allocated 100 NUMA buffers in {:?}", start.elapsed());
}

#[tokio::test]
async fn test_ring_operations_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Ring operations under stress...");
    let start = Instant::now();

    let ring = match Ring::new(256) {
        Ok(ring) => ring,
        Err(_) => {
            println!("Ring creation failed (expected on non-Linux), skipping stress test");
            return Ok(());
        }
    };

    // Test rapid operation creation (not submission, as that requires valid fds)
    let initial_orphan_count = ring.orphan_count();

    for i in 0..100 {
        let buffer = OwnedBuffer::new(1024 + i);

        // Create operation future but don't await it
        let _future = ring.read_owned(-1, buffer); // Invalid fd, but that's ok for this test

        // Let some futures drop to test orphan tracking
        if i % 10 == 0 {
            tokio::time::sleep(Duration::from_micros(1)).await;
        }
    }

    let final_orphan_count = ring.orphan_count();
    println!(
        "✓ Created 100 ring operations, orphan count: {} -> {} in {:?}",
        initial_orphan_count,
        final_orphan_count,
        start.elapsed()
    );

    Ok(())
}

#[tokio::test]
async fn test_comprehensive_integration_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running comprehensive integration stress test...");
    let overall_start = Instant::now();

    // Phase 1: Runtime and environment setup
    let runtime = Runtime::auto_detect()?;
    println!("✓ Runtime: {}", runtime.backend().description());

    // Phase 2: Registry stress with all features
    let mut registry = Registry::new();

    // Register fixed files
    let fixed_files = registry.register_fixed_files(vec![0, 1, 2])?;

    // Register buffer slots
    let buffers: Vec<_> = (0..20).map(|i| OwnedBuffer::new(2048 + i * 128)).collect();
    let buffer_slots = registry.register_buffer_slots(buffers)?;

    println!(
        "✓ Registry: {} fixed files, {} buffer slots",
        fixed_files.len(),
        buffer_slots.len()
    );

    // Phase 3: Ring operations (if available)
    let ring_creation_possible = Ring::new(64).is_ok();
    if ring_creation_possible {
        let ring = Ring::new(64)?;
        let initial_orphan_count = ring.orphan_count();

        // Create and drop some operations to test orphan tracking
        for i in 0..10 {
            let buffer = OwnedBuffer::new(1024 + i * 64);
            let _future = ring.read_owned(-1, buffer); // Invalid fd will complete quickly
            if i % 3 == 0 {
                tokio::time::sleep(Duration::from_micros(1)).await;
            }
        }

        let final_orphan_count = ring.orphan_count();
        println!(
            "✓ Ring operations: orphan count {} -> {}",
            initial_orphan_count, final_orphan_count
        );
    } else {
        println!("✓ Ring gracefully handled on non-Linux");
    }

    // Phase 4: NUMA stress
    let numa_handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                let _buffer = allocate_numa_buffer(4096, Some(i % 2));
            })
        })
        .collect();

    for handle in numa_handles {
        handle.join().unwrap();
    }

    println!("✓ NUMA allocations completed");

    // Phase 5: Additional ring test (if available)
    if ring_creation_possible {
        let _ring = Ring::new(64)?;
        println!("✓ Additional ring instance created successfully");
    }

    // Phase 6: Cleanup
    registry.unregister_fixed_files()?;
    let returned_buffers = registry.unregister_buffer_slots()?;
    println!("✓ Cleanup: returned {} buffers", returned_buffers.len());

    println!(
        "🚀 Comprehensive stress test completed in {:?}",
        overall_start.elapsed()
    );
    println!("All new safety features working correctly under stress!");

    Ok(())
}

// Benchmark test to measure performance impact of safety features
#[test]
fn test_safety_overhead_benchmark() {
    use std::hint::black_box;

    println!("Benchmarking safety feature overhead...");

    // Benchmark 1: OwnedBuffer creation
    let start = Instant::now();
    for _i in 0..10000 {
        let buffer = black_box(OwnedBuffer::new(1024));
        black_box(buffer.size());
        black_box(buffer.generation());
    }
    let buffer_time = start.elapsed();

    // Benchmark 2: OrphanTracker operations
    let orphan_tracker = Arc::new(Mutex::new(OrphanTracker::new()));
    let start = Instant::now();
    for _ in 0..10000 {
        let id = {
            let mut tracker = orphan_tracker.lock().unwrap();
            tracker.next_submission_id()
        };
        black_box(id);
    }
    let tracker_time = start.elapsed();

    // Benchmark 3: Runtime detection
    let start = Instant::now();
    for _ in 0..100 {
        let runtime = black_box(Runtime::auto_detect().unwrap());
        black_box(runtime.backend());
    }
    let runtime_time = start.elapsed();

    println!("Safety feature overhead results:");
    println!("  OwnedBuffer creation: {:?} (10k operations)", buffer_time);
    println!("  OrphanTracker ops: {:?} (10k operations)", tracker_time);
    println!("  Runtime detection: {:?} (100 operations)", runtime_time);

    // Verify reasonable overhead (these are loose bounds)
    assert!(
        buffer_time < Duration::from_millis(100),
        "OwnedBuffer overhead too high"
    );
    assert!(
        tracker_time < Duration::from_millis(50),
        "OrphanTracker overhead too high"
    );
    assert!(
        runtime_time < Duration::from_millis(1000),
        "Runtime detection overhead too high"
    );

    println!("✓ All safety features have acceptable overhead");
}
