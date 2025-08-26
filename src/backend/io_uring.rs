//! io_uring backend implementation.

use std::io;
use std::os::unix::io::RawFd;

use crate::backend::Backend;
use crate::error::{Result, SaferRingError};
use crate::operation::OperationType;

#[cfg(target_os = "linux")]
use io_uring::{opcode, types, IoUring};

/// io_uring-based backend for high-performance I/O.
#[cfg(target_os = "linux")]
pub struct IoUringBackend {
    ring: IoUring,
    in_flight: std::collections::HashMap<u64, ()>,
}

#[cfg(target_os = "linux")]
impl IoUringBackend {
    /// Create a new io_uring backend.
    pub fn new(entries: u32) -> Result<Self> {
        let ring = IoUring::new(entries)?;
        Ok(Self {
            ring,
            in_flight: std::collections::HashMap::new(),
        })
    }
}

#[cfg(target_os = "linux")]
impl Backend for IoUringBackend {
    fn submit_operation(
        &mut self,
        op_type: OperationType,
        fd: RawFd,
        offset: u64,
        buffer_ptr: *mut u8,
        buffer_len: usize,
        user_data: u64,
    ) -> Result<()> {
        let entry = match op_type {
            OperationType::Read => opcode::Read::new(types::Fd(fd), buffer_ptr, buffer_len as u32)
                .offset(offset)
                .build()
                .user_data(user_data),
            OperationType::Write => {
                opcode::Write::new(types::Fd(fd), buffer_ptr, buffer_len as u32)
                    .offset(offset)
                    .build()
                    .user_data(user_data)
            }
            OperationType::Accept => opcode::Accept::new(
                types::Fd(fd),
                buffer_ptr as *mut libc::sockaddr,
                buffer_ptr as *mut libc::socklen_t,
            )
            .build()
            .user_data(user_data),
            OperationType::Send => opcode::Send::new(types::Fd(fd), buffer_ptr, buffer_len as u32)
                .build()
                .user_data(user_data),
            OperationType::Recv => opcode::Recv::new(types::Fd(fd), buffer_ptr, buffer_len as u32)
                .build()
                .user_data(user_data),
            OperationType::ReadVectored | OperationType::WriteVectored => {
                return Err(SaferRingError::Io(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "Operation {:?} not yet implemented in io_uring backend",
                        op_type
                    ),
                )));
            }
        };

        unsafe {
            self.ring.submission().push(&entry).map_err(|e| {
                SaferRingError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to push submission queue entry: {:?}", e),
                ))
            })?;
        }

        self.in_flight.insert(user_data, ());
        self.ring.submit()?;

        Ok(())
    }

    fn try_complete(&mut self) -> Result<Vec<(u64, io::Result<i32>)>> {
        let mut completions = Vec::new();
        let mut cq = self.ring.completion();

        for cqe in &mut cq {
            let user_data = cqe.user_data();
            let result = if cqe.result() < 0 {
                Err(io::Error::from_raw_os_error(-cqe.result()))
            } else {
                Ok(cqe.result())
            };

            self.in_flight.remove(&user_data);
            completions.push((user_data, result));
        }

        cq.sync();
        Ok(completions)
    }

    fn wait_for_completion(&mut self) -> Result<Vec<(u64, io::Result<i32>)>> {
        if self.in_flight.is_empty() {
            return Err(SaferRingError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No operations in flight to wait for",
            )));
        }

        self.ring.submit_and_wait(1)?;
        self.try_complete()
    }

    fn operations_in_flight(&self) -> usize {
        self.in_flight.len()
    }

    fn name(&self) -> &'static str {
        "io_uring"
    }
}

/// Stub implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub struct IoUringBackend;

#[cfg(not(target_os = "linux"))]
impl IoUringBackend {
    /// Create a new io_uring backend (stub for non-Linux platforms).
    pub fn new(_entries: u32) -> Result<Self> {
        Err(SaferRingError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring is only supported on Linux",
        )))
    }
}

#[cfg(not(target_os = "linux"))]
impl Backend for IoUringBackend {
    fn submit_operation(
        &mut self,
        _op_type: OperationType,
        _fd: RawFd,
        _offset: u64,
        _buffer_ptr: *mut u8,
        _buffer_len: usize,
        _user_data: u64,
    ) -> Result<()> {
        Err(SaferRingError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring is only supported on Linux",
        )))
    }

    fn try_complete(&mut self) -> Result<Vec<(u64, io::Result<i32>)>> {
        Ok(Vec::new())
    }

    fn wait_for_completion(&mut self) -> Result<Vec<(u64, io::Result<i32>)>> {
        Err(SaferRingError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring is only supported on Linux",
        )))
    }

    fn operations_in_flight(&self) -> usize {
        0
    }

    fn name(&self) -> &'static str {
        "io_uring (unsupported)"
    }
}
