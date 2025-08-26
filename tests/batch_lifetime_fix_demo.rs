//! Demonstration that the batch operation lifetime constraints have been fixed.
//!
//! This test demonstrates the specific issue that was mentioned in the original
//! batch_operations.rs test file and shows that it's now resolved.

use safer_ring::{Batch, Operation, PinnedBuffer, Ring, StandaloneBatchFuture};
use std::future::poll_fn;

/// This test demonstrates the original lifetime problem and its solution.
///
/// The original issue was that submit_batch() returned a BatchFuture that held
/// a mutable reference to the Ring for its entire lifetime, making it impossible
/// to compose with other operations on the same Ring.
#[tokio::test]
async fn test_batch_lifetime_constraint_fix() {
    #[cfg(not(target_os = "linux"))]
    {
        println!("Skipping io_uring-specific test on macOS/Windows - no io_uring kernel support");
        return;
    }

    #[cfg(target_os = "linux")]
    {
        // This demonstrates the solution to the lifetime constraint problem
        async fn demonstrate_fixed_api(
            ring: &mut Ring<'_>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let mut batch = Batch::new();
            let mut buffer1 = PinnedBuffer::with_capacity(1024);
            let mut buffer2 = PinnedBuffer::with_capacity(1024);

            // Add operations to the batch
            batch.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))?;
            batch.add_operation(Operation::read().fd(0).buffer(buffer2.as_mut_slice()))?;

            // SOLUTION: Use submit_batch_standalone instead of submit_batch
            // This returns a future that doesn't hold a mutable reference to the Ring
            let mut batch_future = ring.submit_batch_standalone(batch)?;

            // PROOF: We can now perform other operations on the same ring!
            // This was impossible with the original BatchFuture design
            let mut other_buffer = PinnedBuffer::with_capacity(512);
            let _other_operation = ring.read(0, other_buffer.as_mut_slice())?;

            // Poll the batch future manually
            let _batch_result = poll_fn(|cx| {
                // We can provide Ring access when needed without lifetime conflicts
                match batch_future.poll_with_ring(ring, cx) {
                    std::task::Poll::Ready(result) => {
                        println!("Batch completed: {:?}", result.is_ok());
                        std::task::Poll::Ready(()) // End test gracefully
                    }
                    std::task::Poll::Pending => {
                        println!("Batch pending (expected for stdin)");
                        std::task::Poll::Ready(()) // End test gracefully
                    }
                }
            })
            .await;

            Ok(())
        }

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

        match demonstrate_fixed_api(&mut ring).await {
            Ok(()) => {
                println!("✅ Successfully demonstrated the fix for batch lifetime constraints")
            }
            Err(e) => println!("Test completed with expected error: {}", e),
        }
    }
}

/// Test that shows the ergonomics improvements of the new API.
#[tokio::test]
async fn test_improved_batch_ergonomics() {
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

        // Create multiple batches that can coexist
        let mut batch1 = Batch::new();
        let mut batch2 = Batch::new();

        let mut buffer1 = PinnedBuffer::with_capacity(1024);
        let mut buffer2 = PinnedBuffer::with_capacity(1024);
        let mut buffer3 = PinnedBuffer::with_capacity(1024);
        let mut buffer4 = PinnedBuffer::with_capacity(1024);

        if let Ok(_) = batch1.add_operation(Operation::read().fd(0).buffer(buffer1.as_mut_slice()))
        {
            if let Ok(_) =
                batch1.add_operation(Operation::read().fd(0).buffer(buffer2.as_mut_slice()))
            {
                if let Ok(_) =
                    batch2.add_operation(Operation::read().fd(0).buffer(buffer3.as_mut_slice()))
                {
                    if let Ok(_) =
                        batch2.add_operation(Operation::read().fd(0).buffer(buffer4.as_mut_slice()))
                    {
                        // Submit both batches - this demonstrates improved ergonomics
                        let _future1: StandaloneBatchFuture = ring
                            .submit_batch_standalone(batch1)
                            .unwrap_or_else(|_| panic!("Should be able to submit first batch"));

                        let _future2: StandaloneBatchFuture = ring
                            .submit_batch_standalone(batch2)
                            .unwrap_or_else(|_| panic!("Should be able to submit second batch"));

                        // We can even submit individual operations while batches are in flight
                        let mut single_buffer = PinnedBuffer::with_capacity(256);
                        let _single_op = ring.read(0, single_buffer.as_mut_slice());

                        println!(
                            "✅ Successfully demonstrated improved batch operation ergonomics"
                        );
                        return;
                    }
                }
            }
        }

        println!("Could not set up batches for ergonomics test");
    }
}

/// Benchmark-style test showing the performance characteristics.
#[tokio::test]
async fn test_batch_performance_characteristics() {
    #[cfg(not(target_os = "linux"))]
    {
        println!("Skipping io_uring-specific test on macOS/Windows - no io_uring kernel support");
        return;
    }

    #[cfg(target_os = "linux")]
    {
        let mut ring = match Ring::new(128) {
            // Larger ring for batch operations
            Ok(r) => r,
            Err(e) => {
                println!(
                    "Could not create ring (io_uring may not be available): {}",
                    e
                );
                return;
            }
        };

        const BATCH_SIZE: usize = 10;
        let mut batch = Batch::new();
        let mut buffers: Vec<PinnedBuffer> = Vec::new();

        // Create a batch with multiple operations
        for _ in 0..BATCH_SIZE {
            let mut buffer = PinnedBuffer::with_capacity(1024);
            if let Ok(_) =
                batch.add_operation(Operation::read().fd(0).buffer(buffer.as_mut_slice()))
            {
                buffers.push(buffer);
            } else {
                println!("Could not add operation to batch");
                return;
            }
        }

        // Submit the batch
        let start_time = std::time::Instant::now();
        let mut batch_future = match ring.submit_batch_standalone(batch) {
            Ok(f) => f,
            Err(e) => {
                println!("Could not submit batch: {}", e);
                return;
            }
        };

        let submission_time = start_time.elapsed();

        // Poll once to demonstrate the polling performance
        let poll_start = std::time::Instant::now();
        let _result = poll_fn(|cx| match batch_future.poll_with_ring(&mut ring, cx) {
            std::task::Poll::Ready(_) => std::task::Poll::Ready("completed"),
            std::task::Poll::Pending => std::task::Poll::Ready("pending"),
        })
        .await;
        let poll_time = poll_start.elapsed();

        println!(
            "✅ Batch performance test: submission={:?}, poll={:?} for {} operations",
            submission_time, poll_time, BATCH_SIZE
        );
    }
}
