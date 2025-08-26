//! Common utilities and macros for I/O futures.

/// Common polling logic for I/O futures that return buffer ownership.
///
/// This macro reduces code duplication while maintaining type safety.
/// Using a macro instead of a generic function allows us to:
/// - Avoid complex trait bounds and lifetime constraints
/// - Maintain zero-cost abstraction with compile-time expansion
/// - Keep the polling logic inline for better optimization
macro_rules! poll_io_operation {
    ($self:expr, $cx:expr, $future_name:literal) => {{
        let operation = match $self.operation.as_ref() {
            Some(op) => op,
            None => {
                // Polling after completion is a programming error
                panic!("{} polled after completion", $future_name);
            }
        };

        let operation_id = operation.id();

        match $self.ring.try_complete_by_id(operation_id) {
            Ok(Some(result)) => {
                // Extract operation and complete it
                let operation = $self.operation.take().unwrap();
                let completed = operation.complete_with_result(result);
                let (io_result, buffer) = completed.into_result_with_buffer();

                // Clean up waker to prevent memory leaks
                $self.waker_registry.remove_waker(operation_id);

                match io_result {
                    Ok(bytes) => {
                        // Defensive conversion - negative values should already be errors
                        let bytes_usize = if bytes >= 0 {
                            bytes as usize
                        } else {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::Other,
                                "Unexpected negative byte count",
                            )));
                        };

                        match buffer {
                            Some(buf) => Poll::Ready(Ok((bytes_usize, buf))),
                            None => Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::Other,
                                concat!($future_name, " completed without buffer"),
                            ))),
                        }
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Ok(None) => {
                // Still pending - register waker for notification
                $self
                    .waker_registry
                    .register_waker(operation_id, $cx.waker().clone());
                Poll::Pending
            }
            Err(e) => {
                // Completion check failed
                $self.waker_registry.remove_waker(operation_id);
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Error checking operation completion: {}", e),
                )))
            }
        }
    }};
}

/// Common polling logic for vectored I/O operations.
///
/// Similar to `poll_io_operation` but handles multiple buffers.
/// Vectored operations require different result extraction methods
/// but follow the same async polling pattern.
macro_rules! poll_vectored_operation {
    ($self:expr, $cx:expr, $future_name:literal) => {{
        let operation = match $self.operation.as_ref() {
            Some(op) => op,
            None => {
                panic!("{} polled after completion", $future_name);
            }
        };

        let operation_id = operation.id();

        match $self.ring.try_complete_by_id(operation_id) {
            Ok(Some(result)) => {
                let operation = $self.operation.take().unwrap();
                let completed = operation.complete_with_result(result);
                let (io_result, buffers) = completed.into_result_with_vectored_buffers();

                $self.waker_registry.remove_waker(operation_id);

                match io_result {
                    Ok(bytes) => {
                        let bytes_usize = if bytes >= 0 {
                            bytes as usize
                        } else {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::Other,
                                "Unexpected negative byte count",
                            )));
                        };

                        match buffers {
                            Some(bufs) => Poll::Ready(Ok((bytes_usize, bufs))),
                            None => Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::Other,
                                concat!($future_name, " completed without buffers"),
                            ))),
                        }
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Ok(None) => {
                $self
                    .waker_registry
                    .register_waker(operation_id, $cx.waker().clone());
                Poll::Pending
            }
            Err(e) => {
                $self.waker_registry.remove_waker(operation_id);
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Error checking {} completion: {}", $future_name, e),
                )))
            }
        }
    }};
}

/// Common drop logic for I/O futures.
///
/// Ensures waker cleanup when futures are dropped before completion.
/// This prevents memory leaks in the waker registry and is critical
/// for proper resource management in async contexts.
macro_rules! impl_future_drop {
    ($self:expr) => {
        if let Some(operation) = &$self.operation {
            $self.waker_registry.remove_waker(operation.id());
        }
    };
}

pub(super) use {impl_future_drop, poll_io_operation, poll_vectored_operation};
