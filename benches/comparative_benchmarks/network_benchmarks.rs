//! Pseudo-device I/O performance benchmarks comparing safer-ring vs raw implementations.
//!
//! These benchmarks test I/O subsystem performance using /dev/zero and /dev/null,
//! which simulates network-like I/O patterns without requiring actual network setup.

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use safer_ring::{OwnedBuffer, Ring};
use std::os::unix::io::AsRawFd;
use tokio::runtime::Runtime;

use super::config;

#[cfg(target_os = "linux")]
use super::raw_io_uring::RawRing;

/// Benchmarks pseudo-device echo operations across different implementations.
///
/// Uses /dev/zero and /dev/null to simulate I/O patterns similar to network operations
/// without requiring actual network setup. This focuses on the I/O subsystem performance
/// and async polling efficiency rather than network stack overhead.
pub fn bench_pseudo_device_io_comparison(c: &mut Criterion) {
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let mut group = c.benchmark_group("pseudo_device_io_comparison");

    // Test different message sizes to characterize scaling behavior
    let message_sizes = [64, 1024, 4096];

    for &msg_size in &message_sizes {
        group.throughput(Throughput::Bytes(msg_size as u64));

        bench_safer_ring_pseudo_io(&mut group, &rt, msg_size);

        #[cfg(target_os = "linux")]
        bench_raw_io_uring_pseudo_io(&mut group, &rt, msg_size);
    }

    group.finish();
}

/// Benchmarks safer-ring pseudo-device I/O implementation.
fn bench_safer_ring_pseudo_io(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    rt: &Runtime,
    msg_size: usize,
) {
    group.bench_with_input(
        BenchmarkId::new("safer_ring_pseudo_io", msg_size),
        &msg_size,
        |b, &msg_size| {
            b.iter(|| {
                rt.block_on(async {
                    let ring = Ring::new(config::DEFAULT_RING_SIZE).expect("Failed to create ring");

                    // Use /dev/zero and /dev/null to simulate network I/O without network setup
                    // Keep File objects alive to prevent fd closure
                    let zero_file =
                        std::fs::File::open("/dev/zero").expect("Failed to open /dev/zero");
                    let null_file =
                        std::fs::File::create("/dev/null").expect("Failed to open /dev/null");
                    let zero_fd = zero_file.as_raw_fd();
                    let null_fd = null_file.as_raw_fd();

                    let mut total_processed = 0;

                    // Perform multiple read-write operations to amortize setup costs
                    for i in 0..config::ECHO_OPERATIONS_PER_ITER {
                        let read_buffer = OwnedBuffer::new(msg_size);
                        let result = ring.read_owned(zero_fd, read_buffer).await;

                        let (bytes_read, echo_buffer) = match result {
                            Ok(data) => data,
                            Err(e) => {
                                eprintln!("Read operation {i} failed with fd {zero_fd}: {e:?}");
                                // Try to continue with remaining operations
                                continue;
                            }
                        };

                        if bytes_read > 0 {
                            let write_result = ring.write_owned(null_fd, echo_buffer).await;
                            match write_result {
                                Ok((bytes_written, _)) => {
                                    total_processed += bytes_written;
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Write operation {i} failed with fd {null_fd}: {e:?}"
                                    );
                                    continue;
                                }
                            }
                        }
                    }

                    black_box(total_processed);
                })
            })
        },
    );
}

/// Benchmarks raw io_uring pseudo-device I/O implementation.
#[cfg(target_os = "linux")]
fn bench_raw_io_uring_pseudo_io(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    rt: &Runtime,
    msg_size: usize,
) {
    group.bench_with_input(
        BenchmarkId::new("raw_io_uring_pseudo_io", msg_size),
        &msg_size,
        |b, &msg_size| {
            b.iter(|| {
                rt.block_on(async {
                    let mut ring =
                        RawRing::new(config::DEFAULT_RING_SIZE).expect("Failed to create raw ring");

                    // Keep File objects alive to prevent fd closure
                    let zero_file =
                        std::fs::File::open("/dev/zero").expect("Failed to open /dev/zero");
                    let null_file =
                        std::fs::File::create("/dev/null").expect("Failed to open /dev/null");
                    let zero_fd = zero_file.as_raw_fd();
                    let null_fd = null_file.as_raw_fd();

                    let mut total_processed = 0;
                    // Pre-allocate buffer to avoid allocation overhead in benchmark
                    let mut buffer = vec![0u8; msg_size];

                    for i in 0..config::ECHO_OPERATIONS_PER_ITER {
                        let read_result = ring.read_raw(zero_fd, &mut buffer).await;
                        let bytes_read = match read_result {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                eprintln!("Raw read operation {i} failed with fd {zero_fd}: {e:?}");
                                continue;
                            }
                        };

                        if bytes_read > 0 {
                            let write_result = ring.write_raw(null_fd, &buffer[..bytes_read]).await;
                            let bytes_written = match write_result {
                                Ok(bytes) => bytes,
                                Err(e) => {
                                    eprintln!(
                                        "Raw write operation {i} failed with fd {null_fd}: {e:?}"
                                    );
                                    continue;
                                }
                            };

                            total_processed += bytes_written;
                        }
                    }

                    black_box(total_processed);
                })
            })
        },
    );
}
