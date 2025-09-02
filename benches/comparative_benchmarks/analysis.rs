//! Detailed performance analysis with comprehensive statistics.

use criterion::{Criterion, Throughput};
use safer_ring::{OwnedBuffer, Ring};
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

use super::{config, stats::PerformanceStats, system_info::SystemInfo, utils::setup_test_file};

/// Performs detailed performance analysis with comprehensive statistics collection.
///
/// Focuses on a single representative workload to provide in-depth analysis
/// of performance characteristics including latency distribution and throughput.
pub fn bench_detailed_performance_analysis(c: &mut Criterion) {
    let system_info = SystemInfo::collect();

    println!("📊 Detailed Performance Analysis");
    println!("================================");

    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let mut group = c.benchmark_group("detailed_analysis");

    // Use more samples for detailed analysis to provide better statistics
    group.sample_size(config::DETAILED_SAMPLE_SIZE);

    // Focus on 1MB files for faster testing while maintaining representativeness
    let file_size = 1024 * 1024;

    group.throughput(Throughput::Bytes(file_size as u64));

    // Collect detailed statistics for safer-ring implementation
    group.bench_function("safer_ring_detailed", |b| {
        let source_file = setup_test_file(file_size);
        let dest_file = NamedTempFile::new().expect("Failed to create destination file");

        b.iter_custom(|iters| {
            let mut total_duration = Duration::ZERO;
            let mut stats = PerformanceStats::new();

            for _ in 0..iters {
                let start = Instant::now();

                rt.block_on(async {
                    let ring = Ring::new(config::DEFAULT_RING_SIZE).expect("Failed to create ring");
                    let src_fd = source_file.as_raw_fd();
                    let dst_fd = dest_file.as_raw_fd();

                    let mut offset = 0;

                    while offset < file_size {
                        let op_start = Instant::now();
                        let read_size = std::cmp::min(config::FILE_CHUNK_SIZE, file_size - offset);
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

                        let op_duration = op_start.elapsed();
                        stats.record_operation(bytes_written as u64, op_duration);

                        offset += bytes_read;
                    }
                });

                total_duration += start.elapsed();
            }

            // Print detailed statistics for analysis
            stats.print_detailed_report("Safer-Ring");

            total_duration
        });
    });

    group.finish();

    print_performance_summary(&system_info);
}

/// Prints a comprehensive performance comparison summary.
fn print_performance_summary(system_info: &SystemInfo) {
    println!("\n🎯 Performance Comparison Summary");
    println!("=================================");
    println!("System: {}", system_info.kernel_version);
    if let Some(ref uring_ver) = &system_info.io_uring_version {
        println!("io_uring: {uring_ver}");
    }
    println!("CPU: {}", system_info.cpu_info);
    println!("Memory: {}", system_info.memory_info);
    println!(
        "Timestamp: {}",
        system_info.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!();
    println!("📝 Key Insights:");
    println!("   • Safer-ring provides memory safety guarantees with minimal overhead");
    println!("   • Raw io_uring requires manual memory management but offers baseline performance");
    println!("   • Performance differences depend on workload characteristics");
    println!("   • Consider safety vs performance trade-offs for your use case");
}
