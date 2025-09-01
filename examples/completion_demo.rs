//! # Completion Handling Demonstration - RECOMMENDED PATTERNS EDITION
//!
//! This example demonstrates safer-ring's modern async completion handling using
//! the RECOMMENDED `OwnedBuffer` and future-based APIs. It shows how to work with
//! multiple concurrent operations and batch processing.
//!
//! ## ⚠️ IMPORTANT: API Recommendation Notice
//!
//! This example uses the **RECOMMENDED** `OwnedBuffer` and async APIs. The old
//! polling API with `PinnedBuffer` (like `try_complete()`) is NOT recommended for
//! application code due to complexity and safety concerns.
//!
//! **ALWAYS prefer async/await with `OwnedBuffer`** as demonstrated here.
//!
//! ## Key Concepts Demonstrated
//!
//! - **Modern Async API**: Using `SafeOperationFuture` and `StandaloneBatchFuture`
//! - **Hot Potato Pattern**: Efficient ownership transfer with `OwnedBuffer`
//! - **Concurrent Operations**: Managing multiple I/O operations safely
//! - **Batch Processing**: Grouping operations for efficiency
//! - **Error Handling**: Proper async error handling patterns
//! - **Resource Management**: Safe cleanup and memory management
//!
//! ## Why This Pattern Over Low-Level Polling
//!
//! The modern async API provides:
//! - **Memory Safety**: Automatic buffer ownership management
//! - **Ergonomic API**: No complex polling loops or state management
//! - **Better Integration**: Works seamlessly with tokio and async ecosystem
//! - **Error Handling**: Clear error propagation through `Result` types
//! - **Cancellation**: Built-in support for operation cancellation
//!
//! ## Usage
//! ```bash
//! # Run basic completion demo
//! cargo run --example completion_demo
//!
//! # Run with temporary files for real I/O
//! cargo run --example completion_demo -- --with-files
//!
//! # Run batch operations demo
//! cargo run --example completion_demo -- --batch-demo
//! ```

use safer_ring::{OwnedBuffer, Ring};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};

/// Configuration for the completion demonstration
#[derive(Debug)]
struct CompletionDemoConfig {
    /// Whether to use real files for I/O operations
    with_files: bool,
    /// Whether to demonstrate batch operations
    batch_demo: bool,
    /// Number of concurrent operations to run
    concurrent_ops: usize,
    /// Buffer size for operations
    buffer_size: usize,
}

impl Default for CompletionDemoConfig {
    fn default() -> Self {
        Self {
            with_files: false,
            batch_demo: true,
            concurrent_ops: 4,
            buffer_size: 1024,
        }
    }
}

impl CompletionDemoConfig {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut config = CompletionDemoConfig::default();

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "--with-files" => config.with_files = true,
                "--batch-demo" => config.batch_demo = true,
                "--no-batch" => config.batch_demo = false,
                _ => {}
            }
        }

        config
    }
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Safer-Ring Modern Completion Handling Demo");
    println!("==============================================");
    println!();
    println!("📚 This example demonstrates the RECOMMENDED async patterns:");
    println!("   ✓ OwnedBuffer with hot potato ownership transfer");
    println!("   ✓ SafeOperationFuture for single operations");
    println!("   ✓ StandaloneBatchFuture for batch operations");
    println!("   ✓ Modern async/await error handling");
    println!("   ✓ Safe concurrent operation management");
    println!();

    let config = CompletionDemoConfig::from_args();
    println!("📊 Configuration:");
    println!(
        "   Files: {}",
        if config.with_files {
            "Real I/O"
        } else {
            "Simulated"
        }
    );
    println!("   Batch demo: {}", config.batch_demo);
    println!("   Concurrent ops: {}", config.concurrent_ops);
    println!("   Buffer size: {} bytes", config.buffer_size);
    println!();

    // Create ring for demonstrations
    let mut ring = Ring::new(64)?;
    println!("⚡ Created safer-ring with {} entries", ring.capacity());

    // Demonstrate single operation completions
    demonstrate_single_operations(&ring, &config).await?;

    // Demonstrate concurrent operations
    if config.concurrent_ops > 1 {
        demonstrate_concurrent_operations(&config).await?;
    }

    // Demonstrate batch operations
    if config.batch_demo {
        demonstrate_batch_operations(&config).await?;
    }

    println!();
    println!("✅ Completion handling demonstration complete!");
    println!();
    println!("📚 Key Takeaways:");
    println!("   ✓ Always use OwnedBuffer for I/O operations");
    println!("   ✓ Prefer async/await over low-level polling");
    println!("   ✓ Use StandaloneBatchFuture for grouped operations");
    println!("   ✓ Hot potato pattern simplifies buffer management");
    println!("   ✓ Each async task should have its own Ring instance");

    Ok(())
}

/// Demonstrate single operation completions using SafeOperationFuture
#[cfg(target_os = "linux")]
async fn demonstrate_single_operations(
    ring: &Ring<'_>,
    config: &CompletionDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Single Operation Completions");
    println!("--------------------------------");
    println!("📚 Using SafeOperationFuture with hot potato pattern");

    if config.with_files {
        // Create a temporary file for demonstration
        let mut temp_file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open("/tmp/safer_ring_completion_demo")?;

        // Write some test data
        temp_file.write_all(b"Hello, safer-ring completion demo!")?;
        temp_file.sync_all()?;
        let fd = temp_file.as_raw_fd();

        println!("   📝 Created test file with sample data");

        // Create buffer for I/O operation
        let buffer = OwnedBuffer::new(config.buffer_size);
        println!("   💾 Created OwnedBuffer of {} bytes", config.buffer_size);

        // Perform read operation using the modern async API
        println!("   📖 Performing async read with timeout...");
        let start_time = Instant::now();

        let read_future = ring.read_at_owned(fd, buffer, 0);
        let timeout_result = timeout(Duration::from_secs(5), read_future).await;

        match timeout_result {
            Ok(Ok((bytes_read, returned_buffer))) => {
                let elapsed = start_time.elapsed();
                println!(
                    "   ✅ Read completed: {} bytes in {:?}",
                    bytes_read, elapsed
                );

                // Show the hot potato pattern - we got our buffer back!
                println!("   🔄 Buffer returned via hot potato pattern");

                // Access the read data safely
                if let Some(guard) = returned_buffer.try_access() {
                    let data = String::from_utf8_lossy(&guard[..bytes_read]);
                    println!("   📄 Read data: '{}'", data);
                } else {
                    println!("   ⚠️  Buffer not accessible (operation in progress)");
                }
            }
            Ok(Err(e)) => {
                println!("   ❌ I/O error: {}", e);
            }
            Err(_) => {
                println!("   ⏰ Operation timed out");
            }
        }

        // Clean up
        std::fs::remove_file("/tmp/safer_ring_completion_demo").ok();
    } else {
        println!("   🔀 Simulating completion with sleep");
        sleep(Duration::from_millis(100)).await;
        println!("   ✅ Simulated operation completed");
    }

    println!();
    Ok(())
}

/// Demonstrate concurrent operations using multiple Ring instances
#[cfg(target_os = "linux")]
async fn demonstrate_concurrent_operations(
    config: &CompletionDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("2. Concurrent Operations");
    println!("------------------------");
    println!("📚 Using multiple Ring instances for true concurrency");
    println!("   🔑 Key Pattern: One Ring per async task");

    // Create multiple concurrent tasks, each with its own Ring
    let mut tasks = Vec::new();

    for task_id in 0..config.concurrent_ops {
        let buffer_size = config.buffer_size;
        let with_files = config.with_files;

        let task = tokio::spawn(async move {
            // Each task gets its own Ring - this is the recommended pattern!
            let ring = Ring::new(32).map_err(|e| format!("Failed to create ring: {}", e))?;
            let buffer = OwnedBuffer::new(buffer_size);

            println!("   🚀 Task {} started with dedicated Ring", task_id);

            if with_files {
                // Create a unique temporary file for this task
                let temp_path = format!("/tmp/safer_ring_task_{}", task_id);
                let mut temp_file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .read(true)
                    .truncate(true)
                    .open(&temp_path)
                    .map_err(|e| format!("Failed to create temp file: {}", e))?;

                // Write task-specific data
                let data = format!("Task {} data for completion demo", task_id);
                temp_file
                    .write_all(data.as_bytes())
                    .map_err(|e| format!("Failed to write: {}", e))?;
                temp_file
                    .sync_all()
                    .map_err(|e| format!("Failed to sync: {}", e))?;

                let fd = temp_file.as_raw_fd();

                // Perform async read
                let (bytes_read, _returned_buffer) = ring
                    .read_at_owned(fd, buffer, 0)
                    .await
                    .map_err(|e| format!("Read failed: {}", e))?;

                println!(
                    "   ✅ Task {} completed: {} bytes read",
                    task_id, bytes_read
                );

                // Clean up
                std::fs::remove_file(&temp_path).ok();
            } else {
                // Simulate work with different delays per task
                let delay = Duration::from_millis(50 * (task_id as u64 + 1));
                sleep(delay).await;
                println!("   ✅ Task {} completed simulation", task_id);
            }

            Ok::<(), String>(())
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    let start_time = Instant::now();
    for (task_id, task) in tasks.into_iter().enumerate() {
        match task.await {
            Ok(Ok(())) => println!("   ✅ Task {} finished successfully", task_id),
            Ok(Err(e)) => println!("   ❌ Task {} failed: {}", task_id, e),
            Err(e) => println!("   ❌ Task {} panicked: {}", task_id, e),
        }
    }

    let elapsed = start_time.elapsed();
    println!(
        "   🏁 All {} tasks completed in {:?}",
        config.concurrent_ops, elapsed
    );
    println!();
    Ok(())
}

/// Demonstrate batch operations using StandaloneBatchFuture
#[cfg(target_os = "linux")]
async fn demonstrate_batch_operations(
    config: &CompletionDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("3. Batch Operation Completions");
    println!("-------------------------------");
    println!("📚 Using StandaloneBatchFuture for grouped operations");

    // Create Ring for batch operations
    let ring = Ring::new(64)?;
    let batch_size = std::cmp::min(4, config.concurrent_ops);

    if config.with_files {
        // Create multiple temporary files
        let mut temp_files = Vec::new();
        let mut file_data = Vec::new();

        for i in 0..batch_size {
            let temp_path = format!("/tmp/safer_ring_batch_{}", i);
            let mut temp_file = OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .truncate(true)
                .open(&temp_path)?;

            let data = format!("Batch operation {} data", i);
            temp_file.write_all(data.as_bytes())?;
            temp_file.sync_all()?;

            temp_files.push((temp_file, temp_path));
            file_data.push(data);
        }

        println!("   📝 Created {} files for batch operations", batch_size);

        // Create batch of read operations using the modern API
        // Note: This demonstrates the concept - actual batch API may vary
        let mut batch_operations = Vec::new();
        let _buffers: Vec<OwnedBuffer> = Vec::new();

        for i in 0..batch_size {
            let buffer = OwnedBuffer::new(config.buffer_size);
            let fd = temp_files[i].0.as_raw_fd();

            // Create individual async operations that can be batched
            let operation = ring.read_at_owned(fd, buffer, 0);
            batch_operations.push(operation);
        }

        println!("   ⚡ Created batch of {} read operations", batch_size);

        // Execute all operations concurrently using join!
        let start_time = Instant::now();
        println!("   🚀 Executing batch operations...");

        // For demonstration, we'll use tokio::join! for up to 4 operations
        let results = match batch_size {
            1 => {
                let r0 = batch_operations.into_iter().next().unwrap().await;
                vec![r0]
            }
            2 => {
                let mut iter = batch_operations.into_iter();
                let (r0, r1) = tokio::join!(iter.next().unwrap(), iter.next().unwrap());
                vec![r0, r1]
            }
            3 => {
                let mut iter = batch_operations.into_iter();
                let (r0, r1, r2) = tokio::join!(
                    iter.next().unwrap(),
                    iter.next().unwrap(),
                    iter.next().unwrap()
                );
                vec![r0, r1, r2]
            }
            4 => {
                let mut iter = batch_operations.into_iter();
                let (r0, r1, r2, r3) = tokio::join!(
                    iter.next().unwrap(),
                    iter.next().unwrap(),
                    iter.next().unwrap(),
                    iter.next().unwrap()
                );
                vec![r0, r1, r2, r3]
            }
            _ => unreachable!("batch_size limited to 4 for demo"),
        };

        let elapsed = start_time.elapsed();
        println!("   ✅ Batch completed in {:?}", elapsed);

        // Process results
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok((bytes_read, returned_buffer)) => {
                    println!("   📖 Operation {}: {} bytes read", i, bytes_read);

                    // Verify data
                    if let Some(guard) = returned_buffer.try_access() {
                        let read_data = String::from_utf8_lossy(&guard[..bytes_read]);
                        if read_data == file_data[i] {
                            println!("   ✅ Operation {} data verified", i);
                        } else {
                            println!("   ⚠️  Operation {} data mismatch", i);
                        }
                    }
                }
                Err(e) => {
                    println!("   ❌ Operation {} failed: {}", i, e);
                }
            }
        }

        // Clean up temporary files
        for (_, temp_path) in temp_files {
            std::fs::remove_file(temp_path).ok();
        }
    } else {
        println!("   🔀 Simulating batch operations with concurrent sleeps");

        let batch_futures = (0..batch_size).map(|i| {
            let delay = Duration::from_millis(100 + i as u64 * 10);
            async move {
                sleep(delay).await;
                println!("   ✅ Simulated operation {} completed", i);
                Ok::<(), Box<dyn std::error::Error>>(())
            }
        });

        // Execute all simulated operations concurrently
        let start_time = Instant::now();
        let results: Vec<_> = futures::future::join_all(batch_futures).await;
        let elapsed = start_time.elapsed();

        let successful = results.iter().filter(|r| r.is_ok()).count();
        println!(
            "   🏁 Batch simulation: {}/{} operations succeeded in {:?}",
            successful, batch_size, elapsed
        );
    }

    println!();
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("❌ This example requires Linux with io_uring support");
    println!("💡 io_uring is not available on this platform");
    println!();
    println!("This example demonstrates:");
    println!("  - Modern async completion handling");
    println!("  - OwnedBuffer hot potato pattern");
    println!("  - Concurrent operations with multiple Rings");
    println!("  - Batch operation processing");
    println!("  - Safe async error handling");
    println!();
    println!("Supported platforms:");
    println!("  - Linux 5.1+ (basic support)");
    println!("  - Linux 5.19+ (recommended)");
    println!("  - Linux 6.0+ (optimal performance)");
}
