//! TCP Echo Server Example using safer-ring
//!
//! Demonstrates basic io_uring network operations with proper memory safety.
//!
//! ## Usage
//! ```bash
//! cargo run --example echo_server
//! ```
//!
//! Connect with: `telnet localhost 8080` or `nc localhost 8080`

// Note: Using the echo_server module from the subdirectory
use echo_server::{handle_client, ServerConfig, ServerStats};
use safer_ring::Ring;
use std::net::TcpListener;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::time::Instant;

#[cfg(target_os = "linux")]
#[tokio::main]
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
    let ring = Ring::new(config.ring_size)?;
    println!("⚡ Created io_uring with {} entries", config.ring_size);
    println!("💾 Buffer size: {} bytes", config.buffer_size);
    println!("🔗 Max connections: {}", config.max_connections);
    println!();

    // Start statistics reporting task
    let stats_clone = Arc::clone(&stats);
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

    // Accept and handle connections
    loop {
        // Handle new connections
        match ring.accept(listener_fd).await {
            Ok(client_fd) => {
                // Update statistics
                {
                    let mut stats = stats.lock().await;
                    stats.connections_accepted += 1;
                    stats.active_connections += 1;
                }

                println!("🔌 New connection: fd {}", client_fd);

                // Handle the client in a separate task
                let ring_ref = &ring;
                let stats_clone = Arc::clone(&stats);
                let buffer_size = config.buffer_size;

                tokio::spawn(async move {
                    let start_time = Instant::now();
                    let result =
                        handle_client(ring_ref, client_fd, buffer_size, &stats_clone).await;

                    // Update statistics
                    {
                        let mut stats = stats_clone.lock().await;
                        stats.active_connections -= 1;
                    }

                    match result {
                        Ok(bytes_processed) => {
                            println!(
                                "✅ Connection {} closed cleanly ({} bytes, {:?})",
                                client_fd,
                                bytes_processed,
                                start_time.elapsed()
                            );
                        }
                        Err(e) => {
                            eprintln!("❌ Error handling client {}: {}", client_fd, e);
                        }
                    }

                    // Close the client socket
                    unsafe {
                        libc::close(client_fd);
                    }
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept connection: {}", e);
                // Continue accepting other connections
            }
        }
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
async fn handle_client(
    ring: &Ring<'_>,
    client_fd: i32,
    buffer_size: usize,
    stats: &Arc<tokio::sync::Mutex<ServerStats>>,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut buffer = PinnedBuffer::with_capacity(buffer_size);
    let mut total_bytes_processed = 0u64;

    loop {
        // Receive data from the client with timeout handling
        let receive_result = tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            ring.recv(client_fd, buffer.as_mut_slice()),
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
            stats.bytes_received += bytes_received as u64;
        }

        // Log received data (truncated for readability)
        let data_preview = if bytes_received > 50 {
            format!(
                "{}... ({} bytes)",
                String::from_utf8_lossy(&buffer_back.as_ref()[..50]),
                bytes_received
            )
        } else {
            format!(
                "{} ({} bytes)",
                String::from_utf8_lossy(&buffer_back.as_ref()[..bytes_received]),
                bytes_received
            )
        };
        println!("📥 fd {}: {}", client_fd, data_preview);

        // Echo the data back to the client
        // We only send back the actual data received, not the full buffer
        let echo_slice = &mut buffer_back.as_mut()[..bytes_received];
        let (bytes_sent, buffer_returned) = ring.send(client_fd, echo_slice).await?;

        // Update send statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_sent += bytes_sent as u64;
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

        // Reuse the returned buffer for the next iteration
        buffer = buffer_returned;
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
