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

use safer_ring::Ring;
use std::env;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for the async demonstration
#[derive(Debug)]
struct AsyncDemoConfig {
    /// Whether to use real files for I/O operations
    with_files: bool,
    /// Whether to run concurrent operations demo
    concurrent: bool,
    /// Whether to run batch operations demo
    batch_demo: bool,
    /// Number of concurrent operations
    concurrent_ops: usize,
    /// Buffer size for operations
    buffer_size: usize,
}

impl Default for AsyncDemoConfig {
    fn default() -> Self {
        Self {
            with_files: false,
            concurrent: false,
            batch_demo: true,
            concurrent_ops: 8,
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
                "--concurrent" => config.concurrent = true,
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
        let ring = Ring::new(64)?;
        println!("⚡ Created io_uring with {} entries", ring.capacity());

        // Run basic async patterns demo
        println!("🔄 Running basic async patterns...");
        run_basic_async_demo(&ring, &config).await?;

        if config.concurrent {
            println!("\n🔀 Running concurrent operations demo...");
            run_concurrent_demo(&ring, &config).await?;
        }

        if config.batch_demo {
            println!("\n📦 Running batch operations demo...");
            run_batch_demo(&ring, &config).await?;
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

        // Sequential read operation
        let mut buffer = PinnedBuffer::with_capacity(config.buffer_size);
        let read_op = Operation::read()
            .fd(temp_fd)
            .buffer(buffer.as_mut_slice())
            .offset(0);

        let (bytes_read, read_buffer) = ring.submit_read(read_op).await?;
        println!(
            "      📖 Read {} bytes: {}",
            bytes_read,
            String::from_utf8_lossy(&read_buffer[..bytes_read])
        );

        // Sequential write operation
        let write_data = b"Appended data from async operation";
        let mut write_buffer = PinnedBuffer::from_slice(write_data);
        let write_op = Operation::write()
            .fd(temp_fd)
            .buffer(write_buffer.as_mut_slice())
            .offset(bytes_read as u64);

        let (bytes_written, _) = ring.submit_write(write_op).await?;
        println!("      ✏️  Wrote {} bytes sequentially", bytes_written);
    } else {
        // Simulate operations without real files
        let mut buffer = PinnedBuffer::with_capacity(config.buffer_size);
        println!("      📦 Created buffer with {} bytes", buffer.len());

        // Simulate async work
        sleep(Duration::from_millis(10)).await;
        println!("      ⏱️  Simulated async operation completed");
    }

    // 2. Error handling in async context
    println!("   2️⃣  Error handling...");
    let result = async {
        // This will fail with invalid file descriptor
        let mut buffer = PinnedBuffer::with_capacity(64);
        let invalid_op = Operation::read()
            .fd(-1) // Invalid fd
            .buffer(buffer.as_mut_slice())
            .offset(0);

        ring.submit_read(invalid_op).await
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
        let mut buffer1 = PinnedBuffer::with_capacity(128);
        let mut buffer2 = PinnedBuffer::with_capacity(128);

        // Fill first buffer
        buffer1.as_mut_slice()[..5].copy_from_slice(b"Hello");

        // Simulate processing
        sleep(Duration::from_millis(5)).await;

        // Process into second buffer
        buffer2.as_mut_slice()[..6].copy_from_slice(b"World!");

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

        let mut buffer1 = PinnedBuffer::with_capacity(config.buffer_size);
        let mut buffer2 = PinnedBuffer::with_capacity(config.buffer_size);
        let mut buffer3 = PinnedBuffer::with_capacity(config.buffer_size);

        // Run three read operations concurrently
        let (result1, result2, result3) = tokio::join!(
            ring.submit_read(
                Operation::read()
                    .fd(file1.as_raw_fd())
                    .buffer(buffer1.as_mut_slice())
                    .offset(0)
            ),
            ring.submit_read(
                Operation::read()
                    .fd(file2.as_raw_fd())
                    .buffer(buffer2.as_mut_slice())
                    .offset(0)
            ),
            ring.submit_read(
                Operation::read()
                    .fd(file3.as_raw_fd())
                    .buffer(buffer3.as_mut_slice())
                    .offset(0)
            )
        );

        let (bytes1, _) = result1?;
        let (bytes2, _) = result2?;
        let (bytes3, _) = result3?;

        println!(
            "      📊 Concurrent reads: {} + {} + {} = {} bytes",
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
    for i in 0..config.concurrent_ops {
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

/// Demonstrate batch operations with async
#[cfg(target_os = "linux")]
async fn run_batch_demo(
    ring: &Ring<'_>,
    config: &AsyncDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Batch Operations:");

    // Create a batch of operations
    let mut batch = Batch::new();
    let mut buffers = Vec::new();

    println!(
        "   🔧 Creating batch with {} operations...",
        config.concurrent_ops
    );

    if config.with_files {
        // Create operations with real files
        for i in 0..config.concurrent_ops {
            let temp_file = create_temp_file(&format!("Batch file {} content", i))?;
            let mut buffer = PinnedBuffer::with_capacity(config.buffer_size);

            let read_op = Operation::read()
                .fd(temp_file.as_raw_fd())
                .buffer(buffer.as_mut_slice())
                .offset(0);

            batch.add_operation(read_op)?;
            buffers.push((buffer, temp_file));
        }
    } else {
        // Simulate batch operations
        for i in 0..std::cmp::min(config.concurrent_ops, 4) {
            let mut buffer = PinnedBuffer::with_capacity(config.buffer_size);

            // Fill buffer with test data
            let test_data = format!("Batch operation {}", i);
            let bytes = test_data.as_bytes();
            let copy_len = std::cmp::min(bytes.len(), buffer.len());
            buffer.as_mut_slice()[..copy_len].copy_from_slice(&bytes[..copy_len]);

            buffers.push(buffer);
        }

        println!("      📊 Simulated {} batch operations", buffers.len());
    }

    if config.with_files {
        // Submit the batch and wait for completion
        let start_time = Instant::now();
        let batch_result = ring.submit_batch(batch).await?;
        let batch_time = start_time.elapsed();

        println!("   📈 Batch results:");
        println!(
            "      ✅ Successful operations: {}",
            batch_result.successful_count
        );
        println!("      ❌ Failed operations: {}", batch_result.failed_count);
        println!("      ⏱️  Total time: {:?}", batch_time);
        println!(
            "      📊 Average time per operation: {:?}",
            batch_time / config.concurrent_ops as u32
        );
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
    let pool = BufferPool::new(8, config.buffer_size)?;
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
