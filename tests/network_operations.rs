//! Integration tests for network operations.

use safer_ring::Ring;
use std::io::{self};
use std::net::{TcpListener, TcpStream};
use std::thread;

#[cfg(target_os = "linux")]
use safer_ring::PinnedBuffer;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(target_os = "linux")]
use std::pin::Pin;
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use tokio::time::timeout;

/// Helper function to create a listening socket for testing.
#[allow(dead_code)]
fn create_test_listener() -> io::Result<(TcpListener, u16)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?; // Set non-blocking for async accept
    Ok((listener, port))
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_accept_operation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ring = Ring::new(32)?;
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    // Spawn a client to connect to the listener
    let connect_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        TcpStream::connect(format!("127.0.0.1:{}", port))
    });

    // Create the future before the timeout to avoid lifetime issues
    let accept_future = ring.accept(listener_fd)?;
    let client_fd = timeout(Duration::from_secs(5), accept_future).await??;

    assert!(client_fd >= 0);
    connect_handle.await??;
    unsafe { libc::close(client_fd) };
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_send_recv_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut server, client) = create_connected_pair()?;
    let client_fd = client.as_raw_fd();

    let mut ring = Ring::new(32)?;
    let test_data = b"Hello, safer-ring!";

    // Send data
    let mut send_buffer = PinnedBuffer::from_slice(test_data);
    let send_future = ring.send(client_fd, send_buffer.as_mut_slice())?;
    let (bytes_sent, _) = timeout(Duration::from_secs(5), send_future).await??;
    assert_eq!(bytes_sent, test_data.len());

    // Receive data on the other end
    let mut received_data = vec![0u8; test_data.len()];
    server.read_exact(&mut received_data)?;
    assert_eq!(received_data.as_slice(), test_data);

    // Now test recv
    let mut recv_buffer = PinnedBuffer::with_capacity(1024);
    let recv_future = ring.recv(client_fd, recv_buffer.as_mut_slice())?;
    
    // Write from server to trigger recv completion
    tokio::spawn(async move {
        server.write_all(test_data).unwrap();
        server.flush().unwrap();
    });

    let (bytes_received, received_pinned_buffer) = timeout(Duration::from_secs(5), recv_future).await??;
    assert_eq!(bytes_received, test_data.len());
    assert_eq!(&received_pinned_buffer.as_ref()[..bytes_received], test_data);

    Ok(())
}


// This test now correctly models the "one ring per task" pattern.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_echo_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    // The server task creates its own Ring instance.
    let echo_task = tokio::spawn(async move {
        let mut ring = Ring::new(32)?;
        
        let client_fd = ring.accept(listener_fd)?.await?;

        let mut buffer = PinnedBuffer::with_capacity(1024);
        let (bytes_received, mut buffer) = ring.recv(client_fd, buffer.as_mut_slice())?.await?;

        // FIX: Create a variable for the slice to extend its lifetime
        let echo_slice = &mut buffer.as_mut()[..bytes_received];
        let echo_pin = Pin::new(echo_slice);
        let (bytes_sent, _) = ring.send(client_fd, echo_pin)?.await?;

        unsafe { libc::close(client_fd); }

        Ok::<(usize, usize), Box<dyn std::error::Error + Send + Sync>>((bytes_received, bytes_sent))
    });

    // Client task remains the same
    let client_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port))?;
        let test_data = b"Echo test message";
        client.write_all(test_data)?;
        client.flush()?;
        let mut received = vec![0u8; test_data.len()];
        client.read_exact(&mut received)?;
        Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(received)
    });

    let (echo_result, client_result) = tokio::try_join!(echo_task, client_task)?;
    let (bytes_received, bytes_sent) = echo_result?;
    let received_data = client_result?;

    assert_eq!(bytes_received, bytes_sent);
    assert_eq!(received_data.as_slice(), b"Echo test message");

    Ok(())
}


#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_multiple_concurrent_connections() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    const NUM_CONNECTIONS: usize = 5;

    // The server task handles all connections, but it does so sequentially within its own task.
    // A more advanced server would spawn a new task *with a new ring* for each connection.
    let server_task = tokio::spawn(async move {
        let mut ring = Ring::new(32)?;
        let mut results = Vec::new();
        for _ in 0..NUM_CONNECTIONS {
            let client_fd = ring.accept(listener_fd)?.await?;
            let mut buffer = PinnedBuffer::with_capacity(1024);
            let (bytes_received, mut buffer) = ring.recv(client_fd, buffer.as_mut_slice())?.await?;
            
            let echo_slice = &mut buffer.as_mut()[..bytes_received];
            let echo_pin = Pin::new(echo_slice);
            let (bytes_sent, _) = ring.send(client_fd, echo_pin)?.await?;

            results.push((bytes_received, bytes_sent));
            unsafe { libc::close(client_fd); }
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(results)
    });

    let mut client_tasks = Vec::new();
    for i in 0..NUM_CONNECTIONS {
        let port = port;
        client_tasks.push(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(i as u64 * 20)).await;
            let mut client = TcpStream::connect(format!("127.0.0.1:{}", port))?;
            let test_data = format!("Message from client {}", i);
            client.write_all(test_data.as_bytes())?;
            client.flush()?;
            let mut received = vec![0; test_data.len()];
            client.read_exact(&mut received)?;
            Ok::<String, Box<dyn std::error::Error + Send + Sync>>(String::from_utf8(received)?)
        }));
    }

    let server_results = server_task.await??;
    let mut client_results = Vec::new();
    for task in client_tasks {
        client_results.push(task.await??);
    }

    assert_eq!(server_results.len(), NUM_CONNECTIONS);
    assert_eq!(client_results.len(), NUM_CONNECTIONS);
    
    // Sort client results as they may complete out of order
    client_results.sort();
    for i in 0..NUM_CONNECTIONS {
        let expected_message = format!("Message from client {}", i);
        assert_eq!(client_results[i], expected_message);
        assert_eq!(server_results[i].0, expected_message.len());
        assert_eq!(server_results[i].1, expected_message.len());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_operation_error_handling() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = PinnedBuffer::with_capacity(1024);
    let mut ring = Ring::new(32)?;

    let invalid_fd: RawFd = -1;

    // Test send operation
    let result = ring.send(invalid_fd, buffer.as_mut_slice())?.await;
    assert!(result.is_err());

    // Test recv operation
    let result = ring.recv(invalid_fd, buffer.as_mut_slice())?.await;
    assert!(result.is_err());

    // Test accept operation
    let result = ring.accept(invalid_fd)?.await;
    assert!(result.is_err());

    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn test_network_operations_unsupported_platform() {
    let ring_result = Ring::new(32);
    assert!(ring_result.is_err());
}

// Helper to create connected pair for tests.
fn create_connected_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let client = TcpStream::connect(addr)?;
    let (server, _) = listener.accept()?;
    Ok((server, client))
}