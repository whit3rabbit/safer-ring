//! Pinned buffer management for safe io_uring operations.
//!
//! This module provides [`PinnedBuffer<T>`] which ensures buffers remain pinned in memory
//! during io_uring operations, preventing use-after-free bugs and ensuring memory safety.

pub mod allocation;
pub mod generation;
pub mod numa;

pub use allocation::*;
pub use generation::*;
pub use numa::*;

use std::pin::Pin;

/// A buffer that is pinned in memory for io_uring operations.
///
/// This type ensures the underlying buffer cannot be moved in memory while being used
/// for I/O operations. It uses [`Pin<Box<T>>`] to provide stable memory addresses.
pub struct PinnedBuffer<T: ?Sized> {
    /// Heap-allocated, pinned buffer data - guarantees stable memory address
    inner: Pin<Box<T>>,
    /// Generation counter for tracking buffer lifecycle and reuse  
    generation: GenerationCounter,
}

impl<T: ?Sized> PinnedBuffer<T> {
    /// Returns a pinned reference to the buffer data.
    #[inline]
    pub fn as_pin(&self) -> Pin<&T> {
        self.inner.as_ref()
    }

    /// Returns a mutable pinned reference to the buffer data.
    #[inline]
    pub fn as_pin_mut(&mut self) -> Pin<&mut T> {
        self.inner.as_mut()
    }

    /// Returns the current generation of this buffer.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// Mark this buffer as in use and increment generation.
    ///
    /// This is used for lifecycle tracking and debugging.
    pub fn mark_in_use(&mut self) {
        self.generation.increment();
    }

    /// Mark this buffer as available and increment generation.
    ///
    /// This is used for lifecycle tracking and debugging.
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
    pub fn with_capacity(size: usize) -> Self {
        let data = vec![0u8; size].into_boxed_slice();
        Self {
            inner: Pin::from(data),
            generation: GenerationCounter::new(),
        }
    }

    /// Creates a new pinned buffer from a vector.
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
    /// Uses page-aligned allocation for better DMA performance.
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
