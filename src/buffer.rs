//! Pinned buffer management for safe io_uring operations.
//!
//! This module provides [`PinnedBuffer<T>`] which ensures buffers remain pinned in memory
//! during io_uring operations, preventing use-after-free bugs and ensuring memory safety.
//!
//! The key insight is that io_uring operations work with raw pointers to buffer data,
//! so we must guarantee the buffer won't move in memory while operations are in flight.
//! [`Pin<Box<T>>`] provides this guarantee at compile time.

use std::cell::Cell;
use std::pin::Pin;

/// A buffer that is pinned in memory for io_uring operations.
///
/// This type ensures the underlying buffer cannot be moved in memory while being used
/// for I/O operations. It uses [`Pin<Box<T>>`] to provide stable memory addresses and
/// includes generation tracking to manage buffer usage state.
///
/// The generation counter serves two purposes:
/// 1. Detecting buffer reuse patterns for debugging
/// 2. Providing a lightweight way to track buffer state changes
///
/// # Memory Layout
///
/// The buffer is heap-allocated and pinned, ensuring stable addresses required
/// by io_uring's zero-copy semantics.
///
/// # Thread Safety
///
/// [`PinnedBuffer`] is [`Send`] and [`Sync`] when `T` is [`Send`] and [`Sync`].
/// The generation counter uses [`Cell`] for interior mutability without locks.
pub struct PinnedBuffer<T: ?Sized> {
    /// Heap-allocated, pinned buffer data - guarantees stable memory address
    inner: Pin<Box<T>>,
    /// Generation counter for tracking buffer lifecycle and reuse
    generation: Cell<u64>,
}

impl<T: ?Sized> PinnedBuffer<T> {
    /// Returns a pinned reference to the buffer data.
    ///
    /// This maintains pinning guarantees while providing access to the underlying data.
    #[inline]
    pub fn as_pin(&self) -> Pin<&T> {
        self.inner.as_ref()
    }

    /// Returns a mutable pinned reference to the buffer data.
    ///
    /// This maintains pinning guarantees while providing mutable access.
    #[inline]
    pub fn as_pin_mut(&mut self) -> Pin<&mut T> {
        self.inner.as_mut()
    }

    /// Returns the current generation of this buffer.
    ///
    /// The generation counter tracks buffer lifecycle events and can be used
    /// to detect buffer reuse patterns or debug buffer management issues.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// Increments the generation counter.
    ///
    /// Called internally when buffer state changes (submission, completion, reuse).
    /// Uses wrapping arithmetic to handle overflow gracefully.
    #[inline]
    #[allow(dead_code)] // Used by other modules in the crate
    pub(crate) fn increment_generation(&self) {
        // Use wrapping_add to handle u64::MAX overflow gracefully
        self.generation.set(self.generation.get().wrapping_add(1));
    }

    /// Returns a raw pointer to the buffer data.
    ///
    /// The pointer is guaranteed to remain stable for the buffer's lifetime,
    /// which is essential for io_uring's zero-copy operations.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The buffer is not accessed mutably through other means while this pointer is in use
    /// - The pointer is not used after the [`PinnedBuffer`] is dropped
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        // Use get_ref() to extract the pointer without affecting Pin guarantees
        Pin::as_ref(&self.inner).get_ref() as *const T
    }

    /// Returns a mutable raw pointer to the buffer data.
    ///
    /// The pointer is guaranteed to remain stable for the buffer's lifetime.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The buffer is not accessed through other means while this pointer is in use
    /// - The pointer is not used after the [`PinnedBuffer`] is dropped
    /// - Proper synchronization if used across threads
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        // SAFETY: We have exclusive access via &mut self, so get_unchecked_mut is safe
        unsafe { Pin::as_mut(&mut self.inner).get_unchecked_mut() as *mut T }
    }
}

impl<T> PinnedBuffer<T> {
    /// Creates a new pinned buffer from the given data.
    ///
    /// The data is moved into a [`Pin<Box<T>>`] to ensure it cannot be moved in memory.
    /// This is essential for io_uring operations which require stable buffer addresses.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let data = [1u8, 2, 3, 4];
    /// let buffer = PinnedBuffer::new(data);
    /// assert_eq!(buffer.generation(), 0);
    /// ```
    #[inline]
    pub fn new(data: T) -> Self {
        Self {
            // Box::pin ensures heap allocation and pinning in one step
            inner: Box::pin(data),
            // Start with generation 0 for new buffers
            generation: Cell::new(0),
        }
    }
}

impl<const N: usize> PinnedBuffer<[u8; N]> {
    /// Creates a new pinned buffer from a fixed-size array.
    ///
    /// This is a convenience constructor for compile-time known array sizes.
    /// Prefer this over [`Self::new`] for better API clarity.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::from_array([0u8; 1024]);
    /// assert_eq!(buffer.len(), 1024);
    /// ```
    #[inline]
    pub fn from_array(array: [u8; N]) -> Self {
        Self::new(array)
    }

    /// Creates a new zero-initialized pinned buffer.
    ///
    /// This is optimized for read operations where you need a clean buffer.
    /// The compiler can often optimize the zero-initialization.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::<[u8; 1024]>::zeroed();
    /// assert!(buffer.as_slice().iter().all(|&b| b == 0));
    /// ```
    #[inline]
    pub fn zeroed() -> Self {
        // The compiler can optimize [0u8; N] initialization
        Self::new([0u8; N])
    }

    /// Returns an immutable slice reference to the array.
    ///
    /// This provides zero-cost access to the buffer contents as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::from_array([1u8, 2, 3, 4]);
    /// let slice = buffer.as_slice();
    /// assert_eq!(slice, &[1, 2, 3, 4]);
    /// ```
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        // Direct deref of Pin<Box<[u8; N]>> to &[u8; N], then coerce to &[u8]
        &*self.inner
    }

    /// Returns a mutable slice reference with pinning guarantees.
    ///
    /// The returned [`Pin<&mut [u8]>`] ensures the slice cannot be moved or reallocated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let mut buffer = PinnedBuffer::from_array([0u8; 4]);
    /// {
    ///     let slice = buffer.as_mut_slice();
    ///     // Use std::pin::Pin::get_mut() for safe access if T: Unpin
    /// }
    /// ```
    #[inline]
    pub fn as_mut_slice(&mut self) -> Pin<&mut [u8]> {
        // SAFETY: We convert Pin<&mut [u8; N]> to Pin<&mut [u8]>
        // This is safe because:
        // 1. We have exclusive access via &mut self
        // 2. Arrays are Unpin, so we can safely get mutable access
        // 3. The slice points to the same memory as the array
        unsafe {
            let array_ptr = self.inner.as_mut().get_unchecked_mut().as_mut_ptr();
            let slice = std::slice::from_raw_parts_mut(array_ptr, N);
            Pin::new_unchecked(slice)
        }
    }

    /// Returns the length of the buffer.
    ///
    /// This is a compile-time constant, so it's zero-cost.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::<[u8; 1024]>::zeroed();
    /// assert_eq!(buffer.len(), 1024);
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        N
    }

    /// Checks if the buffer is empty.
    ///
    /// This is a compile-time constant for fixed-size arrays.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::<[u8; 0]>::zeroed();
    /// assert!(buffer.is_empty());
    ///
    /// let buffer = PinnedBuffer::<[u8; 1024]>::zeroed();
    /// assert!(!buffer.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

impl PinnedBuffer<[u8]> {
    /// Creates a new zero-initialized pinned buffer with the specified size.
    ///
    /// This allocates a heap buffer, zero-initializes it, and pins it in memory.
    /// Prefer fixed-size arrays when the size is known at compile time for better performance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::with_capacity(1024);
    /// assert_eq!(buffer.len(), 1024);
    /// assert!(buffer.as_slice().iter().all(|&b| b == 0));
    /// ```
    pub fn with_capacity(size: usize) -> Self {
        // Convert Vec to Box<[u8]> to avoid keeping excess capacity
        let data = vec![0u8; size].into_boxed_slice();
        Self {
            // Pin::from works directly with Box<[u8]>
            inner: Pin::from(data),
            generation: Cell::new(0),
        }
    }

    /// Creates a new pinned buffer from a vector.
    ///
    /// The vector is converted to a boxed slice (dropping excess capacity) and pinned.
    /// This is more efficient than cloning the data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let data = vec![1u8, 2, 3, 4];
    /// let buffer = PinnedBuffer::from_vec(data);
    /// assert_eq!(buffer.len(), 4);
    /// assert_eq!(buffer.as_slice(), &[1, 2, 3, 4]);
    /// ```
    #[inline]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        // into_boxed_slice() drops excess capacity, which is good for memory efficiency
        Self::from_boxed_slice(vec.into_boxed_slice())
    }

    /// Creates a new pinned buffer from a boxed slice.
    ///
    /// This takes ownership of the boxed slice and pins it without additional allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let data = vec![1u8, 2, 3, 4].into_boxed_slice();
    /// let buffer = PinnedBuffer::from_boxed_slice(data);
    /// assert_eq!(buffer.len(), 4);
    /// ```
    #[inline]
    pub fn from_boxed_slice(slice: Box<[u8]>) -> Self {
        Self {
            inner: Pin::from(slice),
            generation: Cell::new(0),
        }
    }

    /// Creates a new pinned buffer by copying from a slice.
    ///
    /// This allocates a new buffer and copies the data from the provided slice.
    /// Use this when you need to create a pinned buffer from borrowed data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let data = b"Hello, world!";
    /// let buffer = PinnedBuffer::from_slice(data);
    /// assert_eq!(buffer.as_slice(), data);
    /// ```
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Self {
        Self::from_vec(slice.to_vec())
    }

    /// Returns a mutable slice reference with pinning guarantees.
    ///
    /// The returned [`Pin<&mut [u8]>`] ensures the slice cannot be moved or reallocated,
    /// which is essential for io_uring operations that hold pointers to the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let mut buffer = PinnedBuffer::with_capacity(4);
    /// {
    ///     let slice = buffer.as_mut_slice();
    ///     // Use std::pin::Pin::get_mut() for safe access since [u8] is Unpin
    /// }
    /// ```
    #[inline]
    pub fn as_mut_slice(&mut self) -> Pin<&mut [u8]> {
        self.inner.as_mut()
    }

    /// Returns an immutable slice reference.
    ///
    /// This provides zero-cost read-only access to the buffer contents.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::with_capacity(4);
    /// let slice = buffer.as_slice();
    /// assert_eq!(slice.len(), 4);
    /// ```
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        // Direct deref of Pin<Box<[u8]>> to &[u8]
        &self.inner
    }

    /// Returns the length of the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::with_capacity(1024);
    /// assert_eq!(buffer.len(), 1024);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Checks if the buffer is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use safer_ring::buffer::PinnedBuffer;
    /// let buffer = PinnedBuffer::with_capacity(0);
    /// assert!(buffer.is_empty());
    ///
    /// let buffer = PinnedBuffer::with_capacity(1024);
    /// assert!(!buffer.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// Debug implementation that's useful for debugging without exposing sensitive data
impl<T: std::fmt::Debug> std::fmt::Debug for PinnedBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedBuffer")
            .field("inner", &*self.inner)
            .field("generation", &self.generation.get())
            .finish()
    }
}

// Specialized Debug for [u8] to avoid printing large byte arrays
impl std::fmt::Debug for PinnedBuffer<[u8]> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedBuffer")
            .field("len", &self.len())
            .field("generation", &self.generation.get())
            .finish()
    }
}

// SAFETY: PinnedBuffer can be sent between threads when T is Send
// The Pin<Box<T>> is Send when T is Send, and Cell<u64> is Send
unsafe impl<T: Send + ?Sized> Send for PinnedBuffer<T> {}

// SAFETY: PinnedBuffer can be shared between threads when T is Sync
// Even though Cell<u64> is not Sync, we only access it through &self methods
// which are safe for concurrent read access. The generation counter is used
// for debugging/tracking purposes and doesn't affect memory safety.
unsafe impl<T: Sync + ?Sized> Sync for PinnedBuffer<T> {}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinned_buffer_new() {
        let data = [1u8, 2, 3, 4];
        let buffer = PinnedBuffer::new(data);

        assert_eq!(buffer.generation(), 0);
        assert_eq!(buffer.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_pinned_buffer_with_capacity() {
        let buffer = PinnedBuffer::with_capacity(1024);

        assert_eq!(buffer.len(), 1024);
        assert_eq!(buffer.generation(), 0);
        assert!(!buffer.is_empty());

        // All bytes should be zero-initialized
        assert!(buffer.as_slice().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_pinned_buffer_from_vec() {
        let data = vec![1u8, 2, 3, 4, 5];
        let buffer = PinnedBuffer::from_vec(data);

        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.as_slice(), &[1, 2, 3, 4, 5]);
        assert_eq!(buffer.generation(), 0);
    }

    #[test]
    fn test_pinned_buffer_from_boxed_slice() {
        let data = vec![1u8, 2, 3, 4].into_boxed_slice();
        let buffer = PinnedBuffer::from_boxed_slice(data);

        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_pinned_buffer_from_array() {
        let buffer = PinnedBuffer::from_array([1u8, 2, 3, 4]);

        assert_eq!(buffer.as_slice(), &[1, 2, 3, 4]);
        assert_eq!(buffer.generation(), 0);
    }

    #[test]
    fn test_pinned_buffer_zeroed() {
        let buffer = PinnedBuffer::<[u8; 1024]>::zeroed();

        assert_eq!(buffer.as_slice().len(), 1024);
        assert!(buffer.as_slice().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_empty_buffer() {
        let buffer = PinnedBuffer::with_capacity(0);

        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_generation_tracking() {
        let buffer = PinnedBuffer::with_capacity(100);

        assert_eq!(buffer.generation(), 0);

        buffer.increment_generation();
        assert_eq!(buffer.generation(), 1);

        buffer.increment_generation();
        assert_eq!(buffer.generation(), 2);
    }

    #[test]
    fn test_generation_wrapping() {
        let buffer = PinnedBuffer::with_capacity(100);

        // Set generation to max value
        buffer.generation.set(u64::MAX);
        assert_eq!(buffer.generation(), u64::MAX);

        // Increment should wrap to 0
        buffer.increment_generation();
        assert_eq!(buffer.generation(), 0);
    }

    #[test]
    fn test_as_mut_slice() {
        let mut buffer = PinnedBuffer::with_capacity(4);

        {
            let slice = buffer.as_mut_slice();
            // We can get a mutable reference to modify the data
            unsafe {
                let slice_mut = std::pin::Pin::get_unchecked_mut(slice);
                slice_mut[0] = 42;
                slice_mut[1] = 43;
            }
        }

        assert_eq!(buffer.as_slice()[0], 42);
        assert_eq!(buffer.as_slice()[1], 43);
    }

    #[test]
    fn test_pinned_references() {
        let mut buffer = PinnedBuffer::new([1u8, 2, 3, 4]);

        let pin_ref = buffer.as_pin();
        assert_eq!(&*pin_ref, &[1, 2, 3, 4]);

        let pin_mut_ref = buffer.as_pin_mut();
        assert_eq!(&*pin_mut_ref, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_raw_pointers() {
        let mut buffer = PinnedBuffer::with_capacity(4);

        let const_ptr = buffer.as_ptr();
        let mut_ptr = buffer.as_mut_ptr();

        // Pointers should be non-null and point to the same location
        assert!(!const_ptr.is_null());
        assert!(!mut_ptr.is_null());
        assert_eq!(const_ptr as *const u8, mut_ptr as *const u8);
    }

    #[test]
    fn test_pointer_stability() {
        let mut buffer = PinnedBuffer::with_capacity(1024);

        // Get initial pointer
        let ptr1 = buffer.as_ptr();

        // Modify buffer state
        buffer.increment_generation();
        let _slice = buffer.as_mut_slice();

        // Pointer should remain stable
        let ptr2 = buffer.as_ptr();
        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn test_debug_formatting() {
        let buffer = PinnedBuffer::new([1u8, 2, 3]);
        let debug_str = format!("{:?}", buffer);

        assert!(debug_str.contains("PinnedBuffer"));
        assert!(debug_str.contains("generation"));
    }

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        // PinnedBuffer should be Send and Sync for appropriate T
        assert_send::<PinnedBuffer<[u8]>>();
        assert_sync::<PinnedBuffer<[u8]>>();

        assert_send::<PinnedBuffer<[u8; 1024]>>();
        assert_sync::<PinnedBuffer<[u8; 1024]>>();
    }

    #[test]
    fn test_large_buffer() {
        // Test with a larger buffer to ensure no issues with size
        let buffer = PinnedBuffer::with_capacity(1024 * 1024); // 1MB

        assert_eq!(buffer.len(), 1024 * 1024);
        assert!(!buffer.is_empty());
        assert_eq!(buffer.generation(), 0);
    }

    #[test]
    fn test_buffer_modification_through_slice() {
        let mut buffer = PinnedBuffer::from_vec(vec![0u8; 10]);

        // Modify through mutable slice
        {
            let slice = buffer.as_mut_slice();
            unsafe {
                let slice_mut = std::pin::Pin::get_unchecked_mut(slice);
                for (i, byte) in slice_mut.iter_mut().enumerate() {
                    *byte = i as u8;
                }
            }
        }

        // Verify changes
        for (i, &byte) in buffer.as_slice().iter().enumerate() {
            assert_eq!(byte, i as u8);
        }
    }

    #[test]
    fn test_concurrent_generation_access() {
        use std::sync::Arc;
        use std::thread;

        let buffer = Arc::new(PinnedBuffer::with_capacity(100));
        let buffer_clone = Arc::clone(&buffer);

        // Spawn a thread that increments generation
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                buffer_clone.increment_generation();
            }
        });

        // Main thread also increments generation
        for _ in 0..100 {
            buffer.increment_generation();
        }

        handle.join().unwrap();

        // Final generation should be 200
        assert_eq!(buffer.generation(), 200);
    }
}
