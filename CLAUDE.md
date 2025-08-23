# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Build & Test
```bash
cargo build          # Build the project
cargo test           # Run all tests including unit and compile-fail tests
cargo test --lib     # Run library tests only
cargo clippy         # Run linter for code quality
cargo fmt            # Format code
```

### Platform-Specific Testing
The project uses conditional compilation for Linux vs non-Linux platforms:
```bash
# Linux-specific features
cargo test --features unstable

# Test compile-fail safety invariants
cargo test compile_fail_tests
```

### Examples
```bash
cargo run --example echo_server
cargo run --example file_copy
```

## Project Architecture

**safer-ring** is a memory-safe Rust wrapper around Linux's io_uring that prevents common safety issues through compile-time guarantees.

```
src
├── buffer.rs
├── error.rs
├── future
│   ├── io_futures.rs
│   ├── mod.rs
│   ├── operation_future.rs
│   ├── tests.rs
│   └── waker.rs
├── lib.rs
├── operation
│   ├── building.rs
│   ├── completed.rs
│   ├── core.rs
│   ├── mod.rs
│   ├── states.rs
│   ├── submitted.rs
│   ├── tests.rs
│   ├── tracker.rs
│   └── types.rs
├── pool
│   ├── buffer_pool.rs
│   ├── mod.rs
│   ├── pooled_buffer.rs
│   ├── stats.rs
│   └── tests.rs
├── registry.rs
└── ring
    ├── completion
    │   ├── mod.rs
    │   └── result.rs
    ├── core.rs
    ├── mod.rs
    ├── submission.rs
    └── tests.rs

6 directories, 29 files
```

### Core Architecture Principles

1. **Lifetime Safety**: The `'ring` lifetime ensures operations cannot outlive the ring, and `'buf` lifetime ensures buffers outlive operations (`'buf: 'ring`)

2. **Type-State Machine**: Operations progress through compile-time enforced states:
   - `Building` → `Submitted` → `Completed<T>`
   - State transitions prevent double-submission, polling before submission, etc.

3. **Pinned Memory Management**: Buffers use `Pin<Box<T>>` to guarantee stable memory addresses required by io_uring's zero-copy semantics

### Key Components

- **`Ring<'ring>`** (src/ring.rs): Main wrapper with operation tracking and lifecycle management. Uses `RefCell<OperationTracker>` to track in-flight operations and panics on drop if operations remain.

- **`Operation<'ring, 'buf, State>`** (src/operation.rs): Type-safe state machine for I/O operations with phantom type parameters for compile-time state tracking.

- **`PinnedBuffer<T>`** (src/buffer.rs): Memory-pinned buffers with generation tracking. Supports both fixed-size arrays and dynamic slices.

- **Platform Abstraction**: Conditional compilation with `#[cfg(target_os = "linux")]` for actual io_uring integration vs stub implementations.

### Safety Mechanisms

- **Compile-fail tests** (tests/compile-fail/): Verify safety invariants like buffer lifetime constraints and operation state transitions
- **Operation tracking**: Ring tracks all in-flight operations and prevents unsafe drops
- **Pinned buffers**: Guarantee stable memory addresses during async operations

### Development Notes

- Use `trybuild` for compile-fail tests that verify safety at compile time
- The project is designed to compile on all platforms but only function on Linux 5.1+
- Buffer management uses generation counters for debugging and lifecycle tracking
- Operations are zero-cost state machines - state transitions have no runtime overhead