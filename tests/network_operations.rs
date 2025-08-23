//! Integration tests for network operations.
//!
//! These tests verify that network operations (accept, send, recv) work correctly
//! with the safer-ring library. The tests use real sockets to ensure proper
//! integration with the kernel's io_uring implementation.

use safer_ring::{Operation, PinnedBuffer, Ring};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::thread;
use std::time::Duration;
use tokio::time::timeout;

/// Helper function to create a listening socket for testing.
fn create_test_listener() -> io::Result<(TcpListener, u16)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

/// Helper function to create a connected socket pair for testing.
fn create_connected_pair() -> io::Result<(TcpStream, TcpStream)> {
    let (listener, port) = create_test_listener()?;
    
    // Connect from another thread to avoid blocking
    let client_handle = thread::spawn(move || -> io::Result<TcpStream> {
        TcpStream::connect(format!("127.0.0.1:{}", port))
    });
    
    // Accept the connection
    let (server_stream, _) = listener.accept()?;
    let client_stream = client_handle.join().unwrap()?;
    
    Ok((server_stream, client_stream))
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_accept_operation() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    // Start accept operation
    let accept_future = ring.accept(listener_fd);

    // Connect from another thread
    let connect_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        TcpStream::connect(format!("127.0.0.1:{}", port))
    });

    // Wait for accept to complete
    let client_fd = timeout(Duration::from_secs(5), accept_future).await??;
    
    // Verify we got a valid file descriptor
    assert!(client_fd >= 0);
    
    // Ensure the connection was established
    let _client_stream = connect_handle.await??;
    
    // Clean up - close the accepted socket
    unsafe {
        libc::close(client_fd);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_send_operation() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    let (mut server_stream, client_stream) = create_connected_pair()?;
    let client_fd = client_stream.as_raw_fd();

    // Prepare data to send
    let test_data = b"Hello, network world!";
    let mut buffer = PinnedBuffer::from_slice(test_data);

    // Send data using safer-ring
    let send_future = ring.send(client_fd, buffer.as_mut_slice());
    let (bytes_sent, _buffer) = timeout(Duration::from_secs(5), send_future).await??;

    // Verify the correct number of bytes were sent
    assert_eq!(bytes_sent, test_data.len());

    // Verify the data was received on the server side
    let mut received_data = vec![0u8; test_data.len()];
    server_stream.read_exact(&mut received_data)?;
    assert_eq!(received_data, test_data);

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_recv_operation() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    let (mut server_stream, client_stream) = create_connected_pair()?;
    let client_fd = client_stream.as_raw_fd();

    // Prepare buffer for receiving
    let mut buffer = PinnedBuffer::with_capacity(1024);

    // Start receive operation
    let recv_future = ring.recv(client_fd, buffer.as_mut_slice());

    // Send data from server side
    let test_data = b"Hello from server!";
    thread::spawn(move || -> io::Result<()> {
        thread::sleep(Duration::from_millis(10));
        server_stream.write_all(test_data)?;
        server_stream.flush()?;
        Ok(())
    });

    // Wait for receive to complete
    let (bytes_received, buffer) = timeout(Duration::from_secs(5), recv_future).await??;

    // Verify the correct number of bytes were received
    assert_eq!(bytes_received, test_data.len());

    // Verify the received data matches what was sent
    let received_slice = &buffer.as_ref()[..bytes_received];
    assert_eq!(received_slice, test_data);

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_echo_server() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    // Start echo server task
    let echo_task = tokio::spawn(async move {
        // Accept a connection
        let client_fd = ring.accept(listener_fd).await?;

        // Receive data
        let mut buffer = PinnedBuffer::with_capacity(1024);
        let (bytes_received, buffer) = ring.recv(client_fd, buffer.as_mut_slice()).await?;

        // Echo the data back
        let echo_buffer = Pin::new(&mut buffer.as_mut()[..bytes_received]);
        let (bytes_sent, _) = ring.send(client_fd, echo_buffer).await?;

        // Clean up
        unsafe {
            libc::close(client_fd);
        }

        Ok::<(usize, usize), Box<dyn std::error::Error + Send + Sync>>((bytes_received, bytes_sent))
    });

    // Connect as client and send data
    let client_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port))?;
        
        let test_data = b"Echo test message";
        client.write_all(test_data)?;
        client.flush()?;

        let mut received = vec![0u8; test_data.len()];
        client.read_exact(&mut received)?;

        Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(received)
    });

    // Wait for both tasks to complete
    let (echo_result, client_result) = tokio::try_join!(echo_task, client_task)?;
    let (bytes_received, bytes_sent) = echo_result?;
    let received_data = client_result?;

    // Verify the echo worked correctly
    assert_eq!(bytes_received, bytes_sent);
    assert_eq!(received_data, b"Echo test message");

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_multiple_concurrent_connections() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    const NUM_CONNECTIONS: usize = 5;
    let mut tasks = Vec::new();

    // Start multiple client connections
    for i in 0..NUM_CONNECTIONS {
        let client_task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(i as u64 * 10)).await;
            let mut client = TcpStream::connect(format!("127.0.0.1:{}", port))?;
            
            let test_data = format!("Message from client {}", i);
            client.write_all(test_data.as_bytes())?;
            client.flush()?;

            let mut received = vec![0u8; test_data.len()];
            client.read_exact(&mut received)?;

            Ok::<String, Box<dyn std::error::Error + Send + Sync>>(String::from_utf8(received)?)
        });
        tasks.push(client_task);
    }

    // Handle connections on server side
    let server_task = tokio::spawn(async move {
        let mut results = Vec::new();
        
        for _ in 0..NUM_CONNECTIONS {
            // Accept connection
            let client_fd = ring.accept(listener_fd).await?;

            // Receive data
            let mut buffer = PinnedBuffer::with_capacity(1024);
            let (bytes_received, buffer) = ring.recv(client_fd, buffer.as_mut_slice()).await?;

            // Echo back
            let echo_buffer = Pin::new(&mut buffer.as_mut()[..bytes_received]);
            let (bytes_sent, _) = ring.send(client_fd, echo_buffer).await?;

            results.push((bytes_received, bytes_sent));

            // Clean up
            unsafe {
                libc::close(client_fd);
            }
        }

        Ok::<Vec<(usize, usize)>, Box<dyn std::error::Error + Send + Sync>>(results)
    });

    // Wait for all tasks to complete
    let server_results = server_task.await??;
    let mut client_results = Vec::new();
    for task in tasks {
        client_results.push(task.await??);
    }

    // Verify all connections were handled correctly
    assert_eq!(server_results.len(), NUM_CONNECTIONS);
    assert_eq!(client_results.len(), NUM_CONNECTIONS);

    for (i, (bytes_received, bytes_sent)) in server_results.iter().enumerate() {
        let expected_message = format!("Message from client {}", i);
        assert_eq!(*bytes_received, expected_message.len());
        assert_eq!(*bytes_sent, expected_message.len());
    }

    // Verify client messages were echoed correctly
    for (i, received_message) in client_results.iter().enumerate() {
        let expected_message = format!("Message from client {}", i);
        assert!(received_message.contains(&expected_message));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_operation_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;

    // Test with invalid file descriptor
    let invalid_fd: RawFd = -1;
    let mut buffer = PinnedBuffer::with_capacity(1024);

    // These operations should fail gracefully
    let send_result = ring.send(invalid_fd, buffer.as_mut_slice()).await;
    assert!(send_result.is_err());

    let recv_result = ring.recv(invalid_fd, buffer.as_mut_slice()).await;
    assert!(recv_result.is_err());

    let accept_result = ring.accept(invalid_fd).await;
    assert!(accept_result.is_err());

    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn test_network_operations_unsupported_platform() {
    // On non-Linux platforms, we should still be able to create operations
    // but they won't actually execute
    let ring_result = Ring::new(32);
    
    // Ring creation should fail gracefully on unsupported platforms
    assert!(ring_result.is_err());
}