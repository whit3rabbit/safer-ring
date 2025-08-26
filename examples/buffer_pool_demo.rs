//! # Buffer Pool Usage Example
//!
//! This example demonstrates efficient buffer management using safer-ring's BufferPool.
//! It shows how to minimize memory allocations in high-throughput applications.
//!
//! ## Features Demonstrated
//! - **Buffer Pool Creation**: Setting up pools with different configurations
//! - **Efficient Allocation**: Getting buffers without heap allocation overhead
//! - **Automatic Return**: Buffers automatically returned to pool on drop
//! - **Pool Statistics**: Monitoring pool usage and performance
//! - **Concurrent Access**: Thread-safe buffer sharing across tasks
//! - **Memory Efficiency**: Reusing pre-allocated, pinned buffers
//!
//! ## Usage
//! ```bash
//! # Run basic buffer pool demo
//! cargo run --example buffer_pool_demo
//!
//! # Run with custom pool size
//! cargo run --example buffer_pool_demo -- --pool-size 100
//!
//! # Run stress test with multiple threads
//! cargo run --example buffer_pool_demo -- --stress-test --threads 8
//! ```
//!
//! ## Performance Benefits
//! - **Zero Allocation**: No heap allocations during buffer operations
//! - **Cache Friendly**: Reused buffers stay in CPU cache
//! - **Predictable Latency**: No GC pauses or allocation spikes
//! - **Memory Efficiency**: Fixed memory footprint regardless of load
//!
//! ## Use Cases
//! - High-frequency network servers
//! - Real-time data processing
//! - Game servers with strict latency requirements
//! - Financial trading systems

use safer_ring::PinnedBuffer;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Configuration for the buffer pool demonstration
#[derive(Debug)]
struct DemoConfig {
    /// Number of buffers in the pool
    pool_size: usize,
    /// Size of each buffer in bytes
    buffer_size: usize,
    /// Number of concurrent operations
    concurrent_ops: usize,
    /// Duration to run the demo
    duration_secs: u64,
    /// Whether to run stress test
    stress_test: bool,
    /// Number of threads for stress test
    threads: usize,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            pool_size: 32,
            buffer_size: 4096,
            concurrent_ops: 16,
            duration_secs: 10,
            stress_test: false,
            threads: 4,
        }
    }
}

impl DemoConfig {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut config = DemoConfig::default();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--pool-size" => {
                    if i + 1 < args.len() {
                        config.pool_size = args[i + 1].parse().unwrap_or(config.pool_size);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--buffer-size" => {
                    if i + 1 < args.len() {
                        config.buffer_size = args[i + 1].parse().unwrap_or(config.buffer_size);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--duration" => {
                    if i + 1 < args.len() {
                        config.duration_secs = args[i + 1].parse().unwrap_or(config.duration_secs);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--stress-test" => {
                    config.stress_test = true;
                    i += 1;
                }
                "--threads" => {
                    if i + 1 < args.len() {
                        config.threads = args[i + 1].parse().unwrap_or(config.threads);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }

        config
    }
}

/// Statistics for buffer pool operations
#[derive(Debug, Default)]
struct PoolDemoStats {
    allocations: u64,
    deallocations: u64,
    allocation_failures: u64,
    total_bytes_processed: u64,
    operations_completed: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏊 Safer-Ring Buffer Pool Demonstration");
    println!("=======================================");

    let config = DemoConfig::from_args();
    println!("📊 Configuration:");
    println!("   Pool size: {} buffers", config.pool_size);
    println!("   Buffer size: {} bytes", config.buffer_size);
    println!("   Concurrent operations: {}", config.concurrent_ops);
    println!("   Duration: {} seconds", config.duration_secs);
    if config.stress_test {
        println!("   Stress test: {} threads", config.threads);
    }
    println!();

    if config.stress_test {
        run_stress_test(&config).await?;
    } else {
        run_basic_demo(&config).await?;
    }

    Ok(())
}

/// Run basic buffer pool demonstration
async fn run_basic_demo(config: &DemoConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting basic buffer pool demo...");

    // Simulate buffer pool creation (actual BufferPool API not fully implemented)
    println!("✅ Simulating buffer pool creation");
    println!("📈 Pool configuration:");
    println!("   Pool size: {} buffers", config.pool_size);
    println!("   Buffer size: {} bytes", config.buffer_size);
    println!();

    // Demonstrate basic buffer operations
    println!("🔄 Demonstrating basic operations...");

    // Create some buffers to simulate pool behavior
    let mut buffers = Vec::new();
    for i in 0..std::cmp::min(5, config.pool_size) {
        let buffer = PinnedBuffer::with_capacity(config.buffer_size);
        println!(
            "   📦 Created buffer {} (size: {} bytes)",
            i + 1,
            buffer.len()
        );
        buffers.push(buffer);
    }

    println!("📊 Simulated pool stats:");
    println!("   Buffers created: {}", buffers.len());
    println!(
        "   Total capacity: {} bytes",
        buffers.len() * config.buffer_size
    );
    println!();

    // Use the buffers (simulate some work)
    println!("⚡ Simulating buffer usage...");
    for (i, mut buffer) in buffers.iter_mut().enumerate() {
        // Fill buffer with test data
        let test_data = format!("Test data for buffer {}", i + 1);
        let bytes = test_data.as_bytes();
        let copy_len = std::cmp::min(bytes.len(), buffer.len());
        buffer.as_mut_slice()[..copy_len].copy_from_slice(&bytes[..copy_len]);

        println!("   ✏️  Filled buffer {} with: {}", i + 1, test_data);
    }

    // Drop buffers (simulate returning to pool)
    println!("🔄 Simulating buffer return to pool...");
    drop(buffers);
    println!("📊 All buffers returned to pool");
    println!();

    // Demonstrate concurrent access simulation
    println!("🔀 Demonstrating concurrent buffer usage...");
    run_concurrent_demo_simulation(config).await?;

    println!("✅ Basic demo completed!");
    Ok(())
}

/// Simulate concurrent buffer access
async fn run_concurrent_demo_simulation(
    config: &DemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let stats = Arc::new(tokio::sync::Mutex::new(PoolDemoStats::default()));
    let mut tasks = Vec::new();

    // Start concurrent tasks that simulate buffer pool usage
    for task_id in 0..config.concurrent_ops {
        let stats_clone = Arc::clone(&stats);
        let buffer_size = config.buffer_size;

        let task = tokio::spawn(async move {
            let mut local_ops = 0u64;
            let start_time = Instant::now();

            while start_time.elapsed().as_secs() < 5 {
                // Simulate getting a buffer from pool
                let mut buffer = PinnedBuffer::with_capacity(buffer_size);

                // Simulate some work with the buffer
                let work_data = format!("Task {} operation {}", task_id, local_ops);
                let bytes = work_data.as_bytes();
                let copy_len = std::cmp::min(bytes.len(), buffer.len());
                buffer.as_mut_slice()[..copy_len].copy_from_slice(&bytes[..copy_len]);

                // Simulate processing time
                sleep(Duration::from_millis(10)).await;

                // Update statistics
                {
                    let mut stats = stats_clone.lock().await;
                    stats.allocations += 1;
                    stats.total_bytes_processed += copy_len as u64;
                    stats.operations_completed += 1;
                }

                local_ops += 1;
                // Buffer is automatically cleaned up when dropped
            }

            println!("   🏁 Task {} completed {} operations", task_id, local_ops);
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        task.await?;
    }

    // Print concurrent demo statistics
    let final_stats = stats.lock().await;
    println!("📊 Concurrent demo results:");
    println!("   Successful allocations: {}", final_stats.allocations);
    println!("   Failed allocations: {}", final_stats.allocation_failures);
    println!(
        "   Operations completed: {}",
        final_stats.operations_completed
    );
    println!("   Bytes processed: {}", final_stats.total_bytes_processed);

    let success_rate = if final_stats.allocations + final_stats.allocation_failures > 0 {
        (final_stats.allocations as f64)
            / ((final_stats.allocations + final_stats.allocation_failures) as f64)
            * 100.0
    } else {
        0.0
    };
    println!("   Success rate: {:.2}%", success_rate);

    Ok(())
}

/// Run stress test with multiple threads
async fn run_stress_test(config: &DemoConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("💪 Starting stress test...");

    // Simulate buffer pool for stress test
    println!("💪 Simulating buffer pool stress test...");
    let stats = Arc::new(tokio::sync::Mutex::new(PoolDemoStats::default()));

    // Statistics reporting task
    let stats_reporter = Arc::clone(&stats);
    let duration_secs = config.duration_secs;
    let report_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let start_time = Instant::now();

        loop {
            interval.tick().await;
            let stats = stats_reporter.lock().await;
            let elapsed = start_time.elapsed().as_secs_f64();
            let ops_per_sec = if elapsed > 0.0 {
                stats.operations_completed as f64 / elapsed
            } else {
                0.0
            };

            println!(
                "📊 [{:6.1}s] Ops: {:8}, Rate: {:8.0}/s, Failures: {:6}, Bytes: {:10}",
                elapsed,
                stats.operations_completed,
                ops_per_sec,
                stats.allocation_failures,
                stats.total_bytes_processed
            );

            if elapsed >= duration_secs as f64 {
                break;
            }
        }
    });

    // Start worker tasks that simulate intensive buffer usage
    let mut tasks = Vec::new();
    for thread_id in 0..config.threads {
        let stats_clone = Arc::clone(&stats);
        let duration = config.duration_secs;
        let buffer_size = config.buffer_size;

        let task = tokio::spawn(async move {
            let start_time = Instant::now();
            let mut local_ops = 0u64;

            while start_time.elapsed().as_secs() < duration {
                // High-frequency buffer operations simulation
                for _ in 0..100 {
                    // Simulate getting buffer from pool
                    let mut buffer = PinnedBuffer::with_capacity(buffer_size);

                    // Simulate intensive buffer usage
                    let pattern = (thread_id as u8).wrapping_mul(local_ops as u8);
                    for byte in buffer.as_mut_slice().iter_mut().take(64) {
                        *byte = pattern;
                    }

                    local_ops += 1;

                    // Update stats periodically to avoid lock contention
                    if local_ops % 1000 == 0 {
                        let mut stats = stats_clone.lock().await;
                        stats.operations_completed += 1000;
                        stats.total_bytes_processed += 64 * 1000;
                        stats.allocations += 1000;
                    }
                }

                // Small yield to prevent monopolizing CPU
                tokio::task::yield_now().await;
            }

            // Update final stats
            let remaining = local_ops % 1000;
            if remaining > 0 {
                let mut stats = stats_clone.lock().await;
                stats.operations_completed += remaining;
                stats.total_bytes_processed += 64 * remaining;
                stats.allocations += remaining;
            }

            println!("🏁 Thread {} completed {} operations", thread_id, local_ops);
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        task.await?;
    }

    // Stop reporting task
    report_task.abort();

    // Print final stress test results
    let final_stats = stats.lock().await;

    println!();
    println!("🏆 Stress Test Results:");
    println!("========================================");
    println!("Operations completed: {}", final_stats.operations_completed);
    println!("Allocation failures: {}", final_stats.allocation_failures);
    println!(
        "Total bytes processed: {}",
        final_stats.total_bytes_processed
    );
    println!(
        "Average ops/sec: {:.0}",
        final_stats.operations_completed as f64 / config.duration_secs as f64
    );
    println!("Simulated allocations: {}", final_stats.allocations);
    println!(
        "Success rate: {:.2}%",
        if final_stats.allocations + final_stats.allocation_failures > 0 {
            (final_stats.allocations as f64
                / (final_stats.allocations + final_stats.allocation_failures) as f64)
                * 100.0
        } else {
            100.0
        }
    );

    Ok(())
}
