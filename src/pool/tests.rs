//! Tests for buffer pool functionality.

use super::*;
use crate::buffer::PinnedBuffer;
use std::pin::Pin;

#[test]
fn pool_creation() {
    let pool = BufferPool::new(5, 1024);
    assert_eq!(pool.capacity(), 5);
    assert_eq!(pool.available().unwrap(), 5);
    assert_eq!(pool.in_use().unwrap(), 0);
    assert_eq!(pool.buffer_size(), 1024);
    assert!(pool.is_full().unwrap());
    assert!(!pool.is_empty().unwrap());
}

#[test]
#[should_panic(expected = "Pool capacity must be greater than zero")]
fn pool_creation_zero_capacity() {
    BufferPool::new(0, 1024);
}

#[test]
#[should_panic(expected = "Buffer size must be greater than zero")]
fn pool_creation_zero_buffer_size() {
    BufferPool::new(5, 0);
}

#[test]
fn pool_with_factory() {
    let pool = BufferPool::with_factory(3, || PinnedBuffer::from_vec(vec![0xFF; 512]));
    assert_eq!(pool.capacity(), 3);
    assert_eq!(pool.buffer_size(), 512);

    let buffer = pool.try_get().unwrap().unwrap();
    assert_eq!(buffer.len(), 512);
    assert!(buffer.as_slice().iter().all(|&b| b == 0xFF));
}

#[test]
fn buffer_allocation_and_return() {
    let pool = BufferPool::new(2, 1024);

    // Initially full
    assert_eq!(pool.available().unwrap(), 2);
    assert_eq!(pool.in_use().unwrap(), 0);

    // Get first buffer
    let buffer1 = pool.try_get().unwrap().unwrap();
    assert_eq!(pool.available().unwrap(), 1);
    assert_eq!(pool.in_use().unwrap(), 1);

    // Get second buffer
    let buffer2 = pool.try_get().unwrap().unwrap();
    assert_eq!(pool.available().unwrap(), 0);
    assert_eq!(pool.in_use().unwrap(), 2);

    // Pool is now empty
    let buffer3 = pool.try_get().unwrap();
    assert!(buffer3.is_none());

    // Drop buffers - should return to pool
    drop(buffer1);
    drop(buffer2);
    assert_eq!(pool.available().unwrap(), 2);
    assert_eq!(pool.in_use().unwrap(), 0);
}

#[test]
fn pooled_buffer_operations() {
    let pool = BufferPool::new(1, 1024);
    let mut buffer = pool.try_get().unwrap().unwrap();

    assert_eq!(buffer.len(), 1024);
    assert!(!buffer.is_empty());

    // Test slice access
    let slice = buffer.as_slice();
    assert_eq!(slice.len(), 1024);

    // Test mutable slice access
    {
        let mut_slice = buffer.as_mut_slice();
        unsafe {
            let slice_mut = Pin::get_unchecked_mut(mut_slice);
            slice_mut[0] = 42;
        }
    }

    assert_eq!(buffer.as_slice()[0], 42);
}

#[test]
fn pool_stats() {
    let pool = BufferPool::new(10, 2048);
    let stats = pool.stats().unwrap();

    assert_eq!(stats.capacity, 10);
    assert_eq!(stats.available, 10);
    assert_eq!(stats.in_use, 0);
    assert_eq!(stats.buffer_size, 2048);
    assert_eq!(stats.total_allocations, 0);
    assert_eq!(stats.utilization, 0.0);

    // Allocate some buffers
    let _buffer1 = pool.try_get().unwrap().unwrap();
    let _buffer2 = pool.try_get().unwrap().unwrap();

    let stats = pool.stats().unwrap();
    assert_eq!(stats.available, 8);
    assert_eq!(stats.in_use, 2);
    assert_eq!(stats.total_allocations, 2);
    assert_eq!(stats.utilization, 0.2);
}
#[test]

fn pool_stats_enhanced_methods() {
    let pool = BufferPool::new(10, 1024);
    let stats = pool.stats().unwrap();

    // Test new methods
    assert_eq!(stats.utilization_percent(), 0.0);
    assert_eq!(stats.success_rate_percent(), 100.0); // No attempts yet
    assert_eq!(stats.total_memory_bytes(), 10 * 1024);
    assert_eq!(stats.memory_in_use_bytes(), 0);
    assert!(!stats.is_under_pressure());
    assert!(!stats.has_allocation_failures());

    // Allocate some buffers to test utilization (9 out of 10 = 90%)
    let _buffers: Vec<_> = (0..9).map(|_| pool.try_get().unwrap().unwrap()).collect();
    let stats = pool.stats().unwrap();

    assert_eq!(stats.utilization_percent(), 90.0);
    assert_eq!(stats.memory_in_use_bytes(), 9 * 1024);
    assert!(stats.is_under_pressure()); // 90% utilization should trigger pressure

    // Try to get more buffers than available to test failure tracking
    let _extra1 = pool.try_get().unwrap().unwrap(); // This should get the last buffer
    let failed = pool.try_get().unwrap(); // This should fail
    assert!(failed.is_none());

    let stats = pool.stats().unwrap();
    assert!(stats.has_allocation_failures());
    assert!(stats.success_rate_percent() < 100.0);
}

#[test]
fn pool_snapshot() {
    let pool = BufferPool::new(5, 512);

    let (available, in_use, is_empty, is_full) = pool.snapshot().unwrap();
    assert_eq!(available, 5);
    assert_eq!(in_use, 0);
    assert!(!is_empty);
    assert!(is_full);

    // Get some buffers
    let _buffer1 = pool.try_get().unwrap().unwrap();
    let _buffer2 = pool.try_get().unwrap().unwrap();

    let (available, in_use, is_empty, is_full) = pool.snapshot().unwrap();
    assert_eq!(available, 3);
    assert_eq!(in_use, 2);
    assert!(!is_empty);
    assert!(!is_full);
}
