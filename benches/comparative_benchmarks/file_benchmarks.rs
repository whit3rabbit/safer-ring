//! File I/O performance benchmarks comparing safer-ring vs raw implementations.
//!
//! # Important Note on Benchmark Results
//!
//! The `std_fs` benchmark uses `std::fs::copy()` which is highly optimized by the OS kernel.
//! This operation can leverage copy_file_range() syscalls, page cache optimizations, and
//! in-kernel data movement without userspace copies. In contrast, the io_uring benchmarks
//! perform explicit read-then-write operations that move data through userspace buffers.
//!
//! For more realistic I/O performance comparisons that bypass page cache effects,
//! consider using O_DIRECT flags or testing with files larger than available RAM.

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use safer_ring::{OwnedBuffer, Ring};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

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
    // Configure for reasonable benchmark execution time
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(3))
        .warm_up_time(Duration::from_secs(1));

    // Test file sizes: 64KB, 1MB for performance comparison
    let test_sizes = [64 * 1024, 1024 * 1024];

    for &size in &test_sizes {
        group.throughput(Throughput::Bytes(size as u64));

        bench_safer_ring_file_copy(&mut group, &rt, size);

        #[cfg(target_os = "linux")]
        bench_raw_io_uring_file_copy(&mut group, &rt, size);

        bench_std_fs_file_copy(&mut group, size);

        #[cfg(target_os = "linux")]
        bench_safer_ring_file_copy_direct(&mut group, &rt, size);
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
        // Setup: Create test files and ring once
        let source_file = setup_test_file(size);
        let dest_file = NamedTempFile::new().expect("Failed to create destination file");

        // Pre-create ring for reuse across iterations
        let ring = Ring::new(config::DEFAULT_RING_SIZE).expect("Failed to create ring");

        b.iter(|| {
            rt.block_on(async {
                let src_fd = source_file.as_raw_fd();
                let dst_fd = dest_file.as_raw_fd();
                let mut offset = 0;

                while offset < size {
                    let read_size = std::cmp::min(config::FILE_CHUNK_SIZE, size - offset);
                    let buffer = OwnedBuffer::new(read_size);

                    // Use async operations with optimized completion waiting
                    let (bytes_read, buffer) =
                        match ring.read_at_owned(src_fd, buffer, offset as u64).await {
                            Ok(data) => data,
                            Err(e) => panic!("Read operation failed at offset {}: {:?}", offset, e),
                        };

                    if bytes_read == 0 {
                        break;
                    }

                    let (bytes_written, _) = match ring
                        .write_at_owned(dst_fd, buffer, offset as u64, bytes_read)
                        .await
                    {
                        Ok(data) => data,
                        Err(e) => panic!("Write operation failed at offset {}: {:?}", offset, e),
                    };

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
        // Setup: Create test files once
        let source_file = setup_test_file(size);
        let dest_file = NamedTempFile::new().expect("Failed to create destination file");

        // Pre-allocate buffer once to avoid allocation overhead in benchmark
        let mut buffer = vec![0u8; config::FILE_CHUNK_SIZE];

        b.iter(|| {
            rt.block_on(async {
                let mut ring =
                    RawRing::new(config::DEFAULT_RING_SIZE).expect("Failed to create raw ring");
                let src_fd = source_file.as_raw_fd();
                let dst_fd = dest_file.as_raw_fd();

                let mut offset = 0;

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
        // Setup: Create test files once
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

/// Benchmarks safer-ring file copy implementation with O_DIRECT (Linux only).
///
/// Uses O_DIRECT to bypass page cache for more realistic I/O performance measurement.
/// File sizes are aligned to 4KB boundaries as required by O_DIRECT.
#[cfg(target_os = "linux")]
fn bench_safer_ring_file_copy_direct(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    rt: &Runtime,
    size: usize,
) {
    // O_DIRECT requires aligned buffer sizes, so we use 4KB chunks minimum
    let aligned_size = ((size + 4095) / 4096) * 4096;

    group.bench_with_input(
        BenchmarkId::new("safer_ring_direct", aligned_size),
        &aligned_size,
        |b, &aligned_size| {
            b.iter(|| {
                rt.block_on(async {
                    // Create temporary files with O_DIRECT
                    let source_file = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .read(true)
                        .custom_flags(libc::O_DIRECT)
                        .open("/tmp/bench_src_direct")
                        .expect("Failed to create O_DIRECT source file");

                    let dest_file = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .read(true)
                        .truncate(true)
                        .custom_flags(libc::O_DIRECT)
                        .open("/tmp/bench_dst_direct")
                        .expect("Failed to create O_DIRECT destination file");

                    // Write test data to source file first (4KB aligned)
                    let test_data = vec![0x42u8; aligned_size];
                    std::fs::write("/tmp/bench_src_direct_temp", &test_data)
                        .expect("Failed to write test data");
                    std::fs::copy("/tmp/bench_src_direct_temp", "/tmp/bench_src_direct")
                        .expect("Failed to copy test data");

                    let ring = Ring::new(config::DEFAULT_RING_SIZE).expect("Failed to create ring");
                    let src_fd = source_file.as_raw_fd();
                    let dst_fd = dest_file.as_raw_fd();
                    let mut offset = 0;

                    while offset < aligned_size {
                        let read_size = std::cmp::min(4096, aligned_size - offset); // 4KB chunks for O_DIRECT
                        let buffer = OwnedBuffer::new(read_size);

                        let (bytes_read, buffer) = ring
                            .read_at_owned(src_fd, buffer, offset as u64)
                            .await
                            .expect("O_DIRECT read operation failed");

                        if bytes_read == 0 {
                            break;
                        }

                        let (bytes_written, _) = ring
                            .write_at_owned(dst_fd, buffer, offset as u64, bytes_read)
                            .await
                            .expect("O_DIRECT write operation failed");

                        assert_eq!(
                            bytes_read, bytes_written,
                            "Read/write size mismatch with O_DIRECT"
                        );
                        offset += bytes_read;
                    }

                    // Cleanup temp files
                    let _ = std::fs::remove_file("/tmp/bench_src_direct");
                    let _ = std::fs::remove_file("/tmp/bench_dst_direct");
                    let _ = std::fs::remove_file("/tmp/bench_src_direct_temp");

                    black_box(offset);
                })
            })
        },
    );
}
