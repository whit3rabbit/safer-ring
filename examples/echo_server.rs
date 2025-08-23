//! Echo server example using safer-ring network operations.
//!
//! This example demonstrates how to use safer-ring's network operations
//! to build a simple TCP echo server that accepts connections and echoes
//! back any data received from clients.

use safer_ring::{PinnedBuffer, Ring};
use std::io;
use std::net::TcpListener;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting safer-ring echo server...");

    // Create a TCP listener
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    let listener_fd = listener.as_raw_fd();
    println!("Listening on 127.0.0.1:8080");

    // Create the io_uring instance
    let ring = Ring::new(32)?;
    println!("Created io_uring with 32 entries");

    // Accept and handle connections
    loop {
        println!("Waiting for connection...");
        
        // Accept a new connection
        let client_fd = ring.accept(listener_fd).await?;
        println!("Accepted connection: fd {}", client_fd);

        // Handle the client in a separate task
        let ring_ref = &ring;
        tokio::spawn(async move {
            if let Err(e) = handle_client(ring_ref, client_fd).await {
                eprintln!("Error handling client {}: {}", client_fd, e);
            }
            
            // Close the client socket
            unsafe {
                libc::close(client_fd);
            }
            println!("Closed connection: fd {}", client_fd);
        });
    }
}

#[cfg(target_os = "linux")]
async fn handle_client(ring: &Ring<'_>, client_fd: i32) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = PinnedBuffer::with_capacity(1024);
    
    loop {
        // Receive data from the client
        let (bytes_received, buffer_back) = ring.recv(client_fd, buffer.as_mut_slice()).await?;
        
        if bytes_received == 0 {
            // Client closed the connection
            println!("Client {} closed connection", client_fd);
            break;
        }

        println!("Received {} bytes from client {}", bytes_received, client_fd);
        
        // Echo the data back to the client
        let echo_slice = Pin::new(&mut buffer_back.as_mut()[..bytes_received]);
        let (bytes_sent, buffer_returned) = ring.send(client_fd, echo_slice).await?;
        
        println!("Echoed {} bytes to client {}", bytes_sent, client_fd);
        
        // Create a new buffer for the next iteration
        buffer = PinnedBuffer::with_capacity(1024);
    }
    
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("This example requires Linux with io_uring support");
    println!("io_uring is not available on this platform");
}