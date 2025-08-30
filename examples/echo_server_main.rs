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
    std::time::Instant,
    std::sync::Arc,
    std::os::unix::io::AsRawFd,
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
        
        println!("🔌 New connection: fd {}", client_fd);

        // Spawn a new task for each client. Each task gets its own Ring.
        let stats_clone = Arc::clone(&stats);
        let buffer_size = config.buffer_size;
        tokio::spawn(async move {
            // Each task creates its own Ring instance. This is the key to avoiding borrow conflicts.
            let mut client_ring = Ring::new(32).expect("Failed to create ring for client task");
            let start_time = Instant::now();

            let result = handle_client(&mut client_ring, client_fd, buffer_size, &stats_clone).await;

            {
                let mut stats_guard = stats_clone.lock().await;
                stats_guard.connection_closed();
            }

            match result {
                Ok(bytes) => println!(
                    "✅ Connection {} closed ({} bytes, {:?})",
                    client_fd, bytes, start_time.elapsed()
                ),
                Err(e) => eprintln!("❌ Error on fd {}: {}", client_fd, e),
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

/// Handle a single client connection with comprehensive error handling and statistics
#[cfg(target_os = "linux")]
async fn handle_client<'a>(
    ring: &'a mut Ring<'a>,
    client_fd: i32,
    buffer_size: usize,
    stats: &Arc<tokio::sync::Mutex<ServerStats>>,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let mut total_bytes_processed = 0u64;

    loop {
        // Receive data from the client with timeout handling
        let buffer = OwnedBuffer::new(buffer_size);
        let receive_future = ring.read_owned(client_fd, buffer);
        let receive_result = tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            receive_future,
        )
        .await;

        let (bytes_received, buffer_back) = match receive_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                // I/O error
                return Err(format!("Receive error: {}", e).into());
            }
            Err(_) => {
                // Timeout
                println!("⏰ Connection {} timed out", client_fd);
                break;
            }
        };

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
            let slice = guard.as_slice();
            if bytes_received > 50 {
                format!(
                    "{}... ({} bytes)",
                    String::from_utf8_lossy(&slice[..50]),
                    bytes_received
                )
            } else {
                format!(
                    "{} ({} bytes)",
                    String::from_utf8_lossy(&slice[..bytes_received]),
                    bytes_received
                )
            }
        } else {
            format!("Buffer not accessible ({} bytes)", bytes_received)
        };
        println!("📥 fd {}: {}", client_fd, data_preview);

        // Echo the data back to the client
        // Create echo buffer with just the data we received
        let echo_data = if let Some(guard) = buffer_back.try_access() {
            guard.as_slice()[..bytes_received].to_vec()
        } else {
            return Err("Buffer not accessible for echo".into());
        };
        
        let echo_buffer = OwnedBuffer::from_slice(&echo_data);
        let (bytes_sent, _buffer_returned) = ring.write_owned(client_fd, echo_buffer).await?;

        // Update send statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_sent(bytes_sent);
        }

        total_bytes_processed += bytes_received as u64;

        println!("📤 fd {}: Echoed {} bytes", client_fd, bytes_sent);

        // Verify we sent all the data
        if bytes_sent != bytes_received {
            eprintln!(
                "⚠️  Warning: Partial send on fd {} ({}/{} bytes)",
                client_fd, bytes_sent, bytes_received
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
