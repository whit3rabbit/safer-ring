use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use safer_ring::{BufferPool, PinnedBuffer, Ring};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

// Memory tracking allocator
struct TrackingAllocator {
    allocated: AtomicUsize,
    deallocated: AtomicUsize,
    peak: AtomicUsize,
}

impl TrackingAllocator {
    const fn new() -> Self {
        Self {
            allocated: AtomicUsize::new(0),
            deallocated: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    fn reset(&self) {
        self.allocated.store(0, Ordering::SeqCst);
        self.deallocated.store(0, Ordering::SeqCst);
        self.peak.store(0, Ordering::SeqCst);
    }

    fn current_usage(&self) -> usize {
        self.allocated.load(Ordering::SeqCst) - self.deallocated.load(Ordering::SeqCst)
    }

    fn peak_usage(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    fn total_allocated(&self) -> usize {
        self.allocated.load(Ordering::SeqCst)
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let size = layout.size();
            let old_allocated = self.allocated.fetch_add(size, Ordering::SeqCst);
            let current = old_allocated + size - self.deallocated.load(Ordering::SeqCst);

            // Update peak if necessary
            let mut peak = self.peak.load(Ordering::SeqCst);
            while current > peak {
                match self.peak.compare_exchange_weak(
                    peak,
                    current,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(x) => peak = x,
                }
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        self.deallocated.fetch_add(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator::new();

fn setup_test_file() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    let data = vec![0u8; 64 * 1024];
    std::io::Write::write_all(&mut file, &data).unwrap();
    file.flush().unwrap();
    file
}

fn bench_memory_usage_buffer_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage_buffer_creation");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(
            BenchmarkId::new("pinned_buffer_memory", size),
            size,
            |b, &size| {
                b.iter_custom(|iters| {
                    ALLOCATOR.reset();
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        let buffer = PinnedBuffer::new(vec![0u8; size]);
                        black_box(buffer);
                    }

                    let duration = start.elapsed();

                    // Report memory statistics
                    eprintln!(
                        "Buffer size: {}, Peak memory: {} bytes, Total allocated: {} bytes",
                        size,
                        ALLOCATOR.peak_usage(),
                        ALLOCATOR.total_allocated()
                    );

                    duration
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("regular_vec_memory", size),
            size,
            |b, &size| {
                b.iter_custom(|iters| {
                    ALLOCATOR.reset();
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        let buffer = vec![0u8; size];
                        black_box(buffer);
                    }

                    let duration = start.elapsed();

                    eprintln!(
                        "Vec size: {}, Peak memory: {} bytes, Total allocated: {} bytes",
                        size,
                        ALLOCATOR.peak_usage(),
                        ALLOCATOR.total_allocated()
                    );

                    duration
                })
            },
        );
    }

    group.finish();
}

fn bench_memory_usage_buffer_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage_buffer_pool");

    for pool_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("pool_creation", pool_size),
            pool_size,
            |b, &pool_size| {
                b.iter_custom(|iters| {
                    ALLOCATOR.reset();
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        let pool = BufferPool::new(*pool_size, 4096).unwrap();
                        black_box(pool);
                    }

                    let duration = start.elapsed();

                    eprintln!(
                        "Pool size: {}, Peak memory: {} bytes, Total allocated: {} bytes",
                        pool_size,
                        ALLOCATOR.peak_usage(),
                        ALLOCATOR.total_allocated()
                    );

                    duration
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("pool_usage", pool_size),
            pool_size,
            |b, &pool_size| {
                let pool = BufferPool::new(*pool_size, 4096).unwrap();

                b.iter_custom(|iters| {
                    ALLOCATOR.reset();
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        if let Some(buffer) = pool.acquire() {
                            black_box(&buffer);
                            // Buffer automatically returned to pool on drop
                        }
                    }

                    let duration = start.elapsed();

                    eprintln!(
                        "Pool usage - size: {}, Peak memory: {} bytes, Total allocated: {} bytes",
                        pool_size,
                        ALLOCATOR.peak_usage(),
                        ALLOCATOR.total_allocated()
                    );

                    duration
                })
            },
        );
    }

    group.finish();
}

fn bench_memory_usage_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_usage_operations");

    for concurrent_ops in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_operations", concurrent_ops),
            concurrent_ops,
            |b, &concurrent_ops| {
                b.iter_custom(|iters| {
                    ALLOCATOR.reset();
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        rt.block_on(async {
                            let ring = Ring::new(256).unwrap();
                            let file = setup_test_file();
                            let fd = file.as_raw_fd();

                            let mut futures = Vec::new();
                            for i in 0..concurrent_ops {
                                let buffer = PinnedBuffer::new(vec![0u8; 4096]);
                                futures.push(ring.read_at(fd, buffer, (i * 4096) as u64));
                            }

                            let results = futures::future::join_all(futures).await;
                            black_box(results);
                        });
                    }

                    let duration = start.elapsed();

                    eprintln!(
                        "Concurrent ops: {}, Peak memory: {} bytes, Total allocated: {} bytes",
                        concurrent_ops,
                        ALLOCATOR.peak_usage(),
                        ALLOCATOR.total_allocated()
                    );

                    duration
                })
            },
        );
    }

    group.finish();
}

fn bench_memory_fragmentation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_fragmentation");

    group.bench_function("fragmentation_test", |b| {
        b.iter_custom(|iters| {
            ALLOCATOR.reset();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                rt.block_on(async {
                    let ring = Ring::new(256).unwrap();
                    let file = setup_test_file();
                    let fd = file.as_raw_fd();

                    // Create buffers of varying sizes to test fragmentation
                    let sizes = [1024, 2048, 4096, 8192, 16384];
                    let mut futures = Vec::new();

                    for (i, &size) in sizes.iter().enumerate() {
                        let buffer = PinnedBuffer::new(vec![0u8; size]);
                        futures.push(ring.read_at(fd, buffer, (i * 4096) as u64));
                    }

                    let results = futures::future::join_all(futures).await;
                    black_box(results);
                });
            }

            let duration = start.elapsed();

            eprintln!(
                "Fragmentation test - Peak memory: {} bytes, Total allocated: {} bytes",
                ALLOCATOR.peak_usage(),
                ALLOCATOR.total_allocated()
            );

            duration
        })
    });

    group.finish();
}

fn bench_memory_leak_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_leak_detection");

    group.bench_function("leak_detection", |b| {
        b.iter_custom(|iters| {
            let initial_usage = ALLOCATOR.current_usage();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                rt.block_on(async {
                    let ring = Ring::new(256).unwrap();
                    let pool = BufferPool::new(50, 4096).unwrap();
                    let file = setup_test_file();
                    let fd = file.as_raw_fd();

                    // Perform operations that should not leak memory
                    let mut futures = Vec::new();
                    for i in 0..10 {
                        let buffer = pool.acquire().unwrap();
                        futures.push(ring.read_at(fd, buffer, (i * 4096) as u64));
                    }

                    let results = futures::future::join_all(futures).await;
                    black_box(results);

                    // Force cleanup
                    drop(pool);
                    drop(ring);
                });
            }

            let duration = start.elapsed();
            let final_usage = ALLOCATOR.current_usage();

            eprintln!(
                "Leak detection - Initial: {} bytes, Final: {} bytes, Difference: {} bytes",
                initial_usage,
                final_usage,
                final_usage as i64 - initial_usage as i64
            );

            duration
        })
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
        .sample_size(20);
    targets =
        bench_memory_usage_buffer_creation,
        bench_memory_usage_buffer_pool,
        bench_memory_usage_operations,
        bench_memory_fragmentation,
        bench_memory_leak_detection
}

criterion_main!(benches);
