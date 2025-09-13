# `safer-ring` API Reference & Cheat Sheet

This document provides a quick reference to the public API of `safer-ring`. It is organized by concept to help you find what you need quickly.

## Checking io_uring Support

Before using `safer-ring`, you may want to confirm whether your kernel supports io_uring. On Linux, `safer_ring::runtime` provides helpers:

- Quick boolean check: `safer_ring::runtime::is_io_uring_available()`
- Auto-select backend and inspect it:
  - `let rt = safer_ring::runtime::Runtime::auto_detect()?;`
  - `match rt.backend() { Backend::IoUring => ..., Backend::Epoll | Backend::Stub => ... }`

Notes:
- io_uring requires Linux 5.1+ and may be disabled by security policies (containers, seccomp, AppArmor).
- On systems where io_uring is unavailable/restricted, use the epoll fallback via the runtime, or expect `Unsupported` errors from direct `Ring` construction.
- Advanced probing is available on Linux via `advanced::FeatureDetector` (version-based) or `advanced::feature_detection::DirectProbeDetector` (kernel probe).

The core safety principle of `safer-ring` is the **ownership transfer model ("hot potato" pattern)**. 

> **🎯 CRITICAL: Always use OwnedBuffer with `*_owned` methods.** The `PinnedBuffer` API is fundamentally broken and cannot be used in real applications due to insurmountable lifetime constraints that prevent loops and buffer reuse.

For maximum safety, performance, and ease of use, you **must** use the `*_owned` methods which take ownership of an `OwnedBuffer` and return it to you upon completion. The positioned variants (`read_at_owned`, `write_at_owned`) are perfect for file operations like copying and enable efficient buffer reuse.

## Table of Contents

1.  [**Check Support**](#checking-io_uring-support) - Verify io_uring availability.
2.  [**Core: `Ring`**](#ring---the-core-component) - The main entry point for all I/O.
3.  [**Buffer Management**](#buffer-management) - `OwnedBuffer` (✅ Use This) & `PinnedBuffer` (❌ Broken - Educational Only).
4.  [**Batch Operations**](#batch-operations---batchring-buf) - Submitting multiple operations at once.
5.  [**Buffer Pooling**](#buffer-pooling---bufferpool) - Efficient buffer reuse.
6.  [**Performance & Registration**](#performance--registration---registryring) - Advanced performance tuning.
7.  [**Async Compatibility Layer**](#async-compatibility-layer) - For `tokio` integration.
8.  [**Configuration**](#configuration---saferringconfig) - Customizing `Ring` behavior.
9.  [**Error Handling**](#error-handling---saferringerror) - Understanding failure modes.

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

| Method | Description | Example |
| :--- | :--- | :--- |
| `Ring::new(entries: u32)` | Creates a new `Ring` with a specified submission queue depth. | `let mut ring = Ring::new(128)?;` |
| `Ring::with_config(config: SaferRingConfig)` | Creates a new `Ring` with detailed custom configuration. | `let cfg = SaferRingConfig::low_latency(); let mut ring = Ring::with_config(cfg)?;` |
| `read_owned(fd, buffer: OwnedBuffer)` | Safely reads from a file descriptor, returning the buffer after completion. | `let (n, buf) = ring.read_owned(fd, buf).await?;` |
| `write_owned(fd, buffer: OwnedBuffer)` | Safely writes to a file descriptor; returns the buffer. | `let (n, buf) = ring.write_owned(fd, buf).await?;` |
| `read_at_owned(fd, buffer: OwnedBuffer, offset: u64)` | Safe positioned read with buffer ownership round‑trip. | `let (n, buf) = ring.read_at_owned(fd, buf, 100).await?;` |
| `write_at_owned(fd, buffer: OwnedBuffer, offset: u64, len: usize)` | Safe positioned write; specify exact length. | `let (n, buf) = ring.write_at_owned(fd, buf, 100, len).await?;` |
| `accept_safe(listening_fd)` | Safely accepts a new connection, returning client FD. | `let client = ring.accept_safe(listen_fd).await?;` |
| `submit_batch_standalone(batch)` | Submit a batch returning a future that does not borrow the ring; poll via `poll_with_ring`. | `let mut fut = ring.submit_batch_standalone(batch)?;` |
| `read(fd, Pin<&mut [u8]>)` | Advanced API. Returns a future that borrows the ring; `.await` immediately (use `?.await?`). | `let (n, _) = ring.read(fd, p.as_mut_slice())?.await?;` |
| `write(fd, Pin<&mut [u8]>)` | Advanced API; must `.await` immediately (cannot hold multiple). | `let (n, _) = ring.write(fd, p.as_mut_slice())?.await?;` |
| `send(fd, Pin<&mut [u8]>)` | Advanced socket send; `.await` immediately. | `let (n, _) = ring.send(fd, p.as_mut_slice())?.await?;` |
| `recv(fd, Pin<&mut [u8]>)` | Advanced socket recv; `.await` immediately. | `let (n, _) = ring.recv(fd, p.as_mut_slice())?.await?;` |
| `get_buffer(size)` | Acquire ring-managed `OwnedBuffer` from internal pool. | `let buf = ring.get_buffer(4096)?;` |
| `orphan_count()` | Number of tracked orphaned operations. | `println!("{}", ring.orphan_count());` |

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
| `add_operation(op: Operation<Building>)` | Adds an operation; returns its index. | `let idx = batch.add_operation(op)?;` |
| `add_dependency(dependent_idx, dep_idx)` | Ensure one operation completes before another. | `batch.add_dependency(write_idx, read_idx)?;` |

---

## Buffer Pooling - `BufferPool`

A thread-safe pool of `PinnedBuffer`s to reduce allocation overhead in high-throughput applications.

| Method | Description | Example Usage |
| :--- | :--- | :--- |
| `BufferPool::new(capacity, buffer_size)` | Creates a new pool with pre-allocated buffers. | `let pool = BufferPool::new(100, 4096);` |
| `get()` | Acquire a `PooledBuffer` (returned to pool on drop). | `if let Some(mut b) = pool.get() { /* use b */ }` |
| `stats()` | Pool usage metrics. | `let s = pool.stats(); println!("{}", s.in_use_buffers);` |

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
| `ring.file(fd)` | Wrap a file descriptor as `AsyncRead`/`AsyncWrite`. | `let mut f = ring.file(fd);` |
| `ring.socket(fd)` | Wrap a socket as `AsyncRead`/`AsyncWrite`. | `let mut s = ring.socket(fd);` |
| `ring.async_read(fd)` | Standalone `AsyncReadAdapter`. | `let mut r = ring.async_read(fd);` |
| `ring.async_write(fd)` | Standalone `AsyncWriteAdapter`. | `let mut w = ring.async_write(fd);` |

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
