use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use safer_ring::{BufferPool, PinnedBuffer, Registry, Ring};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use tempfile::NamedTempFile;

/// Creates a temporary file with 64KB of test data for benchmarking I/O operations.
///
/// # Returns
/// A [`NamedTempFile`] containing 64KB of zero-filled data, positioned at the start.
fn setup_test_file() -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temporary file");
    // Use 64KB as it's a common buffer size that exercises realistic I/O patterns
    let data = vec![0u8; 64 * 1024];
    std::io::Write::write_all(&mut file, &data).expect("Failed to write test data");
    file.flush().expect("Failed to flush test data");
    file
}

/// Benchmarks buffer creation overhead comparing [`PinnedBuffer`] vs regular [`Vec`].
///
/// Tests common buffer sizes from 1KB to 64KB to measure allocation and pinning costs.
/// The comparison helps quantify the overhead of memory pinning for io_uring operations.
fn bench_buffer_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_creation");

    // Test power-of-2 sizes that are common in I/O workloads
    const BUFFER_SIZES: &[usize] = &[1024, 4096, 16384, 65536];

    for &size in BUFFER_SIZES {
        group.bench_with_input(
            BenchmarkId::new("pinned_buffer", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    // PinnedBuffer allocation includes heap allocation + pinning overhead
                    let buffer = PinnedBuffer::new(vec![0u8; size]);
                    black_box(buffer)
                })
            },
        );

        group.bench_with_input(BenchmarkId::new("regular_vec", size), &size, |b, &size| {
            b.iter(|| {
                // Baseline: just heap allocation without pinning
                let buffer = vec![0u8; size];
                black_box(buffer)
            })
        });
    }

    group.finish();
}

/// Benchmarks buffer pool performance vs direct allocation.
///
/// Measures the cost of acquiring/releasing pooled buffers compared to
/// allocating new [`PinnedBuffer`] instances. Pool reuse should be faster
/// for frequent allocations but has upfront memory cost.
fn bench_buffer_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool");

    // Pre-allocate pool with 100 4KB buffers - common size for network I/O
    let pool = BufferPool::new(100, 4096);

    group.bench_function("pool_acquire_release", |b| {
        b.iter(|| {
            // Pool acquisition should be O(1) from free list
            let buffer = pool.get().expect("Pool exhausted");
            black_box(&buffer);
            // Buffer automatically returned to pool on drop via RAII
        })
    });

    group.bench_function("direct_allocation", |b| {
        b.iter(|| {
            // Direct allocation: heap alloc + pinning on every iteration
            let buffer = PinnedBuffer::new(vec![0u8; 4096]);
            black_box(buffer);
        })
    });

    group.finish();
}

/// Benchmarks Ring creation overhead.
///
/// Measures the cost of initializing io_uring instances with different queue sizes.
fn bench_ring_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_creation");

    const QUEUE_SIZES: &[u32] = &[32, 128, 256, 512, 1024];

    for &size in QUEUE_SIZES {
        group.bench_with_input(BenchmarkId::new("ring_new", size), &size, |b, &size| {
            b.iter(|| {
                // Measure cost of io_uring initialization
                let ring = Ring::new(size);
                let _ = black_box(ring);
            })
        });
    }

    group.finish();
}

/// Benchmarks Registry operations.
///
/// Measures the cost of file descriptor and buffer registration operations.
fn bench_registry_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_operations");

    let file = setup_test_file();
    let fd = file.as_raw_fd();

    group.bench_function("register_fd", |b| {
        b.iter(|| {
            let mut registry = Registry::new();
            // FD registration: kernel syscall to register file descriptor
            let registered_fd = registry.register_fd(fd);
            let _ = black_box(registered_fd);
        })
    });

    group.bench_function("register_buffer", |b| {
        b.iter(|| {
            let mut registry = Registry::new();
            let buffer = vec![0u8; 4096].into_boxed_slice();
            let pinned = std::pin::Pin::new(buffer);
            // Buffer registration: kernel syscall + memory pinning
            let registered_buffer = registry.register_buffer(pinned);
            let _ = black_box(registered_buffer);
        })
    });

    group.finish();
}

criterion_group! {
    name = benches;
    // Enable flamegraph profiling to identify performance bottlenecks
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets =
        bench_buffer_creation,
        bench_buffer_pool,
        bench_ring_creation,
        bench_registry_operations
}

criterion_main!(benches);
