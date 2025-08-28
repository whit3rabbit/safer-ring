//! Demonstration of completion queue processing.
//!
//! This example shows how to use the completion queue processing functionality
//! to handle completed io_uring operations safely.

use safer_ring::Ring;

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Completion Queue Processing Demo");
    println!("================================");

    // Create a ring with 32 entries
    let ring = Ring::new(32)?;
    println!("Created ring with capacity: {}", ring.capacity());

    // Create a buffer for I/O operations
    let mut buffer = PinnedBuffer::with_capacity(1024);
    println!("Created buffer with {} bytes", buffer.len());

    // Demonstrate completion queue statistics
    let (ready, capacity) = ring.completion_queue_stats();
    println!("Completion queue: {} ready, {} capacity", ready, capacity);

    // Try to complete operations (should be empty initially)
    let completions = ring.try_complete()?;
    println!("Initial completions: {}", completions.len());

    // Demonstrate error handling for wait_for_completion with no operations
    match ring.wait_for_completion() {
        Ok(_) => println!("Unexpected success waiting for completions"),
        Err(e) => println!("Expected error waiting with no operations: {}", e),
    }

    // Submit a read operation (this will likely fail since stdin might not be ready)
    let operation = Operation::read()
        .fd(0) // stdin
        .buffer(Pin::new(buffer.as_mut_slice()));

    match ring.submit(operation) {
        Ok(submitted) => {
            println!("Submitted operation with ID: {}", submitted.id());
            println!("Operations in flight: {}", ring.operations_in_flight());

            // Try to check for completion of this specific operation
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
            let completions = ring.try_complete()?;
            println!(
                "Bulk completion check found {} completions",
                completions.len()
            );

            // Show final statistics
            let (ready, _) = ring.completion_queue_stats();
            println!("Final completion queue ready count: {}", ready);
            println!(
                "Final operations in flight: {}",
                ring.operations_in_flight()
            );
        }
        Err(e) => {
            println!("Failed to submit operation: {}", e);
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
