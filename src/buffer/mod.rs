//! Pinned buffer management for educational and benchmarking purposes.
//!
//! # ⚠️ Important: PinnedBuffer is NOT for I/O Operations
//!
//! **This module provides [`PinnedBuffer<T>`] primarily for educational purposes and allocation benchmarking.**
//! The `PinnedBuffer` type is fundamentally limited by Rust's lifetime system and **cannot be used**
//! **for practical I/O operations** such as loops or concurrent operations.
//!
//! **For all I/O operations, use [`OwnedBuffer`](crate::OwnedBuffer) with the `*_owned` methods on [`Ring`](crate::Ring).**
//!
//! # Key Features
//!
//! - **Memory Pinning**: Guarantees stable memory addresses using [`Pin<Box<T>>`]
//! - **Generation Tracking**: Atomic counters for buffer lifecycle debugging
//! - **NUMA Awareness**: Platform-specific NUMA-aware allocation (Linux) - useful for benchmarking
//! - **DMA Optimization**: Page-aligned allocation for optimal hardware performance - useful for benchmarking
//! - **Thread Safety**: Safe sharing and transfer between threads
//!
//! # Valid Usage Examples
//!
//! ```rust
//! use safer_ring::buffer::PinnedBuffer;
//!
//! // ✅ VALID: Allocation benchmarking
//! let standard_buffer = PinnedBuffer::with_capacity(4096);
//! let aligned_buffer = PinnedBuffer::with_capacity_aligned(4096);
//! let numa_buffer = PinnedBuffer::with_capacity_numa(4096, Some(0));
//!
//! // ✅ VALID: Single operation, then immediate drop
//! let data = b"Hello, io_uring!".to_vec();
//! let buffer = PinnedBuffer::from_vec(data);
//! assert_eq!(buffer.as_slice(), b"Hello, io_uring!");
//! ```
//!
//! # Invalid Usage (Will Not Compile)
//!
//! ```rust,compile_fail
//! use safer_ring::{Ring, buffer::PinnedBuffer};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut ring = Ring::new(32)?;
//! let mut buffer = PinnedBuffer::with_capacity(4096);
//!
//! // ❌ BROKEN: This will not compile due to lifetime constraints
//! for _ in 0..2 {
//!     let (_, buf) = ring.read(0, buffer.as_mut_slice())?.await?;
//!     buffer = buf;  // Error: ring is still borrowed
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # The Technical Problem
//!
//! Methods like [`Ring::read()`](crate::Ring::read) return futures that hold mutable borrows of both the
//! [`Ring`](crate::Ring) and buffer for their entire lifetime. Rust's borrow checker prevents
//! subsequent operations until the borrow is released, making loops and concurrent operations impossible.

/// Memory allocation utilities for creating aligned and optimized buffers.
///
/// This module provides functions for allocating buffers with specific alignment
/// requirements, particularly page-aligned buffers for optimal DMA performance
/// with io_uring operations.
pub mod allocation;

/// Generation tracking utilities for buffer lifecycle management.
///
/// This module provides atomic counters for tracking buffer state changes,
/// helping with debugging buffer lifecycle issues and detecting potential
/// use-after-free scenarios in development builds.
pub mod generation;

/// NUMA-aware buffer allocation for multi-socket systems.
///
/// This module provides NUMA-aware memory allocation functions that attempt
/// to allocate buffers on specific NUMA nodes for optimal performance on
/// multi-socket systems. On Linux systems, it uses CPU affinity and sysfs
/// to determine NUMA topology and allocate memory locally.
pub mod numa;

pub use allocation::*;
pub use generation::*;
pub use numa::*;

use std::pin::Pin;

/// A buffer that is pinned in memory, primarily for educational purposes.
///
/// # ⚠️ FUNDAMENTALLY LIMITED - DO NOT USE FOR I/O OPERATIONS
///
/// **This API is considered educational and is not suitable for practical applications involving I/O.**
/// It suffers from fundamental lifetime constraints in Rust that make it impossible to use in loops
/// or for multiple concurrent operations on the same [`Ring`](crate::Ring) instance. It exists to
/// demonstrate the complexities that the [`OwnedBuffer`](crate::OwnedBuffer) model successfully solves.
///
/// **For all applications, you MUST use [`OwnedBuffer`](crate::OwnedBuffer) with the `*_owned` methods on [`Ring`](crate::Ring).**
///
/// ## The Core Problem
///
/// The [`Ring`](crate::Ring) methods that accept `PinnedBuffer` (e.g., [`ring.read()`](crate::Ring::read)) return a `Future` that
/// holds a mutable borrow on both the [`Ring`](crate::Ring) and the buffer for their entire lifetimes. This
/// makes it impossible for the borrow checker to allow a second operation in a loop or
/// concurrently, as the first borrow is never released.
///
/// ```rust,compile_fail
/// use safer_ring::{Ring, PinnedBuffer};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut ring = Ring::new(32)?;
/// let mut buffer = PinnedBuffer::with_capacity(1024);
///
/// // This fails to compile due to lifetime constraints:
/// for _ in 0..2 {
///     let (_, buf) = ring.read(0, buffer.as_mut_slice())?.await?;
///     buffer = buf;  // Error: cannot use ring again while borrowed
/// }
/// # Ok(())
/// # }
/// ```
///
/// ## When is `PinnedBuffer` useful?
///
/// - Benchmarking allocation strategies (e.g., [`with_capacity_aligned`](Self::with_capacity_aligned), [`with_capacity_numa`](Self::with_capacity_numa)).
/// - Situations where you need a single, one-shot I/O operation and the buffer and ring
///   will be dropped immediately after.
/// - As a building block for more complex, `unsafe` abstractions.
///
/// For all other cases, and especially for application-level code, **use [`OwnedBuffer`](crate::OwnedBuffer)**.
///
/// # Memory Layout
///
/// The buffer uses heap allocation via [`Pin<Box<T>>`] which guarantees:
/// - Stable memory addresses (required for io_uring)
/// - Automatic cleanup when dropped
/// - Zero-copy semantics for I/O operations
///
/// # Generation Tracking
///
/// Each buffer includes a [`GenerationCounter`] for lifecycle tracking and debugging.
/// This helps identify buffer reuse patterns and can assist in detecting potential
/// use-after-free scenarios during development.
///
/// # Examples
///
/// Valid use cases (allocation benchmarking):
///
/// ```rust
/// use safer_ring::buffer::PinnedBuffer;
/// use std::pin::Pin;
///
/// // Benchmarking different allocation strategies
/// let standard_buffer = PinnedBuffer::with_capacity(4096);
/// let aligned_buffer = PinnedBuffer::with_capacity_aligned(4096);
/// let numa_buffer = PinnedBuffer::with_capacity_numa(4096, Some(0));
///
/// // Single, one-shot operation (not practical for real apps)
/// let buffer = PinnedBuffer::new([1, 2, 3, 4]);
/// let pinned_ref: Pin<&[u8; 4]> = buffer.as_pin();
/// ```
pub struct PinnedBuffer<T: ?Sized> {
    /// Heap-allocated, pinned buffer data - guarantees stable memory address
    inner: Pin<Box<T>>,
    /// Generation counter for tracking buffer lifecycle and reuse  
    generation: GenerationCounter,
}

impl<T: ?Sized> PinnedBuffer<T> {
    /// Returns a pinned reference to the buffer data.
    ///
    /// This method provides safe access to the pinned data while maintaining
    /// the pinning guarantees required for io_uring operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    /// use std::pin::Pin;
    ///
    /// let buffer = PinnedBuffer::new([1, 2, 3, 4]);
    /// let pinned_ref: Pin<&[u8; 4]> = buffer.as_pin();
    /// assert_eq!(&*pinned_ref, &[1, 2, 3, 4]);
    /// ```
    #[inline]
    pub fn as_pin(&self) -> Pin<&T> {
        self.inner.as_ref()
    }

    /// Returns a mutable pinned reference to the buffer data.
    ///
    /// This method provides safe mutable access to the pinned data while
    /// maintaining the pinning guarantees. Essential for io_uring write operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    /// use std::pin::Pin;
    ///
    /// let mut buffer = PinnedBuffer::new([0; 4]);
    /// let mut pinned_ref: Pin<&mut [u8; 4]> = buffer.as_pin_mut();
    /// // Safe to modify through pinned reference
    /// ```
    #[inline]
    pub fn as_pin_mut(&mut self) -> Pin<&mut T> {
        self.inner.as_mut()
    }

    /// Returns the current generation of this buffer.
    ///
    /// The generation counter tracks buffer lifecycle events and can be used
    /// for debugging buffer reuse patterns and detecting potential issues.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    ///
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    /// let initial_gen = buffer.generation();
    ///
    /// buffer.mark_in_use();
    /// assert!(buffer.generation() > initial_gen);
    /// ```
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// Mark this buffer as in use and increment generation.
    ///
    /// This method should be called when the buffer is being used for I/O
    /// operations. It helps track buffer lifecycle for debugging purposes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    ///
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    /// let gen_before = buffer.generation();
    ///
    /// buffer.mark_in_use();
    /// assert_eq!(buffer.generation(), gen_before + 1);
    /// ```
    pub fn mark_in_use(&mut self) {
        self.generation.increment();
    }

    /// Mark this buffer as available and increment generation.
    ///
    /// This method should be called when the buffer is no longer being used
    /// for I/O operations and is available for reuse.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    ///
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    /// buffer.mark_in_use();
    /// let gen_after_use = buffer.generation();
    ///
    /// buffer.mark_available();
    /// assert_eq!(buffer.generation(), gen_after_use + 1);
    /// ```
    pub fn mark_available(&mut self) {
        self.generation.increment();
    }

    /// Check if this buffer is available for use.
    ///
    /// Note: This is a simple implementation - a more sophisticated
    /// version might track actual usage state.
    pub fn is_available(&self) -> bool {
        true // For now, always return true
    }

    /// Returns a raw pointer to the buffer data.
    ///
    /// # Safety
    ///
    /// The pointer is valid only while the buffer exists.
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        Pin::as_ref(&self.inner).get_ref() as *const T
    }

    /// Returns a mutable raw pointer to the buffer data.
    ///
    /// # Safety
    ///
    /// The pointer is valid only while the buffer exists.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        unsafe { Pin::as_mut(&mut self.inner).get_unchecked_mut() as *mut T }
    }
}

impl<T> PinnedBuffer<T> {
    /// Creates a new pinned buffer from the given data.
    ///
    /// This constructor takes ownership of the provided data and pins it in memory,
    /// making it suitable for io_uring operations. The data is moved to the heap
    /// and its address becomes stable for the lifetime of the buffer.
    ///
    /// # Parameters
    ///
    /// * `data` - The data to pin in memory. Can be any type T.
    ///
    /// # Returns
    ///
    /// Returns a new `PinnedBuffer<T>` with the data pinned and generation counter
    /// initialized to 0.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    ///
    /// // Pin an array
    /// let buffer = PinnedBuffer::new([1, 2, 3, 4]);
    /// assert_eq!(buffer.len(), 4);
    ///
    /// // Pin a custom struct
    /// #[derive(Debug, PartialEq)]
    /// struct Data { value: u32 }
    ///
    /// let buffer = PinnedBuffer::new(Data { value: 42 });
    /// assert_eq!(buffer.as_pin().value, 42);
    /// ```
    #[inline]
    pub fn new(data: T) -> Self {
        Self {
            inner: Box::pin(data),
            generation: GenerationCounter::new(),
        }
    }
}

impl PinnedBuffer<[u8]> {
    /// Creates a new zero-initialized pinned buffer with the specified size.
    ///
    /// This is the primary method for creating buffers for I/O operations.
    /// The buffer is heap-allocated, zero-initialized, and pinned for stable
    /// memory addresses required by io_uring.
    ///
    /// # Parameters
    ///
    /// * `size` - The size of the buffer in bytes. Must be greater than 0 for meaningful use.
    ///
    /// # Returns
    ///
    /// Returns a `PinnedBuffer<[u8]>` containing a zero-initialized buffer of the
    /// specified size, ready for I/O operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    ///
    /// // Create a 4KB buffer for file I/O
    /// let buffer = PinnedBuffer::with_capacity(4096);
    /// assert_eq!(buffer.len(), 4096);
    /// assert!(buffer.as_slice().iter().all(|&b| b == 0)); // All zeros
    ///
    /// // Create buffer for network I/O
    /// let net_buffer = PinnedBuffer::with_capacity(1500); // MTU size
    /// assert_eq!(net_buffer.len(), 1500);
    /// ```
    pub fn with_capacity(size: usize) -> Self {
        let data = vec![0u8; size].into_boxed_slice();
        Self {
            inner: Pin::from(data),
            generation: GenerationCounter::new(),
        }
    }

    /// Creates a new pinned buffer from a vector.
    ///
    /// This method takes ownership of a vector and converts it into a pinned
    /// buffer. The vector's data is preserved and the buffer can be used
    /// immediately for I/O operations.
    ///
    /// # Parameters
    ///
    /// * `vec` - The vector to convert into a pinned buffer.
    ///
    /// # Returns
    ///
    /// Returns a `PinnedBuffer<[u8]>` containing the vector's data, pinned
    /// and ready for I/O operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    ///
    /// let data = vec![1, 2, 3, 4, 5];
    /// let buffer = PinnedBuffer::from_vec(data);
    /// assert_eq!(buffer.as_slice(), &[1, 2, 3, 4, 5]);
    /// assert_eq!(buffer.len(), 5);
    /// ```
    #[inline]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        Self::from_boxed_slice(vec.into_boxed_slice())
    }

    /// Creates a new pinned buffer from a boxed slice.
    #[inline]
    pub fn from_boxed_slice(slice: Box<[u8]>) -> Self {
        Self {
            inner: Pin::from(slice),
            generation: GenerationCounter::new(),
        }
    }

    /// Creates a new pinned buffer by copying from a slice.
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Self {
        Self::from_vec(slice.to_vec())
    }

    /// Creates a new aligned pinned buffer with the specified size.
    ///
    /// This method creates a pinned buffer using page-aligned allocation (4096 bytes)
    /// for optimal DMA performance with io_uring operations. The alignment helps
    /// reduce memory copy overhead in the kernel.
    ///
    /// # Parameters
    ///
    /// * `size` - The size of the buffer in bytes. The buffer will be page-aligned
    ///   regardless of the size specified.
    ///
    /// # Returns
    ///
    /// Returns a `PinnedBuffer<[u8]>` with page-aligned, zero-initialized memory
    /// optimized for high-performance I/O operations.
    ///
    /// # Performance Notes
    ///
    /// Page-aligned buffers can provide significant performance benefits for:
    /// - Large sequential I/O operations
    /// - Direct memory access (DMA) operations
    /// - Kernel bypass operations with io_uring
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::buffer::PinnedBuffer;
    ///
    /// // Create aligned buffer for high-performance I/O
    /// let buffer = PinnedBuffer::with_capacity_aligned(8192);
    /// assert_eq!(buffer.len(), 8192);
    /// assert!(buffer.as_slice().iter().all(|&b| b == 0)); // Zero-initialized
    ///
    /// // Even small sizes get page alignment benefits
    /// let small_aligned = PinnedBuffer::with_capacity_aligned(64);
    /// assert_eq!(small_aligned.len(), 64);
    /// ```
    pub fn with_capacity_aligned(size: usize) -> Self {
        let data = allocate_aligned_buffer(size);
        Self {
            inner: Pin::from(data),
            generation: GenerationCounter::new(),
        }
    }

    /// Creates a new NUMA-aware pinned buffer with the specified size.
    /// On Linux, attempts to allocate memory on the specified NUMA node.
    #[cfg(target_os = "linux")]
    pub fn with_capacity_numa(size: usize, numa_node: Option<usize>) -> Self {
        let data = allocate_numa_buffer(size, numa_node);
        Self {
            inner: Pin::from(data),
            generation: GenerationCounter::new(),
        }
    }

    /// Creates a new NUMA-aware pinned buffer (stub implementation for non-Linux).
    #[cfg(not(target_os = "linux"))]
    pub fn with_capacity_numa(size: usize, _numa_node: Option<usize>) -> Self {
        // On non-Linux platforms, fall back to regular aligned allocation
        Self::with_capacity_aligned(size)
    }

    /// Returns a mutable slice reference with pinning guarantees.
    #[inline]
    pub fn as_mut_slice(&mut self) -> Pin<&mut [u8]> {
        self.inner.as_mut()
    }

    /// Returns an immutable slice reference.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.inner
    }

    /// Returns the length of the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Checks if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<const N: usize> PinnedBuffer<[u8; N]> {
    /// Creates a new pinned buffer from a fixed-size array.
    #[inline]
    pub fn from_array(array: [u8; N]) -> Self {
        Self::new(array)
    }

    /// Creates a new zero-initialized pinned buffer.
    #[inline]
    pub fn zeroed() -> Self {
        Self::new([0u8; N])
    }

    /// Returns an immutable slice reference to the array.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &*self.inner
    }

    /// Returns a mutable slice reference with pinning guarantees.
    #[inline]
    pub fn as_mut_slice(&mut self) -> Pin<&mut [u8]> {
        unsafe {
            let array_ptr = self.inner.as_mut().get_unchecked_mut().as_mut_ptr();
            let slice = std::slice::from_raw_parts_mut(array_ptr, N);
            Pin::new_unchecked(slice)
        }
    }

    /// Returns the length of the buffer.
    #[inline]
    pub const fn len(&self) -> usize {
        N
    }

    /// Checks if the buffer is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

// SAFETY: PinnedBuffer can be sent between threads when T is Send
unsafe impl<T: Send + ?Sized> Send for PinnedBuffer<T> {}

// SAFETY: PinnedBuffer can be shared between threads when T is Sync
unsafe impl<T: Sync + ?Sized> Sync for PinnedBuffer<T> {}
