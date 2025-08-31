//! Benchmark for measuring OrphanTracker mutex contention.
//!
//! This benchmark measures the potential performance impact of the single
//! global mutex used by OrphanTracker under different contention scenarios.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use safer_ring::ownership::OwnedBuffer;
use safer_ring::safety::OrphanTracker;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Benchmark single-threaded OrphanTracker operations
fn bench_single_threaded_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_threaded_orphan_tracker");

    for size in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("submission_ids", size),
            size,
            |b, &size| {
                let tracker = Arc::new(Mutex::new(OrphanTracker::new()));

                b.iter(|| {
                    for _ in 0..size {
                        let mut tracker = tracker.lock().unwrap();
                        black_box(tracker.next_submission_id());
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("register_orphans", size),
            size,
            |b, &size| {
                let tracker = Arc::new(Mutex::new(OrphanTracker::new()));

                b.iter(|| {
                    // Register orphans
                    for i in 0..size {
                        let buffer = OwnedBuffer::new(1024);
                        let mut tracker = tracker.lock().unwrap();
                        tracker.register_orphan(i as u64, buffer);
                    }
                    // Clean up by simulating completion handling
                    let mut tracker = tracker.lock().unwrap();
                    for i in 0..size {
                        let _completion = tracker.handle_completion(i as u64, Ok(1024));
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark multi-threaded OrphanTracker contention
fn bench_multi_threaded_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_threaded_orphan_tracker_contention");

    for threads in [2, 4, 8, 16].iter() {
        for ops_per_thread in [100, 1000].iter() {
            group.throughput(Throughput::Elements((threads * ops_per_thread) as u64));

            group.bench_with_input(
                BenchmarkId::new(
                    "submission_id_contention",
                    format!("{threads}threads_{ops_per_thread}ops"),
                ),
                &(threads, ops_per_thread),
                |b, &(&threads, &ops_per_thread)| {
                    b.iter_custom(|iters| {
                        let mut total_duration = Duration::new(0, 0);

                        for _ in 0..iters {
                            let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
                            let start = Instant::now();

                            let handles: Vec<_> = (0..threads)
                                .map(|_| {
                                    let tracker = tracker.clone();
                                    thread::spawn(move || {
                                        for _ in 0..ops_per_thread {
                                            let mut tracker = tracker.lock().unwrap();
                                            black_box(tracker.next_submission_id());
                                        }
                                    })
                                })
                                .collect();

                            for handle in handles {
                                handle.join().unwrap();
                            }

                            total_duration += start.elapsed();
                        }

                        total_duration
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new(
                    "mixed_operations_contention",
                    format!("{threads}threads_{ops_per_thread}ops"),
                ),
                &(threads, ops_per_thread),
                |b, &(&threads, &ops_per_thread)| {
                    b.iter_custom(|iters| {
                        let mut total_duration = Duration::new(0, 0);

                        for _ in 0..iters {
                            let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
                            let start = Instant::now();

                            let handles: Vec<_> = (0..threads)
                                .map(|_thread_id| {
                                    let tracker = tracker.clone();
                                    thread::spawn(move || {
                                        for i in 0..ops_per_thread {
                                            let mut tracker_guard = tracker.lock().unwrap();
                                            let submission_id = tracker_guard.next_submission_id();
                                            drop(tracker_guard);

                                            // Simulate some work
                                            black_box(submission_id);

                                            // Register an orphan occasionally
                                            if i % 10 == 0 {
                                                let buffer = OwnedBuffer::new(1024);
                                                let mut tracker_guard = tracker.lock().unwrap();
                                                tracker_guard
                                                    .register_orphan(submission_id, buffer);
                                            }

                                            // Handle completion occasionally (simulate reclaiming)
                                            if i % 15 == 0 {
                                                let mut tracker_guard = tracker.lock().unwrap();
                                                let _completion = tracker_guard
                                                    .handle_completion(submission_id, Ok(1024));
                                            }
                                        }
                                    })
                                })
                                .collect();

                            for handle in handles {
                                handle.join().unwrap();
                            }

                            total_duration += start.elapsed();
                        }

                        total_duration
                    });
                },
            );
        }
    }

    group.finish();
}

/// Compare different lock granularities
fn bench_lock_granularity_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock_granularity_comparison");

    // Simulate the current approach (single lock)
    group.bench_function("single_global_lock", |b| {
        let _tracker = Arc::new(Mutex::new(OrphanTracker::new()));
        let threads = 4;
        let ops_per_thread = 1000;

        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);

            for _ in 0..iters {
                let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
                let start = Instant::now();

                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        let tracker = tracker.clone();
                        thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let mut tracker = tracker.lock().unwrap();
                                black_box(tracker.next_submission_id());
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                total_duration += start.elapsed();
            }

            total_duration
        });
    });

    // Simulate sharded approach (separate locks for different operations)
    group.bench_function("sharded_locks_simulation", |b| {
        // Simulate having separate locks for ID generation vs orphan tracking
        let _id_gen_lock = Arc::new(Mutex::new(0u64));
        let _orphan_lock = Arc::new(Mutex::new(
            std::collections::HashMap::<u64, OwnedBuffer>::new(),
        ));

        let threads = 4;
        let ops_per_thread = 1000;

        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);

            for _ in 0..iters {
                let id_gen_lock = Arc::new(Mutex::new(0u64));
                let orphan_lock = Arc::new(Mutex::new(
                    std::collections::HashMap::<u64, OwnedBuffer>::new(),
                ));
                let start = Instant::now();

                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        let id_gen_lock = id_gen_lock.clone();
                        let orphan_lock = orphan_lock.clone();
                        thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                // Separate locks allow more concurrency
                                let id = {
                                    let mut gen = id_gen_lock.lock().unwrap();
                                    *gen += 1;
                                    *gen
                                };
                                black_box(id);

                                // Occasionally touch the orphan map
                                if id % 100 == 0 {
                                    let orphan_map = orphan_lock.lock().unwrap();
                                    black_box(orphan_map.len());
                                }
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }

                total_duration += start.elapsed();
            }

            total_duration
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_threaded_operations,
    bench_multi_threaded_contention,
    bench_lock_granularity_comparison
);
criterion_main!(benches);
