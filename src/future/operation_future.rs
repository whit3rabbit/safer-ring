//! Generic future for any operation type.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin as StdPin;
use std::rc::Rc;
use std::task::{Context, Poll};

use crate::future::waker::WakerRegistry;
use crate::operation::{Operation, Submitted};
use crate::operation::BufferType;
use crate::ring::Ring;

/// Generic future for any operation type.
///
/// This provides a unified interface for different operation types while
/// maintaining type safety and proper buffer ownership semantics.
/// Unlike the specialized I/O futures, this returns the raw i32 result
/// from io_uring without conversion to usize.
///
/// # Type Parameters
///
/// * `'ring` - Lifetime of the io_uring instance
/// * `'buf` - Lifetime of the buffer being used for the operation
///
/// # Returns
///
/// Returns `(i32, Option<Pin<&'buf mut [u8]>>)` where:
/// - `i32` is the raw result from io_uring (can be negative for errors)
/// - `Option<Pin<&'buf mut [u8]>>` is the buffer if the operation used one
pub struct OperationFuture<'ring, 'buf> {
    /// The underlying operation (None after completion)
    /// Using Option to allow taking ownership during completion
    operation: Option<Operation<'ring, 'buf, Submitted>>,
    /// Reference to the ring for polling completions
    ring: &'ring Ring<'ring>,
    /// Waker registry for async notification
    waker_registry: Rc<WakerRegistry>,
    /// Phantom data for lifetime tracking
    _phantom: PhantomData<(&'ring (), &'buf ())>,
}

impl<'ring, 'buf> OperationFuture<'ring, 'buf> {
    /// Create a new operation future from a submitted operation.
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

impl<'ring, 'buf> Future for OperationFuture<'ring, 'buf> {
    type Output = io::Result<(i32, Option<StdPin<&'buf mut [u8]>>)>;

    fn poll(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let operation = match self.operation.as_ref() {
            Some(op) => op,
            None => {
                // Future has already been polled to completion
                // This is a programming error - futures should not be polled after Ready
                panic!("OperationFuture polled after completion");
            }
        };

        let operation_id = operation.id();

        // Check if the operation has completed
        match self.ring.try_complete_by_id(operation_id) {
            Ok(Some(result)) => {
                // Operation completed, extract the result and buffer
                let operation = self.operation.take().unwrap();
                let completed = operation.complete_with_result(result);
                let (io_result, buffer) = completed.into_result();

                // Clean up waker registration to prevent memory leaks
                self.waker_registry.remove_waker(operation_id);

                // Return the raw result and buffer without conversion
                // This allows callers to handle negative results as needed
                // Convert BufferType to Option<Pin<&mut [u8]>> for compatibility
                let buffer_option = match buffer {
                    BufferType::Pinned(buf) => Some(buf),
                    _ => None,
                };
                Poll::Ready(io_result.map(|bytes| (bytes, buffer_option)))
            }
            Ok(None) => {
                // Operation still in flight, register waker and return Pending
                // Clone the waker to avoid borrowing issues
                self.waker_registry
                    .register_waker(operation_id, cx.waker().clone());
                Poll::Pending
            }
            Err(e) => {
                // Error checking completion status
                self.waker_registry.remove_waker(operation_id);
                Poll::Ready(Err(io::Error::other(format!(
                    "Error checking operation completion: {}",
                    e
                ))))
            }
        }
    }
}

impl<'ring, 'buf> Drop for OperationFuture<'ring, 'buf> {
    fn drop(&mut self) {
        // Clean up waker registration if the future is dropped before completion
        // This prevents memory leaks in the waker registry
        if let Some(operation) = &self.operation {
            self.waker_registry.remove_waker(operation.id());
        }
    }
}
