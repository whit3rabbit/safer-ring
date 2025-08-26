//! Network I/O futures for socket operations.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::os::unix::io::RawFd;
use std::pin::Pin as StdPin;
use std::rc::Rc;
use std::task::{Context, Poll};

use super::common::{impl_future_drop, poll_io_operation};
use crate::future::waker::WakerRegistry;
use crate::operation::{Operation, Submitted};
use crate::ring::Ring;

/// Future for socket accept operations.
///
/// Returns the new client file descriptor when the accept completes.
pub struct AcceptFuture<'ring> {
    operation: Option<Operation<'ring, 'static, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Rc<WakerRegistry>,
    // No buffer lifetime needed for accept operations
    _phantom: PhantomData<&'ring ()>,
}

/// Future for socket send operations.
///
/// Returns bytes sent and buffer ownership when the send completes.
pub struct SendFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Rc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

/// Future for socket receive operations.
///
/// Returns bytes received and buffer ownership when the receive completes.
pub struct RecvFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring mut Ring<'ring>,
    waker_registry: Rc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

impl<'ring> AcceptFuture<'ring> {
    pub(crate) fn new(
        operation: Operation<'ring, 'static, Submitted>,
        ring: &'ring mut Ring<'ring>,
        waker_registry: Rc<WakerRegistry>,
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
        waker_registry: Rc<WakerRegistry>,
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
        waker_registry: Rc<WakerRegistry>,
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
                    "Error checking accept completion: {}",
                    e
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
