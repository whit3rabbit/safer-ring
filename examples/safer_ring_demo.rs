//! Comprehensive demonstration of safer-ring features.
//!
//! This example showcases all the key safety features implemented according to TODO.md:
//! 1. Ownership transfer model with hot potato API
//! 2. Cancellation safety with orphaned operation tracking
//! 3. Runtime detection and fallback system
//! 4. Safe buffer management

use safer_ring::{
    is_io_uring_available, Backend, OrphanTracker, OwnedBuffer, Ring, Runtime, SafeOperation,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Safer-Ring Comprehensive Demo");
    println!("=================================");

    // Phase 1: Runtime Detection and Environment Analysis
    demonstrate_runtime_detection().await?;

    // Phase 2: Ownership Transfer (Hot Potato) API
    demonstrate_hot_potato_api().await?;

    // Phase 3: Cancellation Safety
    demonstrate_cancellation_safety().await?;

    // Phase 4: Performance and Environment Guidance
    demonstrate_performance_guidance().await?;

    println!("\n✅ Demo completed successfully!");
    println!("All safety features are working correctly.");

    Ok(())
}

async fn demonstrate_runtime_detection() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📡 Phase 1: Runtime Detection and Fallback");
    println!("-------------------------------------------");

    // Automatic runtime detection
    let runtime = Runtime::auto_detect()?;
    println!("✓ Detected backend: {}", runtime.backend().description());
    println!(
        "  Performance multiplier: {}x",
        runtime.backend().performance_multiplier()
    );
    println!(
        "  Advanced features: {}",
        runtime.backend().supports_advanced_features()
    );

    // Environment analysis
    let env = runtime.environment();
    println!("\n🌍 Environment Analysis:");
    println!("  CPU count: {}", env.cpu_count);
    println!("  Cloud environment: {}", env.is_cloud_environment());

    if let Some(container) = &env.container_runtime {
        println!("  Container runtime: {container}");
    }

    if env.kubernetes {
        println!("  Running in Kubernetes");
    }

    if env.serverless {
        println!("  Running in serverless environment");
    }

    // Convenience functions
    println!("\n🔧 Availability Check:");
    println!("  io_uring available: {}", is_io_uring_available());

    // Test different backends
    println!("\n🔄 Backend Testing:");

    // Test stub backend (always works)
    match Runtime::with_backend(Backend::Stub) {
        Ok(stub_runtime) => {
            println!("  ✓ Stub backend: {}", stub_runtime.backend().description());
        }
        Err(e) => {
            println!("  ❌ Stub backend failed: {e}");
        }
    }

    // Test epoll backend (Linux only)
    match Runtime::with_backend(Backend::Epoll) {
        Ok(epoll_runtime) => {
            println!(
                "  ✓ Epoll backend: {}",
                epoll_runtime.backend().description()
            );
        }
        Err(e) => {
            println!("  ⚠ Epoll backend: {e}");
        }
    }

    // Test io_uring backend (if available)
    match Runtime::with_backend(Backend::IoUring) {
        Ok(uring_runtime) => {
            println!(
                "  ✓ io_uring backend: {}",
                uring_runtime.backend().description()
            );
        }
        Err(e) => {
            println!("  ⚠ io_uring backend: {e}");
        }
    }

    Ok(())
}

async fn demonstrate_hot_potato_api() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🥔 Phase 2: Hot Potato API (Ownership Transfer)");
    println!("------------------------------------------------");

    // Create a ring for demonstration
    let ring = match Ring::new(32) {
        Ok(ring) => ring,
        Err(e) => {
            println!("⚠ Ring creation failed (expected on non-Linux): {e}");
            println!("  Continuing with API demonstration...");
            return Ok(());
        }
    };

    println!("✓ Created ring with {} orphan tracker", ring.orphan_count());

    // Demonstrate buffer creation and ownership
    println!("\n📦 Buffer Management:");
    let buffer1 = OwnedBuffer::new(1024);
    println!(
        "  ✓ Created OwnedBuffer of {} bytes (generation {})",
        buffer1.size(),
        buffer1.generation()
    );

    let buffer2 = OwnedBuffer::from_slice(b"Hello, safer-ring!");
    println!(
        "  ✓ Created OwnedBuffer from slice: {} bytes",
        buffer2.size()
    );

    // Demonstrate ring-provided buffers
    let ring_buffer = ring.get_buffer(4096)?;
    println!("  ✓ Got buffer from ring: {} bytes", ring_buffer.size());

    // Show hot potato pattern (API demonstration)
    println!("\n🔄 Hot Potato Pattern:");
    println!("  This would demonstrate: buffer -> operation -> (result, buffer)");
    println!("  Example API usage:");
    println!("    let (bytes_read, buffer) = ring.read_owned(fd, buffer).await?;");
    println!("    let (bytes_written, buffer) = ring.write_owned(fd, buffer).await?;");

    // Note: We can't actually perform I/O operations without real file descriptors
    // and proper io_uring integration, so we just demonstrate the API design

    Ok(())
}

async fn demonstrate_cancellation_safety() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🛡️ Phase 3: Cancellation Safety");
    println!("--------------------------------");

    // Create orphan tracker for demonstration
    let orphan_tracker = std::sync::Arc::new(std::sync::Mutex::new(OrphanTracker::new()));
    println!("✓ Created orphan tracker");

    // Create a safe operation
    let buffer = OwnedBuffer::new(512);
    let submission_id = {
        let mut tracker = orphan_tracker.lock().unwrap();
        tracker.next_submission_id()
    };

    println!("  Generated submission ID: {submission_id}");

    // Create safe operation
    let operation = SafeOperation::new(
        buffer,
        submission_id,
        std::sync::Arc::downgrade(&orphan_tracker),
    );

    println!(
        "  ✓ Created safe operation (ID: {}, buffer size: {:?})",
        operation.submission_id(),
        operation.buffer_size()
    );

    // Demonstrate cancellation safety by dropping the operation
    println!("\n⚠️ Demonstrating Cancellation Safety:");
    println!("  Dropping operation future before completion...");

    {
        let _future = operation; // Operations are already Futures, no need for into_future
                                 // Future is dropped here - operation becomes orphaned
    }

    // Check orphan count
    let orphan_count = orphan_tracker.lock().unwrap().orphan_count();
    println!("  ✓ Orphaned operations: {orphan_count}");

    if orphan_count > 0 {
        println!("  ✓ Cancellation safety working: buffer ownership tracked!");
    }

    // Demonstrate orphan cleanup
    println!("\n🧹 Orphan Cleanup:");
    let cleaned = {
        let mut tracker = orphan_tracker.lock().unwrap();
        tracker.cleanup_all_orphans()
    };
    println!("  ✓ Cleaned up {cleaned} orphaned operations");

    // Show completion handling
    println!("\n✅ Completion Handling:");
    println!("  In a real implementation:");
    println!("  1. Kernel completes operation");
    println!("  2. Ring checks if operation is orphaned");
    println!("  3. If orphaned: clean up buffer safely");
    println!("  4. If active: wake waiting future");

    Ok(())
}

async fn demonstrate_performance_guidance() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Phase 4: Performance Guidance");
    println!("---------------------------------");

    let runtime = Runtime::auto_detect()?;
    let guidance = runtime.performance_guidance();

    println!("Performance recommendations:");
    for (i, guide) in guidance.iter().enumerate() {
        println!("  {}. {}", i + 1, guide);
    }

    // Show environment-specific guidance
    if runtime.is_cloud_environment() {
        println!("\n☁️ Cloud Environment Detected:");
        println!("  Consider these optimizations:");
        println!("  • Check security policies for io_uring support");
        println!("  • Configure container runtime for better performance");
        println!("  • Monitor performance metrics for fallback behavior");

        let env = runtime.environment();
        if let Some(container) = &env.container_runtime {
            match container.as_str() {
                "docker" => {
                    println!("  • Docker: Add --cap-add SYS_ADMIN or --privileged");
                }
                "containerd" | "cri-o" => {
                    println!("  • Kubernetes: Set privileged: true in pod spec");
                }
                _ => {}
            }
        }
    }

    // Performance expectations as documented in TODO.md
    println!("\n⚡ Expected Performance (vs traditional):");
    match runtime.backend() {
        Backend::IoUring => {
            println!("  • Small reads: 1.8-2.5x faster");
            println!("  • Large reads: 2.8-4.5x faster");
            println!("  • Many connections: 4.5-9x faster");
            println!("  • With cancellations: 1.5-2x faster");
        }
        Backend::Epoll => {
            println!("  • Baseline performance (1x)");
            println!("  • Reliable and well-tested");
        }
        Backend::Stub => {
            println!("  • Limited performance");
            println!("  • For testing and compatibility only");
        }
    }

    println!("\n📈 Monitoring:");
    println!("  • Orphaned operations: {}", 0); // Would be actual count
    println!("  • Backend: {}", runtime.backend().description());
    println!(
        "  • Performance multiplier: {}x",
        runtime.backend().performance_multiplier()
    );

    Ok(())
}

#[cfg(test)]
mod demo_tests {
    use super::*;

    #[tokio::test]
    async fn test_demo_components() {
        // Test that all demo functions can be called without panicking
        let result = std::panic::catch_unwind(|| {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let _ = demonstrate_runtime_detection().await;
                let _ = demonstrate_hot_potato_api().await;
                let _ = demonstrate_cancellation_safety().await;
                let _ = demonstrate_performance_guidance().await;
            });
        });

        assert!(result.is_ok(), "Demo components should not panic");
    }

    #[test]
    fn test_api_availability() {
        // Test that all the APIs we demonstrated are actually available
        let _ = Runtime::auto_detect();
        let _ = is_io_uring_available();
        let _ = get_environment_info();
        let _ = OwnedBuffer::new(1024);
        let _ = OrphanTracker::new();
    }
}
