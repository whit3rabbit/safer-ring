//! Demonstration of async/await support in safer-ring.
//!
//! This example shows how to use the Future implementations for
//! read and write operations with async/await syntax.

use std::pin::Pin;

use safer_ring::{Operation, PinnedBuffer, Ring};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Safer-Ring Async Demo");

    #[cfg(target_os = "linux")]
    {
        // Create a ring with 32 entries
        let ring = Ring::new(32)?;
        println!("Created ring with capacity: {}", ring.capacity());

        // Create a buffer for I/O operations
        let mut buffer = PinnedBuffer::with_capacity(1024);
        println!("Created buffer with size: {}", buffer.len());

        // Demonstrate that we can create futures (even if we can't actually run them without real files)
        let operation = Operation::read()
            .fd(-1) // Invalid fd for demo purposes
            .buffer(buffer.as_mut_slice())
            .offset(0);

        // This would normally be awaited, but we'll just create the future to show it compiles
        let _future = ring.submit_read(operation);
        println!("Successfully created read future");

        // Create a write operation future
        let test_data = b"Hello, async world!";
        let mut write_buffer = PinnedBuffer::from_slice(test_data);

        let write_operation = Operation::write()
            .fd(-1) // Invalid fd for demo purposes
            .buffer(write_buffer.as_mut_slice())
            .offset(0);

        let _write_future = ring.submit_write(write_operation);
        println!("Successfully created write future");

        // Create a generic operation future
        let mut generic_buffer = PinnedBuffer::with_capacity(512);
        let generic_operation = Operation::read()
            .fd(-1) // Invalid fd for demo purposes
            .buffer(generic_buffer.as_mut_slice())
            .offset(0);

        let _generic_future = ring.submit_operation(generic_operation);
        println!("Successfully created generic operation future");

        println!("All futures created successfully!");
        println!("Note: Actual I/O operations require valid file descriptors");
    }

    #[cfg(not(target_os = "linux"))]
    {
        println!("This demo requires Linux for io_uring support");
        println!("On this platform, Ring::new() will return an error");

        match Ring::new(32) {
            Ok(_) => println!("Unexpected success creating ring"),
            Err(e) => println!("Expected error creating ring: {}", e),
        }
    }

    Ok(())
}
