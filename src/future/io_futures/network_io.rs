//! Network I/O futures for socket operations.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::os::unix::io::RawFd;
use std::pin::Pin as StdPin;
use std::sync::Arc;
use std::task::{Context, Poll};

use super::common::{impl_future_drop, poll_io_operation};
use crate::future::waker::WakerRegistry;
use crate::operation::{Operation, Submitted};
use crate::ring::Ring;

/// Future for socket accept operations.
///
/// This future waits for an incoming connection on a listening socket and
/// returns the new client file descriptor when a connection is accepted.
/// Unlike read/write operations, accept operations don't require buffer
/// management, so this future has a simpler interface.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring Ring instance
///
/// # Returns
///
/// Returns the raw file descriptor (`RawFd`) of the accepted client connection
/// on success. The caller is responsible for managing the returned file descriptor.
///
/// # Examples
///
/// ```rust,ignore
/// # use safer_ring::{Ring, Operation};
/// # use std::net::{TcpListener, SocketAddr};
/// # use std::os::unix::io::AsRawFd;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let listener = TcpListener::bind("127.0.0.1:8080")?;
/// listener.set_nonblocking(true)?;
///
/// let accept_future = ring.accept(listener.as_raw_fd())?;
/// let client_fd = accept_future.await?;
///
/// println!("Accepted client connection: fd {}", client_fd);
/// # Ok(())
/// # }
/// ```
pub struct AcceptFuture<'ring> {
    operation: Option<Operation<'ring, 'static, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Arc<WakerRegistry>,
    // No buffer lifetime needed for accept operations
    _phantom: PhantomData<&'ring ()>,
}

/// Future for socket send operations.
///
/// This future sends data over a socket connection and returns the number
/// of bytes sent along with buffer ownership when the operation completes.
/// The future handles partial sends and error conditions appropriately.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring Ring instance
/// * `'buf` - Lifetime of the buffer containing data to send
///
/// # Returns
///
/// Returns `(usize, Pin<&'buf mut [u8]>)` on success where:
/// - `usize` is the number of bytes sent
/// - `Pin<&'buf mut [u8]>` is the buffer that was sent from
///
/// # Examples
///
/// ```rust,ignore
/// # use safer_ring::{Ring, Operation, PinnedBuffer};
/// # use std::net::TcpStream;
/// # use std::os::unix::io::AsRawFd;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let mut buffer = PinnedBuffer::from_slice(b"Hello, client!");
/// let stream = TcpStream::connect("127.0.0.1:8080")?;
/// stream.set_nonblocking(true)?;
///
/// let send_future = ring.send(stream.as_raw_fd(), buffer.as_mut_slice())?;
/// let (bytes_sent, _buffer) = send_future.await?;
///
/// println!("Sent {} bytes", bytes_sent);
/// # Ok(())
/// # }
/// ```
pub struct SendFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Arc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

/// Future for socket receive operations.
///
/// This future receives data from a socket connection and returns the number
/// of bytes received along with buffer ownership when the operation completes.
/// The buffer will contain the received data upon successful completion.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring Ring instance
/// * `'buf` - Lifetime of the buffer to receive data into
///
/// # Returns
///
/// Returns `(usize, Pin<&'buf mut [u8]>)` on success where:
/// - `usize` is the number of bytes received
/// - `Pin<&'buf mut [u8]>` is the buffer containing the received data
///
/// # Examples
///
/// ```rust,ignore
/// # use safer_ring::{Ring, Operation, PinnedBuffer};
/// # use std::net::TcpStream;
/// # use std::os::unix::io::AsRawFd;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let mut buffer = PinnedBuffer::with_capacity(1024);
/// let stream = TcpStream::connect("127.0.0.1:8080")?;
/// stream.set_nonblocking(true)?;
///
/// let recv_future = ring.recv(stream.as_raw_fd(), buffer.as_mut_slice())?;
/// let (bytes_received, _buffer) = recv_future.await?;
///
/// println!("Received {} bytes", bytes_received);
/// # Ok(())
/// # }
/// ```
pub struct RecvFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Arc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

impl<'ring> AcceptFuture<'ring> {
    pub(crate) fn new(
        operation: Operation<'ring, 'static, Submitted>,
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

impl<'ring, 'buf> SendFuture<'ring, 'buf> {
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

impl<'ring, 'buf> RecvFuture<'ring, 'buf> {
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

impl<'ring> Future for AcceptFuture<'ring> {
    type Output = io::Result<RawFd>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let operation = match self.operation.as_ref() {
            Some(op) => op,
            None => {
                panic!("AcceptFuture polled after completion");
            }
        };

        let operation_id = operation.id();

        match self.ring.try_complete_by_id(operation_id) {
            Ok(Some(result)) => {
                let _operation = self.operation.take().unwrap();
                self.waker_registry.remove_waker(operation_id);

                match result {
                    Ok(fd) => {
                        // Accept operations return the new client fd as i32
                        // Negative values indicate errors in io_uring
                        if fd >= 0 {
                            Poll::Ready(Ok(fd))
                        } else {
                            Poll::Ready(Err(io::Error::other(
                                "Accept returned invalid file descriptor",
                            )))
                        }
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Ok(None) => {
                self.waker_registry
                    .register_waker(operation_id, cx.waker().clone());
                Poll::Pending
            }
            Err(e) => {
                self.waker_registry.remove_waker(operation_id);
                Poll::Ready(Err(io::Error::other(format!(
                    "Error checking accept completion: {e}"
                ))))
            }
        }
    }
}

impl<'ring, 'buf> Future for SendFuture<'ring, 'buf> {
    type Output = io::Result<(usize, StdPin<&'buf mut [u8]>)>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        poll_io_operation!(self, cx, "SendFuture")
    }
}

impl<'ring, 'buf> Future for RecvFuture<'ring, 'buf> {
    type Output = io::Result<(usize, StdPin<&'buf mut [u8]>)>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        poll_io_operation!(self, cx, "RecvFuture")
    }
}

impl<'ring> Drop for AcceptFuture<'ring> {
    fn drop(&mut self) {
        impl_future_drop!(self);
    }
}

impl<'ring, 'buf> Drop for SendFuture<'ring, 'buf> {
    fn drop(&mut self) {
        impl_future_drop!(self);
    }
}

impl<'ring, 'buf> Drop for RecvFuture<'ring, 'buf> {
    fn drop(&mut self) {
        impl_future_drop!(self);
    }
}
