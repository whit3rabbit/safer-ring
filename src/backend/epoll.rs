//! epoll-based fallback backend implementation.
//!
//! This backend provides a fallback mechanism for environments where io_uring
//! is not available, such as older kernels, containers with restricted capabilities,
//! or cloud environments that disable io_uring.

use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;

use crate::backend::Backend;
use crate::error::{Result, SaferRingError};
use crate::operation::OperationType;

/// Pending operation in the epoll backend
#[derive(Debug)]
#[allow(dead_code)] // Fields used only in actual epoll implementation, not stub
struct PendingOperation {
    op_type: OperationType,
    fd: RawFd,
    offset: u64,
    buffer_ptr: *mut u8,
    buffer_len: usize,
    user_data: u64,
}

/// epoll-based backend implementation for fallback support
#[allow(dead_code)] // Fields used only in actual epoll implementation, not stub
pub struct EpollBackend {
    epoll_fd: RawFd,
    pending_operations: HashMap<u64, PendingOperation>,
    next_operation_id: u64,
}

impl EpollBackend {
    /// Create a new epoll backend.
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            // Create epoll instance
            let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };

            if epoll_fd == -1 {
                return Err(SaferRingError::Io(io::Error::last_os_error()));
            }

            Ok(Self {
                epoll_fd,
                pending_operations: HashMap::new(),
                next_operation_id: 1,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SaferRingError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll backend is only supported on Linux",
            )))
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for EpollBackend {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.epoll_fd);
        }
    }
}

impl Backend for EpollBackend {
    fn submit_operation(
        &mut self,
        op_type: OperationType,
        fd: RawFd,
        offset: u64,
        buffer_ptr: *mut u8,
        buffer_len: usize,
        user_data: u64,
    ) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let operation = PendingOperation {
                op_type,
                fd,
                offset,
                buffer_ptr,
                buffer_len,
                user_data,
            };

            // For read/recv operations, register for EPOLLIN
            // For write/send operations, register for EPOLLOUT
            let events = match op_type {
                OperationType::Read | OperationType::ReadVectored | OperationType::Recv => {
                    libc::EPOLLIN | libc::EPOLLONESHOT
                }
                OperationType::Write | OperationType::WriteVectored | OperationType::Send => {
                    libc::EPOLLOUT | libc::EPOLLONESHOT
                }
                OperationType::Accept => libc::EPOLLIN | libc::EPOLLONESHOT,
            };

            let mut event = libc::epoll_event {
                events: events as u32,
                u64: user_data,
            };

            let result =
                unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut event) };

            if result == -1 {
                return Err(SaferRingError::Io(io::Error::last_os_error()));
            }

            self.pending_operations.insert(user_data, operation);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (op_type, fd, offset, buffer_ptr, buffer_len, user_data);
            Err(SaferRingError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll backend is only supported on Linux",
            )))
        }
    }

    fn try_complete(&mut self) -> Result<Vec<(u64, io::Result<i32>)>> {
        #[cfg(target_os = "linux")]
        {
            self.poll_completions(0) // Non-blocking
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(Vec::new())
        }
    }

    fn wait_for_completion(&mut self) -> Result<Vec<(u64, io::Result<i32>)>> {
        #[cfg(target_os = "linux")]
        {
            if self.pending_operations.is_empty() {
                return Err(SaferRingError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No operations to wait for",
                )));
            }

            self.poll_completions(-1) // Blocking
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SaferRingError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll backend is only supported on Linux",
            )))
        }
    }

    fn operations_in_flight(&self) -> usize {
        self.pending_operations.len()
    }

    fn name(&self) -> &'static str {
        "epoll"
    }

    fn register_files(&mut self, _fds: &[RawFd]) -> Result<u32> {
        // Epoll doesn't support file descriptor registration
        // This is a no-op that pretends to work for compatibility
        Ok(0)
    }

    fn unregister_files(&mut self) -> Result<()> {
        // Epoll doesn't support file descriptor registration
        // This is a no-op that pretends to work for compatibility
        Ok(())
    }

    fn register_buffers(&mut self, _buffers: &[Pin<Box<[u8]>>]) -> Result<u32> {
        // Epoll doesn't support buffer registration
        // This is a no-op that pretends to work for compatibility
        Ok(0)
    }

    fn unregister_buffers(&mut self) -> Result<()> {
        // Epoll doesn't support buffer registration
        // This is a no-op that pretends to work for compatibility
        Ok(())
    }

    fn capacity(&self) -> u32 {
        // Epoll doesn't have a submission queue, return a reasonable default
        1024
    }

    fn completion_queue_stats(&mut self) -> (usize, usize) {
        // Epoll doesn't have a completion queue, return pending operations count
        (self.pending_operations.len(), 1024)
    }
}

#[cfg(target_os = "linux")]
impl EpollBackend {
    /// Poll for ready file descriptors and execute operations
    fn poll_completions(&mut self, timeout_ms: i32) -> Result<Vec<(u64, io::Result<i32>)>> {
        const MAX_EVENTS: usize = 64;
        let mut events = Vec::with_capacity(MAX_EVENTS);
        events.resize_with(MAX_EVENTS, || libc::epoll_event { events: 0, u64: 0 });

        let num_events = unsafe {
            libc::epoll_wait(
                self.epoll_fd,
                events.as_mut_ptr(),
                MAX_EVENTS as i32,
                timeout_ms,
            )
        };

        if num_events == -1 {
            return Err(SaferRingError::Io(io::Error::last_os_error()));
        }

        let mut completed = Vec::with_capacity(num_events as usize);

        for i in 0..num_events as usize {
            let event = &events[i];
            let user_data = event.u64;

            if let Some(operation) = self.pending_operations.remove(&user_data) {
                // Execute the I/O operation
                let result = self.execute_operation(&operation);
                completed.push((user_data, result));

                // Remove from epoll (already done via EPOLLONESHOT)
            }
        }

        Ok(completed)
    }

    /// Execute a single I/O operation
    fn execute_operation(&self, op: &PendingOperation) -> io::Result<i32> {
        match op.op_type {
            OperationType::Read => {
                // Use pread if offset is provided (non-zero), otherwise use read
                let result = if op.offset > 0 {
                    unsafe {
                        libc::pread(
                            op.fd,
                            op.buffer_ptr as *mut libc::c_void,
                            op.buffer_len,
                            op.offset as libc::off_t,
                        )
                    }
                } else {
                    unsafe { libc::read(op.fd, op.buffer_ptr as *mut libc::c_void, op.buffer_len) }
                };

                if result == -1 {
                    let error = io::Error::last_os_error();
                    // Handle EAGAIN/EWOULDBLOCK by re-registering for epoll
                    if error.kind() == io::ErrorKind::WouldBlock {
                        // This should not happen with epoll, but handle it gracefully
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "Operation would block unexpectedly",
                        ));
                    }
                    Err(error)
                } else {
                    Ok(result as i32)
                }
            }

            OperationType::Write => {
                // Use pwrite if offset is provided (non-zero), otherwise use write
                let result = if op.offset > 0 {
                    unsafe {
                        libc::pwrite(
                            op.fd,
                            op.buffer_ptr as *const libc::c_void,
                            op.buffer_len,
                            op.offset as libc::off_t,
                        )
                    }
                } else {
                    unsafe {
                        libc::write(op.fd, op.buffer_ptr as *const libc::c_void, op.buffer_len)
                    }
                };

                if result == -1 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::WouldBlock {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "Operation would block unexpectedly",
                        ));
                    }
                    Err(error)
                } else {
                    Ok(result as i32)
                }
            }

            OperationType::Recv => {
                let result = unsafe {
                    libc::recv(op.fd, op.buffer_ptr as *mut libc::c_void, op.buffer_len, 0)
                };
                if result == -1 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::WouldBlock {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "Operation would block unexpectedly",
                        ));
                    }
                    Err(error)
                } else {
                    Ok(result as i32)
                }
            }

            OperationType::Send => {
                let result = unsafe {
                    libc::send(
                        op.fd,
                        op.buffer_ptr as *const libc::c_void,
                        op.buffer_len,
                        0,
                    )
                };
                if result == -1 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::WouldBlock {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "Operation would block unexpectedly",
                        ));
                    }
                    Err(error)
                } else {
                    Ok(result as i32)
                }
            }

            OperationType::Accept => {
                let result =
                    unsafe { libc::accept(op.fd, std::ptr::null_mut(), std::ptr::null_mut()) };
                if result == -1 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::WouldBlock {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "Operation would block unexpectedly",
                        ));
                    }
                    Err(error)
                } else {
                    Ok(result as i32)
                }
            }

            OperationType::ReadVectored => {
                // For vectored read, we need to interpret the buffer as an iovec array
                // This is a simplified implementation that treats it as a single buffer
                // A full implementation would need to handle the iovec structure properly
                let result = if op.offset > 0 {
                    unsafe {
                        libc::pread(
                            op.fd,
                            op.buffer_ptr as *mut libc::c_void,
                            op.buffer_len,
                            op.offset as libc::off_t,
                        )
                    }
                } else {
                    unsafe { libc::read(op.fd, op.buffer_ptr as *mut libc::c_void, op.buffer_len) }
                };

                if result == -1 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::WouldBlock {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "Operation would block unexpectedly",
                        ));
                    }
                    Err(error)
                } else {
                    Ok(result as i32)
                }
            }

            OperationType::WriteVectored => {
                // For vectored write, we need to interpret the buffer as an iovec array
                // This is a simplified implementation that treats it as a single buffer
                // A full implementation would need to handle the iovec structure properly
                let result = if op.offset > 0 {
                    unsafe {
                        libc::pwrite(
                            op.fd,
                            op.buffer_ptr as *const libc::c_void,
                            op.buffer_len,
                            op.offset as libc::off_t,
                        )
                    }
                } else {
                    unsafe {
                        libc::write(op.fd, op.buffer_ptr as *const libc::c_void, op.buffer_len)
                    }
                };

                if result == -1 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::WouldBlock {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "Operation would block unexpectedly",
                        ));
                    }
                    Err(error)
                } else {
                    Ok(result as i32)
                }
            }
        }
    }
}
