# Safer-Ring Performance Guide

This guide covers performance optimization strategies, benchmarking, and profiling for the Safer-Ring library.

## Table of Contents

1. [Performance Overview](#performance-overview)
2. [Benchmarking](#benchmarking)
3. [Memory Optimization](#memory-optimization)
4. [NUMA Optimization](#numa-optimization)
5. [Hot Path Optimization](#hot-path-optimization)
6. [Profiling](#profiling)
7. [Best Practices](#best-practices)

## Performance Overview

Safer-Ring is designed to provide zero-cost abstractions over io_uring while maintaining memory safety. The library achieves this through:

- **Compile-time safety checks**: All safety invariants are enforced at compile time
- **Zero-copy operations**: Direct buffer passing without intermediate copies
- **Efficient buffer pooling**: Pre-allocated, reusable buffers
- **NUMA-aware allocation**: Memory allocation optimized for multi-socket systems
- **Batch operations**: Efficient submission of multiple operations

### Performance Targets

- **Latency**: < 1μs overhead compared to raw io_uring for hot paths
- **Throughput**: > 95% of raw io_uring performance for bulk operations
- **Memory**: Zero additional allocations in steady state with buffer pools
- **CPU**: < 5% CPU overhead for safety guarantees

## Benchmarking

### Running Benchmarks

Use the provided benchmark script to run comprehensive performance tests:

```bash
# Run all benchmarks
./scripts/run_benchmarks.sh

# Run specific benchmark categories
./scripts/run_benchmarks.sh --micro    # Individual operations
./scripts/run_benchmarks.sh --macro    # End-to-end scenarios
./scripts/run_benchmarks.sh --memory   # Memory usage profiling
./scripts/run_benchmarks.sh --numa     # NUMA optimizations
```

### Benchmark Categories

#### Micro-benchmarks

Test individual operations in isolation:

- Buffer creation and pinning
- Operation building and submission
- Completion processing
- Buffer pool operations
- Registry operations

#### Macro-benchmarks

Test complete application scenarios:

- File copy operations (vs raw io_uring and std::fs)
- Echo server simulation
- Concurrent I/O operations
- Batch operation processing

#### Memory Benchmarks

Profile memory usage patterns:

- Allocation tracking
- Peak memory usage
- Fragmentation analysis
- Leak detection

#### NUMA Benchmarks

Test NUMA-aware optimizations:

- Local vs remote memory access
- NUMA-aware buffer pools
- Cross-node I/O performance

### Interpreting Results

Benchmark results are available in multiple formats:

- **HTML Reports**: `target/criterion/report/index.html`
- **Flamegraphs**: `target/flamegraphs/`
- **Raw Logs**: `target/benchmark_reports/`

Key metrics to monitor:

- **Throughput**: Operations per second
- **Latency**: Average, P50, P95, P99 operation times
- **Memory**: Peak usage, allocation rate
- **CPU**: Utilization and efficiency

## Memory Optimization

### Buffer Pool Configuration

Optimize buffer pools for your workload:

```rust
use safer_ring::BufferPool;

// For high-throughput, low-latency workloads
let pool = BufferPool::new(1000, 4096)?; // Many small buffers

// For bulk transfer workloads
let pool = BufferPool::new(100, 64 * 1024)?; // Fewer large buffers

// Custom initialization for specific patterns
let pool = BufferPool::with_factory(500, || {
    PinnedBuffer::with_capacity_aligned(8192) // Page-aligned buffers
})?;
```

### Memory Layout Optimization

Use aligned allocations for better DMA performance:

```rust
use safer_ring::PinnedBuffer;

// Standard allocation
let buffer = PinnedBuffer::with_capacity(4096);

// Page-aligned allocation (better for DMA)
let buffer = PinnedBuffer::with_capacity_aligned(4096);

// NUMA-aware allocation
let buffer = PinnedBuffer::with_capacity_numa(4096, Some(0));
```

### Memory Tracking

Monitor memory usage in production:

```rust
use safer_ring::perf::{MemoryTracker, PerfRegistry};

let mut registry = PerfRegistry::new();
let memory_tracker = registry.memory_tracker();

// Track allocations
memory_tracker.record_alloc(4096);
memory_tracker.record_dealloc(4096);

// Get statistics
let stats = memory_tracker.stats();
println!("Peak memory usage: {} bytes", stats.peak);
```

## NUMA Optimization

### NUMA-Aware Buffer Allocation

For multi-socket systems, allocate buffers on the same NUMA node as the processing thread:

```rust
use safer_ring::PinnedBuffer;

// Allocate on specific NUMA node
let buffer = PinnedBuffer::with_capacity_numa(4096, Some(0));

// Let the system choose optimal node
let buffer = PinnedBuffer::with_capacity_numa(4096, None);
```

### NUMA-Aware Buffer Pools

Create separate buffer pools per NUMA node:

```rust
use safer_ring::BufferPool;
use std::collections::HashMap;

struct NumaBufferPools {
    pools: HashMap<usize, BufferPool>,
}

impl NumaBufferPools {
    fn new(buffers_per_node: usize, buffer_size: usize) -> Self {
        let mut pools = HashMap::new();
        
        // Create pool for each NUMA node
        for node in 0..numa_node_count() {
            let pool = BufferPool::with_factory(buffers_per_node, || {
                PinnedBuffer::with_capacity_numa(buffer_size, Some(node))
            });
            pools.insert(node, pool);
        }
        
        Self { pools }
    }
    
    fn get_local_buffer(&self) -> Option<PooledBuffer> {
        let current_node = get_current_numa_node();
        self.pools.get(&current_node)?.acquire()
    }
}
```

### Thread Affinity

Bind threads to specific NUMA nodes for consistent performance:

```rust
use std::thread;

// Bind thread to NUMA node 0
thread::spawn(|| {
    bind_to_numa_node(0).unwrap();
    
    // All allocations and I/O operations will prefer node 0
    let ring = Ring::new(256).unwrap();
    let buffer = PinnedBuffer::with_capacity_numa(4096, Some(0));
    
    // ... perform I/O operations
});
```

## Hot Path Optimization

### Zero-Cost Abstractions

Safer-Ring is designed for zero-cost abstractions. Key optimizations:

#### Inlined Operations

Critical path functions are marked `#[inline]`:

```rust
#[inline]
pub fn as_mut_slice(&mut self) -> Pin<&mut [u8]> {
    self.inner.as_mut()
}

#[inline]
pub fn submit(&mut self) -> Result<SubmittedOperation> {
    // Hot path - no allocations, minimal overhead
}
```

#### Compile-Time Checks

Safety checks happen at compile time, not runtime:

```rust
// This won't compile - lifetime error caught at compile time
fn invalid_operation() {
    let buffer = vec![0u8; 1024];
    let ring = Ring::new(32).unwrap();
    ring.read(0, &buffer, 0); // Error: buffer doesn't live long enough
} // buffer dropped here while operation pending
```

#### Monomorphization

Generic types are specialized for optimal performance:

```rust
// Each buffer size gets its own optimized implementation
let buffer_1k = PinnedBuffer::<[u8; 1024]>::zeroed();
let buffer_4k = PinnedBuffer::<[u8; 4096]>::zeroed();
```

### Fast Path Optimizations

Use fast path methods for high-throughput scenarios:

```rust
use safer_ring::BufferPool;

let pool = BufferPool::new(1000, 4096)?;

// Standard path
let buffer = pool.try_get()?;

// Fast path with reduced locking
let buffer = pool.try_get_fast()?;
```

### Batch Operations

Submit multiple operations efficiently:

```rust
use safer_ring::{Ring, Batch};

let ring = Ring::new(256)?;
let mut batch = Batch::new();

// Add multiple operations
for i in 0..32 {
    let buffer = pool.acquire().unwrap();
    batch.add_read(fd, buffer, i * 4096)?;
}

// Submit all at once
let results = ring.submit_batch(batch).await?;
```

## Profiling

### Performance Counters

Use built-in performance counters for detailed profiling:

```rust
use safer_ring::perf::{PerfCounter, PerfTimer};

let counter = PerfCounter::new();

// Manual timing
let start = std::time::Instant::now();
perform_operation();
counter.record(start.elapsed());

// Automatic timing with RAII
{
    let _timer = PerfTimer::new(&counter);
    perform_operation();
} // Automatically recorded on drop

// Get statistics
let stats = counter.stats();
println!("Average: {:?}, P99: {:?}", stats.avg_time, stats.max_time);
```

### Macro-Based Profiling

Use convenience macros for easy profiling:

```rust
use safer_ring::{time_operation, time_async_operation};

let counter = PerfCounter::new();

// Time synchronous operations
let result = time_operation!(&counter, {
    expensive_computation()
});

// Time asynchronous operations
let result = time_async_operation!(&counter, async {
    ring.read(fd, buffer, 0).await
});
```

### Flamegraph Generation

Generate flamegraphs for detailed performance analysis:

```bash
# Install perf and flamegraph tools
sudo apt install linux-perf
cargo install flamegraph

# Run benchmarks with flamegraph generation
cargo bench --bench micro_benchmarks
```

### Memory Profiling

Track memory usage patterns:

```rust
use safer_ring::perf::MemoryTracker;

let tracker = MemoryTracker::new();

// Simulate allocations
tracker.record_alloc(4096);
tracker.record_alloc(8192);
tracker.record_dealloc(4096);

let stats = tracker.stats();
println!("Current: {} bytes, Peak: {} bytes", stats.current, stats.peak);
```

## Best Practices

### Buffer Management

1. **Use Buffer Pools**: Pre-allocate buffers to avoid allocation overhead
2. **Size Appropriately**: Match buffer sizes to your I/O patterns
3. **Align Buffers**: Use page-aligned buffers for better DMA performance
4. **NUMA Awareness**: Allocate buffers on the same NUMA node as processing

### Operation Patterns

1. **Batch Operations**: Submit multiple operations together when possible
2. **Avoid Blocking**: Use async/await instead of blocking operations
3. **Reuse Resources**: Keep rings and pools alive for the application lifetime
4. **Monitor Metrics**: Use performance counters to identify bottlenecks

### System Configuration

1. **CPU Governor**: Set to 'performance' mode for consistent results
2. **IRQ Affinity**: Bind interrupts to specific CPU cores
3. **Kernel Parameters**: Tune io_uring parameters for your workload
4. **Memory**: Ensure sufficient memory for buffer pools

### Development Workflow

1. **Profile Early**: Use benchmarks to establish baseline performance
2. **Measure Changes**: Benchmark before and after optimizations
3. **Focus on Hot Paths**: Optimize the most frequently used code paths
4. **Validate Safety**: Ensure optimizations don't compromise safety

### Production Monitoring

1. **Continuous Profiling**: Monitor performance in production
2. **Alert on Regressions**: Set up alerts for performance degradation
3. **Capacity Planning**: Use metrics for capacity planning
4. **Regular Benchmarking**: Run benchmarks on production hardware

## Troubleshooting Performance Issues

### Common Issues

1. **High Latency**
   - Check for buffer pool exhaustion
   - Verify NUMA node affinity
   - Monitor CPU frequency scaling

2. **Low Throughput**
   - Increase batch sizes
   - Check for lock contention
   - Verify io_uring queue sizes

3. **Memory Usage**
   - Monitor buffer pool utilization
   - Check for memory leaks
   - Verify buffer sizes match workload

4. **CPU Usage**
   - Profile hot paths with flamegraphs
   - Check for unnecessary allocations
   - Verify compiler optimizations

### Debugging Tools

1. **perf**: System-wide performance profiling
2. **flamegraph**: Visual performance analysis
3. **valgrind**: Memory error detection
4. **strace**: System call tracing
5. **htop**: Real-time system monitoring

### Performance Regression Testing

Set up automated performance regression testing:

```bash
#!/bin/bash
# Run in CI/CD pipeline

# Run benchmarks and save results
cargo bench --bench micro_benchmarks > current_results.txt

# Compare with baseline
if ! compare_performance baseline_results.txt current_results.txt; then
    echo "Performance regression detected!"
    exit 1
fi
```

This comprehensive performance guide should help you optimize Safer-Ring for your specific use case while maintaining the safety guarantees that make the library valuable.