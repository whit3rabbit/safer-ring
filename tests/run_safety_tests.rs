//! Simple test runner to verify all safety tests can be executed.

use safer_ring::{BufferPool, PinnedBuffer};
use std::process::Command;

#[test]
fn run_compile_fail_tests() {
    let output = Command::new("cargo")
        .args(["test", "--test", "compile_fail"])
        .output()
        .expect("Failed to run compile-fail tests");

    assert!(
        output.status.success(),
        "Compile-fail tests failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn run_basic_buffer_tests() {
    // Test basic buffer functionality

    // Test pinned buffer creation
    let buffer = PinnedBuffer::with_capacity(1024);
    assert_eq!(buffer.len(), 1024);
    assert_eq!(buffer.generation(), 0);

    // Test buffer pool
    let pool = BufferPool::new(10, 512);
    assert_eq!(pool.capacity(), 10);
    assert_eq!(pool.buffer_size(), 512);

    // Test buffer allocation from pool
    let buffer1 = pool.get();
    assert!(buffer1.is_some());

    let buffer2 = pool.get();
    assert!(buffer2.is_some());

    // Test pool stats
    let stats = pool.stats();
    assert_eq!(stats.total_buffers, 10);
    assert_eq!(stats.in_use_buffers, 2);
    assert_eq!(stats.available_buffers, 8);
}

#[test]
// Methods mark_in_use, mark_available, is_available exist in PinnedBuffer
fn test_buffer_generation_tracking() {
    let mut buffer = PinnedBuffer::with_capacity(100);
    let initial_gen = buffer.generation();

    buffer.mark_in_use();
    assert!(buffer.generation() > initial_gen);

    buffer.mark_available();
    assert!(buffer.generation() > initial_gen);

    assert!(buffer.is_available());
}

#[test]
fn test_buffer_pool_exhaustion() {
    let pool = BufferPool::new(2, 1024);

    let buf1 = pool.get();
    let buf2 = pool.get();
    let buf3 = pool.get();

    assert!(buf1.is_some());
    assert!(buf2.is_some());
    assert!(buf3.is_none()); // Pool should be exhausted

    // Drop one buffer
    drop(buf1);

    // Should be able to get another buffer
    let buf4 = pool.get();
    assert!(buf4.is_some());
}

#[test]
// Methods mark_in_use, mark_available exist in PinnedBuffer
fn test_pinned_buffer_stability() {
    let mut buffer = PinnedBuffer::with_capacity(1024);
    let ptr1 = buffer.as_ptr();

    // Modify buffer state
    buffer.mark_in_use();
    buffer.mark_available();

    let ptr2 = buffer.as_ptr();

    // Pointer should remain stable
    assert_eq!(ptr1, ptr2);
}

#[test]
fn test_buffer_pool_stats_consistency() {
    let pool = BufferPool::new(5, 256);
    let initial_stats = pool.stats();

    assert_eq!(initial_stats.total_buffers, 5);
    assert_eq!(initial_stats.available_buffers, 5);
    assert_eq!(initial_stats.in_use_buffers, 0);

    let _buf1 = pool.get();
    let _buf2 = pool.get();

    let stats = pool.stats();
    assert_eq!(stats.total_buffers, 5);
    assert_eq!(stats.available_buffers, 3);
    assert_eq!(stats.in_use_buffers, 2);
}
