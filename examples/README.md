# safer-ring Examples

This directory contains comprehensive examples demonstrating the capabilities of **safer-ring**, a memory-safe Rust wrapper around Linux's io_uring that provides compile-time safety guarantees for high-performance async I/O operations.

## 📋 Prerequisites

- **Linux 5.1+** with io_uring support (most examples)
- **Rust 1.70+** with async/await support
- **Tokio runtime** for async examples
- Root or appropriate permissions for some network examples

## 🚀 Quick Start

```bash
# Run the comprehensive demo to see all features
cargo run --example safer_ring_demo

# Try the async demo with recommended patterns
cargo run --example async_demo

# Start a high-performance echo server
cargo run --example echo_server_main
```

## 📚 Examples by Category

### 🎓 Getting Started

Perfect for newcomers to safer-ring and io_uring:

#### [`safer_ring_demo.rs`](./safer_ring_demo.rs)
**Comprehensive overview of all safer-ring features**
- Runtime detection and automatic fallback
- Hot potato ownership transfer pattern
- Cancellation safety with orphan tracking
- Performance guidance and environment analysis
- **Start here** if you're new to safer-ring

#### [`registry_demo.rs`](./registry_demo.rs)
**File descriptor and buffer registration**
- Registry creation and resource management
- File descriptor registration and slot reuse
- Buffer registration with various sizes
- Resource validation and cleanup patterns

### ⚡ Core Features

Demonstrates essential safer-ring patterns and APIs:

#### [`async_demo.rs`](./async_demo.rs) ⭐ **RECOMMENDED**
**Modern async/await integration with best practices**
- **OwnedBuffer and `*_owned` methods** (recommended pattern)
- Hot potato ownership transfer for optimal performance
- Sequential and concurrent async operations
- Proper error handling and cancellation
- Timeout handling and resource cleanup
- **Use this pattern in your applications**

#### [`completion_demo.rs`](./completion_demo.rs) ⭐ **RECOMMENDED**
**Modern completion handling patterns**
- Async completion handling with futures
- Batch operation processing
- Multiple concurrent operations
- Error propagation and resource management
- **Modern alternative to low-level polling**

### 🏗️ Advanced Features

For experienced users exploring advanced capabilities:

#### [`advanced_features_demo.rs`](./advanced_features_demo.rs)
**Cutting-edge io_uring features**
- Kernel feature detection and graceful degradation
- Advanced configuration options
- Multi-shot operations and buffer selection
- Comprehensive logging and metrics
- Performance optimization techniques

#### [`buffer_pool_demo.rs`](./buffer_pool_demo.rs)
**High-performance buffer management**
- Buffer pool creation and configuration
- Zero-allocation buffer operations
- Thread-safe concurrent access
- Pool statistics and monitoring
- Memory efficiency optimization

#### [`performance_demo.rs`](./performance_demo.rs)
**Performance monitoring and optimization**
- NUMA-aware memory allocation
- Performance counter integration
- Memory usage tracking
- Batch operation optimization
- Comprehensive performance analysis

### 🌐 Real-World Applications

Production-ready examples for common use cases:

#### [`echo_server_main.rs`](./echo_server_main.rs)
**High-performance TCP echo server**
- Production-ready network server architecture
- Connection handling and multiplexing
- Statistics tracking and monitoring
- Graceful shutdown and error recovery
- Modular design with [`echo_server/`](./echo_server/) components

#### [`file_copy.rs`](./file_copy.rs)
**Zero-copy file operations**
- High-throughput file copying (>10GB/s on NVMe)
- Batch processing for maximum performance
- Progress tracking and error recovery
- Optimal buffer sizing and parallelism
- Atomic operations and data integrity

#### [`https_server_simple.rs`](./https_server_simple.rs)
**TLS/HTTPS server placeholder**
- Currently a work-in-progress placeholder
- Will demonstrate TLS integration with io_uring
- Advanced security and performance features

## 🎯 API Recommendations

### ✅ Recommended: OwnedBuffer Pattern

**Always prefer** `OwnedBuffer` and `*_owned` methods in application code:

```rust
// ✅ RECOMMENDED - Works in loops and concurrent scenarios
let buffer = OwnedBuffer::new(vec![0u8; 4096]);
let result = ring.read_owned(fd, buffer, 0).await?;
```

### ❌ Not Recommended: PinnedBuffer Pattern

**Avoid** `PinnedBuffer` and methods like `ring.read()` in application code:

```rust
// ❌ NOT RECOMMENDED - Lifetime issues in complex scenarios
let buffer = PinnedBuffer::new([0u8; 4096]);
let operation = ring.read(fd, &buffer, 0)?; // Lifetime constraints
```

**Why OwnedBuffer?**
- **Works everywhere**: No lifetime constraints in loops or async contexts
- **Better ergonomics**: Seamless integration with Rust's async ecosystem
- **Memory safety**: Automatic ownership transfer prevents use-after-free
- **Performance**: Optimized hot potato pattern for minimal overhead

## 🏃 Running Examples

### Basic Usage
```bash
# Run any example
cargo run --example <example_name>

# Run with arguments (where supported)
cargo run --example file_copy -- source.txt destination.txt
cargo run --example buffer_pool_demo -- --pool-size 100
```

### With Features
```bash
# Enable unstable features (Linux-specific)
cargo run --example async_demo --features unstable

# Run tests including compile-fail safety tests
cargo test compile_fail_tests
```

### Development Commands
```bash
# Build all examples
cargo build --examples

# Check examples for errors
cargo check --examples

# Format code
cargo fmt

# Lint code
cargo clippy --examples
```

## 🎛️ Environment Variables

Some examples support configuration through environment variables:

- `RUST_LOG=debug` - Enable detailed logging
- `SAFER_RING_BACKEND=epoll` - Force epoll backend (testing)
- `SAFER_RING_RING_SIZE=256` - Set ring size

## 🔧 Platform Support

- **Linux 5.1+**: Full io_uring support with all features
- **Other platforms**: Automatic fallback to epoll/select (reduced performance)
- **Compile-time detection**: Code compiles everywhere, runtime detection handles backend selection

## 📖 Learning Path

1. **Start with**: [`safer_ring_demo.rs`](./safer_ring_demo.rs) - Overview of all features
2. **Learn async patterns**: [`async_demo.rs`](./async_demo.rs) - Modern recommended API
3. **Understand completions**: [`completion_demo.rs`](./completion_demo.rs) - Batch processing
4. **Try real applications**: [`echo_server_main.rs`](./echo_server_main.rs) or [`file_copy.rs`](./file_copy.rs)
5. **Optimize performance**: [`buffer_pool_demo.rs`](./buffer_pool_demo.rs) and [`performance_demo.rs`](./performance_demo.rs)
6. **Explore advanced features**: [`advanced_features_demo.rs`](./advanced_features_demo.rs)

## 🤝 Contributing

When adding new examples:

1. **Follow the pattern**: Include comprehensive documentation comments
2. **Use recommended APIs**: Prefer `OwnedBuffer` and async patterns
3. **Add usage instructions**: Command-line help and argument parsing
4. **Include error handling**: Demonstrate proper error patterns
5. **Update this README**: Add your example to the appropriate category

## 📚 Additional Resources

- **API Documentation**: [`../docs/API.md`](../docs/API.md)
- **Project Architecture**: [`../CLAUDE.md`](../CLAUDE.md)
- **Safety Documentation**: [`../src/safety.rs`](../src/safety.rs)
- **Performance Guide**: [`../src/perf.rs`](../src/perf.rs)

---

**Note**: safer-ring prioritizes memory safety and ergonomics over raw performance. While it achieves excellent performance, the primary goal is providing a safe, easy-to-use interface to io_uring's powerful capabilities.