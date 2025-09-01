//! File I/O performance benchmarks comparing safer-ring vs raw implementations.

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use safer_ring::{OwnedBuffer, Ring};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

use super::{config, system_info::SystemInfo, utils::setup_test_file};

#[cfg(target_os = "linux")]
use super::raw_io_uring::RawRing;

/// Benchmarks file copy operations across different implementations.
///
/// Tests safer-ring, raw io_uring, and standard library implementations
/// across multiple file sizes to characterize performance scaling.
pub fn bench_file_copy_comparison(c: &mut Criterion) {
    let system_info = SystemInfo::collect();
    system_info.log_system_info();

    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let mut group = c.benchmark_group("file_copy_comparison");

    // Test file sizes: 1MB, 10MB for reasonable benchmark duration
    let test_sizes = [1024 * 1024, 10 * 1024 * 1024];

    for &size in &test_sizes {
        group.throughput(Throughput::Bytes(size as u64));

        bench_safer_ring_file_copy(&mut group, &rt, size);

        #[cfg(target_os = "linux")]
        bench_raw_io_uring_file_copy(&mut group, &rt, size);

        bench_std_fs_file_copy(&mut group, size);
    }

    group.finish();
}

/// Benchmarks safer-ring file copy implementation.
fn bench_safer_ring_file_copy(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    rt: &Runtime,
    size: usize,
) {
    group.bench_with_input(BenchmarkId::new("safer_ring", size), &size, |b, &size| {
        let source_file = setup_test_file(size);
        let dest_file = NamedTempFile::new().expect("Failed to create destination file");

        b.iter(|| {
            rt.block_on(async {
                let ring = Ring::new(config::DEFAULT_RING_SIZE).expect("Failed to create ring");
                let src_fd = source_file.as_raw_fd();
                let dst_fd = dest_file.as_raw_fd();

                let mut offset = 0;

                while offset < size {
                    let read_size = std::cmp::min(config::FILE_CHUNK_SIZE, size - offset);
                    let buffer = OwnedBuffer::new(read_size);

                    let (bytes_read, buffer) = ring
                        .read_at_owned(src_fd, buffer, offset as u64)
                        .await
                        .expect("Read operation failed");

                    if bytes_read == 0 {
                        break;
                    }

                    let (bytes_written, _) = ring
                        .write_at_owned(dst_fd, buffer, offset as u64, bytes_read)
                        .await
                        .expect("Write operation failed");

                    assert_eq!(bytes_read, bytes_written, "Read/write size mismatch");
                    offset += bytes_read;
                }

                black_box(offset);
            })
        })
    });
}

/// Benchmarks raw io_uring file copy implementation.
#[cfg(target_os = "linux")]
fn bench_raw_io_uring_file_copy(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    rt: &Runtime,
    size: usize,
) {
    group.bench_with_input(BenchmarkId::new("raw_io_uring", size), &size, |b, &size| {
        let source_file = setup_test_file(size);
        let dest_file = NamedTempFile::new().expect("Failed to create destination file");

        b.iter(|| {
            rt.block_on(async {
                let mut ring =
                    RawRing::new(config::DEFAULT_RING_SIZE).expect("Failed to create raw ring");
                let src_fd = source_file.as_raw_fd();
                let dst_fd = dest_file.as_raw_fd();

                let mut offset = 0;
                // Pre-allocate buffer to avoid allocation overhead in benchmark
                let mut buffer = vec![0u8; config::FILE_CHUNK_SIZE];

                while offset < size {
                    let read_size = std::cmp::min(config::FILE_CHUNK_SIZE, size - offset);
                    let bytes_read = ring
                        .read_at_raw(src_fd, &mut buffer[..read_size], offset as u64)
                        .await
                        .expect("Raw read operation failed");

                    if bytes_read == 0 {
                        break;
                    }

                    let bytes_written = ring
                        .write_at_raw(dst_fd, &buffer[..bytes_read], offset as u64)
                        .await
                        .expect("Raw write operation failed");

                    assert_eq!(bytes_read, bytes_written, "Read/write size mismatch");
                    offset += bytes_read;
                }

                black_box(offset);
            })
        })
    });
}

/// Benchmarks standard library file copy implementation.
fn bench_std_fs_file_copy(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    size: usize,
) {
    group.bench_with_input(BenchmarkId::new("std_fs", size), &size, |b, &size| {
        let source_file = setup_test_file(size);
        let dest_file = NamedTempFile::new().expect("Failed to create destination file");

        b.iter(|| {
            let mut src = File::open(source_file.path()).expect("Failed to open source file");
            let mut dst =
                File::create(dest_file.path()).expect("Failed to create destination file");
            let bytes_copied = std::io::copy(&mut src, &mut dst).expect("File copy failed");
            black_box(bytes_copied);
        })
    });
}
