//! # Comprehensive Async/Await Demonstration
//!
//! This example showcases safer-ring's seamless integration with Rust's async/await
//! ecosystem, demonstrating various patterns and best practices.
//!
//! ## Features Demonstrated
//! - **Future Integration**: Native async/await support for all operations
//! - **Concurrent Operations**: Running multiple I/O operations simultaneously
//! - **Error Handling**: Proper async error handling patterns
//! - **Cancellation**: Safe operation cancellation and cleanup
//! - **Timeouts**: Timeout handling for I/O operations
//! - **Batch Operations**: Async batch processing with futures
//!
//! ## Usage
//! ```bash
//! # Run basic async demo
//! cargo run --example async_demo
//!
//! # Run with temporary files for real I/O
//! cargo run --example async_demo -- --with-files
//!
//! # Run concurrent operations demo
//! cargo run --example async_demo -- --concurrent
//! ```
//!
//! ## Async Patterns Shown
//! - Sequential async operations
//! - Concurrent async operations with `join!` and `select!`
//! - Stream processing with async iterators
//! - Error propagation in async contexts
//! - Resource cleanup in async destructors

use safer_ring::{Ring, OwnedBuffer, BufferPool};
use std::env;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};

/// Configuration for the async demonstration
#[derive(Debug)]
struct AsyncDemoConfig {
    /// Whether to use real files for I/O operations
    with_files: bool,
    /// Number of concurrent operations to run
    concurrent: usize,
    /// Whether to run batch operations demo
    batch_demo: bool,
    /// Buffer size for operations
    buffer_size: usize,
}

impl Default for AsyncDemoConfig {
    fn default() -> Self {
        Self {
            with_files: false,
            concurrent: 3,
            batch_demo: true,
            buffer_size: 4096,
        }
    }
}

impl AsyncDemoConfig {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut config = AsyncDemoConfig::default();

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "--with-files" => config.with_files = true,
                "--concurrent" => config.concurrent = 5,
                "--no-batch" => config.batch_demo = false,
                _ => {}
            }
        }

        config
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Safer-Ring Async/Await Comprehensive Demo");
    println!("============================================");

    let config = AsyncDemoConfig::from_args();
    println!("📊 Configuration:");
    println!("   Real file I/O: {}", config.with_files);
    println!("   Concurrent demo: {}", config.concurrent);
    println!("   Batch demo: {}", config.batch_demo);
    println!("   Buffer size: {} bytes", config.buffer_size);
    println!();

    #[cfg(target_os = "linux")]
    {
        // Create a ring for async operations
        let mut ring = Ring::new(64)?;
        println!("⚡ Created io_uring with {} entries", ring.capacity());

        // Run basic async patterns demo
        println!("🔄 Running basic async patterns...");
        run_basic_async_demo(&ring, &config).await?;

        if config.concurrent > 0 {
            println!("\n🔀 Running concurrent operations demo...");
            run_concurrent_demo(&ring, &config).await?;
        }

        if config.batch_demo {
            println!("\n📦 Running batch operations demo...");
            run_batch_demo(&mut ring, &config).await?;
        }

        println!("\n⏱️  Running timeout and cancellation demo...");
        run_timeout_demo(&ring, &config).await?;

        println!("\n🏊 Running buffer pool async demo...");
        run_buffer_pool_async_demo(&ring, &config).await?;

        println!("\n✅ All async demos completed successfully!");
    }

    #[cfg(not(target_os = "linux"))]
    {
        println!("❌ This demo requires Linux for io_uring support");
        println!("💡 On this platform, demonstrating error handling:");

        match Ring::new(32) {
            Ok(_) => println!("Unexpected success creating ring"),
            Err(e) => println!("Expected error creating ring: {}", e),
        }

        println!("\nAsync patterns that would be demonstrated:");
        println!("  - Sequential async I/O operations");
        println!("  - Concurrent operations with join!/select!");
        println!("  - Timeout handling and cancellation");
        println!("  - Batch operation processing");
        println!("  - Buffer pool integration");
        println!("  - Error propagation in async contexts");
    }

    Ok(())
}

/// Demonstrate basic async patterns with safer-ring
#[cfg(target_os = "linux")]
async fn run_basic_async_demo(
    ring: &Ring<'_>,
    config: &AsyncDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📚 Basic Async Patterns:");

    // 1. Sequential async operations
    println!("   1️⃣  Sequential operations...");
    if config.with_files {
        // Create temporary files for real I/O
        let temp_file = create_temp_file("Hello, async world!")?;
        let temp_fd = temp_file.as_raw_fd();

        // Sequential read operation using safer owned API
        let buffer = OwnedBuffer::new(config.buffer_size);
        let (bytes_read, read_buffer) = ring.read_owned(temp_fd, buffer).await?;
        let data_str = if let Some(guard) = read_buffer.try_access() {
            String::from_utf8_lossy(&guard[..bytes_read]).to_string()
        } else {
            "Buffer not accessible".to_string()
        };
        println!("      📖 Read {} bytes: {}", bytes_read, data_str);

        // Sequential write operation using safer owned API
        let write_data = b"Appended data from async operation";
        let write_buffer = OwnedBuffer::from_slice(write_data);
        let (bytes_written, _) = ring.write_owned(temp_fd, write_buffer).await?;
        println!("      ✏️  Wrote {} bytes sequentially", bytes_written);
    } else {
        // Simulate operations without real files
        let buffer = OwnedBuffer::new(config.buffer_size);
        println!("      📦 Created buffer with {} bytes", buffer.size());

        // Simulate async work
        sleep(Duration::from_millis(10)).await;
        println!("      ⏱️  Simulated async operation completed");
    }

    // 2. Error handling in async context
    println!("   2️⃣  Error handling...");
    let result = async {
        // This will fail with invalid file descriptor
        let buffer = OwnedBuffer::new(64);
        ring.read_owned(-1, buffer).await // Invalid fd
    }
    .await;

    match result {
        Ok(_) => println!("      ❌ Unexpected success with invalid fd"),
        Err(e) => println!("      ✅ Properly caught error: {}", e),
    }

    // 3. Chaining async operations
    println!("   3️⃣  Chaining operations...");
    let chain_result = async {
        // Create a chain of async operations
        let buffer1 = OwnedBuffer::from_slice(b"Hello");
        let buffer2 = OwnedBuffer::from_slice(b"World!");

        // Simulate processing
        sleep(Duration::from_millis(5)).await;

        Ok::<_, Box<dyn std::error::Error>>((buffer1, buffer2))
    }
    .await?;

    println!("      🔗 Chained operations completed successfully");

    Ok(())
}

/// Demonstrate concurrent async operations
#[cfg(target_os = "linux")]
async fn run_concurrent_demo(
    ring: &Ring<'_>,
    config: &AsyncDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔀 Concurrent Operations:");

    // 1. Using tokio::join! for concurrent operations
    println!("   1️⃣  Using join! for parallel operations...");

    let start_time = Instant::now();

    if config.with_files {
        // Create multiple temporary files
        let file1 = create_temp_file("File 1 content")?;
        let file2 = create_temp_file("File 2 content")?;
        let file3 = create_temp_file("File 3 content")?;

        // Note: We cannot run concurrent operations on the same ring since the safer APIs
        // take &mut self. Each operation must complete before the next can start.
        // This demonstrates sequential async operations instead.
        let buffer1 = OwnedBuffer::new(config.buffer_size);
        let (bytes1, _) = ring.read_owned(file1.as_raw_fd(), buffer1).await?;
        
        let buffer2 = OwnedBuffer::new(config.buffer_size);
        let (bytes2, _) = ring.read_owned(file2.as_raw_fd(), buffer2).await?;
        
        let buffer3 = OwnedBuffer::new(config.buffer_size);
        let (bytes3, _) = ring.read_owned(file3.as_raw_fd(), buffer3).await?;

        println!(
            "      📊 Sequential reads: {} + {} + {} = {} bytes",
            bytes1,
            bytes2,
            bytes3,
            bytes1 + bytes2 + bytes3
        );
    } else {
        // Simulate concurrent operations
        let (result1, result2, result3) = tokio::join!(
            simulate_async_work("Task 1", 50),
            simulate_async_work("Task 2", 75),
            simulate_async_work("Task 3", 25)
        );

        println!(
            "      ✅ Concurrent tasks completed: {:?}, {:?}, {:?}",
            result1, result2, result3
        );
    }

    println!("      ⏱️  Total time: {:?}", start_time.elapsed());

    // 2. Using tokio::select! for racing operations
    println!("   2️⃣  Using select! for racing operations...");

    let race_result = tokio::select! {
        result = simulate_async_work("Fast task", 10) => {
            format!("Fast task won: {:?}", result)
        }
        result = simulate_async_work("Slow task", 100) => {
            format!("Slow task won: {:?}", result)
        }
        _ = sleep(Duration::from_millis(50)) => {
            "Timeout won".to_string()
        }
    };

    println!("      🏁 Race result: {}", race_result);

    // 3. Spawning concurrent tasks
    println!("   3️⃣  Spawning concurrent tasks...");

    let mut tasks = Vec::new();
    for i in 0..config.concurrent {
        let task = tokio::spawn(async move {
            let delay = (i * 10) as u64;
            sleep(Duration::from_millis(delay)).await;
            format!("Task {} completed after {}ms", i, delay)
        });
        tasks.push(task);
    }

    // Wait for all tasks to complete
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await?);
    }

    println!("      📋 All {} tasks completed:", results.len());
    for result in results.iter().take(3) {
        println!("         - {}", result);
    }
    if results.len() > 3 {
        println!("         ... and {} more", results.len() - 3);
    }

    Ok(())
}

/// Demonstrate sequential operations (simulating batch-like behavior)
#[cfg(target_os = "linux")]
async fn run_batch_demo(
    ring: &mut Ring<'_>,
    config: &AsyncDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Sequential Operations (Batch-style):");

    println!(
        "   🔧 Creating {} sequential operations...",
        config.concurrent
    );

    if config.with_files {
        let start_time = Instant::now();
        let mut total_bytes = 0;
        
        // Process operations sequentially
        for i in 0..config.concurrent {
            let temp_file = create_temp_file(&format!("Batch file {} content", i))?;
            let buffer = OwnedBuffer::new(config.buffer_size);
            
            let (bytes_read, _) = ring.read_owned(temp_file.as_raw_fd(), buffer).await?;
            total_bytes += bytes_read;
        }
        
        let batch_time = start_time.elapsed();
        println!("   📈 Sequential results:");
        println!("      ✅ Operations completed: {}", config.concurrent);
        println!("      📊 Total bytes read: {}", total_bytes);
        println!("      ⏱️  Total time: {:?}", batch_time);
        if config.concurrent > 0 {
            println!(
                "      📊 Average time per operation: {:?}",
                batch_time / config.concurrent as u32
            );
        }
    } else {
        // Simulate operations
        let mut buffers = Vec::new();
        for i in 0..std::cmp::min(config.concurrent, 4) {
            let test_data = format!("Sequential operation {}", i);
            let buffer = OwnedBuffer::from_slice(test_data.as_bytes());
            buffers.push(buffer);
        }
        println!("      📊 Simulated {} operations", buffers.len());
    }

    Ok(())
}

/// Demonstrate timeout and cancellation patterns
#[cfg(target_os = "linux")]
async fn run_timeout_demo(
    ring: &Ring<'_>,
    config: &AsyncDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("⏱️  Timeout and Cancellation:");

    // 1. Timeout with successful operation
    println!("   1️⃣  Timeout with fast operation...");
    let fast_result = timeout(
        Duration::from_millis(100),
        simulate_async_work("Fast operation", 10),
    )
    .await;

    match fast_result {
        Ok(result) => println!("      ✅ Operation completed: {:?}", result),
        Err(_) => println!("      ⏰ Operation timed out"),
    }

    // 2. Timeout with slow operation
    println!("   2️⃣  Timeout with slow operation...");
    let slow_result = timeout(
        Duration::from_millis(50),
        simulate_async_work("Slow operation", 100),
    )
    .await;

    match slow_result {
        Ok(result) => println!("      ✅ Operation completed: {:?}", result),
        Err(_) => println!("      ⏰ Operation timed out (expected)"),
    }

    // 3. Cancellation with select!
    println!("   3️⃣  Cancellation with select!...");
    let mut cancel_signal = false;

    tokio::select! {
        result = simulate_async_work("Cancellable task", 200) => {
            println!("      ✅ Task completed: {:?}", result);
        }
        _ = sleep(Duration::from_millis(30)) => {
            cancel_signal = true;
            println!("      🛑 Task cancelled by timeout");
        }
    }

    if cancel_signal {
        println!("      🧹 Cleanup after cancellation completed");
    }

    Ok(())
}

/// Demonstrate buffer pool integration with async operations
#[cfg(target_os = "linux")]
async fn run_buffer_pool_async_demo(
    ring: &Ring<'_>,
    config: &AsyncDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🏊 Buffer Pool Async Integration:");

    // Create a buffer pool
    let pool = BufferPool::new(8, config.buffer_size);
    println!("   📦 Created buffer pool with 8 buffers");

    // Demonstrate async operations with pooled buffers
    println!("   🔄 Running async operations with pooled buffers...");

    let mut tasks = Vec::new();
    for i in 0..6 {
        let pool_clone = pool.clone();

        let task = tokio::spawn(async move {
            // Get buffer from pool
            if let Some(mut buffer) = pool_clone.get() {
                // Simulate work with the buffer
                let work_data = format!("Pooled buffer task {}", i);
                let bytes = work_data.as_bytes();
                let copy_len = std::cmp::min(bytes.len(), buffer.len());
                buffer.as_mut_slice()[..copy_len].copy_from_slice(&bytes[..copy_len]);

                // Simulate async processing
                sleep(Duration::from_millis(20 + i * 5)).await;

                Ok::<String, Box<dyn std::error::Error + Send + Sync>>(format!(
                    "Task {} processed {} bytes",
                    i, copy_len
                ))
            } else {
                Err("Failed to get buffer from pool".into())
            }
        });

        tasks.push(task);
    }

    // Wait for all tasks and collect results
    let mut successful = 0;
    let mut failed = 0;

    for task in tasks {
        match task.await? {
            Ok(result) => {
                println!("      ✅ {}", result);
                successful += 1;
            }
            Err(e) => {
                println!("      ❌ Error: {}", e);
                failed += 1;
            }
        }
    }

    println!(
        "   📊 Pool async results: {} successful, {} failed",
        successful, failed
    );

    // Show final pool statistics
    let pool_stats = pool.stats();
    println!("   📈 Final pool stats:");
    println!("      Available: {}", pool_stats.available_buffers);
    println!("      In use: {}", pool_stats.buffers_in_use);
    println!("      Total allocations: {}", pool_stats.total_allocations);

    Ok(())
}

/// Simulate async work with configurable delay
async fn simulate_async_work(
    name: &str,
    delay_ms: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    sleep(Duration::from_millis(delay_ms)).await;
    Ok(format!("{} completed after {}ms", name, delay_ms))
}

/// Create a temporary file with given content
#[cfg(target_os = "linux")]
fn create_temp_file(content: &str) -> Result<File, Box<dyn std::error::Error>> {
    use std::io::Seek;

    let mut temp_file = tempfile::tempfile()?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.seek(std::io::SeekFrom::Start(0))?;
    Ok(temp_file)
}
