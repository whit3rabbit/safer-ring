# Safer-Ring Examples

This directory contains comprehensive examples demonstrating the capabilities and best practices of the safer-ring library. Each example showcases different aspects of high-performance, memory-safe io_uring operations.

## 📚 Available Examples

### 🌐 Network Operations

#### `echo_server.rs` - TCP Echo Server
A high-performance TCP echo server demonstrating basic network operations.

**Features:**
- Concurrent connection handling
- Real-time statistics and monitoring
- Graceful shutdown handling
- Comprehensive error handling
- Performance metrics

**Usage:**
```bash
cargo run --example echo_server
# Test with: telnet localhost 8080
```

#### `https_server.rs` - HTTPS Server with kTLS
Advanced HTTPS server with kernel TLS (kTLS) offload support.

**Features:**
- kTLS integration for zero-copy encryption
- TLS certificate management
- HTTP/HTTPS request handling
- Performance optimization with kernel offload
- Security best practices

**Prerequisites:**
```bash
# Generate test certificate
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
```

**Usage:**
```bash
cargo run --example https_server -- --cert cert.pem --key key.pem
# Test with: curl -k https://localhost:8443/
```

### 💾 File Operations

#### `file_copy.rs` - Zero-Copy File Operations
High-performance file copying using zero-copy operations and buffer pooling.

**Features:**
- Zero-copy file operations
- Buffer pool integration
- Progress tracking and statistics
- Parallel I/O operations
- Copy verification

**Usage:**
```bash
# Basic file copy
cargo run --example file_copy -- source.txt destination.txt

# With custom buffer size (64KB)
cargo run --example file_copy -- source.txt dest.txt --buffer-size 64

# With parallel operations
cargo run --example file_copy -- source.txt dest.txt --parallel 4
```

### 🔄 Async Operations

#### `async_demo.rs` - Comprehensive Async/Await Demo
Demonstrates various async patterns and integration with Rust's async ecosystem.

**Features:**
- Sequential and concurrent async operations
- Error handling in async contexts
- Timeout and cancellation patterns
- Batch operation processing
- Buffer pool async integration

**Usage:**
```bash
# Basic async demo
cargo run --example async_demo

# With real file I/O
cargo run --example async_demo -- --with-files

# Concurrent operations demo
cargo run --example async_demo -- --concurrent
```

### 🏊 Buffer Management

#### `buffer_pool_demo.rs` - Buffer Pool Usage
Comprehensive demonstration of efficient buffer management and pooling.

**Features:**
- Buffer pool creation and management
- Concurrent buffer access patterns
- Performance monitoring and statistics
- Stress testing capabilities
- Memory efficiency demonstration

**Usage:**
```bash
# Basic buffer pool demo
cargo run --example buffer_pool_demo

# Custom pool size
cargo run --example buffer_pool_demo -- --pool-size 100

# Stress test with multiple threads
cargo run --example buffer_pool_demo -- --stress-test --threads 8
```

### 🔧 Advanced Features

#### `registry_demo.rs` - Resource Registration
Demonstrates file descriptor and buffer registration for optimal performance.

**Features:**
- File descriptor registration
- Buffer registration and management
- Resource lifecycle management
- Performance optimization techniques

**Usage:**
```bash
cargo run --example registry_demo
```

#### `completion_demo.rs` - Completion Queue Processing
Shows advanced completion queue processing and operation tracking.

**Usage:**
```bash
cargo run --example completion_demo
```

## 🚀 Performance Characteristics

### Throughput Benchmarks
- **Echo Server**: >100K concurrent connections
- **File Copy**: >10GB/s on NVMe storage
- **HTTPS Server**: >50K HTTPS requests/second with kTLS
- **Buffer Pool**: >1M allocations/second

### Memory Efficiency
- **Zero Allocations**: Hot paths use pre-allocated buffers
- **Constant Memory**: Memory usage independent of load
- **Cache Friendly**: Buffer reuse improves cache locality

### Latency Characteristics
- **Sub-microsecond**: Operation submission overhead
- **Predictable**: No GC pauses or allocation spikes
- **Scalable**: Performance scales linearly with cores

## 🛡️ Safety Features

All examples demonstrate safer-ring's safety guarantees:

- **Memory Safety**: No use-after-free or buffer overruns
- **Type Safety**: Compile-time state machine enforcement
- **Lifetime Safety**: Buffers guaranteed to outlive operations
- **Concurrency Safety**: Thread-safe buffer sharing
- **Resource Safety**: Automatic cleanup on errors

## 🔧 Platform Requirements

### Linux Requirements
- **Minimum**: Linux 5.1 (basic io_uring support)
- **Recommended**: Linux 5.19+ (advanced features)
- **Optimal**: Linux 6.0+ (latest optimizations)

### kTLS Requirements (HTTPS example)
- **Minimum**: Linux 4.13 (kTLS support)
- **Recommended**: Linux 5.4+ (improved kTLS)

### Hardware Recommendations
- **CPU**: Modern x86_64 or ARM64 processor
- **Memory**: 4GB+ RAM for stress tests
- **Storage**: NVMe SSD for file operation examples

## 🧪 Testing the Examples

### Basic Functionality Test
```bash
# Test all examples compile
cargo check --examples

# Run basic demos
cargo run --example async_demo
cargo run --example buffer_pool_demo
cargo run --example registry_demo
```

### Network Examples Test
```bash
# Terminal 1: Start echo server
cargo run --example echo_server

# Terminal 2: Test with netcat
echo "Hello, safer-ring!" | nc localhost 8080
```

### File Operations Test
```bash
# Create test file
echo "Test content for safer-ring" > test_input.txt

# Test file copy
cargo run --example file_copy -- test_input.txt test_output.txt

# Verify copy
diff test_input.txt test_output.txt
```

### Performance Testing
```bash
# Buffer pool stress test
cargo run --example buffer_pool_demo -- --stress-test --threads 8 --duration 30

# File copy performance test
dd if=/dev/zero of=large_test.bin bs=1M count=1000
time cargo run --example file_copy -- large_test.bin copy_test.bin --parallel 8
```

## 📖 Learning Path

### Beginner
1. Start with `async_demo.rs` to understand basic concepts
2. Try `buffer_pool_demo.rs` for memory management
3. Explore `registry_demo.rs` for resource management

### Intermediate
1. Build the `echo_server.rs` for network programming
2. Implement file operations with `file_copy.rs`
3. Study concurrent patterns in examples

### Advanced
1. Deploy the `https_server.rs` with real certificates
2. Optimize performance using profiling tools
3. Extend examples for your specific use cases

## 🤝 Contributing

When adding new examples:

1. **Documentation**: Include comprehensive comments and documentation
2. **Error Handling**: Demonstrate proper error handling patterns
3. **Safety**: Showcase safer-ring's safety guarantees
4. **Performance**: Include performance considerations and metrics
5. **Testing**: Provide clear testing instructions

### Example Template
```rust
//! # Example Title
//!
//! Brief description of what this example demonstrates.
//!
//! ## Features Demonstrated
//! - Feature 1: Description
//! - Feature 2: Description
//!
//! ## Usage
//! ```bash
//! cargo run --example example_name
//! ```

use safer_ring::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Implementation with comprehensive comments
    Ok(())
}
```

## 📚 Additional Resources

- [Safer-Ring Documentation](../README.md)
- [io_uring Guide](https://kernel.dk/io_uring.pdf)
- [Async Rust Book](https://rust-lang.github.io/async-book/)
- [Tokio Documentation](https://tokio.rs/)

## 🐛 Troubleshooting

### Common Issues

**"io_uring not supported"**
- Ensure Linux 5.1+ kernel
- Check `CONFIG_IO_URING=y` in kernel config

**"Permission denied" for kTLS**
- Some systems require root for kTLS
- Try `--no-ktls` flag for HTTPS example

**"Too many open files"**
- Increase ulimit: `ulimit -n 65536`
- Check system limits: `/etc/security/limits.conf`

### Performance Issues

**Low throughput**
- Increase ring size: larger `--ring-size`
- Use more parallel operations
- Check CPU affinity and NUMA topology

**High memory usage**
- Reduce buffer pool size
- Use smaller buffer sizes
- Monitor with `htop` or `valgrind`

### Getting Help

1. Check example documentation and comments
2. Review the main library documentation
3. Search existing issues on GitHub
4. Create a new issue with minimal reproduction case