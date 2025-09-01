# `safer-ring` API Reference & Cheat Sheet

This document provides a quick reference to the public API of `safer-ring`. It is organized by concept to help you find what you need quickly.

The core safety principle of `safer-ring` is the **ownership transfer model ("hot potato" pattern)**. 

> **🎯 CRITICAL: Always use OwnedBuffer with `*_owned` methods.** The `PinnedBuffer` API is fundamentally broken and cannot be used in real applications due to insurmountable lifetime constraints that prevent loops and buffer reuse.

For maximum safety, performance, and ease of use, you **must** use the `*_owned` methods which take ownership of an `OwnedBuffer` and return it to you upon completion. The positioned variants (`read_at_owned`, `write_at_owned`) are perfect for file operations like copying and enable efficient buffer reuse.

## Table of Contents

1.  [**Core: `Ring`**](#ring---the-core-component) - The main entry point for all I/O.
2.  [**Buffer Management**](#buffer-management) - `OwnedBuffer` (✅ Use This) & `PinnedBuffer` (❌ Broken - Educational Only).
3.  [**Batch Operations**](#batch-operations---batchring-buf) - Submitting multiple operations at once.
4.  [**Buffer Pooling**](#buffer-pooling---bufferpool) - Efficient buffer reuse.
5.  [**Performance & Registration**](#performance--registration---registryring) - Advanced performance tuning.
6.  [**Async Compatibility Layer**](#async-compatibility-layer) - For `tokio` integration.
7.  [**Configuration**](#configuration---saferringconfig) - Customizing `Ring` behavior.
8.  [**Error Handling**](#error-handling---saferringerror) - Understanding failure modes.

---

## `Ring<'ring>` - The Core Component

The `Ring` is the central struct for submitting and managing all io_uring operations.

#### Method Usage and Lifetimes

Most I/O methods on `Ring` (e.g., `read`, `write`, `send`, `recv`) are *advanced APIs* that take `&mut self` and return a `Future` holding a mutable borrow of the `Ring`. This design avoids internal locks for maximum performance but requires an "await-immediately" pattern.

The recommended safe APIs (`read_owned`, `write_owned`, `accept_safe`, `submit_batch_standalone`) are designed to be more flexible and composable.

> **The `?.await?` Pattern**
> This pattern is essential for robust error handling with the advanced APIs:
> - The first `?` handles submission errors: `ring.method(...) -> Result<Future, Error>`.
> - `.await` waits for the I/O to complete.
> - The second `?` handles completion errors: `Future::Output -> Result<Success, Error>`.

| Method | Description | Example Usage |
| :--- | :--- | :--- |
| `Ring::new(entries: u32)` | Creates a new `Ring` with a specified submission queue depth. Automatically detects the best backend. | `let mut ring = Ring::new(128)?;` |
| `Ring::with_config(config: SaferRingConfig)` | Creates a new `Ring` with detailed custom configuration. | `let cfg = SaferRingConfig::low_latency();`<br>`let mut ring = Ring::with_config(cfg)?;` |
| `read_owned(fd, buffer: OwnedBuffer)` | **(Recommended)** Safely reads from a file descriptor. Takes ownership of the buffer and returns it upon completion. | ```rust
let buffer = OwnedBuffer::new(1024);
let (bytes, buf) = ring.read_owned(fd, buffer).await?;
``` |
| `write_owned(fd, buffer: OwnedBuffer)` | **(Recommended)** Safely writes to a file descriptor. Takes ownership of the buffer and returns it upon completion. Note: writes up to the buffer's capacity; to write exactly N bytes, use `write_at_owned(fd, buffer, 0, N)`. | ```rust
let buffer = OwnedBuffer::from_slice(b"data");
let (bytes, buf) = ring.write_owned(fd, buffer).await?;
``` |
| `read_at_owned(fd, buffer: OwnedBuffer, offset: u64)` | **(Recommended)** Safely reads from a file descriptor at a specific offset. Perfect for file operations and the hot potato pattern. | ```rust
let buffer = OwnedBuffer::new(1024);
let (bytes, buf) = ring.read_at_owned(fd, buffer, 100).await?;
// Can reuse the returned buffer
let (bytes2, buf2) = ring.read_at_owned(fd, buf, 200).await?;
``` |
| `write_at_owned(fd, buffer: OwnedBuffer, offset: u64, len: usize)` | **(Recommended)** Safely writes to a file descriptor at a specific offset. Takes ownership and returns the buffer, making it perfect for file copy operations. | ```rust
let buffer = OwnedBuffer::from_slice(b"Hello, world!");
let (bytes, buf) = ring.write_at_owned(fd, buffer, 100, 13).await?;
``` |
| `accept_safe(listening_fd)` | Safely accepts a new network connection. Returns the new client file descriptor as reported by the kernel (CQE result). | ```rust
// Assumes listen_fd is a valid listening socket
let client_fd = ring.accept_safe(listen_fd).await?;
``` |
| `submit_batch_standalone(batch)` | **(Recommended for composition)** Submits a batch and returns a `StandaloneBatchFuture` that **does not** hold a borrow of the ring. Useful when composing batches with other operations on the same `Ring`. The future must be polled with `poll_with_ring`. | ```rust
use std::future::poll_fn;
let mut future = ring.submit_batch_standalone(batch)?;
// `ring` can still be used here for other ops
let res = poll_fn(|cx| {
    future.poll_with_ring(&mut ring, cx)
}).await?;
``` |
| `read(fd, buffer: Pin<&mut [u8]>)` | **(Advanced)** Low-level read using a pinned buffer slice.<br><br>⚠️ **Usage Note:** Returns a `Future` that holds a mutable borrow of the `Ring`. You **must** `.await` it immediately before making other calls on the same `Ring` to release the borrow. The `?.await?` pattern is recommended for robust error handling. | **Correct:**<br><br>```rust
let (bytes, _) = ring.read(
    fd, p_buf.as_mut_slice()
)?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
// ERROR: `ring` is borrowed mutably by `future1`
let future1 = ring.read(fd1, buf1)?;
let future2 = ring.read(fd2, buf2)?;
tokio::join!(future1, future2);
``` |
| `write(fd, buffer: Pin<&mut [u8]>)` | **(Advanced)** Low-level write using a pinned buffer slice.<br><br>⚠️ **Usage Note:** Returns a `Future` that holds a mutable borrow of the `Ring`. You **must** `.await` it immediately before making other calls on the same `Ring`. Use `?.await?` for robust error handling. | **Correct:**<br><br>```rust
let (bytes, _) = ring.write(
    fd, p_buf.as_mut_slice()
)?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
let future1 = ring.write(fd1, buf1)?;
let future2 = ring.write(fd2, buf2)?;
tokio::join!(future1, future2);
``` |
| `send(fd, buffer: Pin<&mut [u8]>)` | **(Advanced)** Low-level send on a socket using a pinned buffer slice.<br><br>⚠️ **Usage Note:** Returns a `Future` that holds a mutable borrow of the `Ring`. You **must** `.await` it immediately before other calls on the same `Ring`. | **Correct:**<br><br>```rust
let (sent, _) = ring.send(
    sock_fd, p_buf.as_mut_slice()
)?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
let f1 = ring.send(sock1, buf1)?;
let f2 = ring.send(sock2, buf2)?;
tokio::join!(f1, f2);
``` |
| `recv(fd, buffer: Pin<&mut [u8]>)` | **(Advanced)** Low-level receive on a socket using a pinned buffer slice.<br><br>⚠️ **Usage Note:** Returns a `Future` that holds a mutable borrow of the `Ring`. `.await` immediately to release the borrow before other calls. | **Correct:**<br><br>```rust
let (n, _) = ring.recv(
    sock_fd, p_buf.as_mut_slice()
)?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
let f1 = ring.recv(sock1, buf1)?;
let f2 = ring.recv(sock2, buf2)?;
tokio::join!(f1, f2);
``` |
| `get_buffer(size)` | Gets a ring-managed `OwnedBuffer` from an internal pool (or allocates one). | `let buffer = ring.get_buffer(4096)?;` |
| `orphan_count()` | Returns the number of currently tracked "orphaned" operations (cancelled futures). Useful for monitoring. | `println!("Orphans: {}", ring.orphan_count());` |

### Echo exactly the received bytes

When echoing over sockets, prefer limiting the write to the number of bytes actually received to avoid sending the buffer's entire capacity:

```rust
// After a read_owned:
let (bytes_received, buf) = ring.read_owned(client_fd, buf).await?;
// Echo exactly what was received:
let (bytes_sent, buf) = ring.write_at_owned(client_fd, buf, 0, bytes_received).await?;
```

---

## Buffer Management

### `OwnedBuffer` (✅ **REQUIRED - The Only Practical Choice**)

A buffer designed for safe ownership transfer with the kernel. **This is the only buffer type that works for real applications.**

| Method | Description | Example Usage |
| :--- | :--- | :--- |
| `OwnedBuffer::new(size)` | Creates a new zero-initialized buffer of a given size. | `let buffer = OwnedBuffer::new(4096);` |
| `OwnedBuffer::from_slice(data)` | Creates a new buffer by copying data from a slice. | `let buffer = OwnedBuffer::from_slice(b"init data");` |
| `size()` | Returns the total capacity of the buffer. | `assert_eq!(buffer.size(), 4096);` |
| `try_access()` | Returns an `Option<BufferAccessGuard>` for safe, exclusive access if the buffer is user-owned. | `if let Some(mut guard) = buffer.try_access() { guard[0] = 1; }` |
| `is_user_owned()` | Checks if the buffer is currently owned by the user (not in-flight with the kernel). | `assert!(buffer.is_user_owned());` |
| `as_slice()` (*On `BufferAccessGuard`*) | Provides read-only access to the buffer's data. | `if let Some(guard) = buffer.try_access() { println!("{:?}", guard.as_slice()); }` |
| `as_mut_slice()` (*On `BufferAccessGuard`*) | Provides mutable access to the buffer's data. | `if let Some(mut guard) = buffer.try_access() { guard.as_mut_slice().fill(0); }` |

### `PinnedBuffer<T>` (⚠️ **FUNDAMENTALLY BROKEN - Do Not Use**)

> **🚨 CRITICAL WARNING**: The PinnedBuffer API has insurmountable design flaws that make it unusable for practical applications. This API exists only for educational purposes to demonstrate why certain Rust patterns don't work with io_uring.

**The PinnedBuffer API is fundamentally broken and cannot be used in real applications due to Rust's borrow checker constraints.**

#### ❌ **The Fundamental Problem: Loops Are Impossible**

The `read_at`, `write_at`, and similar methods have this signature:
```rust
pub fn read_at<'buf>(&'ring mut self, ...) where 'buf: 'ring
```

When used with `ring: &'a mut Ring<'a>`, this creates borrowing constraints that **cannot be satisfied in loops**:

```rust
// ❌ THIS NEVER WORKS - Fundamental API Limitation
async fn impossible_example<'a>(ring: &'a mut Ring<'a>, ...) {
    while condition {
        let mut buffer = PinnedBuffer::with_capacity(1024);
        let (bytes, _) = ring.read_at(fd, buffer.as_mut_slice(), offset)?.await?;
        //                      ^^^^ First call borrows `ring` for entire 'a lifetime
        //                           This prevents ALL subsequent iterations!
        // COMPILE ERROR: cannot borrow `*ring` as mutable more than once
    }
}

// ❌ RECURSION DOESN'T FIX IT EITHER
async fn recursive_attempt<'a>(ring: &'a mut Ring<'a>, ...) {
    let bytes_written = copy_one_chunk(ring, ...).await?; // Borrows ring for 'a
    recursive_attempt(ring, ...).await?;                  // ERROR: ring still borrowed!
}

// ❌ EVEN BUFFER REUSE IS IMPOSSIBLE
async fn buffer_reuse_attempt<'a>(
    ring: &'a mut Ring<'a>, 
    buffer: &'a mut PinnedBuffer<[u8]>  // Try to reuse buffer
) {
    // First operation works
    let (bytes1, _) = ring.read_at(fd, buffer.as_mut_slice(), 0)?.await?;
    
    // Second operation FAILS - buffer is still borrowed!
    let (bytes2, _) = ring.read_at(fd, buffer.as_mut_slice(), 100)?.await?;
    // ERROR: cannot borrow `*buffer` as mutable more than once
}
```

#### 🔥 **Why This Cannot Be Fixed**

1. **The `&'ring mut self` signature** borrows the ring for the entire `'ring` lifetime
2. **With `ring: &'a mut Ring<'a>`**, each operation borrows the ring for the full `'a` lifetime  
3. **This makes multiple operations impossible** - the first operation's borrow prevents all subsequent operations
4. **No Rust pattern can work around this** - not loops, not recursion, not scoping
5. **Buffer reuse is impossible** - each operation requires new buffer allocations

#### 🚨 **Performance Reality**

- **Cannot reuse buffers** → Constant allocations for every operation
- **No loops possible** → Must allocate new buffers in every "iteration" 
- **Worse than OwnedBuffer** in every measurable way (allocations, performance, complexity)
- **The "zero-copy" promise is false** - you still must copy data out of returned slices

#### ✅ **Use OwnedBuffer Instead**

OwnedBuffer solves all these problems:

```rust
// ✅ THIS WORKS - Natural loops with buffer reuse
async fn working_example(ring: &Ring<'_>, mut buffer: OwnedBuffer) -> Result<(), Error> {
    while condition {
        // Transfer ownership to kernel, get it back when done
        let (bytes, returned_buffer) = ring.read_at_owned(fd, buffer, offset).await?;
        buffer = returned_buffer;  // Reuse the same buffer efficiently!
        
        // Can do more operations immediately
        let (bytes2, buffer2) = ring.write_at_owned(fd2, buffer, offset2, bytes).await?;
        buffer = buffer2;
    }
    Ok(())
}
```

**This complexity is why OwnedBuffer is not just recommended but REQUIRED for any real application.**

#### ❌ **PinnedBuffer Methods (Educational Only - Do Not Use)**

> **Warning**: These methods exist only to show the API design that doesn't work. Use `OwnedBuffer` instead.

| Method | Description | ⚠️ Reality Check |
| :--- | :--- | :--- |
| `PinnedBuffer::with_capacity(size)` | Creates a new zero-initialized, pinned buffer. | **Must create new buffer for every operation** - cannot reuse |
| `PinnedBuffer::from_slice(data)` | Creates a new pinned buffer by copying data from a slice. | **Defeats zero-copy goals** - copies data twice |
| `as_mut_slice()` | Returns a `Pin<&mut [u8]>` for use with `Ring` methods. | **Cannot be called twice** - borrow checker prevents reuse |
| `as_slice()` | Returns a read-only `&[u8]` slice of the buffer's contents. | **Only useful after copying data out** - not zero-copy |
| `len()` | Returns the length of the buffer. | **Only method that actually works** as expected |

---

## Batch Operations - `Batch<'ring, 'buf>`

Build a collection of operations to be submitted in a single syscall for high efficiency.

| Method | Description | Example Usage |
| :--- | :--- | :--- |
| `Batch::new()` | Creates a new, empty batch. | `let mut batch = Batch::new();` |
| `add_operation(op: Operation<Building>)` | Adds a configured `Operation` to the batch. Returns the operation's index in the batch. | ```rust
let mut p_buf = PinnedBuffer::with_capacity(1024);
let read_op = Operation::read()
    .fd(fd)
    .buffer(p_buf.as_mut_slice());
let read_idx = batch.add_operation(read_op)?;
``` |
| `add_dependency(dependent_idx, dep_idx)` | Specifies that one operation must complete before another starts. | `// Make write_idx depend on read_idx` <br> `batch.add_dependency(write_idx, read_idx)?;` |

---

## Buffer Pooling - `BufferPool`

A thread-safe pool of `PinnedBuffer`s to reduce allocation overhead in high-throughput applications.

| Method | Description | Example Usage |
| :--- | :--- | :--- |
| `BufferPool::new(capacity, buffer_size)` | Creates a new pool with a fixed number of pre-allocated buffers. | `let pool = BufferPool::new(100, 4096);` |
| `get()` | Acquires a `PooledBuffer` from the pool. The buffer is automatically returned on drop. Returns `Option`. | ```rust
if let Some(mut buffer) = pool.get() {
    // buffer is a PooledBuffer, use it
    let (bytes, _) = ring.read(
        fd, buffer.as_mut_slice()
    )?.await?;
} // buffer is returned to pool here
``` |
| `stats()` | Returns a `PoolStats` struct with current usage metrics (available, in-use, utilization, etc.). | `let stats = pool.stats();`<br>`println!("In use: {}", stats.in_use_buffers);` |

---

## Performance & Registration - `Registry<'ring>`

Advanced API for registering file descriptors and buffers with the kernel to reduce overhead on repeated operations.

| Method | Description | Example Usage |
| :--- | :--- | :--- |
| `Registry::new()` | Creates a new, empty registry. | `let mut registry = Registry::new();` |
| `register_fixed_files(fds: Vec<RawFd>)` | Registers a set of file descriptors for optimized access via a `FixedFile` handle. | `let fixed_files = registry.register_fixed_files(vec![fd1, fd2])?;` |
| `register_buffer_slots(buffers: Vec<OwnedBuffer>)` | Registers a set of owned buffers for kernel-side buffer selection. | `let bufs = vec![OwnedBuffer::new(4096)];`<br>`let slots = registry.register_buffer_slots(bufs)?;` |
| `unregister_fixed_files()` | Unregisters all fixed files. Fails if any are in use. | `registry.unregister_fixed_files()?;` |
| `unregister_buffer_slots()` | Unregisters all buffer slots, returning the `OwnedBuffer`s. Fails if any are in use. | `let buffers = registry.unregister_buffer_slots()?;` |

---

## Async Compatibility Layer

Integrate `safer-ring` with `tokio`'s `AsyncRead` and `AsyncWrite` traits. Implemented via the `AsyncCompat` trait on `Ring`.

| Method | Description | Example Usage |
| :--- | :--- | :--- |
| `ring.file(fd)` | Wraps a file descriptor in a `File` struct that implements `AsyncRead` and `AsyncWrite`. | ```rust
use tokio::io::AsyncWriteExt;
let mut file = ring.file(fd);
file.write_all(b"data").await?;
``` |
| `ring.socket(fd)` | Wraps a socket descriptor in a `Socket` struct that implements `AsyncRead` and `AsyncWrite`. | ```rust
use tokio::io::AsyncReadExt;
let mut socket = ring.socket(fd);
let mut buf = vec![0; 1024];
let n = socket.read(&mut buf).await?;
``` |
| `ring.async_read(fd)` | Creates a standalone `AsyncReadAdapter` for a file descriptor. | `let mut reader = ring.async_read(fd);` |
| `ring.async_write(fd)` | Creates a standalone `AsyncWriteAdapter` for a file descriptor. | `let mut writer = ring.async_write(fd);` |

---

## Configuration - `SaferRingConfig`

Presets for configuring `Ring` behavior for common use cases.

| Constructor | Use Case | Example Usage |
| :--- | :--- | :--- |
| `SaferRingConfig::default()` | A balanced, general-purpose configuration. | `let config = SaferRingConfig::default();` |
| `SaferRingConfig::low_latency()` | Optimized for minimal latency (e.g., real-time apps). Enables kernel polling. | `let config = SaferRingConfig::low_latency();` |
| `SaferRingConfig::high_throughput()` | Optimized for maximum throughput (e.g., file servers). Uses larger batches. | `let config = SaferRingConfig::high_throughput();` |
| `SaferRingConfig::development()` | Enables extensive logging and debug assertions for development. | `let config = SaferRingConfig::development();` |
| `SaferRingConfig::auto_detect()` | Intelligently configures based on detected system capabilities (e.g., kernel version, NUMA). | `let config = SaferRingConfig::auto_detect()?;` |

---

## Error Handling - `SaferRingError`

The unified error type for all fallible operations in the crate.

| Variant | When It Occurs |
| :--- | :--- |
| `BufferInFlight` | Attempting to access or unregister a buffer that is currently in use by an operation. |
| `OperationPending` | Attempting to get a result from an operation that has not yet completed. |
| `OperationsInFlight { count }` | `Ring` was dropped while `count` operations were still active. **This causes a panic.** |
| `NotRegistered` | An operation tried to use a registered resource (FD/buffer) that was not registered. |
| `PoolEmpty` | A request was made to a `BufferPool` that had no available buffers. |
| `Io(std::io::Error)` | An underlying I/O error occurred, either from the kernel or a system call. |