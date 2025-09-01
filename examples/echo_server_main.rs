//! TCP Echo Server Example using safer-ring
//!
//! Demonstrates basic io_uring network operations with proper memory safety.
//!
//! ## Usage
//! ```bash
//! cargo run --example echo_server_main
//! ```
//!
//! Connect with: `telnet localhost 8080` or `nc localhost 8080`

// Note: Using the echo_server module from the subdirectory
#[path = "echo_server/mod.rs"]
mod echo_server;
#[cfg(target_os = "linux")]
use {
    echo_server::{ServerConfig, ServerStats},
    safer_ring::{OwnedBuffer, Ring},
    std::net::TcpListener,
    std::os::unix::io::AsRawFd,
    std::sync::Arc,
    std::time::Instant,
};

#[cfg(target_os = "linux")]
#[tokio::main]
#[allow(unreachable_code)] // The main server loop is intended to be infinite.
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting safer-ring TCP Echo Server");
    println!("=====================================");

    let config = ServerConfig::default();
    let stats = Arc::new(tokio::sync::Mutex::new(ServerStats::default()));

    // Create a TCP listener
    let listener = TcpListener::bind(config.bind_address)?;
    let listener_fd = listener.as_raw_fd();
    println!("📡 Listening on {}", config.bind_address);

    // Create the io_uring instance with optimized settings
    let mut ring = Ring::new(config.ring_size)?;
    println!("⚡ Created io_uring with {} entries", ring.capacity());
    println!("💾 Buffer size: {} bytes", config.buffer_size);
    println!("🔗 Max connections: {}", config.max_connections);
    println!();

    // Start statistics reporting task
    let stats_clone: Arc<tokio::sync::Mutex<ServerStats>> = Arc::clone(&stats);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let stats = stats_clone.lock().await;
            println!(
                "📊 Stats: {} connections, {} bytes RX, {} bytes TX, {} active",
                stats.connections_accepted,
                stats.bytes_received,
                stats.bytes_sent,
                stats.active_connections
            );
        }
    });

    println!("✅ Server ready! Press Ctrl+C to stop");
    println!(
        "💡 Test with: telnet {} or nc {}",
        config.bind_address.split(':').next().unwrap_or("localhost"),
        config.bind_address.split(':').nth(1).unwrap_or("8080")
    );
    println!();

    // Main accept loop - each connection gets its own task with its own Ring
    loop {
        let client_fd = ring.accept_safe(listener_fd).await?;
        {
            let mut stats_guard = stats.lock().await;
            stats_guard.connection_accepted();
        }

        println!("🔌 New connection: fd {client_fd}");

        // EDUCATIONAL NOTE: This is the recommended concurrency model for safer-ring.
        // Each client connection is handled in its own async task, and each task
        // creates its own `Ring` instance. `Ring` is `Send` but not `Sync`,
        // so it cannot be shared across threads/tasks. This model avoids locks,
        // maximizes performance, and scales well with modern async runtimes.
        //
        // Key benefits of "one Ring per task" model:
        // ✓ No contention between tasks - each has dedicated io_uring instance
        // ✓ Perfect scaling with async runtimes like tokio
        // ✓ Memory safety guaranteed by Rust's ownership system
        // ✓ No complex synchronization or locking required
        // ✓ Each task can use the optimal "hot potato" buffer pattern
        let stats_clone = Arc::clone(&stats);
        let buffer_size = config.buffer_size;
        tokio::spawn(async move {
            // Each task creates its own Ring instance. This is the key to avoiding borrow conflicts
            // and achieving optimal performance. This pattern scales linearly with concurrent connections.
            let mut client_ring = Ring::new(32).expect("Failed to create ring for client task");
            let start_time = Instant::now();

            let result =
                handle_client(&mut client_ring, client_fd, buffer_size, &stats_clone).await;

            {
                let mut stats_guard = stats_clone.lock().await;
                stats_guard.connection_closed();
            }

            match result {
                Ok(bytes) => println!(
                    "✅ Connection {} closed ({} bytes, {:?})",
                    client_fd,
                    bytes,
                    start_time.elapsed()
                ),
                Err(e) => eprintln!("❌ Error on fd {client_fd}: {e}"),
            }

            unsafe { libc::close(client_fd) };
        });
    }

    println!("👋 Server shutting down gracefully...");

    // Print final statistics
    let final_stats = stats.lock().await;
    println!("📈 Final Statistics:");
    println!("   Total connections: {}", final_stats.connections_accepted);
    println!("   Total bytes received: {}", final_stats.bytes_received);
    println!("   Total bytes sent: {}", final_stats.bytes_sent);
    println!("   Active connections: {}", final_stats.active_connections);

    Ok(())
}

/// Handle a single client connection with comprehensive error handling and statistics.
///
/// # Educational Overview: Echo Server with "Hot Potato" Pattern
///
/// This function demonstrates the optimal way to handle client connections using safer-ring's
/// "hot potato" ownership transfer pattern. Each connection runs in its own task with its
/// own Ring instance, enabling perfect scaling and memory safety.
///
/// ## Key Patterns Demonstrated:
/// - **Hot Potato Buffer Reuse**: Single buffer efficiently reused for read/write cycles
/// - **One Ring Per Task**: Each connection gets dedicated io_uring instance
/// - **Timeout Handling**: Robust timeout management for network operations
/// - **Error Recovery**: Graceful handling of network errors and client disconnections
#[cfg(target_os = "linux")]
async fn handle_client(
    ring: &mut Ring<'_>,
    client_fd: i32,
    buffer_size: usize,
    stats: &Arc<tokio::sync::Mutex<ServerStats>>,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let mut total_bytes_processed = 0u64;
    
    // Create a single buffer for this connection that we'll reuse (hot potato pattern)
    // This is more efficient than creating new buffers for each operation
    let mut connection_buffer = OwnedBuffer::new(buffer_size);

    loop {
        // Receive data from the client with timeout handling
        // Hot potato: throw buffer ownership to kernel for read operation
        let receive_future = ring.read_owned(client_fd, connection_buffer);
        let receive_result =
            tokio::time::timeout(tokio::time::Duration::from_secs(30), receive_future).await;

        let (bytes_received, buffer_back) = match receive_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                // I/O error
                return Err(format!("Receive error: {e}").into());
            }
            Err(_) => {
                // Timeout
                println!("⏰ Connection {client_fd} timed out");
                break;
            }
        };
        
        // Hot potato: catch the buffer back from kernel
        connection_buffer = buffer_back;

        if bytes_received == 0 {
            // Client closed the connection gracefully
            break;
        }

        // Update receive statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_received(bytes_received);
        }

        // Log received data (truncated for readability)
        let data_preview = if let Some(guard) = buffer_back.try_access() {
            if bytes_received > 50 {
                format!(
                    "{}... ({} bytes)",
                    String::from_utf8_lossy(&guard[..50]),
                    bytes_received
                )
            } else {
                format!(
                    "{} ({} bytes)",
                    String::from_utf8_lossy(&guard[..bytes_received]),
                    bytes_received
                )
            }
        } else {
            format!("Buffer not accessible ({bytes_received} bytes)")
        };
        println!("📥 fd {client_fd}: {data_preview}");

        // Echo the data back to the client using the same buffer (hot potato pattern)
        // We reuse the same buffer that just received data - this is the most efficient approach
        // Hot potato: throw buffer ownership to kernel for write operation
        let (bytes_sent, buffer_returned) = ring.write_owned(client_fd, connection_buffer).await?;
        
        // Hot potato: catch the buffer back from kernel for next iteration
        connection_buffer = buffer_returned;

        // Update send statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_sent(bytes_sent);
        }

        total_bytes_processed += bytes_received as u64;

        println!("📤 fd {client_fd}: Echoed {bytes_sent} bytes");

        // Verify we sent all the data
        if bytes_sent != bytes_received {
            eprintln!(
                "⚠️  Warning: Partial send on fd {client_fd} ({bytes_sent}/{bytes_received} bytes)"
            );
        }
    }

    Ok(total_bytes_processed)
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("❌ This example requires Linux with io_uring support");
    println!("💡 io_uring is not available on this platform");
    println!();
    println!("Supported platforms:");
    println!("  - Linux 5.1+ (basic support)");
    println!("  - Linux 5.19+ (recommended for full features)");
    println!("  - Linux 6.0+ (optimal performance)");
}
