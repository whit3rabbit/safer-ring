//! Simple benchmark to measure OrphanTracker mutex contention.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use safer_ring::safety::OrphanTracker;
use safer_ring::ownership::OwnedBuffer;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Quick benchmark of single vs multi-threaded contention
fn bench_contention_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("orphan_tracker_contention");
    
    // Single-threaded baseline
    group.bench_function("single_thread_1000_ids", |b| {
        let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
        
        b.iter(|| {
            for _ in 0..1000 {
                let mut guard = tracker.lock().unwrap();
                black_box(guard.next_submission_id());
            }
        });
    });
    
    // Multi-threaded contention with 4 threads
    group.bench_function("4_threads_250_ids_each", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);
            
            for _ in 0..iters {
                let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
                let start = Instant::now();
                
                let handles: Vec<_> = (0..4)
                    .map(|_| {
                        let tracker = tracker.clone();
                        thread::spawn(move || {
                            for _ in 0..250 {
                                let mut guard = tracker.lock().unwrap();
                                black_box(guard.next_submission_id());
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
    
    // Compare with mixed operations (ID generation + orphan registration)
    group.bench_function("4_threads_mixed_ops", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::new(0, 0);
            
            for _ in 0..iters {
                let tracker = Arc::new(Mutex::new(OrphanTracker::new()));
                let start = Instant::now();
                
                let handles: Vec<_> = (0..4)
                    .map(|_| {
                        let tracker = tracker.clone();
                        thread::spawn(move || {
                            for i in 0..100 {
                                // Get submission ID (always)
                                let submission_id = {
                                    let mut guard = tracker.lock().unwrap();
                                    guard.next_submission_id()
                                };
                                black_box(submission_id);
                                
                                // Occasionally register and complete an orphan
                                if i % 5 == 0 {
                                    let buffer = OwnedBuffer::new(1024);
                                    {
                                        let mut guard = tracker.lock().unwrap();
                                        guard.register_orphan(submission_id, buffer);
                                    }
                                    {
                                        let mut guard = tracker.lock().unwrap();
                                        guard.handle_completion(submission_id, Ok(1024));
                                    }
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

criterion_group!(benches, bench_contention_comparison);
criterion_main!(benches);