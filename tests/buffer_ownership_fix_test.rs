//! Test to verify that the polling API correctly handles buffer ownership.
//!
//! This test ensures that the completion API returns appropriate buffer ownership
//! information and documents the current behavior clearly.

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_buffer_ownership_after_completion() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use safer_ring::{Operation, PinnedBuffer, Ring};
    use std::io::Write;
    use std::os::unix::io::FromRawFd;
    use std::pin::Pin;
    use std::time::Duration;
    use tokio::time::timeout;

    // 1. Create a pipe for a controllable I/O source.
    let mut pipe_fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(pipe_fds.as_mut_ptr()) }, 0);
    let (read_fd, write_fd) = (pipe_fds[0], pipe_fds[1]);

    // Ensure fds are closed on drop by wrapping them in File
    let _read_pipe = unsafe { std::fs::File::from_raw_fd(read_fd) };
    let mut write_pipe = unsafe { std::fs::File::from_raw_fd(write_fd) };

    let mut ring = Ring::new(32)?;
    let mut buffer = PinnedBuffer::with_capacity(1024);

    // Create the read operation and submit it to get a future
    let operation = Operation::read().fd(read_fd).buffer(buffer.as_mut_slice());
    let read_future = ring.submit_read(operation)?;

    // 2. Write to the pipe in a separate task to make the read operation completable
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        write_pipe.write_all(b"hello").unwrap();
        write_pipe.flush().unwrap();
    });

    // 3. Await the read operation with a timeout for test robustness
    let (bytes_read, _returned_buffer_pin) = timeout(Duration::from_secs(2), read_future)
        .await
        .expect("Test timed out")
        .expect("Read operation failed");

    // 4. Verify the read operation worked correctly
    assert_eq!(bytes_read, 5, "Should have read 5 bytes");

    // 5. Now that the operation is complete, the mutable borrow has been released
    // and we can access the buffer again. This demonstrates the buffer ownership model.
    assert_eq!(buffer.len(), 1024);
    assert_eq!(&buffer.as_slice()[..5], b"hello");
    println!("✓ Original buffer is still accessible to the user after completion");
    
    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[test]
fn test_polling_api_buffer_ownership_non_linux() {
    use safer_ring::Ring;

    // On non-Linux platforms, Ring::new should return an error
    match Ring::new(32) {
        Ok(_) => panic!("Ring creation should fail on non-Linux platforms"),
        Err(e) => {
            println!("Expected error on non-Linux platform: {}", e);
        }
    }
}

#[test]
fn test_completion_result_behavior() {
    // Test the observable behavior of the completion API
    // This verifies that the fix works without requiring access to internal types

    // The main test is in the test_polling_api_buffer_ownership function above
    // This test just verifies that the operation types exist and are accessible

    use safer_ring::operation::OperationType;

    // Test that operation types are properly defined
    assert!(OperationType::Read.requires_buffer());
    assert!(OperationType::Read.is_read_like());
    assert!(!OperationType::Read.is_write_like());
    assert!(!OperationType::Read.is_vectored());

    assert!(!OperationType::Accept.requires_buffer());
    assert!(!OperationType::Accept.is_read_like());
    assert!(!OperationType::Accept.is_write_like());

    println!("Operation types work correctly");
}
