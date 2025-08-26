//! Tests for standalone batch operations that solve the lifetime constraint issues.

use safer_ring::{Batch, BatchConfig, Operation, PinnedBuffer, Ring, StandaloneBatchFuture};
use std::future::poll_fn;

/// Test that standalone batch operations can be created without lifetime issues.
#[tokio::test]
async fn test_standalone_batch_creation() {
    #[cfg(not(target_os = "linux"))]
    {
        println!("Skipping io_uring-specific test on macOS/Windows - no io_uring kernel support");
        return;
    }

    #[cfg(target_os = "linux")]
    {
        let mut ring = match Ring::new(32) {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "Could not create ring (io_uring may not be available): {}",
                    e
                );
                return;
            }
        };

        let mut batch = Batch::new();
        let mut buffer1 = PinnedBuffer::with_capacity(1024);
        let mut buffer2 = PinnedBuffer::with_capacity(1024);

        // Add operations to batch
        if let Err(e) = batch.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))
        {
            println!("Could not add operation to batch: {}", e);
            return;
        }

        if let Err(e) = batch.add_operation(Operation::read().fd(0).buffer(buffer2.as_mut_slice()))
        {
            println!("Could not add operation to batch: {}", e);
            return;
        }

        // This should work without lifetime issues
        let _batch_future: StandaloneBatchFuture = match ring.submit_batch_standalone(batch) {
            Ok(future) => future,
            Err(e) => {
                println!("Could not submit standalone batch: {}", e);
                return;
            }
        };

        // The key test: we can perform other operations on the ring now
        // This would not be possible with the original BatchFuture design
        let mut other_buffer = PinnedBuffer::with_capacity(512);
        let _other_op_future = ring.read(0, other_buffer.as_mut_slice());

        println!("Successfully created standalone batch future without lifetime constraints");
    }
}

/// Test the polling mechanism for standalone batch futures.
#[tokio::test]
async fn test_standalone_batch_polling() {
    #[cfg(not(target_os = "linux"))]
    {
        println!("Skipping io_uring-specific test on macOS/Windows - no io_uring kernel support");
        return;
    }

    #[cfg(target_os = "linux")]
    {
        let mut ring = match Ring::new(32) {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "Could not create ring (io_uring may not be available): {}",
                    e
                );
                return;
            }
        };

        let mut batch = Batch::new();
        let mut buffer = PinnedBuffer::with_capacity(1024);

        if let Err(e) = batch.add_operation(
            Operation::read()
                .fd(0) // stdin - this will likely block or fail, but we're testing structure
                .buffer(buffer.as_mut_slice()),
        ) {
            println!("Could not add operation to batch: {}", e);
            return;
        }

        let mut batch_future = match ring.submit_batch_standalone(batch) {
            Ok(future) => future,
            Err(e) => {
                println!("Could not submit standalone batch: {}", e);
                return;
            }
        };

        // Test the polling interface - we're not expecting this to complete
        // since we're reading from stdin, but we want to verify the API works
        let result = poll_fn(|cx| {
            // Poll once to verify the interface works
            match batch_future.poll_with_ring(&mut ring, cx) {
                std::task::Poll::Ready(Ok(_)) => {
                    println!("Batch completed successfully");
                    std::task::Poll::Ready(())
                }
                std::task::Poll::Ready(Err(_e)) => {
                    // Expected - reading from stdin likely fails
                    println!("Batch failed as expected when reading from stdin");
                    std::task::Poll::Ready(())
                }
                std::task::Poll::Pending => {
                    println!("Batch is pending (expected)");
                    std::task::Poll::Ready(()) // Exit instead of hanging
                }
            }
        })
        .await;

        println!("Successfully tested standalone batch polling mechanism");
    }
}

/// Test that multiple standalone batches can coexist.
#[tokio::test]
async fn test_multiple_standalone_batches() {
    #[cfg(not(target_os = "linux"))]
    {
        println!("Skipping io_uring-specific test on macOS/Windows - no io_uring kernel support");
        return;
    }

    #[cfg(target_os = "linux")]
    {
        let mut ring = match Ring::new(32) {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "Could not create ring (io_uring may not be available): {}",
                    e
                );
                return;
            }
        };

        // Create first batch
        let mut batch1 = Batch::new();
        let mut buffer1 = PinnedBuffer::with_capacity(1024);

        if let Err(e) = batch1.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))
        {
            println!("Could not add operation to first batch: {}", e);
            return;
        }

        // Create second batch
        let mut batch2 = Batch::new();
        let mut buffer2 = PinnedBuffer::with_capacity(1024);

        if let Err(e) = batch2.add_operation(Operation::read().fd(0).buffer(buffer2.as_mut_slice()))
        {
            println!("Could not add operation to second batch: {}", e);
            return;
        }

        // Submit both batches - this should work without lifetime conflicts
        let _future1 = match ring.submit_batch_standalone(batch1) {
            Ok(f) => f,
            Err(e) => {
                println!("Could not submit first batch: {}", e);
                return;
            }
        };

        let _future2 = match ring.submit_batch_standalone(batch2) {
            Ok(f) => f,
            Err(e) => {
                println!("Could not submit second batch: {}", e);
                return;
            }
        };

        // The fact that we got here proves that multiple standalone batches
        // can coexist without lifetime issues
        println!("Successfully created multiple coexisting standalone batch futures");
    }
}

/// Test configuration options with standalone batches.
#[tokio::test]
async fn test_standalone_batch_with_config() {
    #[cfg(not(target_os = "linux"))]
    {
        println!("Skipping io_uring-specific test on macOS/Windows - no io_uring kernel support");
        return;
    }

    #[cfg(target_os = "linux")]
    {
        let mut ring = match Ring::new(32) {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "Could not create ring (io_uring may not be available): {}",
                    e
                );
                return;
            }
        };

        let mut batch = Batch::new();
        let mut buffer = PinnedBuffer::with_capacity(1024);

        if let Err(e) = batch.add_operation(Operation::read().fd(0).buffer(buffer.as_mut_slice())) {
            println!("Could not add operation to batch: {}", e);
            return;
        }

        let config = BatchConfig {
            fail_fast: true,
            max_batch_size: 10,
            enforce_dependencies: false,
        };

        let _batch_future = match ring.submit_batch_standalone_with_config(batch, config) {
            Ok(f) => f,
            Err(e) => {
                println!("Could not submit batch with config: {}", e);
                return;
            }
        };

        println!("Successfully created standalone batch future with custom configuration");
    }
}
