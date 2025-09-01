//! Network I/O performance benchmarks comparing safer-ring vs raw implementations.

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use safer_ring::{OwnedBuffer, Ring};
use std::os::unix::io::AsRawFd;
use tokio::runtime::Runtime;

use super::config;

#[cfg(target_os = "linux")]
use super::raw_io_uring::RawRing;

/// Benchmarks network echo operations across different implementations.
///
/// Uses /dev/zero and /dev/null to simulate network I/O patterns without
/// requiring actual network setup, focusing on the I/O subsystem performance.
pub fn bench_network_echo_comparison(c: &mut Criterion) {
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let mut group = c.benchmark_group("network_echo_comparison");

    // Test different message sizes to characterize scaling behavior
    let message_sizes = [64, 1024, 4096];

    for &msg_size in &message_sizes {
        group.throughput(Throughput::Bytes(msg_size as u64));

        bench_safer_ring_echo(&mut group, &rt, msg_size);

        #[cfg(target_os = "linux")]
        bench_raw_io_uring_echo(&mut group, &rt, msg_size);
    }

    group.finish();
}

/// Benchmarks safer-ring echo implementation.
fn bench_safer_ring_echo(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    rt: &Runtime,
    msg_size: usize,
) {
    group.bench_with_input(
        BenchmarkId::new("safer_ring_echo", msg_size),
        &msg_size,
        |b, &msg_size| {
            b.iter(|| {
                rt.block_on(async {
                    let ring = Ring::new(config::DEFAULT_RING_SIZE).expect("Failed to create ring");

                    // Use /dev/zero and /dev/null to simulate network I/O without network setup
                    let zero_fd = std::fs::File::open("/dev/zero")
                        .expect("Failed to open /dev/zero")
                        .as_raw_fd();
                    let null_fd = std::fs::File::create("/dev/null")
                        .expect("Failed to open /dev/null")
                        .as_raw_fd();

                    let mut total_processed = 0;

                    // Perform multiple echo operations to amortize setup costs
                    for _ in 0..config::ECHO_OPERATIONS_PER_ITER {
                        let read_buffer = OwnedBuffer::new(msg_size);
                        let (bytes_read, echo_buffer) = ring
                            .read_owned(zero_fd, read_buffer)
                            .await
                            .expect("Read operation failed");

                        if bytes_read > 0 {
                            let (bytes_written, _) = ring
                                .write_owned(null_fd, echo_buffer)
                                .await
                                .expect("Write operation failed");

                            total_processed += bytes_written;
                        }
                    }

                    black_box(total_processed);
                })
            })
        },
    );
}

/// Benchmarks raw io_uring echo implementation.
#[cfg(target_os = "linux")]
fn bench_raw_io_uring_echo(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    rt: &Runtime,
    msg_size: usize,
) {
    group.bench_with_input(
        BenchmarkId::new("raw_io_uring_echo", msg_size),
        &msg_size,
        |b, &msg_size| {
            b.iter(|| {
                rt.block_on(async {
                    let mut ring =
                        RawRing::new(config::DEFAULT_RING_SIZE).expect("Failed to create raw ring");

                    let zero_fd = std::fs::File::open("/dev/zero")
                        .expect("Failed to open /dev/zero")
                        .as_raw_fd();
                    let null_fd = std::fs::File::create("/dev/null")
                        .expect("Failed to open /dev/null")
                        .as_raw_fd();

                    let mut total_processed = 0;
                    // Pre-allocate buffer to avoid allocation overhead in benchmark
                    let mut buffer = vec![0u8; msg_size];

                    for _ in 0..config::ECHO_OPERATIONS_PER_ITER {
                        let bytes_read = ring
                            .read_raw(zero_fd, &mut buffer)
                            .await
                            .expect("Raw read operation failed");

                        if bytes_read > 0 {
                            let bytes_written = ring
                                .write_raw(null_fd, &buffer[..bytes_read])
                                .await
                                .expect("Raw write operation failed");

                            total_processed += bytes_written;
                        }
                    }

                    black_box(total_processed);
                })
            })
        },
    );
}
