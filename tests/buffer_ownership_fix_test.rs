//! Test to verify that the polling API correctly handles buffer ownership.
//!
//! This test ensures that the completion API returns appropriate buffer ownership
//! information and documents the current behavior clearly.

#[cfg(target_os = "linux")]
#[test]
fn test_polling_api_buffer_ownership() {
    use safer_ring::{Operation, PinnedBuffer, Ring};
    use std::io::Write;
    use std::os::unix::io::FromRawFd;

    // 1. Create a pipe for a controllable I/O source.
    let mut pipe_fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(pipe_fds.as_mut_ptr()) }, 0);
    let (read_fd, write_fd) = (pipe_fds[0], pipe_fds[1]);

    // Ensure fds are closed on drop by wrapping them in File
    let _read_pipe = unsafe { std::fs::File::from_raw_fd(read_fd) };
    let mut write_pipe = unsafe { std::fs::File::from_raw_fd(write_fd) };

    let mut ring = Ring::new(32).expect("Failed to create ring");
    let mut buffer = PinnedBuffer::with_capacity(1024);
    let operation_id;

    // Scope the submission to control borrow lifetimes
    {
        let operation = Operation::read()
            .fd(read_fd)
            .buffer(buffer.as_mut_slice());

        let submitted = ring.submit(operation).expect("Failed to submit operation");
        operation_id = submitted.id();
    } // `operation` and `submitted` are dropped, but the borrow is held by `ring`'s tracker.

    // 2. Write to the pipe to make the read operation completable.
    write_pipe
        .write_all(b"hello")
        .expect("Failed to write to pipe");
    write_pipe.flush().expect("Failed to flush pipe");

    // 3. Poll for completion.
    let mut completion_found = false;
    for _ in 0..10 {
        // Poll a few times to give the kernel time to process
        let completions = ring.try_complete().expect("Failed to check completions");
        for completion in completions {
            if completion.id() == operation_id {
                let (result, buffer_ownership) = completion.into_result();

                // 4. Assert on the CompletionResult.
                // The current API uses borrows, so ownership isn't returned here.
                assert!(
                    buffer_ownership.is_none(),
                    "Buffer ownership should be None with the current borrowed reference API"
                );

                let bytes_read = result.expect("Read operation should succeed");
                assert_eq!(bytes_read, 5);
                completion_found = true;
                break;
            }
        }
        if completion_found {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(completion_found, "The read operation did not complete");

    // 5. Now that the operation is complete and removed from the ring's tracker,
    // the mutable borrow has ended, and we can access the buffer again.
    assert_eq!(buffer.len(), 1024);
    assert_eq!(&buffer.as_slice()[..5], b"hello");
    println!("Original buffer is still accessible to the user after completion");
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