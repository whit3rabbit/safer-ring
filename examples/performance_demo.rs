//! Performance optimization demonstration for Safer-Ring.
//!
//! This example shows how to use various performance optimization features
//! including buffer pools, NUMA-aware allocation, and performance monitoring.

use safer_ring::perf::{MemoryTracker, PerfCounter, PerfRegistry, PerfTimer};
use safer_ring::{BufferPool, PinnedBuffer, Ring};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Safer-Ring Performance Optimization Demo");
    println!("=======================================");

    // Create test file
    let test_file = create_test_file(1024 * 1024)?; // 1MB test file
    let fd = test_file.as_raw_fd();

    // Initialize performance tracking
    let mut perf_registry = PerfRegistry::new();
    let read_counter = perf_registry.counter("read_operations");
    let write_counter = perf_registry.counter("write_operations");
    let memory_tracker = perf_registry.memory_tracker();

    println!("\n1. Basic Buffer Operations");
    demo_basic_buffers(&read_counter, &memory_tracker).await?;

    println!("\n2. Buffer Pool Optimization");
    demo_buffer_pool(&read_counter, fd).await?;

    println!("\n3. NUMA-Aware Allocation");
    demo_numa_allocation(&memory_tracker).await?;

    println!("\n4. Batch Operations");
    demo_batch_operations(&read_counter, fd).await?;

    println!("\n5. Performance Monitoring");
    demo_performance_monitoring(&mut perf_registry).await?;

    println!("\n6. Memory Optimization");
    demo_memory_optimization(&memory_tracker).await?;

    // Final performance summary
    print_performance_summary(&perf_registry);

    Ok(())
}

fn create_test_file(size: usize) -> Result<NamedTempFile, Box<dyn std::error::Error>> {
    let mut file = NamedTempFile::new()?;
    let data = vec![0u8; size];
    std::io::Write::write_all(&mut file, &data)?;
    file.flush()?;
    Ok(file)
}

async fn demo_basic_buffers(
    counter: &PerfCounter,
    memory_tracker: &MemoryTracker,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Testing different buffer allocation strategies...");

    let initial_memory = memory_tracker.current_usage();

    // Standard allocation
    let start = Instant::now();
    let _buffer1 = PinnedBuffer::with_capacity(4096);
    let standard_time = start.elapsed();

    // Aligned allocation
    let start = Instant::now();
    let _buffer2 = PinnedBuffer::with_capacity_aligned(4096);
    let aligned_time = start.elapsed();

    // NUMA-aware allocation (if available)
    let start = Instant::now();
    let _buffer3 = PinnedBuffer::with_capacity_numa(4096, Some(0));
    let numa_time = start.elapsed();

    memory_tracker.record_alloc(4096 * 3);

    println!("    Standard allocation: {:?}", standard_time);
    println!("    Aligned allocation:  {:?}", aligned_time);
    println!("    NUMA allocation:     {:?}", numa_time);
    println!(
        "    Memory used: {} bytes",
        memory_tracker.current_usage() - initial_memory
    );

    Ok(())
}

async fn demo_buffer_pool(
    counter: &PerfCounter,
    fd: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Comparing direct allocation vs buffer pool...");

    let mut ring = Ring::new(256)?;
    let pool = BufferPool::new(100, 4096);

    // Test direct allocation
    let start = Instant::now();
    for i in 0..50 {
        let _timer = PerfTimer::new(counter);
        let mut buffer = PinnedBuffer::with_capacity(4096);
        let read_future = ring.read_at(fd, buffer.as_mut_slice(), (i * 4096) as u64)?;
        let _ = timeout(Duration::from_secs(1), read_future).await;
    }
    let direct_time = start.elapsed();

    // Test buffer pool
    let start = Instant::now();
    for i in 0..50 {
        let _timer = PerfTimer::new(counter);
        if let Some(mut buffer) = pool.get() {
            let read_future = ring.read_at(fd, buffer.as_mut_slice(), (i * 4096) as u64)?;
            let _ = timeout(Duration::from_secs(1), read_future).await;
        }
    }
    let pool_time = start.elapsed();

    println!(
        "    Direct allocation: {:?} ({:.2} ops/sec)",
        direct_time,
        50.0 / direct_time.as_secs_f64()
    );
    println!(
        "    Buffer pool:       {:?} ({:.2} ops/sec)",
        pool_time,
        50.0 / pool_time.as_secs_f64()
    );

    let speedup = direct_time.as_secs_f64() / pool_time.as_secs_f64();
    println!("    Speedup: {:.2}x", speedup);

    Ok(())
}

async fn demo_numa_allocation(
    memory_tracker: &MemoryTracker,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Testing NUMA-aware buffer allocation...");

    let initial_memory = memory_tracker.current_usage();

    // Allocate buffers on different NUMA nodes
    let mut buffers = Vec::new();

    for node in 0..2 {
        let start = Instant::now();
        for _ in 0..10 {
            let buffer = PinnedBuffer::with_capacity_numa(4096, Some(node));
            buffers.push(buffer);
        }
        let allocation_time = start.elapsed();

        println!(
            "    NUMA node {}: {:?} for 10 buffers",
            node, allocation_time
        );
    }

    memory_tracker.record_alloc(4096 * 20);

    println!(
        "    Total memory allocated: {} bytes",
        memory_tracker.current_usage() - initial_memory
    );

    Ok(())
}

async fn demo_batch_operations(
    counter: &PerfCounter,
    fd: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Comparing individual vs batch operations...");

    let mut ring = Ring::new(256)?;
    let pool = BufferPool::new(100, 4096);

    // Individual operations
    let start = Instant::now();
    for i in 0..20 {
        let _timer = PerfTimer::new(counter);
        if let Some(mut buffer) = pool.get() {
            let read_future = ring.read_at(fd, buffer.as_mut_slice(), (i * 4096) as u64)?;
            let _ = timeout(Duration::from_secs(1), read_future).await;
        }
    }
    let individual_time = start.elapsed();

    // Batch operations
    let start = Instant::now();
    {
        let _timer = PerfTimer::new(counter);
        let mut futures = Vec::new();

        for i in 0..20 {
            if let Some(mut buffer) = pool.get() {
                let read_future = ring.read_at(fd, buffer.as_mut_slice(), (i * 4096) as u64)?;
                futures.push(read_future);
            }
        }

        let _ = timeout(Duration::from_secs(5), futures::future::join_all(futures)).await;
    }
    let batch_time = start.elapsed();

    println!("    Individual operations: {:?}", individual_time);
    println!("    Batch operations:      {:?}", batch_time);

    let speedup = individual_time.as_secs_f64() / batch_time.as_secs_f64();
    println!("    Speedup: {:.2}x", speedup);

    Ok(())
}

async fn demo_performance_monitoring(
    registry: &mut PerfRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Demonstrating performance monitoring...");

    let operation_counter = registry.counter("demo_operations");
    let memory_tracker = registry.memory_tracker();

    // Simulate some operations with timing
    for i in 0..100 {
        let _timer = PerfTimer::new(&operation_counter);

        // Simulate work
        tokio::time::sleep(Duration::from_micros(10 + i % 50)).await;

        // Simulate memory allocation
        if i % 10 == 0 {
            memory_tracker.record_alloc(1024);
        }
    }

    let stats = operation_counter.stats();
    println!("    Operations completed: {}", stats.count);
    println!("    Average time: {:?}", stats.avg_time);
    println!("    Min time: {:?}", stats.min_time);
    println!("    Max time: {:?}", stats.max_time);
    println!("    Ops/sec: {:.2}", stats.ops_per_sec());

    let memory_stats = memory_tracker.stats();
    println!("    Memory allocations: {}", memory_stats.total_allocs);
    println!("    Current usage: {} bytes", memory_stats.current);
    println!("    Peak usage: {} bytes", memory_stats.peak);

    Ok(())
}

async fn demo_memory_optimization(
    memory_tracker: &MemoryTracker,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Testing memory optimization strategies...");

    let initial_usage = memory_tracker.current_usage();

    // Test different buffer sizes and their memory efficiency
    let sizes = [1024, 4096, 16384, 65536];

    for &size in &sizes {
        let start_memory = memory_tracker.current_usage();

        // Allocate 10 buffers of this size
        let mut buffers = Vec::new();
        for _ in 0..10 {
            buffers.push(PinnedBuffer::with_capacity_aligned(size));
        }

        memory_tracker.record_alloc(size * 10);
        let end_memory = memory_tracker.current_usage();

        println!(
            "    {} byte buffers: {} bytes allocated",
            size,
            end_memory - start_memory
        );
    }

    // Test buffer pool efficiency
    let pool_start = memory_tracker.current_usage();
    let pool = BufferPool::new(50, 4096)?;
    memory_tracker.record_alloc(4096 * 50);
    let pool_end = memory_tracker.current_usage();

    println!("    Buffer pool (50x4KB): {} bytes", pool_end - pool_start);

    // Test pool utilization
    let mut acquired_buffers = Vec::new();
    for _ in 0..25 {
        if let Some(buffer) = pool.acquire() {
            acquired_buffers.push(buffer);
        }
    }

    let pool_stats = pool.stats();
    println!(
        "    Pool utilization: {:.1}%",
        pool_stats.utilization * 100.0
    );
    println!("    Available buffers: {}", pool_stats.available);

    Ok(())
}

fn print_performance_summary(registry: &PerfRegistry) {
    println!("\nPerformance Summary");
    println!("==================");

    let all_stats = registry.all_stats();
    for (name, stats) in all_stats {
        println!("{}:", name);
        println!("  Operations: {}", stats.count);
        println!("  Avg time: {:?}", stats.avg_time);
        println!("  Throughput: {:.2} ops/sec", stats.ops_per_sec());
    }

    let memory_stats = registry.memory_tracker().stats();
    println!("\nMemory Usage:");
    println!("  Current: {} bytes", memory_stats.current);
    println!("  Peak: {} bytes", memory_stats.peak);
    println!("  Total allocations: {}", memory_stats.total_allocs);
    println!("  Total deallocations: {}", memory_stats.total_deallocs);

    println!("\nOptimization Recommendations:");

    if memory_stats.peak > 10 * 1024 * 1024 {
        println!("  - Consider using smaller buffer pools to reduce memory usage");
    }

    if memory_stats.total_allocs > memory_stats.total_deallocs * 2 {
        println!("  - High allocation rate detected, consider buffer reuse");
    }

    for (name, stats) in registry.all_stats() {
        if stats.ops_per_sec() < 1000.0 && stats.count > 10 {
            println!("  - {} has low throughput, consider optimization", name);
        }
    }

    println!("\nFor detailed profiling, run: ./scripts/run_benchmarks.sh");
}
