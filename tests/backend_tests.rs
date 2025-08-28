//! Backend-specific tests to verify that both io_uring and epoll backends work correctly.
//!
//! These tests ensure that the backend abstraction works properly and that
//! the epoll fallback provides correct functionality when io_uring is unavailable.

use safer_ring::backend::detect_backend;
use safer_ring::backend::epoll::EpollBackend;
use safer_ring::backend::Backend;
#[cfg(target_os = "linux")]
use safer_ring::operation::OperationType;
#[cfg(target_os = "linux")]
use std::io;

/// Test that backend detection works correctly
#[test]
fn test_backend_detection() {
    #[cfg(target_os = "linux")]
    {
        // On Linux, we should be able to detect a backend
        let backend_result = detect_backend(32);
        assert!(
            backend_result.is_ok(),
            "Backend detection should work on Linux: {:?}",
            backend_result.err()
        );

        let backend = backend_result.unwrap();
        let name = backend.name();
        assert!(
            name == "io_uring" || name == "epoll",
            "Backend should be either io_uring or epoll, got: {}",
            name
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        // On non-Linux platforms, backend detection should fail gracefully
        let backend_result = detect_backend(32);
        assert!(
            backend_result.is_err(),
            "Backend detection should fail on non-Linux platforms"
        );
    }
}

/// Test epoll backend creation
#[test]
fn test_epoll_backend_creation() {
    #[cfg(target_os = "linux")]
    {
        let epoll_backend = EpollBackend::new();
        assert!(
            epoll_backend.is_ok(),
            "EpollBackend creation should succeed on Linux: {:?}",
            epoll_backend.err()
        );

        let backend = epoll_backend.unwrap();
        assert_eq!(backend.name(), "epoll");
        assert_eq!(backend.operations_in_flight(), 0);
    }

    #[cfg(not(target_os = "linux"))]
    {
        let epoll_backend = EpollBackend::new();
        assert!(
            epoll_backend.is_err(),
            "EpollBackend creation should fail on non-Linux platforms"
        );
    }
}

/// Test basic epoll backend operations with a pipe
#[test]
fn test_epoll_backend_basic_operations() {
    #[cfg(target_os = "linux")]
    {
        let mut backend = EpollBackend::new().expect("Failed to create EpollBackend");

        // Create a pipe for testing
        let (read_fd, write_fd) = create_pipe().expect("Failed to create pipe");

        // Test submitting a read operation
        let buffer = vec![0u8; 64];
        let buffer_ptr = buffer.as_ptr() as *mut u8;
        let user_data = 42;

        let submit_result = backend.submit_operation(
            OperationType::Read,
            read_fd,
            0, // offset (ignored for pipes)
            buffer_ptr,
            buffer.len(),
            user_data,
        );

        assert!(
            submit_result.is_ok(),
            "Submit read operation should succeed: {:?}",
            submit_result.err()
        );

        assert_eq!(backend.operations_in_flight(), 1);

        // Write some data to the pipe to make the read operation ready
        let test_data = b"Hello, epoll backend!";
        unsafe {
            libc::write(
                write_fd,
                test_data.as_ptr() as *const libc::c_void,
                test_data.len(),
            );
        }

        // Try to complete the operation
        let completion_result = backend.try_complete();
        assert!(
            completion_result.is_ok(),
            "try_complete should succeed: {:?}",
            completion_result.err()
        );

        let completions = completion_result.unwrap();
        if !completions.is_empty() {
            assert_eq!(completions.len(), 1);
            let (completed_user_data, io_result) = &completions[0];
            assert_eq!(*completed_user_data, user_data);
            assert!(
                io_result.is_ok(),
                "I/O operation should succeed: {:?}",
                io_result
            );

            let bytes_read = io_result.as_ref().unwrap();
            assert!(*bytes_read > 0, "Should have read some bytes");
        }

        // Clean up
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }
}

/// Test that epoll backend handles multiple operations correctly
#[test]
fn test_epoll_backend_multiple_operations() {
    #[cfg(target_os = "linux")]
    {
        let mut backend = EpollBackend::new().expect("Failed to create EpollBackend");

        // Create multiple pipes for testing
        let pipes: Vec<_> = (0..3)
            .map(|_| create_pipe().expect("Failed to create pipe"))
            .collect();

        let buffers: Vec<Vec<u8>> = (0..3).map(|_| vec![0u8; 32]).collect();

        // Submit read operations for all pipes
        for (i, ((read_fd, _write_fd), buffer)) in pipes.iter().zip(buffers.iter()).enumerate() {
            let submit_result = backend.submit_operation(
                OperationType::Read,
                *read_fd,
                0,
                buffer.as_ptr() as *mut u8,
                buffer.len(),
                i as u64,
            );
            assert!(
                submit_result.is_ok(),
                "Submit operation {} should succeed",
                i
            );
        }

        assert_eq!(backend.operations_in_flight(), 3);

        // Write data to make operations ready
        for (i, ((_read_fd, write_fd), _buffer)) in pipes.iter().zip(buffers.iter()).enumerate() {
            let test_data = format!("Data for pipe {}", i);
            unsafe {
                libc::write(
                    *write_fd,
                    test_data.as_ptr() as *const libc::c_void,
                    test_data.len(),
                );
            }
        }

        // Complete operations
        let completion_result = backend.wait_for_completion();
        assert!(
            completion_result.is_ok(),
            "wait_for_completion should succeed"
        );

        // Clean up
        for (read_fd, write_fd) in pipes {
            unsafe {
                libc::close(read_fd);
                libc::close(write_fd);
            }
        }
    }
}

/// Test error handling in epoll backend
#[test]
fn test_epoll_backend_error_handling() {
    #[cfg(target_os = "linux")]
    {
        let mut backend = EpollBackend::new().expect("Failed to create EpollBackend");

        // Try to submit operation with invalid file descriptor
        let buffer = vec![0u8; 32];
        let submit_result = backend.submit_operation(
            OperationType::Read,
            -1, // Invalid fd
            0,
            buffer.as_ptr() as *mut u8,
            buffer.len(),
            123,
        );

        // This should fail because the fd is invalid
        assert!(submit_result.is_err(), "Submit with invalid fd should fail");
    }
}

#[cfg(target_os = "linux")]
fn create_pipe() -> io::Result<(i32, i32)> {
    let mut pipe_fds = [0i32; 2];
    let result = unsafe { libc::pipe(pipe_fds.as_mut_ptr()) };

    if result == -1 {
        return Err(io::Error::last_os_error());
    }

    // Make both ends non-blocking for testing
    for &fd in &pipe_fds {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags != -1 {
            unsafe {
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }
    }

    Ok((pipe_fds[0], pipe_fds[1]))
}

/// Test backend name reporting
#[test]
fn test_backend_names() {
    #[cfg(target_os = "linux")]
    {
        let epoll_backend = EpollBackend::new().expect("Failed to create EpollBackend");
        assert_eq!(epoll_backend.name(), "epoll");
    }
}

/// Smoke test for non-Linux platforms
#[test]
fn test_non_linux_backend_behavior() {
    #[cfg(not(target_os = "linux"))]
    {
        // On non-Linux platforms, all backend operations should fail gracefully
        let backend_result = detect_backend(32);
        assert!(
            backend_result.is_err(),
            "Backend detection should fail on non-Linux"
        );

        let epoll_result = EpollBackend::new();
        assert!(
            epoll_result.is_err(),
            "EpollBackend creation should fail on non-Linux"
        );
    }
}
