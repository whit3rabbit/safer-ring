use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use safer_ring::PinnedBuffer;
use std::time::Duration;

fn bench_buffer_allocation_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_allocation");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(BenchmarkId::new("standard", size), size, |b, &size| {
            b.iter(|| {
                let buffer = PinnedBuffer::with_capacity(size);
                black_box(buffer)
            })
        });

        group.bench_with_input(BenchmarkId::new("aligned", size), size, |b, &size| {
            b.iter(|| {
                let buffer = PinnedBuffer::with_capacity_aligned(size);
                black_box(buffer)
            })
        });

        #[cfg(target_os = "linux")]
        group.bench_with_input(BenchmarkId::new("numa_aware", size), size, |b, &size| {
            b.iter(|| {
                let buffer = PinnedBuffer::with_capacity_numa(size, Some(0));
                black_box(buffer)
            })
        });
    }

    group.finish();
}

fn bench_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_operations");

    let mut buffer = PinnedBuffer::with_capacity(4096);

    group.bench_function("generation_increment", |b| {
        b.iter(|| {
            buffer.mark_in_use();
            buffer.mark_available();
            black_box(buffer.generation());
        })
    });

    group.bench_function("slice_access", |b| {
        b.iter(|| {
            let slice = buffer.as_slice();
            black_box(slice.len());
        })
    });

    group.bench_function("pointer_access", |b| {
        b.iter(|| {
            let ptr = buffer.as_ptr();
            black_box(ptr);
        })
    });

    group.finish();
}

fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    // Test different allocation patterns
    group.bench_function("sequential_allocation", |b| {
        b.iter(|| {
            let mut buffers = Vec::new();
            for i in 0..100 {
                buffers.push(PinnedBuffer::with_capacity(1024 + i * 10));
            }
            black_box(buffers);
        })
    });

    group.bench_function("uniform_allocation", |b| {
        b.iter(|| {
            let mut buffers = Vec::new();
            for _ in 0..100 {
                buffers.push(PinnedBuffer::with_capacity(4096));
            }
            black_box(buffers);
        })
    });

    group.bench_function("large_buffer_allocation", |b| {
        b.iter(|| {
            let buffer = PinnedBuffer::with_capacity(1024 * 1024); // 1MB
            black_box(buffer);
        })
    });

    group.finish();
}

fn bench_performance_counters(c: &mut Criterion) {
    use safer_ring::perf::{MemoryTracker, PerfCounter, PerfTimer};

    let mut group = c.benchmark_group("performance_counters");

    let counter = PerfCounter::new();

    group.bench_function("counter_record", |b| {
        b.iter(|| {
            counter.record(Duration::from_nanos(1000));
            black_box(counter.stats());
        })
    });

    group.bench_function("timer_overhead", |b| {
        b.iter(|| {
            let _timer = PerfTimer::new(&counter);
            // Simulate minimal work
            black_box(42);
        })
    });

    let memory_tracker = MemoryTracker::new();

    group.bench_function("memory_tracking", |b| {
        b.iter(|| {
            memory_tracker.record_alloc(4096);
            memory_tracker.record_dealloc(4096);
            black_box(memory_tracker.current_usage());
        })
    });

    group.finish();
}

fn bench_zero_cost_abstractions(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_cost_abstractions");

    // Compare direct operations vs abstracted operations
    group.bench_function("direct_vec_allocation", |b| {
        b.iter(|| {
            let vec = vec![0u8; 4096];
            black_box(vec);
        })
    });

    group.bench_function("pinned_buffer_allocation", |b| {
        b.iter(|| {
            let buffer = PinnedBuffer::with_capacity(4096);
            black_box(buffer);
        })
    });

    // Test compile-time vs runtime checks
    group.bench_function("compile_time_size_check", |b| {
        b.iter(|| {
            let buffer = PinnedBuffer::<[u8; 4096]>::zeroed();
            black_box(buffer.len()); // Compile-time constant
        })
    });

    group.bench_function("runtime_size_check", |b| {
        b.iter(|| {
            let buffer = PinnedBuffer::with_capacity(4096);
            black_box(buffer.len()); // Runtime check
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_allocation_strategies,
    bench_buffer_operations,
    bench_memory_patterns,
    bench_performance_counters,
    bench_zero_cost_abstractions
);

criterion_main!(benches);
