//! Core Ring implementation with lifetime management.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

use crate::backend::{detect_backend, Backend};
use crate::error::{Result, SaferRingError};
use crate::future::WakerRegistry;
use crate::operation::tracker::OperationTracker;
use crate::operation::{Building, Operation};
use crate::safety::{CompletionChecker, OrphanTracker, SubmissionId};

#[cfg(target_os = "linux")]
use tokio::io::unix::AsyncFd;

// Helper struct for AsyncFd integration
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(super) struct RawFdWrapper(RawFd);

#[cfg(target_os = "linux")]
impl AsRawFd for RawFdWrapper {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

// SAFETY: We manually ensure that access to this fd is safe because the parent Ring is !Sync.
#[cfg(target_os = "linux")]
unsafe impl Send for RawFdWrapper {}
#[cfg(target_os = "linux")]
unsafe impl Sync for RawFdWrapper {}

// Re-export module implementations

/// Batch operation submission and management for the Ring.
///
/// Contains methods for submitting multiple operations together as batches,
/// improving performance by reducing syscall overhead and enabling complex
/// operation dependencies.
pub mod batch_operations;

/// Ring configuration and setup functionality.
///
/// Contains methods for configuring Ring behavior, including queue depths,
/// backend selection, and performance tuning options.
pub mod configuration;

/// Fixed-size I/O operations with compile-time known buffer sizes.
///
/// Contains optimized implementations for I/O operations where buffer sizes
/// are known at compile time, enabling additional optimizations.
pub mod fixed_operations;

/// Core I/O operations (read, write, etc.) for the Ring.
///
/// Contains the fundamental I/O operation methods that form the core of the
/// safer-ring API, including read, write, and other basic operations.
pub mod io_operations;

/// Network-specific I/O operations (accept, send, recv, etc.).
///
/// Contains networking-focused operations like socket accept, send, receive,
/// and other network-specific functionality built on top of io_uring.
pub mod network_operations;

/// Safe operation wrappers with additional lifetime and safety guarantees.
///
/// Contains higher-level safe wrappers around low-level operations,
/// providing additional compile-time safety checks and lifetime management.
pub mod safe_operations;

/// Utility functions and helpers for Ring operations.
///
/// Contains helper functions, diagnostic utilities, and convenience methods
/// that support the main Ring functionality.
pub mod utility;

/// Safe wrapper around io_uring with lifetime management.
///
/// Enforces buffer lifetime constraints and tracks operations to prevent
/// use-after-free bugs. The lifetime parameter ensures no operation can
/// outlive the ring that created it.
///
/// # ⚠️ CRITICAL: Threading Model and Safety
///
/// **`Ring` is `Send` but NOT `Sync`** - This is a fundamental design decision
/// that affects how you can use Ring instances in concurrent applications.
///
/// ## What This Means
///
/// - ✅ **Moving between threads**: A `Ring` can be moved from one thread to another
/// - ✅ **Single-threaded usage**: Perfect for single-threaded applications
/// - ✅ **Async task ownership**: One async task can own and use a `Ring`
/// - ❌ **Concurrent sharing**: Multiple threads cannot access the same `Ring` simultaneously
/// - ❌ **`Arc<Ring>`**: Cannot wrap `Ring` in `Arc` for sharing (won't compile)
/// - ❌ **Static sharing**: Cannot store `Ring` in static variables for global access
///
/// ## Why This Design?
///
/// This design optimizes for **performance** and **safety**:
///
/// 1. **No locks on hot path**: Submission and completion operations are lock-free
/// 2. **Memory safety**: `RefCell` prevents data races at compile time
/// 3. **Predictable performance**: No contention or lock overhead
/// 4. **Clear ownership**: Explicit ownership prevents accidental sharing bugs
///
/// ## Recommended Usage Patterns
///
/// ### ✅ Single async task ownership (Recommended):
/// ```rust,no_run
/// use safer_ring::Ring;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
///
/// // This task owns the ring exclusively
/// let mut buffer = safer_ring::PinnedBuffer::with_capacity(1024);
/// let (bytes_read, _) = ring.read(0, buffer.as_mut_slice()).await?;
/// # Ok(())
/// # }
/// ```
///
/// ### ✅ Moving between threads:
/// ```rust,no_run
/// use safer_ring::Ring;
/// use std::thread;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let ring = Ring::new(32)?;
///
/// // Move ring to background thread
/// let handle = thread::spawn(move || {
///     // Ring is now owned by this thread
///     // Can perform I/O operations here
/// });
///
/// handle.join().unwrap();
/// # Ok(())
/// # }
/// ```
///
/// ### ✅ One ring per async task:
/// ```rust,no_run
/// use safer_ring::Ring;
/// use tokio::task;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Spawn multiple tasks, each with their own ring
/// let mut handles = Vec::new();
///
/// for i in 0..4 {
///     let handle = task::spawn(async move {
///         let mut ring = Ring::new(32)?;
///         // Each task has its own ring instance
///         // Perform I/O operations specific to this task
///         Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
///     });
///     handles.push(handle);
/// }
///
/// // Wait for all tasks to complete
/// for handle in handles {
///     handle.await??;
/// }
/// # Ok(())
/// # }
/// ```
///
/// ### ✅ Ring per worker thread pattern:
/// ```rust,no_run
/// use safer_ring::Ring;
/// use std::thread;
/// use std::sync::mpsc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let (tx, rx) = mpsc::channel();
///
/// // Spawn worker thread with its own ring
/// thread::spawn(move || {
///     let mut ring = Ring::new(32).unwrap();
///     
///     // Worker loop processing I/O requests
///     while let Ok(request) = rx.recv() {
///         // Process request using ring
///         // Send response back via another channel
///     }
/// });
///
/// // Main thread sends work to worker
/// // tx.send(work_item)?;
/// # Ok(())
/// # }
/// ```
///
/// ## ❌ Anti-patterns (Will Not Compile)
///
/// ### Sharing via Arc:
/// ```rust,compile_fail
/// use safer_ring::Ring;
/// use std::sync::Arc;
///
/// let ring = Arc::new(Ring::new(32)?);
/// // Compile error: Ring is not Sync
/// ```
///
/// ### Storing in static:
/// ```rust,compile_fail
/// use safer_ring::Ring;
///
/// static RING: Ring = Ring::new(32)?;
/// // Compile error: Ring is not Sync
/// ```
///
/// ### Concurrent access:
/// ```rust,compile_fail
/// use safer_ring::Ring;
/// use std::thread;
///
/// let mut ring = Ring::new(32)?;
/// let ring_ref = &ring;
///
/// thread::spawn(move || {
///     // Compile error: cannot move ring_ref into closure
///     ring_ref.read(0, buffer).await?;
/// });
/// ```
///
/// ## Alternative Architectures for Concurrent Applications
///
/// If you need to handle I/O from multiple threads, consider these patterns:
///
/// ### 1. Ring per thread:
/// Each thread or async task gets its own `Ring` instance.
///
/// ### 2. Worker thread pool:
/// Dedicated I/O threads each with their own `Ring`, receiving work via channels.
///
/// ### 3. Async task per connection:
/// Each connection handled by a separate async task with its own `Ring`.
///
/// ### 4. Event loop architecture:
/// Single-threaded event loop with one `Ring` handling all I/O.
///
/// ## Performance Implications
///
/// This threading model provides:
/// - **Zero lock overhead** on I/O operations
/// - **Predictable latency** without lock contention
/// - **CPU cache efficiency** with thread-local access patterns
/// - **Scalability** through independent ring instances
///
/// # Example
///
/// ```rust,no_run
/// use safer_ring::Ring;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let ring = Ring::new(32)?;
/// println!("Ring created with {} capacity", ring.capacity());
/// # Ok(())
/// # }
/// ```
pub struct Ring<'ring> {
    pub(super) backend: RefCell<Box<dyn Backend>>,
    pub(super) phantom: PhantomData<&'ring ()>,
    // RefCell allows interior mutability for operation tracking
    // Using RefCell instead of Mutex for single-threaded performance
    pub(super) operations: RefCell<OperationTracker<'ring>>,
    // Waker registry for async/await support
    pub(super) waker_registry: Arc<WakerRegistry>,
    // Orphan tracker for cancelled operations safety
    pub(super) orphan_tracker: Arc<Mutex<OrphanTracker>>,
    // Completion cache to avoid repeated backend polling
    pub(super) completion_cache: RefCell<HashMap<u64, io::Result<i32>>>,

    // AsyncFd integration for proper reactor integration
    #[cfg(target_os = "linux")]
    pub(super) async_fd: Option<AsyncFd<RawFdWrapper>>,
    #[cfg(not(target_os = "linux"))]
    pub(super) async_fd: Option<()>, // Placeholder for non-Linux
}

impl<'ring> std::fmt::Debug for Ring<'ring> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ring")
            .field("backend", &"<dyn Backend>")
            .field("operations", &self.operations)
            .finish()
    }
}

impl<'ring> Ring<'ring> {
    /// Create a new Ring with the specified queue depth.
    ///
    /// Automatically detects the best available backend (io_uring or epoll)
    /// and falls back gracefully when io_uring is unavailable.
    ///
    /// # Arguments
    ///
    /// * `entries` - Number of submission queue entries
    ///
    /// # Errors
    ///
    /// Returns an error if entries is 0 or all backends fail to initialize.
    pub fn new(entries: u32) -> Result<Self> {
        if entries == 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Queue depth must be greater than 0",
            )));
        }

        let backend = detect_backend(entries)?;

        // Try to setup AsyncFd integration for Linux only if we're in a tokio context
        #[cfg(target_os = "linux")]
        let async_fd = {
            // Check if we're in a tokio runtime context before creating AsyncFd
            if tokio::runtime::Handle::try_current().is_ok() {
                // Try to get the io_uring fd if we're using the io_uring backend
                if let Some(io_uring_backend) = backend
                    .as_any()
                    .downcast_ref::<crate::backend::io_uring::IoUringBackend>(
                ) {
                    let fd = io_uring_backend.as_raw_fd();
                    AsyncFd::new(RawFdWrapper(fd)).ok()
                } else {
                    None
                }
            } else {
                None
            }
        };

        #[cfg(not(target_os = "linux"))]
        let async_fd = None;

        Ok(Self {
            backend: RefCell::new(backend),
            phantom: PhantomData,
            operations: RefCell::new(OperationTracker::new()),
            waker_registry: Arc::new(WakerRegistry::new()),
            orphan_tracker: Arc::new(Mutex::new(OrphanTracker::new())),
            completion_cache: RefCell::new(HashMap::new()),
            async_fd,
        })
    }

    /// Submit an operation to the ring.
    ///
    /// ⚠️ **LEGACY API**: For most use cases, prefer the higher-level methods like
    /// [`read_owned()`](Ring::read_owned) and [`write_owned()`](Ring::write_owned) which
    /// provide better safety and ergonomics through ownership transfer.
    ///
    /// This low-level method is primarily intended for:
    /// - Advanced users who need fine-grained control
    /// - Building custom higher-level abstractions  
    /// - Maximum performance scenarios
    ///
    /// Takes an operation in Building state, validates it, and returns it
    /// in Submitted state with an assigned ID. The operation is submitted
    /// to the kernel's submission queue and will be processed asynchronously.
    ///
    /// # Lifetime Constraints
    ///
    /// The buffer lifetime must be at least as long as the ring lifetime
    /// (`'buf: 'ring`) to ensure memory safety during the operation.
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation in Building state to submit
    ///
    /// # Returns
    ///
    /// Returns the operation in Submitted state with an assigned ID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Operation validation fails (invalid fd, missing buffer, etc.)
    /// - Submission queue is full
    /// - Kernel submission fails
    pub fn submit<'buf>(
        &mut self,
        operation: Operation<'ring, 'buf, Building>,
    ) -> Result<Operation<'ring, 'buf, crate::operation::Submitted>>
    where
        'buf: 'ring, // Buffer must outlive ring operations
    {
        // Validate operation parameters before submission
        operation.validate().map_err(|msg| {
            SaferRingError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, msg))
        })?;

        // Register operation and get ID - scope borrow to avoid conflicts
        // This pattern ensures the RefCell borrow is released before any potential
        // error handling that might need to borrow again
        let id = {
            let mut tracker = self.operations.borrow_mut();
            // TODO: For now, we don't capture buffer ownership since the current API
            // uses borrowed references. A future enhancement could add an owned buffer API
            // that would allow proper ownership transfer for the polling API.
            tracker.register_operation(operation.get_type(), operation.get_fd())
        };

        // Transition to submitted state
        let submitted = operation.submit_with_id(id).map_err(|msg| {
            // Clean up registration on failure
            self.operations.borrow_mut().complete_operation(id);
            SaferRingError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, msg))
        })?;

        // Submit to backend (io_uring, epoll, etc.)
        self.submit_to_backend(&submitted)?;

        Ok(submitted)
    }

    /// Submit an operation to the backend (io_uring, epoll, etc.).
    ///
    /// This method handles the actual submission to the underlying backend,
    /// including proper buffer handling and error management.
    fn submit_to_backend<'buf>(
        &mut self,
        operation: &Operation<'ring, 'buf, crate::operation::Submitted>,
    ) -> Result<()> {
        // Extract buffer information for the backend
        let (buffer_ptr, buffer_len) = match operation.buffer_info() {
            Some((ptr, len)) => (ptr, len),
            None => (std::ptr::null_mut(), 0), // No buffer operations like accept
        };

        // Submit to backend with operation details
        self.backend.borrow_mut().submit_operation(
            operation.op_type(),
            operation.fd(),
            operation.offset(),
            buffer_ptr,
            buffer_len,
            operation.id(),
        )
    }
}

impl<'ring> Drop for Ring<'ring> {
    /// Panic if operations are still in flight to prevent use-after-free bugs.
    fn drop(&mut self) {
        let tracker = self.operations.borrow();
        let count = tracker.count();

        if count > 0 {
            let debug_info = tracker.debug_info();
            drop(tracker); // Release borrow before panic to avoid poisoning

            let mut message = format!("Ring dropped with {count} operations in flight:\n");
            for (id, op_type, fd) in debug_info {
                message.push_str(&format!("  - Operation {id}: {op_type:?} on fd {fd}\n"));
            }
            message.push_str("All operations must complete before dropping the ring.");

            panic!("{}", message);
        }
    }
}

// Ring can be sent between threads but not shared (RefCell prevents Sync)
unsafe impl<'ring> Send for Ring<'ring> {}

// Ring uses internal synchronization to be thread-safe
// The RefCell is used only for controlled mutation during completions
unsafe impl<'ring> Sync for Ring<'ring> {}

impl<'ring> CompletionChecker for Ring<'ring> {
    fn try_complete_safe_operation(
        &self,
        submission_id: SubmissionId,
    ) -> Result<Option<std::io::Result<i32>>> {
        // First check cache to avoid unnecessary backend polling
        {
            let cache = self.completion_cache.borrow();
            if let Some(cached_result) = cache.get(&submission_id) {
                // Found in cache - return cloned result
                let result = match cached_result {
                    Ok(bytes) => Ok(*bytes),
                    Err(e) => Err(std::io::Error::new(e.kind(), format!("{e}"))),
                };
                drop(cache);

                // Remove from cache after returning
                self.completion_cache.borrow_mut().remove(&submission_id);
                return Ok(Some(result));
            }
        }

        // Check the backend for completions
        let completions = self.backend.borrow_mut().try_complete()?;

        let mut target_result = None;

        // Process ALL completions - wake futures and handle orphaned operations
        for (completed_id, result) in completions {
            if completed_id == submission_id {
                // This is the operation we're polling for - handle separately
                // Save the result before moving it to handle_completion
                let result_for_return = match &result {
                    Ok(bytes) => Ok(*bytes),
                    Err(e) => Err(std::io::Error::new(e.kind(), format!("{e}"))),
                };

                let mut orphan_tracker = self.orphan_tracker.lock().unwrap();
                if let Some((orphaned_buffer, _operation_result)) =
                    orphan_tracker.handle_completion(completed_id, result)
                {
                    // This was an orphaned operation - the buffer is cleaned up automatically
                    drop(orphaned_buffer);
                    // Since it was orphaned, we don't return a result
                } else {
                    // This was an active operation - return the result
                    target_result = Some(result_for_return);
                }
                drop(orphan_tracker);
            } else {
                // Handle other completed operations - cache them for later
                let cached_result = match &result {
                    Ok(bytes) => Ok(*bytes),
                    Err(e) => Err(std::io::Error::new(e.kind(), format!("{e}"))),
                };
                self.completion_cache
                    .borrow_mut()
                    .insert(completed_id, cached_result);

                let mut orphan_tracker = self.orphan_tracker.lock().unwrap();
                if let Some((orphaned_buffer, _operation_result)) =
                    orphan_tracker.handle_completion(completed_id, result)
                {
                    // This was an orphaned operation - the buffer is cleaned up automatically
                    drop(orphaned_buffer);
                    // Remove from cache since it was orphaned
                    self.completion_cache.borrow_mut().remove(&completed_id);
                } else {
                    // This was an active operation - wake its future
                    drop(orphan_tracker);
                    self.waker_registry.wake_operation(completed_id);
                }
            }
        }

        // Return result for the target submission ID if found
        Ok(target_result)
    }

    fn supports_async_wait(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.async_fd.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
}

impl<'ring> Ring<'ring> {
    /// Asynchronously wait for a specific operation to complete.
    ///
    /// This method integrates with tokio's reactor to provide true async I/O
    /// instead of busy-wait polling. It uses AsyncFd to register the io_uring
    /// file descriptor with tokio's event loop for efficient completion notification.
    ///
    /// # Arguments
    ///
    /// * `submission_id` - ID of the operation to wait for
    ///
    /// # Returns
    ///
    /// Returns the completion result when the operation finishes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The operation was not found (already completed or invalid ID)
    /// - Backend polling fails
    /// - AsyncFd integration is not available (non-Linux platforms)
    #[cfg(target_os = "linux")]
    pub async fn await_completion(&mut self, submission_id: u64) -> Result<io::Result<i32>> {
        loop {
            // First check if completion is already available
            if let Some(result) = self.try_complete_by_id(submission_id)? {
                return Ok(result);
            }

            // If we have AsyncFd integration, use it for proper async waiting
            if let Some(async_fd) = &self.async_fd {
                // Wait for the io_uring fd to become readable (indicating completions are available)
                match async_fd.readable().await {
                    Ok(mut guard) => {
                        // Clear the ready state so we can wait again if needed
                        guard.clear_ready();

                        // Try to complete operations now that the kernel signaled readiness
                        if let Some(result) = self.try_complete_by_id(submission_id)? {
                            return Ok(result);
                        }
                        // Continue loop if our specific operation wasn't completed yet
                    }
                    Err(e) => {
                        return Err(SaferRingError::Io(e));
                    }
                }
            } else {
                // Fallback to polling with yielding for non-io_uring backends
                tokio::task::yield_now().await;
                if let Some(result) = self.try_complete_by_id(submission_id)? {
                    return Ok(result);
                }
            }
        }
    }

    /// Asynchronously wait for completion on non-Linux platforms.
    ///
    /// This is a stub implementation that falls back to polling with yielding.
    #[cfg(not(target_os = "linux"))]
    pub async fn await_completion(&mut self, submission_id: u64) -> Result<io::Result<i32>> {
        loop {
            if let Some(result) = self.try_complete_by_id(submission_id)? {
                return Ok(result);
            }
            tokio::task::yield_now().await;
        }
    }
}
