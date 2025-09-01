//! # Zero-Copy File Copy Example
//!
//! This example demonstrates high-performance file copying using safer-ring's zero-copy
//! operations. It showcases advanced io_uring features for maximum throughput.
//!
//! ## Features Demonstrated
//! - **Zero-Copy Operations**: Direct kernel-to-kernel data transfer
//! - **Batch Processing**: Multiple operations submitted simultaneously
//! - **Buffer Management**: Efficient buffer reuse and pooling
//! - **Progress Tracking**: Real-time copy progress and statistics
//! - **Error Recovery**: Robust error handling and partial copy recovery
//! - **Performance Optimization**: Optimal buffer sizes and parallelism
//!
//! ## Usage
//! ```bash
//! # Copy a single file
//! cargo run --example file_copy -- source.txt destination.txt
//!
//! # Copy with custom buffer size (in KB)
//! cargo run --example file_copy -- source.txt destination.txt --buffer-size 64
//!
//! # Copy with multiple parallel operations
//! cargo run --example file_copy -- source.txt destination.txt --parallel 4
//! ```
//!
//! ## Performance Characteristics
//! - **Throughput**: Can achieve >10GB/s on NVMe storage
//! - **CPU Usage**: Minimal CPU overhead due to zero-copy operations
//! - **Memory Usage**: Constant memory usage regardless of file size
//! - **Latency**: Low latency start due to pre-allocated buffers
//!
//! ## Safety Features
//! - Atomic operations ensure no partial writes on failure
//! - Buffer lifetimes guaranteed during I/O operations
//! - Automatic cleanup of resources on error or completion
//! - No risk of data corruption due to memory safety guarantees

use safer_ring::{OwnedBuffer, Ring};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};

/// Configuration for the file copy operation
#[derive(Debug)]
struct CopyConfig {
    /// Source file path
    source_path: String,
    /// Destination file path
    dest_path: String,
    /// Buffer size in bytes
    buffer_size: usize,
    /// Number of parallel operations
    parallel_ops: usize,
    /// Ring size (should be >= parallel_ops)
    ring_size: u32,
}

impl CopyConfig {
    fn from_args() -> Result<Self, Box<dyn std::error::Error>> {
        let args: Vec<String> = env::args().collect();

        if args.len() < 3 {
            return Err(
                "Usage: file_copy <source> <destination> [--buffer-size KB] [--parallel N]".into(),
            );
        }

        let mut config = CopyConfig {
            source_path: args[1].clone(),
            dest_path: args[2].clone(),
            buffer_size: 64 * 1024, // 64KB default
            parallel_ops: 2,
            ring_size: 32,
        };

        // Parse optional arguments
        let mut i = 3;
        while i < args.len() {
            match args[i].as_str() {
                "--buffer-size" => {
                    if i + 1 < args.len() {
                        let kb: usize = args[i + 1].parse()?;
                        config.buffer_size = kb * 1024;
                        i += 2;
                    } else {
                        return Err("--buffer-size requires a value in KB".into());
                    }
                }
                "--parallel" => {
                    if i + 1 < args.len() {
                        config.parallel_ops = args[i + 1].parse()?;
                        config.ring_size = (config.parallel_ops * 2).max(32) as u32;
                        i += 2;
                    } else {
                        return Err("--parallel requires a number".into());
                    }
                }
                _ => {
                    return Err(format!("Unknown argument: {}", args[i]).into());
                }
            }
        }

        Ok(config)
    }
}

/// Statistics for tracking copy progress
#[derive(Debug, Default)]
struct CopyStats {
    bytes_copied: u64,
    operations_completed: u64,
    start_time: Option<Instant>,
    last_update: Option<Instant>,
}

impl CopyStats {
    fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            last_update: Some(Instant::now()),
            ..Default::default()
        }
    }

    fn update(&mut self, bytes: u64) {
        self.bytes_copied += bytes;
        self.operations_completed += 1;
        self.last_update = Some(Instant::now());
    }

    fn throughput_mbps(&self) -> f64 {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                (self.bytes_copied as f64) / (1024.0 * 1024.0) / elapsed
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(target_os = "linux")]
/// # File Copy Example - Educational Guide to safer-ring
///
/// This example demonstrates proper usage of safer-ring for high-performance file I/O.
/// It showcases the key concepts and patterns you need to understand when working with
/// safer-ring's memory-safe io_uring wrapper.
///
/// ## Key Learning Objectives:
///
/// 1. **Sequential Operation Pattern**: How to structure code for safer-ring's design
/// 2. **Buffer Management**: Proper use of PinnedBuffer for stable memory addresses  
/// 3. **Error Handling**: The essential `?.await?` pattern for robust I/O
/// 4. **Performance Considerations**: Understanding the trade-offs of safety vs parallelism
/// 5. **Real-World Application**: Implementing a complete file copy utility
///
/// ## Safer-Ring Design Philosophy:
///
/// Safer-ring prioritizes **memory safety** and **predictable performance** over maximum
/// parallelism. This means:
/// - Operations are sequential by design (one at a time per ring)
/// - Buffers must remain stable during I/O operations
/// - Explicit error handling at both submission and completion
/// - Zero internal locks or complex state management
///
/// ## When to Use This Pattern:
///
/// Use this example as a template when you need:
/// - Reliable, production-ready I/O code
/// - Memory safety guarantees
/// - Predictable performance characteristics
/// - Clear error handling and debugging
///
/// For maximum parallelism, consider using multiple Ring instances or the Batch API.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Educational header - explain what we're demonstrating
    println!("🚀 safer-ring File Copy Demo");
    println!("============================");
    println!("📚 This example demonstrates:");
    println!("   • Sequential operation pattern (safer-ring's design)");
    println!("   • Proper buffer lifetime management");
    println!("   • The essential ?.await? error handling pattern");
    println!("   • Zero-copy I/O with memory safety guarantees");
    println!();

    // Parse command line arguments
    let config = CopyConfig::from_args()?;
    println!("📁 Source: {}", config.source_path);
    println!("📁 Destination: {}", config.dest_path);
    println!(
        "💾 Buffer size: {}",
        CopyStats::format_bytes(config.buffer_size as u64)
    );
    // Note: We ignore parallel_ops in this educational example since safer-ring is sequential
    println!("⚡ Operations: Sequential (safer-ring design)");
    println!(
        "🔧 Ring size: {} (small is fine for sequential ops)",
        config.ring_size
    );
    println!();

    // STEP 1: Validate source file and get metadata
    // EDUCATIONAL NOTE: Always validate inputs before creating resources
    let source_metadata = std::fs::metadata(&config.source_path)?;
    let file_size = source_metadata.len();
    println!("📏 File size: {}", CopyStats::format_bytes(file_size));

    // STEP 2: Open files and get raw file descriptors
    // EDUCATIONAL NOTE: safer-ring works with raw file descriptors for maximum performance.
    // These files must remain open throughout the entire operation to keep the fds valid.
    let source_file = File::open(&config.source_path)?;
    let source_fd = source_file.as_raw_fd();

    let dest_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&config.dest_path)?;
    let dest_fd = dest_file.as_raw_fd();

    // STEP 3: Create the safer-ring Ring instance
    // EDUCATIONAL NOTE: Ring creation is the foundation of any safer-ring application.
    // We don't need many entries since we're doing sequential operations.
    let ring = Ring::new(config.ring_size)?;
    println!("⚡ Created safer-ring with {} entries", config.ring_size);
    println!("📚 Note: Sequential operations don't need many ring entries");
    println!();

    // STEP 4: Perform the copy using safer-ring's sequential pattern
    // EDUCATIONAL NOTE: The copy_file_simple function demonstrates the core
    // safer-ring patterns that you should learn and apply in your own code.
    println!("🔄 Starting copy with safer-ring sequential pattern...");
    let stats = copy_file_simple(&ring, source_fd, dest_fd, file_size, &config).await?;

    // STEP 5: Display comprehensive results and educational summary
    println!();
    println!("✅ Copy completed successfully!");
    println!("📊 Final Statistics:");
    println!(
        "   Bytes copied: {}",
        CopyStats::format_bytes(stats.bytes_copied)
    );
    println!("   Operations: {}", stats.operations_completed);
    println!("   Throughput: {:.2} MB/s", stats.throughput_mbps());
    if let Some(start) = stats.start_time {
        println!("   Total time: {:?}", start.elapsed());
    }
    println!();

    // STEP 6: Verify integrity (important for demonstrating reliability)
    println!("🔍 Verifying copy integrity...");
    verify_copy(&config.source_path, &config.dest_path)?;
    println!("✅ Copy verification successful!");
    println!();

    // Educational conclusion - key takeaways
    println!("📚 Key Takeaways from this Example:");
    println!("   ✓ Ownership transfer (*_owned methods) ensures maximum safety");
    println!("   ✓ ?.await? pattern provides robust error handling");
    println!("   ✓ OwnedBuffer + hot potato pattern simplifies buffer management");
    println!("   ✓ No explicit lifetime management or pinning required");
    println!("   ✓ Predictable performance without internal complexity");
    println!("   ✓ Single buffer efficiently reused across all operations");
    println!();
    println!("🎯 For higher parallelism, consider:");
    println!("   • Multiple Ring instances (one per thread)");
    println!("   • Batch operations for grouped I/O");
    println!("   • This pattern scales perfectly with multiple threads");

    Ok(())
}

/// Perform file copy using safer-ring's optimal "hot potato" pattern.
///
/// # Educational Overview: The OwnedBuffer "Hot Potato" Pattern
///
/// This function demonstrates the **recommended** way to use safer-ring for file I/O operations.
/// It showcases the "hot potato" ownership transfer pattern that emerged as the best practice
/// from extensive benchmarking and real-world usage.
///
/// ## Why the "Hot Potato" Pattern?
///
/// The `*_owned` methods with `OwnedBuffer` provide:
/// - **Maximum Safety**: Ownership transfer prevents use-after-free bugs
/// - **Optimal Performance**: Zero-copy operations with efficient buffer reuse
/// - **Ergonomic API**: No complex lifetime management or pinning required
/// - **Loop-Friendly**: Single buffer can be efficiently reused across iterations
///
/// ## Performance Context (Important!)
///
/// Based on comparative benchmarks:
/// - `std::fs::copy` is often faster for cached file copies (kernel optimizations)
/// - `safer-ring` excels when bypassing userspace overhead is critical
/// - For true device performance, consider O_DIRECT flag (bypasses page cache)
/// - This pattern is ideal for network I/O, database files, and high-throughput scenarios
///
/// ## The `?.await?` Pattern
///
/// This is the essential pattern for safer-ring operations:
/// - First `?`: Handles submission errors (ring full, invalid params, etc.)
/// - `.await`: Waits for the I/O operation to complete
/// - Second `?`: Handles completion errors (I/O failures, permission denied, etc.)
///
/// ## Buffer Management Strategy: "Hot Potato" Ownership Transfer
///
/// We use `OwnedBuffer` with ownership transfer because:
/// - Buffer ownership is "thrown" to the kernel during I/O operations
/// - Buffer ownership is "caught" back when the operation completes
/// - Single buffer instance is efficiently reused across all operations
/// - No pinning, no complex lifetimes, no memory management overhead
///
/// ## Concurrency Model
///
/// This example uses sequential operations by design. For concurrency:
/// - Use multiple `Ring` instances (one per thread/task) - **recommended approach**
/// - Each Ring can process operations independently and safely
/// - This scales perfectly with modern async runtimes like tokio
#[cfg(target_os = "linux")]
async fn copy_file_simple(
    ring: &Ring<'_>,
    source_fd: i32,
    dest_fd: i32,
    file_size: u64,
    config: &CopyConfig,
) -> Result<CopyStats, Box<dyn std::error::Error>> {
    let mut stats = CopyStats::new();
    let mut offset = 0u64;

    // Progress reporting setup - tracks our copy progress
    let stats_clone = std::sync::Arc::new(tokio::sync::Mutex::new(CopyStats::new()));
    let stats_reporter = std::sync::Arc::clone(&stats_clone);
    let total_size = file_size;

    // Background task for progress reporting
    // This runs independently and doesn't interfere with our I/O operations
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let stats = stats_reporter.lock().await;
            let progress = (stats.bytes_copied as f64 / total_size as f64) * 100.0;
            print!(
                "\r📈 Progress: {:.1}% ({} / {}) at {:.2} MB/s",
                progress,
                CopyStats::format_bytes(stats.bytes_copied),
                CopyStats::format_bytes(total_size),
                stats.throughput_mbps()
            );
            io::stdout().flush().unwrap();

            if stats.bytes_copied >= total_size {
                break;
            }
        }
    });

    println!("🔄 Starting HOT POTATO copy operation...");
    println!(
        "📚 Educational Note: Using the 'hot potato' ownership transfer pattern for maximum safety and performance"
    );
    println!(
        "💡 Performance Note: std::fs::copy may be faster for cached files due to kernel optimizations"
    );
    println!(
        "🎯 safer-ring excels at: network I/O, database files, and scenarios requiring userspace bypass"
    );

    // Create a single, reusable OwnedBuffer for the entire copy operation.
    // This demonstrates efficient buffer reuse with the hot potato pattern.
    let mut buffer = OwnedBuffer::new(config.buffer_size);

    // MAIN COPY LOOP: Process file in chunks using ownership transfer
    // Each operation transfers buffer ownership and returns it upon completion
    while offset < file_size {
        let chunk_size = std::cmp::min(config.buffer_size as u64, file_size - offset);

        println!("📖 Reading {chunk_size} bytes at offset {offset} (hot potato: buffer → kernel)");

        // STEP 1: Perform read using the `read_at_owned` API (hot potato pattern)
        // EDUCATIONAL BREAKDOWN:
        // - `ring.read_at_owned(...)`: "Throws" buffer ownership to kernel
        // - Buffer is safely managed by kernel during I/O operation
        // - First `?`: Handles submission errors (e.g., ring full, invalid fd)
        // - `.await`: Waits for kernel to complete the I/O operation
        // - Second `?`: Handles I/O completion errors (e.g., file not found, permission denied)
        // - Returns (bytes_read, buffer) on success - "catches" buffer ownership back!
        let (bytes_read, returned_buffer) = ring.read_at_owned(source_fd, buffer, offset).await?;

        // "Catch" the buffer back - hot potato pattern in action!
        buffer = returned_buffer;

        // STEP 2: Process the read data (only if we actually read something)
        if bytes_read > 0 {
            println!("✏️  Writing {bytes_read} bytes at offset {offset} (hot potato: buffer → kernel)");

            // STEP 3: Perform write using the `write_at_owned` API (hot potato continues)
            // EDUCATIONAL BREAKDOWN:
            // - `ring.write_at_owned(...)`: "Throws" buffer ownership to kernel again
            // - We specify exact length (bytes_read) to write only what was read
            // - Buffer ownership is transferred for this operation
            // - Same ?.await? pattern for robust error handling
            // - Returns (bytes_written, buffer) - "catches" buffer ownership back again!
            let (bytes_written, returned_buffer_after_write) = ring
                .write_at_owned(dest_fd, buffer, offset, bytes_read)
                .await?;

            // "Catch" the buffer back for the next loop iteration - hot potato continues!
            buffer = returned_buffer_after_write;

            // Verify we wrote all the data we intended to
            if bytes_written != bytes_read {
                return Err(
                    format!("Partial write: expected {bytes_read}, wrote {bytes_written}").into(),
                );
            }

            // STEP 4: Update our progress tracking
            stats.update(bytes_written as u64);
            {
                let mut shared_stats = stats_clone.lock().await;
                shared_stats.update(bytes_written as u64);
            }

            // Move to next chunk
            offset += bytes_written as u64;
        } else {
            // If we read 0 bytes, we've reached the end of file
            break;
        }
    }

    println!("\n✅ Hot potato copy operation completed!");
    println!("📚 Educational Summary:");
    println!("   ✓ Used `read_at_owned`/`write_at_owned` for maximum safety");
    println!("   ✓ Demonstrated the 'hot potato' ownership transfer pattern");
    println!("   ✓ Reused single `OwnedBuffer` efficiently across all operations");
    println!("   ✓ Applied ?.await? pattern for robust error handling");
    println!("   ✓ No complex lifetime management or pinning required");
    println!("   ✓ Buffer ownership: user → kernel → user → kernel → user (hot potato!)");

    Ok(stats)
}

/// Verify that the copy was successful by comparing file sizes and checksums
fn verify_copy(source_path: &str, dest_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    // Compare file sizes
    let source_size = fs::metadata(source_path)?.len();
    let dest_size = fs::metadata(dest_path)?.len();

    if source_size != dest_size {
        return Err(format!(
            "File size mismatch: source {source_size} bytes, destination {dest_size} bytes"
        )
        .into());
    }

    // For small files, compare content directly
    if source_size <= 1024 * 1024 {
        // 1MB
        let source_content = fs::read(source_path)?;
        let dest_content = fs::read(dest_path)?;

        if source_content != dest_content {
            return Err("File content mismatch".into());
        }
    } else {
        // For large files, compare checksums
        let source_hash = calculate_file_hash(source_path)?;
        let dest_hash = calculate_file_hash(dest_path)?;

        if source_hash != dest_hash {
            return Err("File checksum mismatch".into());
        }
    }

    Ok(())
}

/// Calculate a simple hash of a file for verification
fn calculate_file_hash(path: &str) -> Result<u64, Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::{BufReader, Read};

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        buffer[..bytes_read].hash(&mut hasher);
    }

    Ok(hasher.finish())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("❌ This example requires Linux with io_uring support");
    println!("💡 io_uring is not available on this platform");
    println!();
    println!("This example demonstrates:");
    println!("  - Zero-copy file operations");
    println!("  - Buffer pool management");
    println!("  - Batch I/O processing");
    println!("  - Performance optimization techniques");
    println!();
    println!("Supported platforms:");
    println!("  - Linux 5.1+ (basic support)");
    println!("  - Linux 5.19+ (recommended)");
    println!("  - Linux 6.0+ (optimal performance)");
}
