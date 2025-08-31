//! Advanced Features Demo
//!
//! This example demonstrates the advanced features of safer-ring including:
//! - Feature detection and graceful degradation
//! - Advanced configuration options
//! - Comprehensive logging and metrics
//! - Buffer selection and provided buffers
//! - Multi-shot operations
//! - Performance optimization techniques

use safer_ring::{
    BufferGroup, BufferPool, FeatureDetector, LogLevel, MultiShotConfig, PinnedBuffer, Ring,
    SaferRingConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Safer-Ring Advanced Features Demo ===\n");

    // 1. Feature Detection
    demonstrate_feature_detection().await?;

    // 2. Configuration Options
    demonstrate_configuration_options().await?;

    // 3. Logging and Metrics
    demonstrate_logging_and_metrics().await?;

    // 4. Advanced Buffer Management
    demonstrate_advanced_buffers().await?;

    // 5. Performance Optimization
    demonstrate_performance_optimization().await?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

/// Demonstrate kernel feature detection and graceful degradation.
async fn demonstrate_feature_detection() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Feature Detection and Graceful Degradation");
    println!("----------------------------------------------");

    // Detect available features
    let detector = FeatureDetector::new()?;
    println!(
        "Kernel version: {}.{}.{}",
        detector.kernel_version.major, detector.kernel_version.minor, detector.kernel_version.patch
    );

    println!("Available features:");
    for feature in detector.available_features() {
        println!("  ✓ {feature}");
    }

    // Create optimal configuration based on detected features
    let optimal_config = detector.create_optimal_config();
    println!("\nOptimal configuration created:");
    println!("  Buffer selection: {}", optimal_config.buffer_selection);
    println!("  Multi-shot operations: {}", optimal_config.multi_shot);
    println!("  Provided buffers: {}", optimal_config.provided_buffers);
    println!("  Fast poll: {}", optimal_config.fast_poll);

    // Try to create a ring with advanced features
    match Ring::with_advanced_config(optimal_config) {
        Ok(_ring) => println!("  ✓ Ring created with advanced features"),
        Err(e) => println!("  ⚠ Ring creation failed: {e}"),
    }

    println!();
    Ok(())
}

/// Demonstrate different configuration options for various use cases.
async fn demonstrate_configuration_options() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. Configuration Options for Different Use Cases");
    println!("-----------------------------------------------");

    // Low-latency configuration
    println!("Low-latency configuration:");
    let low_latency_config = SaferRingConfig::low_latency();
    println!("  SQ entries: {}", low_latency_config.ring.sq_entries);
    println!("  SQ polling: {}", low_latency_config.ring.sq_poll);
    println!(
        "  Batch submission: {}",
        low_latency_config.performance.batch_submission
    );
    println!(
        "  Buffer pool size: {}",
        low_latency_config.buffer.pool_size
    );

    // High-throughput configuration
    println!("\nHigh-throughput configuration:");
    let high_throughput_config = SaferRingConfig::high_throughput();
    println!("  SQ entries: {}", high_throughput_config.ring.sq_entries);
    println!(
        "  Max batch size: {}",
        high_throughput_config.performance.max_batch_size
    );
    println!(
        "  Buffer pool size: {}",
        high_throughput_config.buffer.pool_size
    );
    println!(
        "  Auto retry: {}",
        high_throughput_config.error_handling.auto_retry
    );

    // Development configuration
    println!("\nDevelopment configuration:");
    let dev_config = SaferRingConfig::development();
    println!("  Logging enabled: {}", dev_config.logging.enabled);
    println!("  Log level: {:?}", dev_config.logging.level);
    println!("  Metrics enabled: {}", dev_config.logging.metrics);
    println!(
        "  Debug assertions: {}",
        dev_config.logging.debug_assertions
    );

    // Auto-detect configuration
    println!("\nAuto-detected configuration:");
    match SaferRingConfig::auto_detect() {
        Ok(auto_config) => {
            println!("  SQ entries: {}", auto_config.ring.sq_entries);
            println!("  NUMA aware: {}", auto_config.buffer.numa_aware);
            println!("  Buffer pool size: {}", auto_config.buffer.pool_size);
        }
        Err(e) => println!("  ⚠ Auto-detection failed: {e}"),
    }

    println!();
    Ok(())
}

/// Demonstrate comprehensive logging and performance metrics.
async fn demonstrate_logging_and_metrics() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. Logging and Performance Metrics");
    println!("----------------------------------");

    // Create a ring with logging enabled
    let mut config = SaferRingConfig::development();
    config.logging.enabled = true;
    config.logging.level = LogLevel::Debug;
    config.logging.metrics = true;

    match Ring::with_config(config) {
        Ok(_ring) => {
            println!("Created ring with comprehensive logging enabled");
            println!("  ✓ Logging level: Debug");
            println!("  ✓ Metrics collection: Enabled");
            println!("  ✓ Log file: safer-ring.log");
        }
        Err(_) => {
            println!("  ⚠ Could not create ring with logging (io_uring not available)");
        }
    }

    // Demonstrate logging API
    println!("\nLogging API demonstration:");
    safer_ring::logging::log(LogLevel::Info, "demo", "This is an info message");
    safer_ring::logging::log(LogLevel::Debug, "demo", "This is a debug message");
    safer_ring::logging::log(LogLevel::Warn, "demo", "This is a warning message");

    // Get and display performance metrics
    let logger = safer_ring::logging::init_logger();
    if let Ok(logger) = logger.lock() {
        if let Ok(metrics) = logger.get_metrics() {
            println!("\nPerformance Metrics:");
            println!("  Total operations: {}", metrics.get_total_operations());
            println!(
                "  Collection duration: {:?}",
                metrics.get_collection_duration()
            );

            for operation_type in metrics.get_operation_types() {
                println!(
                    "  {}: {} operations",
                    operation_type,
                    metrics.get_count(&operation_type)
                );
                if let Some(avg) = metrics.get_average_duration(&operation_type) {
                    println!("    Average duration: {avg:?}");
                }
            }
        }
    }

    println!();
    Ok(())
}

/// Demonstrate advanced buffer management features.
async fn demonstrate_advanced_buffers() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. Advanced Buffer Management");
    println!("-----------------------------");

    // Buffer pooling
    println!("Buffer pooling:");
    let pool = BufferPool::new(10, 4096);
    println!("  Created buffer pool with 10 buffers of 4KB each");

    let buffer1 = pool.try_get().ok();
    let buffer2 = pool.try_get().ok();
    println!("  Acquired 2 buffers from pool");
    println!("  Available buffers: {}", pool.available().unwrap_or(0));

    drop(buffer1);
    drop(buffer2);
    println!("  Returned buffers to pool");
    println!("  Available buffers: {}", pool.available().unwrap_or(0));

    // Buffer groups for kernel buffer selection
    println!("\nBuffer groups (for kernel buffer selection):");
    let mut buffer_group = BufferGroup::new(1, 16, 4096)?;
    println!("  Created buffer group with ID 1, 16 buffers of 4KB each");

    if let Some(buffer_id) = buffer_group.get_buffer() {
        println!("  Allocated buffer ID: {buffer_id}");
        println!("  Available buffers: {}", buffer_group.available_count());

        buffer_group.return_buffer(buffer_id);
        println!("  Returned buffer to group");
        println!("  Available buffers: {}", buffer_group.available_count());
    }

    // NUMA-aware buffer allocation
    println!("\nNUMA-aware buffer allocation:");
    let numa_buffer = PinnedBuffer::with_capacity_numa(4096, Some(0));
    println!(
        "  Created NUMA-aware buffer on node 0: {} bytes",
        numa_buffer.len()
    );

    println!();
    Ok(())
}

/// Demonstrate performance optimization techniques.
async fn demonstrate_performance_optimization() -> Result<(), Box<dyn std::error::Error>> {
    println!("5. Performance Optimization Techniques");
    println!("--------------------------------------");

    // Batch operations
    println!("Batch operations:");
    match Ring::new(64) {
        Ok(_ring) => {
            println!("  ✓ Ring created for batch operations");
            println!("  ✓ Batch submission reduces syscall overhead");
            println!("  ✓ Dependency tracking ensures correct ordering");
        }
        Err(_) => {
            println!("  ⚠ Could not create ring (io_uring not available)");
        }
    }

    // Multi-shot configuration
    println!("\nMulti-shot operations:");
    let multi_shot_config = MultiShotConfig {
        max_completions: Some(10),
        continue_on_error: true,
        buffer_group_id: Some(1),
    };
    println!(
        "  Multi-shot config: max_completions = {:?}",
        multi_shot_config.max_completions
    );
    println!(
        "  Continue on error: {}",
        multi_shot_config.continue_on_error
    );
    println!("  Buffer group ID: {:?}", multi_shot_config.buffer_group_id);

    // Zero-copy operations
    println!("\nZero-copy optimizations:");
    println!("  ✓ Pinned buffers prevent memory copies");
    println!("  ✓ Registered buffers reduce kernel validation overhead");
    println!("  ✓ Vectored I/O enables scatter-gather operations");
    println!("  ✓ Buffer selection allows kernel-managed buffer pools");

    // Performance monitoring
    println!("\nPerformance monitoring:");
    println!("  ✓ Operation timing and metrics collection");
    println!("  ✓ Memory usage tracking");
    println!("  ✓ Throughput and latency analysis");
    println!("  ✓ Resource utilization monitoring");

    println!();
    Ok(())
}
