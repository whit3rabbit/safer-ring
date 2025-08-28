//! File I/O futures for read and write operations.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin as StdPin;
use std::sync::Arc;
use std::task::{Context, Poll};

use super::common::{impl_future_drop, poll_io_operation};
use crate::future::waker::WakerRegistry;
use crate::operation::{Operation, Submitted};
use crate::ring::Ring;

/// Future for file read operations.
///
/// This future polls the completion queue until the read operation completes,
/// then returns the number of bytes read and buffer ownership. The future
/// ensures proper cleanup of waker registrations and handles both successful
/// completions and error conditions.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring Ring instance
/// * `'buf` - Lifetime of the buffer used for the read operation
///
/// # Returns
///
/// Returns `(usize, Pin<&'buf mut [u8]>)` on success where:
/// - `usize` is the number of bytes read
/// - `Pin<&'buf mut [u8]>` is the buffer with read data
///
/// # Examples
///
/// ```rust,ignore
/// # use safer_ring::{Ring, Operation, PinnedBuffer};
/// # use std::fs::File;
/// # use std::os::unix::io::AsRawFd;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let mut buffer = PinnedBuffer::with_capacity(1024);
/// let file = File::open("example.txt")?;
///
/// // Buffer lifetime must outlive the operation
/// let read_future = ring.read(file.as_raw_fd(), buffer.as_mut_slice())?;
/// let (bytes_read, _buffer) = read_future.await?;
///
/// println!("Read {} bytes", bytes_read);
/// # Ok(())
/// # }
/// ```
pub struct ReadFuture<'ring, 'buf> {
    // Option allows taking ownership during completion without Clone
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    // Rc allows sharing waker registry across multiple futures efficiently
    waker_registry: Arc<WakerRegistry>,
    // Explicit lifetime tracking for compile-time safety verification
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

/// Future for file write operations.
///
/// This future polls the completion queue until the write operation completes,
/// then returns the number of bytes written and buffer ownership. Like all
/// I/O futures in this crate, it ensures proper resource cleanup and handles
/// error conditions gracefully.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring Ring instance  
/// * `'buf` - Lifetime of the buffer containing data to write
///
/// # Returns
///
/// Returns `(usize, Pin<&'buf mut [u8]>)` on success where:
/// - `usize` is the number of bytes written
/// - `Pin<&'buf mut [u8]>` is the buffer that was written from
///
/// # Examples
///
/// ```rust,ignore
/// # use safer_ring::{Ring, Operation, PinnedBuffer};
/// # use std::fs::OpenOptions;
/// # use std::os::unix::io::AsRawFd;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let mut buffer = PinnedBuffer::from_slice(b"Hello, world!");
/// let file = OpenOptions::new()
///     .write(true)
///     .create(true)
///     .open("output.txt")?;
///
/// let write_future = ring.write(file.as_raw_fd(), buffer.as_mut_slice())?;
/// let (bytes_written, _buffer) = write_future.await?;
///
/// println!("Wrote {} bytes", bytes_written);
/// # Ok(())
/// # }
/// ```
pub struct WriteFuture<'ring, 'buf> {
    // Same structure as ReadFuture for consistency and shared macro usage
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Arc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

impl<'ring, 'buf> ReadFuture<'ring, 'buf> {
    pub(crate) fn new(
        operation: Operation<'ring, 'buf, Submitted>,
        ring: &'ring mut Ring<'ring>,
        waker_registry: Arc<WakerRegistry>,
    ) -> Self {
        Self {
            operation: Some(operation),
            ring,
            waker_registry,
            _phantom: PhantomData,
        }
    }
}

impl<'ring, 'buf> WriteFuture<'ring, 'buf> {
    pub(crate) fn new(
        operation: Operation<'ring, 'buf, Submitted>,
        ring: &'ring mut Ring<'ring>,
        waker_registry: Arc<WakerRegistry>,
    ) -> Self {
        Self {
            operation: Some(operation),
            ring,
            waker_registry,
            _phantom: PhantomData,
        }
    }
}

impl<'ring, 'buf> Future for ReadFuture<'ring, 'buf> {
    type Output = io::Result<(usize, StdPin<&'buf mut [u8]>)>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        poll_io_operation!(self, cx, "ReadFuture")
    }
}

impl<'ring, 'buf> Future for WriteFuture<'ring, 'buf> {
    type Output = io::Result<(usize, StdPin<&'buf mut [u8]>)>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        poll_io_operation!(self, cx, "WriteFuture")
    }
}

impl<'ring, 'buf> Drop for ReadFuture<'ring, 'buf> {
    fn drop(&mut self) {
        impl_future_drop!(self);
    }
}

impl<'ring, 'buf> Drop for WriteFuture<'ring, 'buf> {
    fn drop(&mut self) {
        impl_future_drop!(self);
    }
}
