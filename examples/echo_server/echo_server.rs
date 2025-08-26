//! TCP echo server example using safer-ring.
//!
//! This example demonstrates how to build a high-performance TCP echo server
//! using safer-ring's async I/O operations with proper buffer management.

mod client;
mod config;
mod stats;

use crate::client::EchoClient;
use crate::config::EchoServerConfig;
use crate::stats::ServerStats;

use safer_ring::{PinnedBuffer, Ring};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::Arc;
use tokio::net::TcpListener as TokioTcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse configuration
    let config = EchoServerConfig::from_env();
    println!("Starting echo server on {}:{}", config.host, config.port);
    println!("Buffer size: {} bytes", config.buffer_size);
    println!("Max concurrent connections: {}", config.max_connections);

    // Create io_uring ring
    let ring = Ring::new(config.ring_size)?;
    println!("Created io_uring ring with {} entries", config.ring_size);

    // Create TCP listener
    let listener = TcpListener::bind((config.host.as_str(), config.port))?;
    listener.set_nonblocking(true)?;

    let tokio_listener = TokioTcpListener::from_std(listener)?;
    println!("Listening on {}:{}", config.host, config.port);

    // Create server stats
    let stats = Arc::new(ServerStats::new());
    let stats_clone = stats.clone();

    // Start stats reporting task
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            stats_clone.print_stats();
        }
    });

    // Accept connections
    loop {
        let (stream, addr) = tokio_listener.accept().await?;
        println!("New connection from {}", addr);

        let ring_clone = &ring; // Share ring reference
        let stats_clone = stats.clone();
        let config_clone = config.clone();

        // Spawn task to handle client
        tokio::spawn(async move {
            if let Err(e) = handle_client(ring_clone, stream, config_clone, stats_clone).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
            println!("Client {} disconnected", addr);
        });
    }
}

async fn handle_client(
    ring: &Ring<'_>,
    stream: TcpStream,
    config: EchoServerConfig,
    stats: Arc<ServerStats>,
) -> Result<(), Box<dyn std::error::Error>> {
    let fd = stream.as_raw_fd();
    let mut client = EchoClient::new(fd, config.buffer_size, stats);

    client.handle_echo_loop(ring).await
}
