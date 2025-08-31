//! # Completion Queue Processing Demonstration
//!
//! This example demonstrates safer-ring's completion queue processing capabilities,
//! showing how to handle completed io_uring operations safely using the polling API.
//!
//! ## Key Concepts Demonstrated
//!
//! - **Completion Queue**: How completed operations are tracked and retrieved
//! - **Polling vs Waiting**: Non-blocking `try_complete()` vs blocking `wait_for_completion()`
//! - **Operation Tracking**: How operations get unique IDs and are correlated with results
//! - **Bulk Processing**: Efficient processing of multiple completions at once
//! - **Error Handling**: Proper handling of operation failures and API limitations
//! - **Buffer Management**: Understanding the current limitation of buffer ownership return
//!
//! ## Educational Notes
//!
//! This example uses the low-level polling API rather than the recommended async API.
//! The polling API is useful for integration with custom event loops or for understanding
//! the underlying mechanics of io_uring completion processing.
//!
//! **Important**: The current polling API has a limitation where buffer ownership is not
//! returned to the caller. Users must retain ownership of their buffers throughout the
//! operation lifecycle. Future versions will enhance this with an owned buffer API.
//!
//! ## Safety Features Demonstrated
//!
//! - **Lifetime Safety**: Buffer must be declared before ring to ensure proper drop order
//! - **Operation Tracking**: Each operation gets a unique ID preventing confusion
//! - **Error Propagation**: Clear error handling for all failure modes
//! - **Resource Management**: Automatic cleanup when ring is dropped

use safer_ring::Ring;

#[cfg(target_os = "linux")]
use safer_ring::{Operation, PinnedBuffer};

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Completion Queue Processing Demo");
    println!("================================");

    // EDUCATIONAL NOTE: Buffer must be declared before ring to ensure proper drop order.
    // Rust drops values in reverse order of declaration. Since the ring may hold
    // references to the buffer during operations, the buffer must outlive the ring.
    let mut buffer = PinnedBuffer::with_capacity(1024);
    println!("Created buffer with {} bytes", buffer.len());

    // Create a ring with 32 entries
    // The ring manages the io_uring submission and completion queues
    let mut ring = Ring::new(32)?;
    println!("Created ring with capacity: {}", ring.capacity());

    // Demonstrate completion queue statistics
    //
    // EDUCATIONAL NOTE: The completion queue stores completed operations from the kernel.
    // - ready: Number of completions waiting to be processed
    // - capacity: Total size of the completion queue
    let (ready, capacity) = ring.completion_queue_stats();
    println!("Completion queue: {} ready, {} capacity", ready, capacity);

    // Try to complete operations (should be empty initially)
    //
    // EDUCATIONAL NOTE: try_complete() is a non-blocking call that checks for
    // any completed operations. It returns immediately even if nothing is ready.
    // This is useful for polling-based patterns where you don't want to block.
    let completions = ring.try_complete()?;
    println!("Initial completions: {}", completions.len());

    // Demonstrate error handling for wait_for_completion with no operations
    //
    // EDUCATIONAL NOTE: wait_for_completion() blocks until at least one operation
    // completes. If no operations are in flight, it returns an error immediately
    // since there's nothing to wait for. This is safer-ring's error handling in action.
    match ring.wait_for_completion() {
        Ok(_) => println!("Unexpected success waiting for completions"),
        Err(e) => println!("Expected error waiting with no operations: {}", e),
    }

    // Submit a read operation to demonstrate the completion queue
    //
    // EDUCATIONAL NOTE: This operation is designed to demonstrate the completion
    // queue, not to actually read from stdin. Reading from stdin without data
    // available will likely complete immediately with an error or block.
    let operation = Operation::read()
        .fd(0) // stdin - using fd 0 as an example
        .buffer(buffer.as_mut_slice()); // PinnedBuffer provides stable memory for kernel

    match ring.submit(operation) {
        Ok(submitted) => {
            // EDUCATIONAL NOTE: Each operation gets a unique ID for tracking.
            // This ID is used to correlate submissions with completions.
            println!("Submitted operation with ID: {}", submitted.id());
            println!("Operations in flight: {}", ring.operations_in_flight());

            // Try to check for completion of this specific operation
            //
            // EDUCATIONAL NOTE: try_complete_by_id() checks if a specific operation
            // has completed without blocking. This is useful when you only care about
            // one particular operation rather than processing all completions.
            match ring.try_complete_by_id(submitted.id()) {
                Ok(Some(result)) => {
                    println!("Operation completed with result: {:?}", result);
                }
                Ok(None) => {
                    println!("Operation not yet completed");
                }
                Err(e) => {
                    println!("Error checking operation completion: {}", e);
                }
            }

            // Try bulk completion check
            //
            // EDUCATIONAL NOTE: This demonstrates bulk completion processing.
            // try_complete() returns all completed operations at once, which is
            // more efficient than checking operations individually.
            let completions = ring.try_complete()?;
            println!(
                "Bulk completion check found {} completions",
                completions.len()
            );

            // Process completions if any were found
            if !completions.is_empty() {
                println!("Processing completions:");
                for completion in completions {
                    println!("  Operation ID: {}", completion.id());
                    println!("  File descriptor: {}", completion.fd());
                    println!("  Operation type: {:?}", completion.op_type());
                    println!("  Success: {}", completion.is_success());
                    
                    // EDUCATIONAL NOTE: Extract the result and buffer ownership
                    let (result, buffer) = completion.into_result();
                    match result {
                        Ok(bytes) => println!("  Transferred {} bytes", bytes),
                        Err(e) => println!("  I/O error: {}", e),
                    }
                    
                    // EDUCATIONAL NOTE: Current API limitation - buffer ownership
                    // is not returned in the polling API due to the borrowed reference design.
                    // Users retain ownership of their original buffers.
                    println!("  Buffer returned: {}", buffer.is_some());
                }
            }

            // Show final statistics
            let (ready, _) = ring.completion_queue_stats();
            println!("Final completion queue ready count: {}", ready);
            println!(
                "Final operations in flight: {}",
                ring.operations_in_flight()
            );
        }
        Err(e) => {
            // EDUCATIONAL NOTE: Operation submission can fail for various reasons:
            // - Ring is full (too many operations in flight)
            // - Invalid operation parameters
            // - System-level errors
            println!("Failed to submit operation: {}", e);
            println!("This is normal for this demo - stdin may not be available");
        }
    }

    println!("\nDemo completed successfully!");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("Completion Queue Processing Demo");
    println!("================================");
    println!("This demo requires Linux with io_uring support.");
    println!("Current platform: {}", std::env::consts::OS);

    // Demonstrate that the API exists even on non-Linux platforms
    match Ring::new(32) {
        Ok(_) => println!("Unexpected success creating ring on non-Linux"),
        Err(e) => println!("Expected error on non-Linux platform: {}", e),
    }
}
