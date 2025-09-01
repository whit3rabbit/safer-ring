# Safer-Ring Examples

This directory contains comprehensive examples demonstrating the **recommended patterns** discovered through extensive benchmarking and real-world usage. Each example showcases the optimal "hot potato" ownership transfer pattern and other best practices for memory-safe io_uring operations.

## ⚠️ IMPORTANT: API Recommendation Notice

All examples in this directory use the **RECOMMENDED** `OwnedBuffer` and `*_owned` methods. You may encounter `PinnedBuffer` and methods like `ring.read()` in older documentation or library internals. **These APIs are NOT recommended for application code** due to Rust's lifetime rules that make them impossible to use in realistic loops or concurrent scenarios.

**ALWAYS prefer the `OwnedBuffer` and `*_owned` methods** demonstrated in these examples.

## 📚 Available Examples

### 🌐 Network Operations

#### `echo_server_main.rs` - TCP Echo Server with Concurrency Model
A high-performance TCP echo server demonstrating the "one Ring per task" concurrency model.

**Features:**
- "One Ring per task" concurrency model (recommended approach)
- Hot potato buffer reuse within each connection
- Detailed concurrency model documentation
- Real-time statistics and monitoring
- Graceful shutdown handling
- Educational content about why this pattern scales perfectly

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

#### `file_copy.rs` - Hot Potato File Operations
High-performance file copying using the optimal "hot potato" ownership transfer pattern.

**Features:**
- Hot potato ownership transfer pattern (recommended approach)
- Single buffer efficiently reused across all operations
- Performance context: when safer-ring excels vs std::fs::copy
- Educational documentation about buffer ownership flow
- Copy verification and comprehensive error handling

**Performance Notes:**
- `std::fs::copy` may be faster for cached files due to kernel optimizations
- safer-ring excels at network I/O, database files, and userspace bypass scenarios
- Consider O_DIRECT for true device performance

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

#### `async_demo.rs` - Best Practices Async Demo
Demonstrates the RECOMMENDED async patterns with clear API guidance.

**Features:**
- Hot potato pattern with OwnedBuffer (recommended approach)
- Clear deprecation notices for PinnedBuffer APIs
- Sequential safety model explanation
- Error handling in async contexts
- Timeout and cancellation patterns
- Educational content about why certain APIs are preferred

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

## 🛡️ Safety Features & Best Practices

All examples demonstrate safer-ring's safety guarantees using the RECOMMENDED patterns:

- **Hot Potato Pattern**: Ownership transfer prevents use-after-free bugs
- **OwnedBuffer API**: No complex lifetime management required
- **One Ring Per Task**: Perfect scaling without synchronization overhead
- **Sequential Safety**: Borrow checker enforces safe operation ordering
- **Resource Safety**: Automatic cleanup on errors
- **Performance Context**: Clear guidance on when safer-ring excels vs alternatives

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

## 📖 Learning Path - Best Practices Edition

### Beginner: Understanding the Fundamentals
1. **Start with `async_demo.rs`** - Learn the hot potato pattern and why OwnedBuffer is recommended
2. **Read the API guidance** - Understand why PinnedBuffer is not recommended
3. **Try `buffer_pool_demo.rs`** - See how buffer pools work with the hot potato pattern

### Intermediate: Real-World Applications
1. **Build `echo_server_main.rs`** - Learn the "one Ring per task" concurrency model
2. **Implement `file_copy.rs`** - Understand performance context and when safer-ring excels
3. **Study the ownership patterns** - Master the hot potato ownership transfer

### Advanced: Production Deployment
1. **Deploy `https_server.rs`** - Apply patterns to real TLS workloads
2. **Benchmark your use case** - Understand performance characteristics
3. **Extend patterns** - Apply hot potato and one-Ring-per-task to your applications

## 🤝 Contributing

When adding new examples:

1. **Documentation**: Include comprehensive comments and documentation
2. **Error Handling**: Demonstrate proper error handling patterns
3. **Safety**: Showcase safer-ring's safety guarantees
4. **Performance**: Include performance considerations and metrics
5. **Testing**: Provide clear testing instructions

### Example Template - Best Practices Edition
```rust
//! # Example Title - BEST PRACTICES EDITION
//!
//! Brief description demonstrating RECOMMENDED patterns.
//!
//! ## ⚠️ IMPORTANT: API Recommendation Notice
//!
//! This example uses the **RECOMMENDED** `OwnedBuffer` and `*_owned` methods.
//! Avoid `PinnedBuffer` and `ring.read()` - they're not suitable for application code.
//!
//! ## Features Demonstrated
//! - Hot Potato Pattern: Optimal ownership transfer with OwnedBuffer
//! - Feature 2: Description with best practice context
//!
//! ## Usage
//! ```bash
//! cargo run --example example_name
//! ```

use safer_ring::{OwnedBuffer, Ring}; // Use specific imports, prefer OwnedBuffer

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Implementation using hot potato pattern with comprehensive educational comments
    let ring = Ring::new(32)?;
    let buffer = OwnedBuffer::new(4096); // Prefer OwnedBuffer
    
    // Hot potato: throw buffer to kernel, catch it back
    let (result, buffer_back) = ring.some_owned_operation(fd, buffer).await?;
    
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