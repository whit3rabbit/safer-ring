# `safer-ring` API Reference & Cheat Sheet

This document provides a quick reference to the public API of `safer-ring`. It is organized by concept to help you find what you need quickly.

The core safety principle of `safer-ring` is the **ownership transfer model ("hot potato" pattern)**. For maximum safety and ease of use, you should prefer the `*_owned` methods which take ownership of an `OwnedBuffer` and return it to you upon completion.

## Table of Contents

1.  [**Core: `Ring`**](#ring---the-core-component) - The main entry point for all I/O.
2.  [**Buffer Management**](#buffer-management) - `OwnedBuffer` (Recommended) & `PinnedBuffer` (Advanced).
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

Most I/O methods on `Ring` (e.g., `read`, `write`, `send`, `recv`, `accept_safe`) take `&mut self` and return a `Future` that holds the mutable borrow of the `Ring` until it is awaited. This avoids internal locks for maximum performance but means:

- You must await each operation before starting another on the same `Ring` instance (the "await-immediately" pattern).
- For concurrency on the same `Ring`, either use `submit_batch_standalone` (see below) or spawn multiple tasks with appropriate ownership.

> The `?.await?` pattern
>
> - The first `?` handles submission errors: `ring.method(...) -> Result<Future, Error>`.
> - `.await` waits for the I/O to complete.
> - The second `?` handles completion errors: `Future::Output -> Result<Success, Error>`.

| Method                                                  | Description                                                                                                                              | Example Usage                                                                                                  |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `Ring::new(entries: u32)`                               | Creates a new `Ring` with a specified submission queue depth. Automatically detects the best backend.                                    | `let mut ring = Ring::new(128)?;`                                                                               |
| `Ring::with_config(config: SaferRingConfig)`            | Creates a new `Ring` with detailed custom configuration.                                                                                 | `let cfg = SaferRingConfig::low_latency();`<br>`let mut ring = Ring::with_config(cfg)?;`                        |
| `read_owned(fd, buffer: OwnedBuffer)`                   | **(Recommended)** Safely reads from a file descriptor. Takes ownership of the buffer and returns it upon completion.                       | `let (bytes, buf) = ring.read_owned(fd, buf).await?;`                                                           |
| `write_owned(fd, buffer: OwnedBuffer)`                  | **(Recommended)** Safely writes to a file descriptor. Takes ownership of the buffer and returns it upon completion.                        | `let (bytes, buf) = ring.write_owned(fd, buf).await?;`                                                          |
| `accept_safe(listening_fd)`                             | Safely accepts a new network connection. Returns a future that resolves to the new client file descriptor.                                 | `let client_fd = ring.accept_safe(listen_fd).await?;`                                                          |
| `submit_batch_standalone(batch)`                        | **(Recommended for composition)** Submits a batch and returns a `StandaloneBatchFuture` that **does not** hold a borrow of the ring. Useful when composing batches with other operations on the same `Ring`. The future must be polled with `poll_with_ring`. | `let mut future = ring.submit_batch_standalone(batch)?;`<br>`// You can still use \`ring\` here for other ops`<br>`let res = poll_fn(\|cx\| future.poll_with_ring(&mut ring, cx)).await?;` |
| `read(fd, buffer: Pin<&mut [u8]>)`                      | **(Advanced)** Low-level read using a pinned buffer slice.<br><br>⚠️ <strong>Usage Note:</strong> Returns a `Future` that holds a mutable borrow of the `Ring`. You <strong>must</strong> `.await` it immediately before making other calls on the same `Ring` to release the borrow. The `?.await?` pattern is recommended for robust error handling. | <strong>Correct:</strong><br><br>```rust
let (bytes, _) = ring.read(fd, p_buf.as_mut_slice())?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
// ERROR: `ring` is borrowed mutably by `future1`
let future1 = ring.read(fd1, buf1)?;
let future2 = ring.read(fd2, buf2)?; // borrow still held
tokio::join!(future1, future2);
``` |
| `write(fd, buffer: Pin<&mut [u8]>)`                     | **(Advanced)** Low-level write using a pinned buffer slice.<br><br>⚠️ <strong>Usage Note:</strong> Returns a `Future` that holds a mutable borrow of the `Ring`. You <strong>must</strong> `.await` it immediately before making other calls on the same `Ring`. Use `?.await?` for robust error handling. | <strong>Correct:</strong><br><br>```rust
let (bytes, _) = ring.write(fd, p_buf.as_mut_slice())?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
let future1 = ring.write(fd1, buf1)?;
let future2 = ring.write(fd2, buf2)?; // `ring` still borrowed
tokio::join!(future1, future2);
``` |
| `send(fd, buffer: Pin<&mut [u8]>)`                      | **(Advanced)** Low-level send on a socket using a pinned buffer slice.<br><br>⚠️ <strong>Usage Note:</strong> Returns a `Future` that holds a mutable borrow of the `Ring`. You <strong>must</strong> `.await` it immediately before other calls on the same `Ring`. | <strong>Correct:</strong><br><br>```rust
let (sent, _) = ring.send(sock_fd, p_buf.as_mut_slice())?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
let f1 = ring.send(sock1, buf1)?;
let f2 = ring.send(sock2, buf2)?; // borrow not released
tokio::join!(f1, f2);
``` |
| `recv(fd, buffer: Pin<&mut [u8]>)`                      | **(Advanced)** Low-level receive on a socket using a pinned buffer slice.<br><br>⚠️ <strong>Usage Note:</strong> Returns a `Future` that holds a mutable borrow of the `Ring`. `.await` immediately to release the borrow before other calls. | <strong>Correct:</strong><br><br>```rust
let (n, _) = ring.recv(sock_fd, p_buf.as_mut_slice())?.await?;
```<br><br><strong>Incorrect (will not compile):</strong><br><br>```rust
let f1 = ring.recv(sock1, buf1)?;
let f2 = ring.recv(sock2, buf2)?; // borrow not released
tokio::join!(f1, f2);
``` |
| `get_buffer(size)`                                      | Gets a ring-managed `OwnedBuffer` from an internal pool (or allocates one).                                                              | `let buffer = ring.get_buffer(4096)?;`                                                                          |
| `orphan_count()`                                        | Returns the number of currently tracked "orphaned" operations (cancelled futures). Useful for monitoring.                                | `println!("Orphans: {}", ring.orphan_count());`                                                                |

---

## Buffer Management

### `OwnedBuffer` (Recommended & Safest)

A buffer designed for safe ownership transfer with the kernel.

| Method                             | Description                                                                                          | Example Usage                                                 |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `OwnedBuffer::new(size)`           | Creates a new zero-initialized buffer of a given size.                                               | `let buffer = OwnedBuffer::new(4096);`                         |
| `OwnedBuffer::from_slice(data)`    | Creates a new buffer by copying data from a slice.                                                   | `let buffer = OwnedBuffer::from_slice(b"init data");`          |
| `size()`                           | Returns the total capacity of the buffer.                                                            | `assert_eq!(buffer.size(), 4096);`                            |
| `try_access()`                     | Returns an `Option<BufferAccessGuard>` for safe, exclusive access if the buffer is user-owned.       | `if let Some(mut guard) = buffer.try_access() { guard[0] = 1; }` |
| `is_user_owned()`                  | Checks if the buffer is currently owned by the user (not in-flight with the kernel).                 | `assert!(buffer.is_user_owned());`                             |
| `as_slice()`                       | *(On `BufferAccessGuard`)* Provides read-only access to the buffer's data.                            | `println!("{:?}", guard.as_slice());`                         |
| `as_mut_slice()`                   | *(On `BufferAccessGuard`)* Provides mutable access to the buffer's data.                              | `guard.as_mut_slice().fill(0);`                               |

### `PinnedBuffer<T>` (Advanced)

A buffer that is pinned in memory, required for the low-level, zero-copy API.

| Method                             | Description                                                                        | Example Usage                                                                          |
| ---------------------------------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `PinnedBuffer::with_capacity(size)`| Creates a new zero-initialized, pinned buffer.                                     | `let mut p_buf = PinnedBuffer::with_capacity(1024);`                                    |
| `PinnedBuffer::from_slice(data)`   | Creates a new pinned buffer by copying data from a slice.                          | `let mut p_buf = PinnedBuffer::from_slice(b"data");`                                    |
| `as_mut_slice()`                   | Returns a `Pin<&mut [u8]>`, which is required by the advanced `Ring` methods.      | `let pinned_slice = p_buf.as_mut_slice();`                                              |
| `as_slice()`                       | Returns a read-only `&[u8]` slice of the buffer's contents.                        | `println!("{:?}", p_buf.as_slice());`                                                  |
| `len()`                            | Returns the length of the buffer.                                                  | `assert_eq!(p_buf.len(), 1024);`                                                        |

---

## Batch Operations - `Batch<'ring, 'buf>`

Build a collection of operations to be submitted in a single syscall for high efficiency.

| Method                                      | Description                                                                           | Example Usage                                                                                                                                                                                                   |
| ------------------------------------------- | ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Batch::new()`                              | Creates a new, empty batch.                                                           | `let mut batch = Batch::new();`                                                                                                                                                                                 |
| `add_operation(op: Operation<Building>)`    | Adds a configured `Operation` to the batch. Returns the operation's index in the batch. | `let read_op = Operation::read().fd(fd).buffer(buf.as_mut_slice());`<br>`let read_idx = batch.add_operation(read_op)?;`                                                                                           |
| `add_dependency(dependent_idx, dep_idx)`    | Specifies that one operation must complete before another starts.                       | `batch.add_dependency(write_idx, read_idx)?;`                                                                                                                                                                    |

---

## Buffer Pooling - `BufferPool`

A thread-safe pool of `PinnedBuffer`s to reduce allocation overhead in high-throughput applications.

| Method                                    | Description                                                                                              | Example Usage                                                                |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `BufferPool::new(capacity, buffer_size)`  | Creates a new pool with a fixed number of pre-allocated buffers.                                         | `let pool = BufferPool::new(100, 4096)?;`                                     |
| `get()`                                   | Acquires a `PooledBuffer` from the pool. The buffer is automatically returned on drop. Returns `Option`. | `if let Some(mut buffer) = pool.get() { /* use buffer */ }`                    |
| `stats()`                                 | Returns a `PoolStats` struct with current usage metrics (available, in-use, utilization, etc.).        | `let stats = pool.stats();`<br>`println!("In use: {}", stats.in_use_buffers);` |

---

## Performance & Registration - `Registry<'ring>`

Advanced API for registering file descriptors and buffers with the kernel to reduce overhead on repeated operations.

| Method                                          | Description                                                                     | Example Usage                                                                                                                                                                   |
| ----------------------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Registry::new()`                               | Creates a new, empty registry.                                                  | `let mut registry = Registry::new();`                                                                                                                                           |
| `register_fixed_files(fds: Vec<RawFd>)`         | Registers a set of file descriptors for optimized access via a `FixedFile` handle.  | `let fixed_files = registry.register_fixed_files(vec![fd1, fd2])?;`                                                                                                             |
| `register_buffer_slots(buffers: Vec<OwnedBuffer>)` | Registers a set of owned buffers for kernel-side buffer selection.              | `let slots = registry.register_buffer_slots(vec![buf1, buf2])?;`                                                                                                                |
| `unregister_fixed_files()`                      | Unregisters all fixed files. Fails if any are in use.                           | `registry.unregister_fixed_files()?;`                                                                                                                                           |
| `unregister_buffer_slots()`                     | Unregisters all buffer slots, returning the `OwnedBuffer`s. Fails if any are in use. | `let buffers = registry.unregister_buffer_slots()?;`                                                                                                                            |

---

## Async Compatibility Layer

Integrate `safer-ring` with `tokio`'s `AsyncRead` and `AsyncWrite` traits. Implemented via the `AsyncCompat` trait on `Ring`.

| Method                  | Description                                                                                                   | Example Usage                                                                                                        |
| ----------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `ring.file(fd)`         | Wraps a file descriptor in a `File` struct that implements `AsyncRead` and `AsyncWrite`.                        | `let mut file = ring.file(fd);`<br>`file.write_all(b"data").await?;`                                                   |
| `ring.socket(fd)`       | Wraps a socket descriptor in a `Socket` struct that implements `AsyncRead` and `AsyncWrite`.                    | `let mut socket = ring.socket(fd);`<br>`let n = socket.read(&mut buf).await?;`                                        |
| `ring.async_read(fd)`   | Creates a standalone `AsyncReadAdapter` for a file descriptor.                                                  | `let mut reader = ring.async_read(fd);`                                                                               |
| `ring.async_write(fd)`  | Creates a standalone `AsyncWriteAdapter` for a file descriptor.                                                 | `let mut writer = ring.async_write(fd);`                                                                              |

---

## Configuration - `SaferRingConfig`

Presets for configuring `Ring` behavior for common use cases.

| Constructor                       | Use Case                                                                    | Example Usage                                                    |
| --------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `SaferRingConfig::default()`      | A balanced, general-purpose configuration.                                  | `let config = SaferRingConfig::default();`                        |
| `SaferRingConfig::low_latency()`  | Optimized for minimal latency (e.g., real-time apps). Enables kernel polling. | `let config = SaferRingConfig::low_latency();`                    |
| `SaferRingConfig::high_throughput()`| Optimized for maximum throughput (e.g., file servers). Uses larger batches. | `let config = SaferRingConfig::high_throughput();`                |
| `SaferRingConfig::development()`  | Enables extensive logging and debug assertions for development.             | `let config = SaferRingConfig::development();`                    |
| `SaferRingConfig::auto_detect()`  | Intelligently configures based on detected system capabilities (e.g., kernel version, NUMA). | `let config = SaferRingConfig::auto_detect()?;`                  |

---

## Error Handling - `SaferRingError`

The unified error type for all fallible operations in the crate.

| Variant                                | When It Occurs                                                                            |
| -------------------------------------- | ----------------------------------------------------------------------------------------- |
| `BufferInFlight`                       | Attempting to access or unregister a buffer that is currently in use by an operation.     |
| `OperationPending`                     | Attempting to get a result from an operation that has not yet completed.                  |
| `OperationsInFlight { count }`         | `Ring` was dropped while `count` operations were still active. **This causes a panic.**   |
| `NotRegistered`                        | An operation tried to use a registered resource (FD/buffer) that was not registered.      |
| `PoolEmpty`                            | A request was made to a `BufferPool` that had no available buffers.                       |
| `Io(std::io::Error)`                   | An underlying I/O error occurred, either from the kernel or a system call.                |