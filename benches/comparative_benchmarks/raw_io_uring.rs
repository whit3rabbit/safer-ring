//! Raw io_uring implementation for performance comparison baseline.

#[cfg(target_os = "linux")]
pub mod linux {
    use io_uring::{opcode, types, IoUring};
    use std::os::unix::io::RawFd;

    /// Raw io_uring wrapper for direct performance comparison.
    ///
    /// This implementation provides minimal abstraction over io_uring
    /// to establish a baseline for measuring safer-ring overhead.
    pub struct RawRing {
        ring: IoUring,
    }

    impl RawRing {
        /// Creates a new raw io_uring instance.
        pub fn new(entries: u32) -> std::io::Result<Self> {
            let ring = IoUring::new(entries)?;
            Ok(Self { ring })
        }

        /// Performs a raw read operation at a specific offset.
        ///
        /// Uses direct syscall interface without safety abstractions
        /// to measure pure io_uring performance characteristics.
        pub async fn read_at_raw(
            &mut self,
            fd: RawFd,
            buf: &mut [u8],
            offset: u64,
        ) -> std::io::Result<usize> {
            let read_e =
                opcode::Read::new(types::Fd(fd), buf.as_mut_ptr(), buf.len() as u32).offset(offset);

            // Direct submission without safety checks for maximum performance
            unsafe {
                let mut sq = self.ring.submission();
                let sqe = sq.next_sqe().expect("submission queue full");
                read_e.build().user_data(0x42).write_to(sqe);
                sq.sync();
            }

            self.ring.submit_and_wait(1)?;

            let mut cq = self.ring.completion();
            let cqe = cq.next().expect("completion queue empty");
            let result = cqe.result();
            cq.sync();

            if result < 0 {
                Err(std::io::Error::from_raw_os_error(-result))
            } else {
                Ok(result as usize)
            }
        }

        /// Performs a raw write operation at a specific offset.
        pub async fn write_at_raw(
            &mut self,
            fd: RawFd,
            buf: &[u8],
            offset: u64,
        ) -> std::io::Result<usize> {
            let write_e =
                opcode::Write::new(types::Fd(fd), buf.as_ptr(), buf.len() as u32).offset(offset);

            unsafe {
                let mut sq = self.ring.submission();
                let sqe = sq.next_sqe().expect("submission queue full");
                write_e.build().user_data(0x43).write_to(sqe);
                sq.sync();
            }

            self.ring.submit_and_wait(1)?;

            let mut cq = self.ring.completion();
            let cqe = cq.next().expect("completion queue empty");
            let result = cqe.result();
            cq.sync();

            if result < 0 {
                Err(std::io::Error::from_raw_os_error(-result))
            } else {
                Ok(result as usize)
            }
        }

        /// Convenience method for reading from offset 0.
        pub async fn read_raw(&mut self, fd: RawFd, buf: &mut [u8]) -> std::io::Result<usize> {
            self.read_at_raw(fd, buf, 0).await
        }

        /// Convenience method for writing to offset 0.
        pub async fn write_raw(&mut self, fd: RawFd, buf: &[u8]) -> std::io::Result<usize> {
            self.write_at_raw(fd, buf, 0).await
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::RawRing;

#[cfg(not(target_os = "linux"))]
pub struct RawRing;

#[cfg(not(target_os = "linux"))]
impl RawRing {
    pub fn new(_entries: u32) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Raw io_uring only available on Linux",
        ))
    }
}
