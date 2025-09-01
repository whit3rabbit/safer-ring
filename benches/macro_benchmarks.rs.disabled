use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use pprof::criterion::{Output, PProfProfiler};
use safer_ring::{BufferPool, Ring};
use std::fs::File;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

// Raw io_uring comparison benchmarks
#[cfg(target_os = "linux")]
mod raw_io_uring {
    use io_uring::{opcode, types, IoUring};
    use std::os::unix::io::RawFd;
    use std::ptr;

    pub struct RawRing {
        ring: IoUring,
    }

    impl RawRing {
        pub fn new(entries: u32) -> std::io::Result<Self> {
            let ring = IoUring::new(entries)?;
            Ok(Self { ring })
        }

        pub async fn read_raw(
            &mut self,
            fd: RawFd,
            buf: &mut [u8],
            offset: u64,
        ) -> std::io::Result<usize> {
            let read_e =
                opcode::Read::new(types::Fd(fd), buf.as_mut_ptr(), buf.len() as u32).offset(offset);

            unsafe {
                let mut sq = self.ring.submission();
                let sqe = sq.next_sqe().expect("submission queue full");
                read_e.build().user_data(0x42).write_to(sqe);
                sq.sync();
            }

            self.ring.submit_and_wait(1)?;

            let mut cq = self.ring.completion();
            let cqe = cq.next().expect("completion queue empty");
            let result = cqe.result();
            cq.sync();

            if result < 0 {
                Err(std::io::Error::from_raw_os_error(-result))
            } else {
                Ok(result as usize)
            }
        }
    }
}

fn setup_large_test_file(size: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    let data = vec![0u8; size];
    std::io::Write::write_all(&mut file, &data).unwrap();
    file.flush().unwrap();
    file
}

fn bench_file_copy_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("file_copy_throughput");

    for size in [1024 * 1024, 10 * 1024 * 1024, 100 * 1024 * 1024].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        // Safer-ring implementation
        group.bench_with_input(BenchmarkId::new("safer_ring", size), size, |b, &size| {
            let source_file = setup_large_test_file(size);
            let dest_file = NamedTempFile::new().unwrap();

            b.iter(|| {
                rt.block_on(async {
                    let ring = Ring::new(256).unwrap();
                    let pool = BufferPool::new(32, 64 * 1024);

                    let src_fd = source_file.as_raw_fd();
                    let dst_fd = dest_file.as_raw_fd();

                    let mut offset = 0;
                    let chunk_size = 64 * 1024;

                    while offset < size {
                        let read_size = std::cmp::min(chunk_size, size - offset);
                        let buffer = pool.get().unwrap();

                        let (bytes_read, buffer) =
                            ring.read_at(src_fd, buffer, offset as u64).await.unwrap();
                        if bytes_read == 0 {
                            break;
                        }

                        let (bytes_written, _) =
                            ring.write_at(dst_fd, buffer, offset as u64).await.unwrap();
                        assert_eq!(bytes_read, bytes_written);

                        offset += bytes_read;
                    }

                    black_box(offset);
                })
            })
        });

        // Raw io_uring implementation for comparison
        #[cfg(target_os = "linux")]
        group.bench_with_input(BenchmarkId::new("raw_io_uring", size), size, |b, &size| {
            let source_file = setup_large_test_file(size);
            let dest_file = NamedTempFile::new().unwrap();

            b.iter(|| {
                rt.block_on(async {
                    let mut ring = raw_io_uring::RawRing::new(256).unwrap();
                    let src_fd = source_file.as_raw_fd();
                    let dst_fd = dest_file.as_raw_fd();

                    let mut offset = 0;
                    let chunk_size = 64 * 1024;
                    let mut buffer = vec![0u8; chunk_size];

                    while offset < size {
                        let read_size = std::cmp::min(chunk_size, size - offset);
                        let bytes_read = ring
                            .read_raw(src_fd, &mut buffer[..read_size], offset as u64)
                            .await
                            .unwrap();
                        if bytes_read == 0 {
                            break;
                        }

                        // Note: Raw implementation would need write operation too
                        offset += bytes_read;
                    }

                    black_box(offset);
                })
            })
        });

        // Standard std::fs implementation for baseline
        group.bench_with_input(BenchmarkId::new("std_fs", size), size, |b, &size| {
            let source_file = setup_large_test_file(size);
            let dest_file = NamedTempFile::new().unwrap();

            b.iter(|| {
                let mut src = File::open(source_file.path()).unwrap();
                let mut dst = File::create(dest_file.path()).unwrap();
                let bytes_copied = std::io::copy(&mut src, &mut dst).unwrap();
                black_box(bytes_copied);
            })
        });
    }

    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_operations");

    for concurrency in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("safer_ring_concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                let files: Vec<_> = (0..concurrency)
                    .map(|_| setup_large_test_file(1024 * 1024))
                    .collect();

                b.iter(|| {
                    rt.block_on(async {
                        let ring = Ring::new(256).unwrap();
                        let pool = BufferPool::new(concurrency * 2, 64 * 1024);

                        let mut futures = Vec::new();

                        for file in &files {
                            let fd = file.as_raw_fd();
                            let buffer = pool.get().unwrap();
                            if let Ok(future) = ring.read_at(fd, buffer, 0) {
                                futures.push(future);
                            }
                        }

                        let results = futures::future::join_all(futures).await;
                        black_box(results);
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_echo_server_simulation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("echo_server_simulation");

    // Simulate echo server workload with different message sizes
    for msg_size in [64, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*msg_size as u64));

        group.bench_with_input(
            BenchmarkId::new("safer_ring_echo", msg_size),
            msg_size,
            |b, &msg_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let ring = Ring::new(256).unwrap();
                        let pool = BufferPool::new(100, msg_size);

                        // Simulate processing 100 echo requests
                        let mut futures = Vec::new();

                        for _ in 0..100 {
                            let read_buffer = pool.acquire().unwrap();
                            let write_buffer = pool.acquire().unwrap();

                            // Simulate reading from client (using /dev/zero)
                            let zero_fd = std::fs::File::open("/dev/zero").unwrap().as_raw_fd();
                            let read_future = ring.read_at(zero_fd, read_buffer, 0);

                            // Simulate writing back to client (using /dev/null)
                            let null_fd = std::fs::File::create("/dev/null").unwrap().as_raw_fd();
                            let write_future = ring.write_at(null_fd, write_buffer, 0);

                            futures.push(async move {
                                let (_, _) = tokio::join!(read_future, write_future);
                            });
                        }

                        futures::future::join_all(futures).await;
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_batch_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_operations");

    for batch_size in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("safer_ring_batch", batch_size),
            batch_size,
            |b, &batch_size| {
                let file = setup_large_test_file(1024 * 1024);

                b.iter(|| {
                    rt.block_on(async {
                        let ring = Ring::new(256).unwrap();
                        let pool = BufferPool::new(*batch_size * 2, 4096);

                        let fd = file.as_raw_fd();
                        let mut futures = Vec::new();

                        for i in 0..*batch_size {
                            let buffer = pool.get().unwrap();
                            if let Ok(future) = ring.read_at(fd, buffer, (i * 4096) as u64) {
                                futures.push(future);
                            }
                        }

                        let results = futures::future::join_all(futures).await;
                        black_box(results);
                    })
                })
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
        .measurement_time(Duration::from_secs(10))
        .sample_size(50);
    targets =
        bench_file_copy_throughput,
        bench_concurrent_operations,
        bench_echo_server_simulation,
        bench_batch_operations
}

criterion_main!(benches);
