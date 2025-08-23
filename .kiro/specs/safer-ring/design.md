# Design Document

## Overview

Safer-Ring is a safe Rust wrapper around io_uring that provides zero-cost abstractions while preventing common memory safety issues. The library leverages Rust's type system, lifetime management, and pinning mechanisms to ensure that buffers remain valid during asynchronous I/O operations, eliminating use-after-free bugs and data races that are common with raw io_uring usage.

The design follows the principle of "making illegal states unrepresentable" by using type-level state machines, lifetime constraints, and compile-time enforcement of safety invariants. This approach ensures that safety violations result in compile-time errors rather than runtime panics or undefined behavior.

## Architecture

### Core Components

The architecture is built around several key components that work together to provide safe io_uring operations:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Application   │    │   Buffer Pool   │    │   Registry      │
│                 │    │                 │    │                 │
│ - User Code     │    │ - Pre-allocated │    │ - FD Management │
│ - Async/Await   │    │ - Pinned Buffers│    │ - Buffer Reg    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Ring<'ring>                              │
│                                                                 │
│ - Lifetime Management    - Operation Tracking                   │
│ - Safety Guarantees      - Submission/Completion Queues        │
└─────────────────────────────────────────────────────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Operation     │    │   Future        │    │   Buffer        │
│                 │    │                 │    │                 │
│ - Type States   │    │ - Async Support │    │ - Pin Safety    │
│ - Builder API   │    │ - Waker Mgmt    │    │ - Lifetime Tied │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Type-Level State Machine

Operations progress through well-defined states enforced at compile time:

```rust
Building → Submitted → Completed<T>
```

Each state transition is explicit and prevents invalid operations:
- `Building`: Can be configured but not submitted
- `Submitted`: Cannot be reconfigured, can be polled for completion
- `Completed<T>`: Contains result and can return buffer ownership

### Lifetime Relationships

The design enforces critical lifetime relationships:

```rust
'buf: 'ring  // Buffers must outlive ring operations
```

This ensures that no buffer can be dropped while an operation referencing it is still in flight.

## Components and Interfaces

### Ring<'ring>

The central component that wraps io_uring and provides safety guarantees:

```rust
pub struct Ring<'ring> {
    inner: io_uring::IoUring,
    phantom: PhantomData<&'ring ()>,
    operations: RefCell<OperationTracker<'ring>>,
}
```

**Key Responsibilities:**
- Manages the underlying io_uring instance
- Tracks all in-flight operations
- Enforces lifetime constraints
- Provides safe submission and completion APIs
- Ensures proper cleanup on drop

**Safety Invariants:**
- All operations must complete before ring destruction
- Buffer lifetimes must exceed operation lifetimes
- No operation can be submitted twice

### PinnedBuffer<T>

Manages buffers that are pinned in memory for io_uring operations:

```rust
pub struct PinnedBuffer<T: ?Sized> {
    inner: Pin<Box<T>>,
    generation: Cell<u64>,
}
```

**Key Features:**
- Prevents buffer movement during operations
- Tracks buffer usage through generation counters
- Provides safe pinned references with lifetime constraints
- Automatic cleanup when operations complete

**Safety Guarantees:**
- Buffers cannot be moved while pinned
- References are tied to buffer lifetime
- Generation tracking prevents use-after-free

### Operation<'ring, 'buf, S>

Type-safe operation builder with compile-time state tracking:

```rust
pub struct Operation<'ring, 'buf, S> {
    ring: PhantomData<&'ring ()>,
    buffer: Option<Pin<&'buf mut [u8]>>,
    fd: RawFd,
    offset: u64,
    state: S,
}
```

**State Types:**
- `Building`: Initial state, allows configuration
- `Submitted`: Operation in flight, prevents modification
- `Completed<T>`: Operation finished, contains result

**Builder Pattern:**
```rust
let operation = Operation::new()
    .set_fd(fd)
    .set_buffer(pinned_buffer)
    .set_offset(0)
    .submit(); // Transitions to Submitted state
```

### Future Integration

Seamless async/await support through custom Future implementations:

```rust
pub struct ReadFuture<'ring, 'buf> {
    operation: Option<Operation<'ring, 'buf, Submitted>>,
}

impl<'ring, 'buf> Future for ReadFuture<'ring, 'buf> {
    type Output = io::Result<(usize, Pin<&'buf mut [u8]>)>;
    // Implementation polls completion queue and returns buffer ownership
}
```

**Key Features:**
- Returns both result and buffer ownership on completion
- Proper waker management for efficient polling
- Automatic cleanup if future is dropped
- Integration with Tokio and other async runtimes

### BufferPool

Pre-allocated, reusable buffer management:

```rust
pub struct BufferPool {
    buffers: Arc<Mutex<PoolInner>>,
}

pub struct PooledBuffer {
    pool: Arc<Mutex<PoolInner>>,
    index: usize,
}
```

**Benefits:**
- Eliminates allocation overhead in hot paths
- All buffers pre-pinned for immediate use
- Automatic return to pool on operation completion
- Thread-safe sharing across multiple rings

### Registry

Safe management of registered file descriptors and buffers:

```rust
pub struct Registry<'ring> {
    registered_fds: Vec<RegisteredFd>,
    registered_buffers: Vec<Pin<Box<[u8]>>>,
}

pub struct RegisteredFd {
    index: u32,
    _marker: PhantomData<*const ()>, // Prevents Send/Sync
}
```

**Safety Features:**
- Prevents use of unregistered resources
- Automatic cleanup on drop
- Compile-time prevention of use-after-unregistration
- Lifetime tracking for registered buffers

## Data Models

### Core Data Structures

#### OperationTracker
```rust
struct OperationTracker<'ring> {
    in_flight: HashMap<u64, OperationHandle<'ring>>,
    next_id: u64,
}
```

Tracks all operations to ensure proper cleanup and prevent ring destruction with pending operations.

#### BufferState
```rust
pub enum BufferState {
    Available,
    InFlight(OperationId),
    Completed,
}
```

Tracks buffer usage state to prevent concurrent access and ensure proper lifecycle management.

#### SaferRingError
```rust
#[derive(Debug, thiserror::Error)]
pub enum SaferRingError {
    #[error("Buffer still in flight")]
    BufferInFlight,
    
    #[error("Operation not completed")]
    OperationPending,
    
    #[error("io_uring error: {0}")]
    IoUring(#[from] io_uring::Error),
}
```

Comprehensive error handling with clear diagnostic messages.

### Memory Layout Considerations

The design carefully manages memory layout to ensure safety:

1. **Pinned Buffers**: All I/O buffers are pinned using `Pin<Box<T>>` to prevent movement
2. **Stable Addresses**: Buffer addresses remain stable throughout operation lifetime
3. **Alignment**: Proper alignment for DMA operations when using registered buffers
4. **Contiguity**: Support for both scattered and contiguous buffer layouts

## Error Handling

### Compile-Time Safety

The primary error prevention strategy is compile-time enforcement:

```rust
// This won't compile - buffer lifetime too short
fn invalid_operation(ring: &Ring) {
    let buffer = vec![0u8; 1024]; // Local buffer
    let future = ring.read(fd, &buffer); // Error: buffer doesn't live long enough
    // buffer would be dropped here while operation is pending
}
```

### Runtime Error Categories

1. **I/O Errors**: Wrapped io_uring errors with additional context
2. **State Errors**: Attempts to use operations in invalid states
3. **Resource Errors**: Issues with file descriptor or buffer management
4. **Lifecycle Errors**: Improper cleanup or resource leaks

### Error Recovery

```rust
match operation.await {
    Ok((bytes_read, buffer)) => {
        // Success: process data and buffer is automatically returned
    }
    Err(e) => {
        // Error: buffer ownership is still returned for cleanup
        // No resource leaks even on error paths
    }
}
```

## Testing Strategy

### Compile-Fail Tests

Critical safety properties are verified through compile-fail tests:

```rust
#[test]
#[compile_fail]
fn test_buffer_outlives_operation() {
    let ring = Ring::new(10).unwrap();
    {
        let buffer = vec![0u8; 100];
        ring.submit_read(0, &buffer); // Should not compile
    } // buffer dropped while operation pending
}
```

### Runtime Safety Tests

Comprehensive runtime testing covers:

1. **Concurrent Operations**: Multiple operations on different buffers
2. **Buffer Pool Stress**: High-frequency allocation/deallocation
3. **Error Injection**: Simulated I/O failures and recovery
4. **Resource Cleanup**: Proper cleanup under various failure scenarios

### Property-Based Testing

Using tools like `proptest` to verify:
- Buffer lifecycle invariants
- Operation state transitions
- Concurrent access patterns
- Resource leak prevention

### Integration Tests

Real-world scenarios including:
- Echo servers with high connection counts
- File copying with various buffer sizes
- Network protocols with complex I/O patterns
- Performance benchmarks against raw io_uring

### Loom Testing

Concurrency testing using `loom` to verify:
- Thread safety of shared data structures
- Proper synchronization in buffer pools
- Race condition prevention in operation tracking

## Performance Considerations

### Zero-Cost Abstractions

The design ensures that safety comes without runtime overhead:

1. **Compile-Time Checks**: All safety invariants enforced at compile time
2. **Monomorphization**: Generic types specialized for optimal performance
3. **Inlining**: Critical paths marked for aggressive inlining
4. **Zero-Copy**: Direct buffer passing without intermediate copies

### Memory Efficiency

1. **Buffer Reuse**: Pool-based allocation eliminates repeated allocations
2. **Registered Buffers**: Kernel-side buffer registration for optimal performance
3. **Batch Operations**: Support for submitting multiple operations efficiently
4. **NUMA Awareness**: Buffer allocation considers NUMA topology

### Benchmarking Strategy

Performance validation through:
- Micro-benchmarks for individual operations
- Macro-benchmarks for complete applications
- Comparison with raw io_uring performance
- Memory usage profiling
- Latency distribution analysis

## Security Considerations

### Memory Safety

1. **No Unsafe Code in Public API**: All unsafe operations encapsulated internally
2. **Buffer Bounds Checking**: Automatic bounds validation for all buffer operations
3. **Use-After-Free Prevention**: Compile-time lifetime enforcement
4. **Double-Free Prevention**: RAII and ownership tracking

### Resource Management

1. **File Descriptor Leaks**: Automatic cleanup through RAII
2. **Memory Leaks**: Guaranteed cleanup even on panic
3. **Kernel Resource Limits**: Proper handling of io_uring resource limits
4. **Privilege Escalation**: No elevation of privileges required

### Attack Surface Reduction

1. **Minimal Dependencies**: Limited external dependencies to reduce attack surface
2. **Input Validation**: Comprehensive validation of all user inputs
3. **Error Information**: Careful error messages that don't leak sensitive information
4. **Audit Trail**: Optional logging for security-sensitive operations

## Platform Considerations

### Linux Kernel Versions

- **Minimum**: Linux 5.1 (basic io_uring support)
- **Recommended**: Linux 5.19+ (buffer rings, multi-shot operations)
- **Optimal**: Linux 6.0+ (latest performance improvements)

### Architecture Support

- **Primary**: x86_64, aarch64
- **Secondary**: Other architectures supported by io_uring
- **Testing**: Comprehensive testing on all supported platforms

### Integration Points

1. **Tokio Runtime**: Seamless integration with Tokio's async ecosystem
2. **async-std**: Compatible with async-std runtime
3. **Custom Runtimes**: Generic Future implementation works with any runtime
4. **Blocking Code**: Bridge functions for integration with synchronous code