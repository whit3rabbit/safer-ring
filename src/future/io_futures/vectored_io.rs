//! Vectored I/O futures for scatter-gather operations.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin as StdPin;
use std::rc::Rc;
use std::task::{Context, Poll};

use super::common::{impl_future_drop, poll_vectored_operation};
use crate::future::waker::WakerRegistry;
use crate::operation::{Operation, Submitted};
use crate::ring::Ring;

/// Future for vectored read operations.
///
/// Handles scatter-gather reads that operate on multiple buffers simultaneously.
/// Returns the total bytes read and all buffer ownership when complete.
pub struct VectoredReadFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring Ring<'ring>,
    waker_registry: Rc<WakerRegistry>,
    // Same lifetime structure as single-buffer operations for consistency
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

/// Future for vectored write operations.
///
/// Handles gather writes that operate on multiple buffers simultaneously.
/// Returns the total bytes written and all buffer ownership when complete.
pub struct VectoredWriteFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    ring: &'ring Ring<'ring>,
    waker_registry: Rc<WakerRegistry>,
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

impl<'ring, 'buf> VectoredReadFuture<'ring, 'buf> {
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

impl<'ring, 'buf> VectoredWriteFuture<'ring, 'buf> {
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