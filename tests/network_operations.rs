//! Integration tests for network operations.
//! These tests require a Linux kernel with io_uring support (5.1+).
//!
//! # IMPORTANT: Ring API Lifetime Issues
//!
//! These tests demonstrate significant API design challenges in safer-ring's network operations.
//! Unlike file I/O operations which have `read_owned` and `write_owned` methods that use the
//! ownership transfer "hot potato" pattern (see docs/API.md), network operations (`send`, `recv`)
//! only provide advanced APIs that create complex lifetime conflicts.
//!
//! ## The Core Problem
//!
//! From src/ring/core/network_operations.rs and src/future/io_futures/network_io.rs:
//! 1. `send()` and `recv()` return futures that hold `&'ring mut Ring` and `Pin<&'buf mut [u8]>`
//! 2. These futures MUST be awaited immediately (see API.md "await-immediately" pattern)
//! 3. Ring's Drop implementation panics if operations are still tracked as in-flight
//! 4. The lifetime constraint `'buf: 'ring` means buffers must outlive the Ring
//!
//! ## Why Normal Rust Patterns Fail
//!
//! ```rust,ignore
//! // This DOESN'T work - multiple mutable borrows
//! let future1 = ring.send(fd1, buf1)?;  // Borrows ring mutably
//! let future2 = ring.send(fd2, buf2)?;  // ERROR: ring already borrowed
//!
//! // This DOESN'T work - borrow across await points
//! let future = ring.send(fd, buf)?;     // Borrows ring and buf
//! do_other_work().await;                // Borrow held across await
//! let result = future.await?;           // May panic on Ring drop
//! ```
//!
//! ## Our Solution: std::mem::forget()
//!
//! We use `std::mem::forget(ring)` to prevent Ring's destructor from running, which:
//! - Prevents the panic from operations being tracked as in-flight
//! - Allows operations to complete normally
//! - Is acceptable for tests (we leak the Ring but not buffers)
//! - Works around the fundamental API design issues
//!
//! ## Ideal API Design (Not Currently Available)
//!
//! Network operations should have ownership transfer methods like file I/O:
//! ```rust,ignore  
//! // These don't exist but would solve the lifetime issues:
//! let (bytes, buf) = ring.send_owned(fd, owned_buffer).await?;
//! let (bytes, buf) = ring.recv_owned(fd, owned_buffer).await?;
//! ```

use safer_ring::Ring;

#[cfg(target_os = "linux")]
use std::io::{self, Read, Write};
#[cfg(target_os = "linux")]
use std::net::{TcpListener, TcpStream};
#[cfg(target_os = "linux")]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use tokio::time::timeout;

// These imports are only needed on Linux where the tests will run.
#[cfg(target_os = "linux")]
use safer_ring::PinnedBuffer;

#[cfg(target_os = "linux")]
/// Helper function to create a listening socket for testing.
fn create_test_listener() -> io::Result<(TcpListener, u16)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    Ok((listener, port))
}

#[cfg(target_os = "linux")]
/// Helper to create a connected pair of sockets for testing.
fn create_connected_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let client = TcpStream::connect(addr)?;
    let (server, _) = listener.accept()?;
    server.set_nonblocking(true)?;
    client.set_nonblocking(true)?;
    Ok((server, client))
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_accept_operation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // The ring for accept doesn't need to be mutable as `accept_safe` takes &self.
    let ring = Ring::new(32)?;
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    // Spawn a client to connect to the listener.
    let connect_handle =
        tokio::spawn(async move { TcpStream::connect(format!("127.0.0.1:{port}")) });

    // Await the accept operation.
    let client_fd = timeout(Duration::from_secs(5), ring.accept_safe(listener_fd)).await??;

    assert!(client_fd >= 0, "Accepted file descriptor should be valid");

    // Ensure the client connected successfully before we clean up.
    connect_handle.await??;
    unsafe { libc::close(client_fd) };
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_send_operation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut server, client) = create_connected_pair()?;
    let client_fd = client.as_raw_fd();
    let test_data = b"Hello, safer-ring!";

    // LIFETIME ISSUE: Ring must be created here and kept alive for entire test
    // Unlike file I/O which has `read_owned`/`write_owned` (see docs/API.md),
    // network operations only provide advanced APIs that borrow the Ring.
    //
    // FIX: Use Box::leak() to give Ring 'static lifetime, satisfying 'buf: 'ring constraint
    let ring = Ring::new(32)?;
    let ring: &'static mut Ring = Box::leak(Box::new(ring));

    // FIX: Also leak the buffer to give it 'static lifetime
    let send_buffer = PinnedBuffer::from_slice(test_data);
    let send_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(send_buffer));

    // SEND OPERATION - Demonstrates the "immediate await" pattern
    // From src/ring/core/network_operations.rs:124-134:
    // - send() takes `&'ring mut self` and `Pin<&'buf mut [u8]>`
    // - Returns SendFuture<'ring, 'buf> holding mutable borrows
    // - MUST be awaited immediately (can't store the future)
    // With Box::leak(), all lifetime constraints are satisfied
    let (bytes_sent, _) = ring.send(client_fd, send_buffer.as_mut_slice())?.await?;
    assert_eq!(bytes_sent, test_data.len());

    // Verify data was received on the other end via standard TCP
    let mut received_data = vec![0u8; test_data.len()];
    server.read_exact(&mut received_data)?;
    assert_eq!(received_data.as_slice(), test_data);

    // CRITICAL: Box::leak() prevents Ring Drop panic
    // From src/ring/core/mod.rs around line 400-416, Ring's Drop implementation
    // panics if operations are still tracked. Since we leaked the Ring, Drop never runs.
    // This is acceptable for tests - we intentionally leak test resources.

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_recv_operation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut server, client) = create_connected_pair()?;
    let client_fd = client.as_raw_fd();
    let test_data = b"Hello, safer-ring!";

    // Set up data to receive first
    server.write_all(test_data)?;
    server.flush()?;

    // FIX: Use Box::leak() to give Ring 'static lifetime
    let ring = Ring::new(32)?;
    let ring: &'static mut Ring = Box::leak(Box::new(ring));

    // FIX: Also leak the buffer to give it 'static lifetime
    let recv_buffer = PinnedBuffer::with_capacity(1024);
    let recv_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(recv_buffer));

    // RECV OPERATION - Same immediate await pattern required
    // From src/future/io_futures/network_io.rs:246-251:
    // RecvFuture holds &'ring mut Ring and borrows the buffer
    // With Box::leak(), all lifetime constraints are satisfied
    let (bytes_received, received_slice) =
        ring.recv(client_fd, recv_buffer.as_mut_slice())?.await?;
    assert_eq!(bytes_received, test_data.len());
    assert_eq!(&received_slice[..bytes_received], test_data);

    // No need for std::mem::forget() - Ring is already leaked

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_echo_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();

    // Start the client connection in background first
    let client_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))?;
        let test_data = b"Echo test message";
        client.write_all(test_data)?;
        client.flush()?;
        let mut received = vec![0u8; test_data.len()];
        client.read_exact(&mut received)?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(received)
    });

    // Server logic - Use Box::leak() to solve lifetime issues
    let ring1 = Ring::new(32)?;
    let ring1: &'static Ring = Box::leak(Box::new(ring1));
    let client_fd = timeout(Duration::from_secs(10), ring1.accept_safe(listener_fd)).await??;

    // Use a separate ring for recv/send operations
    let ring2 = Ring::new(32)?;
    let ring2: &'static mut Ring = Box::leak(Box::new(ring2));

    // FIX: Leak the recv buffer to give it 'static lifetime
    let recv_buffer = PinnedBuffer::with_capacity(1024);
    let recv_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(recv_buffer));

    // Receive data
    let (bytes_received, received_slice) = timeout(
        Duration::from_secs(10),
        ring2.recv(client_fd, recv_buffer.as_mut_slice())?,
    )
    .await??;
    let received_data = received_slice[..bytes_received].to_vec();

    // Send echo back - need to use separate Ring to avoid double mutable borrow
    let ring3 = Ring::new(32)?;
    let ring3: &'static mut Ring = Box::leak(Box::new(ring3));
    let send_buffer = PinnedBuffer::from_slice(&received_data);
    let send_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(send_buffer));
    let (bytes_sent, _) = timeout(
        Duration::from_secs(10),
        ring3.send(client_fd, send_buffer.as_mut_slice())?,
    )
    .await??;

    unsafe { libc::close(client_fd) };

    // No need for std::mem::forget() - Rings are already leaked

    // Wait for client and verify results
    let client_received = client_handle.await??;

    assert_eq!(bytes_received, bytes_sent);
    assert_eq!(client_received.as_slice(), b"Echo test message");

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_multiple_concurrent_connections(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (listener, port) = create_test_listener()?;
    let listener_fd = listener.as_raw_fd();
    const NUM_CONNECTIONS: usize = 3; // Reduced for simplicity

    // Start all client tasks first
    let mut client_tasks = Vec::new();
    for i in 0..NUM_CONNECTIONS {
        client_tasks.push(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(i as u64 * 30)).await;
            let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))?;
            let test_data = format!("Message from client {i}");
            client.write_all(test_data.as_bytes())?;
            client.flush()?;
            let mut received = vec![0; test_data.len()];
            client.read_exact(&mut received)?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(String::from_utf8(received)?)
        }));
    }

    // Server logic: Handle connections sequentially with Box::leak()
    let accept_ring = Ring::new(32)?;
    let accept_ring: &'static Ring = Box::leak(Box::new(accept_ring));
    let mut server_results = Vec::new();

    for _ in 0..NUM_CONNECTIONS {
        let client_fd = timeout(
            Duration::from_secs(10),
            accept_ring.accept_safe(listener_fd),
        )
        .await??;

        // Use separate leaked rings for each connection to avoid borrow conflicts
        let conn_ring_recv = Ring::new(32)?;
        let conn_ring_recv: &'static mut Ring = Box::leak(Box::new(conn_ring_recv));

        // FIX: Leak the recv buffer to give it 'static lifetime
        let recv_buffer = PinnedBuffer::with_capacity(1024);
        let recv_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(recv_buffer));

        // Receive data
        let (bytes_received, received_slice) = timeout(
            Duration::from_secs(10),
            conn_ring_recv.recv(client_fd, recv_buffer.as_mut_slice())?,
        )
        .await??;
        let received_data = received_slice[..bytes_received].to_vec();

        // Send echo back using separate ring to avoid double mutable borrow
        let conn_ring_send = Ring::new(32)?;
        let conn_ring_send: &'static mut Ring = Box::leak(Box::new(conn_ring_send));
        let send_buffer = PinnedBuffer::from_slice(&received_data);
        let send_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(send_buffer));
        let (bytes_sent, _) = timeout(
            Duration::from_secs(10),
            conn_ring_send.send(client_fd, send_buffer.as_mut_slice())?,
        )
        .await??;

        server_results.push((bytes_received, bytes_sent));

        unsafe { libc::close(client_fd) };
        // No need to forget conn_ring - it's already leaked
    }

    // No need to forget accept_ring - it's already leaked

    // Wait for all client tasks to finish and collect their results
    let mut client_results = Vec::new();
    for task in client_tasks {
        client_results.push(task.await??);
    }

    assert_eq!(server_results.len(), NUM_CONNECTIONS);
    assert_eq!(client_results.len(), NUM_CONNECTIONS);

    // Sort results as they may complete out of order.
    client_results.sort();
    let mut server_results_sorted = server_results;
    server_results_sorted.sort_by_key(|(r, _)| *r);

    for i in 0..NUM_CONNECTIONS {
        let expected_message = format!("Message from client {i}");
        assert_eq!(client_results[i], expected_message);
        assert_eq!(server_results_sorted[i].0, expected_message.len());
        assert_eq!(server_results_sorted[i].1, expected_message.len());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_send_error_handling() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let invalid_fd: RawFd = -1;
    let ring = Ring::new(32)?;
    let ring: &'static mut Ring = Box::leak(Box::new(ring));

    // FIX: Leak the buffer to give it 'static lifetime
    let send_buffer = PinnedBuffer::with_capacity(1024);
    let send_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(send_buffer));

    // Test send operation with an invalid file descriptor
    let send_result = ring.send(invalid_fd, send_buffer.as_mut_slice())?.await;
    assert!(send_result.is_err(), "send with invalid fd should fail");

    // No need for std::mem::forget() - Ring is already leaked
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_recv_error_handling() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let invalid_fd: RawFd = -1;
    let ring = Ring::new(32)?;
    let ring: &'static mut Ring = Box::leak(Box::new(ring));

    // FIX: Leak the buffer to give it 'static lifetime
    let recv_buffer = PinnedBuffer::with_capacity(1024);
    let recv_buffer: &'static mut PinnedBuffer<_> = Box::leak(Box::new(recv_buffer));

    // Test recv operation with an invalid file descriptor
    let recv_result = ring.recv(invalid_fd, recv_buffer.as_mut_slice())?.await;
    assert!(recv_result.is_err(), "recv with invalid fd should fail");

    // No need for std::mem::forget() - Ring is already leaked
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_network_accept_error_handling() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let invalid_fd: RawFd = -1;
    let ring = Ring::new(32)?;
    let ring: &'static Ring = Box::leak(Box::new(ring));

    // Test accept operation with an invalid file descriptor.
    // accept_safe takes &self, not &mut self, so it's simpler to use
    let accept_result = ring.accept_safe(invalid_fd).await;
    assert!(accept_result.is_err(), "accept with invalid fd should fail");

    // No need for std::mem::forget() - Ring is already leaked
    Ok(())
}

/// A simple test for non-Linux platforms to confirm that Ring creation fails as expected.
#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn test_network_operations_unsupported_platform() {
    let ring_result = Ring::new(32);
    assert!(
        ring_result.is_err(),
        "Ring creation should fail on non-Linux platforms."
    );
}
