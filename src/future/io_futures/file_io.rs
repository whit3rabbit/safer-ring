//! File I/O futures for read and write operations.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin as StdPin;
use std::rc::Rc;
use std::task::{Context, Poll};

use super::common::{impl_future_drop, poll_io_operation};
use crate::future::waker::WakerRegistry;
use crate::operation::{Operation, Submitted};
use crate::ring::Ring;

/// Future for file read operations.
///
/// Polls the completion queue until the read operation completes,
/// then returns the bytes read and buffer ownership.
pub struct ReadFuture<'ring, 'buf> {
    // Option allows taking ownership during completion without Clone
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring Ring<'ring>,
    // Rc allows sharing waker registry across multiple futures efficiently
    waker_registry: Rc<WakerRegistry>,
    // Explicit lifetime tracking for compile-time safety verification
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

/// Future for file write operations.
///
/// Polls the completion queue until the write operation completes,
/// then returns the bytes written and buffer ownership.
pub struct WriteFuture<'ring, 'buf> {
    // Same structure as ReadFuture for consistency and shared macro usage
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring Ring<'ring>,
    waker_registry: Rc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

impl<'ring, 'buf> ReadFuture<'ring, 'buf> {
    pub(crate) fn new(
        operation: Operation<'ring, 'buf, Submitted>,
        ring: &'ring Ring<'ring>,
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

impl<'ring, 'buf> WriteFuture<'ring, 'buf> {
    pub(crate) fn new(
        operation: Operation<'ring, 'buf, Submitted>,
        ring: &'ring Ring<'ring>,
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