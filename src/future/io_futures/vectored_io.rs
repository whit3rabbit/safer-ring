//! Vectored I/O futures for scatter-gather operations.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin as StdPin;
use std::sync::Arc;
use std::task::{Context, Poll};

use super::common::{impl_future_drop, poll_vectored_operation};
use crate::future::waker::WakerRegistry;
use crate::operation::{Operation, Submitted};
use crate::ring::Ring;

/// Future for vectored read operations.
///
/// This future handles scatter-gather reads that operate on multiple buffers
/// simultaneously, allowing for efficient I/O when data needs to be read into
/// multiple non-contiguous memory regions. Returns the total bytes read and
/// ownership of all buffers when the operation completes.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring Ring instance
/// * `'buf` - Lifetime of all buffers used for the vectored read
///
/// # Returns
///
/// Returns `(usize, Vec<Pin<&'buf mut [u8]>>)` on success where:
/// - `usize` is the total number of bytes read across all buffers
/// - `Vec<Pin<&'buf mut [u8]>>` are all the buffers containing read data
///
/// # Examples
///
/// ```rust,ignore
/// # use safer_ring::{Ring, Operation, PinnedBuffer};
/// # use std::fs::File;
/// # use std::os::unix::io::AsRawFd;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let file = File::open("data.bin")?;
///
/// // Create buffers for vectored read
/// let mut header_buf = PinnedBuffer::with_capacity(512);
/// let mut data_buf = PinnedBuffer::with_capacity(4096);
/// let buffers = vec![header_buf.as_mut_slice(), data_buf.as_mut_slice()];
///
/// let read_future = ring.read_vectored(file.as_raw_fd(), buffers)?;
/// let (total_bytes, _buffers) = read_future.await?;
///
/// println!("Read {} bytes total", total_bytes);
/// # Ok(())
/// # }
/// ```
pub struct VectoredReadFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Arc<WakerRegistry>,
    // Same lifetime structure as single-buffer operations for consistency
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

/// Future for vectored write operations.
///
/// This future handles gather writes that operate on multiple buffers
/// simultaneously, allowing for efficient I/O when data from multiple
/// non-contiguous memory regions needs to be written in a single operation.
/// Returns the total bytes written and ownership of all buffers when complete.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring Ring instance
/// * `'buf` - Lifetime of all buffers used for the vectored write
///
/// # Returns
///
/// Returns `(usize, Vec<Pin<&'buf mut [u8]>>)` on success where:
/// - `usize` is the total number of bytes written from all buffers
/// - `Vec<Pin<&'buf mut [u8]>>` are all the buffers that were written from
///
/// # Examples
///
/// ```rust,ignore
/// # use safer_ring::{Ring, Operation, PinnedBuffer};
/// # use std::fs::OpenOptions;
/// # use std::os::unix::io::AsRawFd;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let file = OpenOptions::new()
///     .write(true)
///     .create(true)
///     .open("output.bin")?;
///
/// // Create buffers for vectored write
/// let mut header_buf = PinnedBuffer::from_slice(b"HEADER: ");
/// let mut data_buf = PinnedBuffer::from_slice(b"Important data content");
/// let buffers = vec![header_buf.as_mut_slice(), data_buf.as_mut_slice()];
///
/// let write_future = ring.write_vectored(file.as_raw_fd(), buffers)?;
/// let (total_bytes, _buffers) = write_future.await?;
///
/// println!("Wrote {} bytes total", total_bytes);
/// # Ok(())
/// # }
/// ```
pub struct VectoredWriteFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Arc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

impl<'ring, 'buf> VectoredReadFuture<'ring, 'buf> {
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

impl<'ring, 'buf> VectoredWriteFuture<'ring, 'buf> {
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

impl<'ring, 'buf> Future for VectoredReadFuture<'ring, 'buf> {
    type Output = io::Result<(usize, Vec<StdPin<&'buf mut [u8]>>)>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        poll_vectored_operation!(self, cx, "VectoredReadFuture")
    }
}

impl<'ring, 'buf> Future for VectoredWriteFuture<'ring, 'buf> {
    type Output = io::Result<(usize, Vec<StdPin<&'buf mut [u8]>>)>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        poll_vectored_operation!(self, cx, "VectoredWriteFuture")
    }
}

impl<'ring, 'buf> Drop for VectoredReadFuture<'ring, 'buf> {
    fn drop(&mut self) {
        impl_future_drop!(self);
    }
}

impl<'ring, 'buf> Drop for VectoredWriteFuture<'ring, 'buf> {
    fn drop(&mut self) {
        impl_future_drop!(self);
    }
}
