//! Registry demonstration showing file descriptor and buffer registration.

use safer_ring::Registry;
use std::pin::Pin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Registry Demo - File Descriptor and Buffer Registration");
    println!("========================================================");

    // Create a new registry
    let mut registry = Registry::new();
    println!("Created empty registry");
    println!(
        "FDs: {}, Buffers: {}",
        registry.fd_count(),
        registry.buffer_count()
    );

    // Register some file descriptors
    println!("\nRegistering file descriptors...");
    let stdin_fd = registry.register_fd(0)?;
    let stdout_fd = registry.register_fd(1)?;
    let stderr_fd = registry.register_fd(2)?;

    println!("Registered stdin (index: {})", stdin_fd.index());
    println!("Registered stdout (index: {})", stdout_fd.index());
    println!("Registered stderr (index: {})", stderr_fd.index());
    println!("Total FDs registered: {}", registry.fd_count());

    // Register some buffers
    println!("\nRegistering buffers...");
    let buffer1 = Pin::new(Box::new([0u8; 1024]));
    let buffer2 = Pin::new(Box::new([0u8; 2048]));
    let buffer3 = Pin::new(Box::new([0u8; 4096]));

    let reg_buf1 = registry.register_buffer(buffer1)?;
    let reg_buf2 = registry.register_buffer(buffer2)?;
    let reg_buf3 = registry.register_buffer(buffer3)?;

    println!(
        "Registered buffer 1 (index: {}, size: {})",
        reg_buf1.index(),
        reg_buf1.size()
    );
    println!(
        "Registered buffer 2 (index: {}, size: {})",
        reg_buf2.index(),
        reg_buf2.size()
    );
    println!(
        "Registered buffer 3 (index: {}, size: {})",
        reg_buf3.index(),
        reg_buf3.size()
    );
    println!("Total buffers registered: {}", registry.buffer_count());

    // Demonstrate resource validation
    println!("\nValidating registered resources...");
    println!(
        "Is stdin FD registered? {}",
        registry.is_fd_registered(&stdin_fd)
    );
    println!(
        "Is buffer 1 registered? {}",
        registry.is_buffer_registered(&reg_buf1)
    );

    // Show resource counts
    println!("FDs in use: {}", registry.fds_in_use_count());
    println!("Buffers in use: {}", registry.buffers_in_use_count());

    // Unregister unused resources
    println!("\nUnregistering unused resources...");
    registry.unregister_fd(stdout_fd)?;
    println!("Successfully unregistered stdout FD");

    let returned_buffer2 = registry.unregister_buffer(reg_buf2)?;
    println!(
        "Successfully unregistered buffer (size: {})",
        returned_buffer2.len()
    );

    println!(
        "Final counts - FDs: {}, Buffers: {}",
        registry.fd_count(),
        registry.buffer_count()
    );

    // Demonstrate slot reuse
    println!("\nDemonstrating slot reuse...");
    let new_fd = registry.register_fd(42)?;
    println!(
        "New FD registered with index: {} (reused slot)",
        new_fd.index()
    );

    let new_buffer = Pin::new(Box::new([42u8; 512]));
    let new_reg_buf = registry.register_buffer(new_buffer)?;
    println!(
        "New buffer registered with index: {} (reused slot)",
        new_reg_buf.index()
    );

    println!("\nRegistry demo completed successfully!");
    Ok(())
}
