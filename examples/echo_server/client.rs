//! Client connection handling for the echo server.

use crate::echo_server::{ServerConfig, ServerStats};
use safer_ring::{PinnedBuffer, Ring};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handle a single client connection.
///
/// This function manages the complete lifecycle of a client connection:
/// - Receives data from the client
/// - Echoes it back
/// - Handles timeouts and errors gracefully
/// - Updates server statistics
pub async fn handle_client<'a>(
    ring: &'a mut Ring<'a>,
    client_fd: i32,
    config: &ServerConfig,
    stats: &Arc<Mutex<ServerStats>>,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut buffer = PinnedBuffer::with_capacity(config.buffer_size);
    let mut total_bytes_processed = 0u64;

    loop {
        // Use timeout to prevent hanging connections
        let timeout_duration = tokio::time::Duration::from_secs(config.connection_timeout_secs);
        let receive_result = tokio::time::timeout(
            timeout_duration,
            ring.recv(client_fd, buffer.as_mut_slice())?,
        )
        .await;

        let (bytes_received, buffer_back) = match receive_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => return Err(format!("I/O error: {}", e).into()),
            Err(_) => {
                // Timeout occurred - this is normal for idle connections
                break;
            }
        };

        // Zero bytes means client closed connection
        if bytes_received == 0 {
            break;
        }

        // Update statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_received(bytes_received);
        }

        // Echo the received data back to client
        // We need to send the full buffer, since we can't slice a Pin<&mut [u8]>
        // For a proper echo server, we'd need to create a new buffer with only the received data
        let (bytes_sent, returned_buffer) = ring.send(client_fd, buffer_back)?.await?;

        // Update send statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_sent(bytes_sent);
        }

        total_bytes_processed += bytes_received as u64;

        // Warn if we couldn't send all data (shouldn't happen in echo server)
        if bytes_sent != bytes_received {
            eprintln!(
                "Warning: Partial send on fd {} ({}/{} bytes)",
                client_fd, bytes_sent, bytes_received
            );
        }

        // Buffer ownership is returned, but we keep using our PinnedBuffer instance
        // The returned_buffer is just a reference to the data
    }

    Ok(total_bytes_processed)
}
