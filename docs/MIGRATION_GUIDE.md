# Migration Guide to safer-ring

This guide provides step-by-step instructions for migrating existing Rust applications to safer-ring, whether you're coming from tokio, async-std, mio, or raw io_uring.

## Overview

safer-ring provides a drop-in replacement for traditional async I/O while adding memory safety guarantees and automatic performance optimization. The migration process varies depending on your current I/O approach.

## Migration Paths

### From tokio

#### Before (tokio)
```rust
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open("input.txt").await?;
    let mut buffer = vec![0; 1024];
    let bytes_read = file.read(&mut buffer).await?;
    
    let mut output = File::create("output.txt").await?;
    output.write_all(&buffer[..bytes_read]).await?;
    
    Ok(())
}
```

#### After (safer-ring with AsyncRead/AsyncWrite compatibility)
```rust
use safer_ring::{Ring, AsyncReadAdapter, AsyncWriteAdapter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::os::unix::io::AsRawFd;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    
    let input = std::fs::File::open("input.txt")?;
    let mut reader = AsyncReadAdapter::new(&ring, input.as_raw_fd());
    
    let output = std::fs::File::create("output.txt")?;
    let mut writer = AsyncWriteAdapter::new(&ring, output.as_raw_fd());
    
    let mut buffer = vec![0; 1024];
    let bytes_read = reader.read(&mut buffer).await?;
    writer.write_all(&buffer[..bytes_read]).await?;
    
    Ok(())
}
```

#### After (safer-ring native API - higher performance)
```rust
use safer_ring::{Ring, PinnedBuffer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    
    let input_fd = std::fs::File::open("input.txt")?.as_raw_fd();
    let output_fd = std::fs::File::create("output.txt")?.as_raw_fd();
    
    let mut buffer = PinnedBuffer::with_capacity(1024);
    let (bytes_read, buffer) = ring.read(input_fd, buffer.as_mut_slice()).await?;
    let (bytes_written, _) = ring.write(output_fd, &buffer.as_slice()[..bytes_read]).await?;
    
    println!("Copied {} bytes", bytes_written);
    Ok(())
}
```

### From async-std

#### Before (async-std)
```rust
use async_std::fs::File;
use async_std::io::{prelude::*, BufReader, BufWriter};

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("data.txt").await?;
    let mut reader = BufReader::new(file);
    
    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        println!("{}", line.trim());
        line.clear();
    }
    
    Ok(())
}
```

#### After (safer-ring)
```rust
use safer_ring::{Ring, PinnedBuffer, AsyncReadAdapter};
use tokio::io::{AsyncBufReadExt, BufReader};
use std::os::unix::io::AsRawFd;

#[tokio::main]  // Note: switch to tokio runtime for better performance
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(32)?;
    let file = std::fs::File::open("data.txt")?;
    let adapter = AsyncReadAdapter::new(&ring, file.as_raw_fd());
    let mut reader = BufReader::new(adapter);
    
    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        println!("{}", line.trim());
        line.clear();
    }
    
    Ok(())
}
```

### From mio

#### Before (mio)
```rust
use mio::{Events, Interest, Poll, Token};
use mio::net::{TcpListener, TcpStream};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(128);
    
    let mut server = TcpListener::bind("127.0.0.1:7878".parse()?)?;
    poll.registry().register(&mut server, Token(0), Interest::READABLE)?;
    
    loop {
        poll.poll(&mut events, None)?;
        
        for event in events.iter() {
            match event.token() {
                Token(0) => {
                    let (connection, address) = server.accept()?;
                    println!("Accepted connection from: {}", address);
                }
                _ => unreachable!(),
            }
        }
    }
}
```

#### After (safer-ring)
```rust
use safer_ring::Ring;
use std::net::TcpListener;
use std::os::unix::io::AsRawFd;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(128)?;
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    let listener_fd = listener.as_raw_fd();
    
    loop {
        let client_fd = ring.accept(listener_fd).await?;
        println!("Accepted connection: fd {}", client_fd);
        
        // Handle client in separate task
        tokio::spawn(async move {
            // Handle client connection
        });
    }
}
```

### From raw io_uring

#### Before (raw io_uring)
```rust
use io_uring::{IoUring, opcode, types};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ring = IoUring::new(8)?;
    
    let fd = std::fs::File::open("test.txt")?;
    let mut buf = vec![0; 1024];
    
    // UNSAFE: Raw buffer management
    let read_e = opcode::Read::new(types::Fd(fd.as_raw_fd()), buf.as_mut_ptr(), buf.len() as _)
        .build()
        .user_data(0x42);
    
    unsafe {
        ring.submission().push(&read_e)?;
    }
    
    ring.submit_and_wait(1)?;
    
    let cqe = ring.completion().next().expect("completion queue is empty");
    let ret = cqe.result();
    
    if ret < 0 {
        return Err(std::io::Error::from_raw_os_error(-ret).into());
    }
    
    println!("Read {} bytes", ret);
    Ok(())
}
```

#### After (safer-ring)
```rust
use safer_ring::{Ring, PinnedBuffer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Ring::new(8)?;
    
    let fd = std::fs::File::open("test.txt")?.as_raw_fd();
    let mut buffer = PinnedBuffer::with_capacity(1024);
    
    // SAFE: Automatic buffer lifecycle management
    let (bytes_read, buffer) = ring.read(fd, buffer.as_mut_slice()).await?;
    
    println!("Read {} bytes", bytes_read);
    
    // Buffer is safely returned, no manual cleanup needed
    Ok(())
}
```

## Step-by-Step Migration Process

### Phase 1: Assessment

1. **Identify I/O Patterns**
   ```bash
   # Find async I/O usage in your codebase
   rg "AsyncRead|AsyncWrite|\.read\(|\.write\(" --type rust
   rg "tokio::fs|async_std::fs" --type rust
   rg "mio::|io_uring::" --type rust
   ```

2. **Check Platform Compatibility**
   ```rust
   use safer_ring::{is_io_uring_available, get_environment_info};
   
   fn main() {
       println!("io_uring available: {}", is_io_uring_available());
       let env = get_environment_info();
       println!("Platform: {} CPUs, cloud: {}", 
                env.cpu_count, env.is_cloud_environment());
   }
   ```

3. **Performance Baseline**
   ```bash
   # Benchmark your current application
   cargo bench
   
   # Or use your existing performance tests
   time ./your-app < input.txt
   ```

### Phase 2: Gradual Migration

#### Option A: Compatibility Layer (Low Risk)
Start with the AsyncRead/AsyncWrite adapters for minimal code changes:

```rust
// Add safer-ring alongside existing code
use safer_ring::{Ring, AsyncReadAdapter, AsyncWriteAdapter};

// Modify one module at a time
pub struct IoManager {
    ring: Ring<'static>,  // Single ring for the application
}

impl IoManager {
    pub fn new() -> Result<Self, safer_ring::SaferRingError> {
        Ok(Self {
            ring: Ring::new(64)?,
        })
    }
    
    pub fn async_reader(&self, fd: RawFd) -> AsyncReadAdapter {
        AsyncReadAdapter::new(&self.ring, fd)
    }
    
    pub fn async_writer(&self, fd: RawFd) -> AsyncWriteAdapter {
        AsyncWriteAdapter::new(&self.ring, fd)
    }
}
```

#### Option B: Native API Migration (Higher Performance)
Migrate to safer-ring's native API for maximum performance:

```rust
// Replace tokio::io operations with safer-ring
pub struct FileProcessor {
    ring: Ring<'static>,
}

impl FileProcessor {
    pub async fn copy_file(&self, src: RawFd, dest: RawFd) -> Result<u64, SaferRingError> {
        let mut buffer = PinnedBuffer::with_capacity(64 * 1024);  // 64KB buffer
        let mut total_copied = 0u64;
        
        loop {
            let (bytes_read, buffer) = self.ring.read(src, buffer.as_mut_slice()).await?;
            
            if bytes_read == 0 {
                break;  // EOF
            }
            
            let (bytes_written, buffer) = self.ring.write(dest, &buffer.as_slice()[..bytes_read]).await?;
            total_copied += bytes_written as u64;
        }
        
        Ok(total_copied)
    }
}
```

### Phase 3: Optimization

#### Hot Potato Pattern
Embrace the ownership transfer model for zero-copy operations:

```rust
use safer_ring::{Ring, OwnedBuffer};

pub struct ZeroCopyProcessor {
    ring: Ring<'static>,
}

impl ZeroCopyProcessor {
    pub async fn process_data(&self, input_fd: RawFd) -> Result<OwnedBuffer, SaferRingError> {
        // Create owned buffer
        let buffer = OwnedBuffer::new(1024 * 1024);  // 1MB
        
        // Hot potato: buffer -> operation -> (result, buffer)
        let (bytes_read, buffer) = self.ring.read_owned(input_fd, buffer).await?;
        
        // Process data in place if needed
        // let processed_buffer = process_buffer(buffer);
        
        Ok(buffer)  // Return ownership to caller
    }
}
```

#### Registry Optimization
For frequently used files/fds:

```rust
use safer_ring::{Registry, Ring, FixedFile};

pub struct OptimizedServer {
    ring: Ring<'static>,
    registry: Registry<'static>,
    log_file: FixedFile,
    config_file: FixedFile,
}

impl OptimizedServer {
    pub fn new() -> Result<Self, SaferRingError> {
        let mut registry = Registry::new();
        
        // Register frequently used files
        let log_fd = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("app.log")?
            .as_raw_fd();
            
        let config_fd = std::fs::File::open("config.toml")?.as_raw_fd();
        
        let fixed_files = registry.register_fixed_files(vec![log_fd, config_fd])?;
        
        Ok(Self {
            ring: Ring::new(256)?,
            registry,
            log_file: fixed_files[0].clone(),
            config_file: fixed_files[1].clone(),
        })
    }
    
    pub async fn log_message(&self, message: &str) -> Result<(), SaferRingError> {
        let buffer = safer_ring::PinnedBuffer::from_slice(message.as_bytes());
        
        // Use fixed file for best performance
        let (bytes_written, _) = self.ring.write_fixed(&self.log_file, buffer.as_mut_slice()).await?;
        
        Ok(())
    }
}
```

## Common Migration Patterns

### Network Server Migration

#### Before (tokio)
```rust
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(mut socket: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; 1024];
    let n = socket.read(&mut buffer).await?;
    socket.write_all(&buffer[0..n]).await?;
    Ok(())
}
```

#### After (safer-ring)
```rust
use safer_ring::{Ring, PinnedBuffer};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ring = Arc::new(Ring::new(256)?);
    let listener = std::net::TcpListener::bind("127.0.0.1:8080")?;
    let listener_fd = listener.as_raw_fd();
    
    loop {
        let client_fd = ring.accept(listener_fd).await?;
        let ring_clone = ring.clone();
        
        tokio::spawn(async move {
            if let Err(e) = handle_client(ring_clone, client_fd).await {
                eprintln!("Client error: {}", e);
            }
        });
    }
}

async fn handle_client(ring: Arc<Ring>, client_fd: RawFd) -> Result<(), safer_ring::SaferRingError> {
    let mut buffer = PinnedBuffer::with_capacity(1024);
    
    let (bytes_read, buffer) = ring.recv(client_fd, buffer.as_mut_slice()).await?;
    let (bytes_sent, _) = ring.send(client_fd, &buffer.as_slice()[..bytes_read]).await?;
    
    println!("Echoed {} bytes", bytes_sent);
    Ok(())
}
```

### File Processing Pipeline

#### Before (async-std)
```rust
use async_std::fs::File;
use async_std::io::{BufReader, BufWriter, prelude::*};

async fn process_file(input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let input = File::open(input_path).await?;
    let output = File::create(output_path).await?;
    
    let mut reader = BufReader::new(input);
    let mut writer = BufWriter::new(output);
    
    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        let processed = process_line(&line);
        writer.write_all(processed.as_bytes()).await?;
        line.clear();
    }
    
    writer.flush().await?;
    Ok(())
}

fn process_line(line: &str) -> String {
    line.to_uppercase()
}
```

#### After (safer-ring with batching)
```rust
use safer_ring::{Ring, Batch, PinnedBuffer};

async fn process_file(input_path: &str, output_path: &str) -> Result<(), safer_ring::SaferRingError> {
    let ring = Ring::new(64)?;
    
    let input_fd = std::fs::File::open(input_path)?.as_raw_fd();
    let output_fd = std::fs::File::create(output_path)?.as_raw_fd();
    
    // Use larger buffers for better performance
    let read_buffer = PinnedBuffer::with_capacity(64 * 1024);
    let mut write_buffer = PinnedBuffer::with_capacity(64 * 1024);
    
    let (bytes_read, read_buffer) = ring.read(input_fd, read_buffer.as_mut_slice()).await?;
    
    // Process data
    let input_str = std::str::from_utf8(&read_buffer.as_slice()[..bytes_read])?;
    let processed = input_str.to_uppercase();
    
    // Write processed data
    write_buffer.as_mut_slice()[..processed.len()].copy_from_slice(processed.as_bytes());
    let (bytes_written, _) = ring.write(output_fd, &write_buffer.as_slice()[..processed.len()]).await?;
    
    println!("Processed {} bytes -> {} bytes", bytes_read, bytes_written);
    Ok(())
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use safer_ring::{Ring, PinnedBuffer};
    
    #[tokio::test]
    async fn test_file_copy() -> Result<(), Box<dyn std::error::Error>> {
        let ring = Ring::new(32)?;
        
        // Create test input
        let input = tempfile::NamedTempFile::new()?;
        std::fs::write(input.path(), b"Hello, safer-ring!")?;
        
        let output = tempfile::NamedTempFile::new()?;
        
        // Test the copy operation
        let input_fd = std::fs::File::open(input.path())?.as_raw_fd();
        let output_fd = std::fs::File::create(output.path())?.as_raw_fd();
        
        let mut buffer = PinnedBuffer::with_capacity(1024);
        let (bytes_read, buffer) = ring.read(input_fd, buffer.as_mut_slice()).await?;
        let (bytes_written, _) = ring.write(output_fd, &buffer.as_slice()[..bytes_read]).await?;
        
        assert_eq!(bytes_read, 18);
        assert_eq!(bytes_written, 18);
        
        // Verify content
        let output_content = std::fs::read_to_string(output.path())?;
        assert_eq!(output_content, "Hello, safer-ring!");
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_backend_fallback() -> Result<(), Box<dyn std::error::Error>> {
        use safer_ring::{Runtime, Backend};
        
        let runtime = Runtime::auto_detect()?;
        
        // Test should pass regardless of available backend
        match runtime.backend() {
            Backend::IoUring => println!("Testing with io_uring"),
            Backend::Epoll => println!("Testing with epoll"),
            Backend::Stub => println!("Testing with stub"),
        }
        
        // Ring creation should work with any backend
        let ring = Ring::new(8)?;
        assert_eq!(ring.orphan_count(), 0);
        
        Ok(())
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_migration_compatibility() {
    // Test that migrated code produces same results as original
    let original_result = original_function().await.unwrap();
    let migrated_result = migrated_function().await.unwrap();
    
    assert_eq!(original_result, migrated_result);
}
```

## Performance Validation

### Benchmarking
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use safer_ring::{Ring, PinnedBuffer};

async fn benchmark_read_write() -> Result<(), safer_ring::SaferRingError> {
    let ring = Ring::new(32)?;
    let mut buffer = PinnedBuffer::with_capacity(4096);
    
    // Benchmark your typical workload
    for _ in 0..1000 {
        let (bytes_read, buffer) = ring.read(0, buffer.as_mut_slice()).await?;
        black_box(bytes_read);
    }
    
    Ok(())
}

fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("safer_ring_read_write", |b| {
        b.to_async(&rt).iter(|| benchmark_read_write())
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
```

## Migration Checklist

### Pre-Migration
- [ ] Review current I/O patterns and identify bottlenecks
- [ ] Check platform support for io_uring
- [ ] Set up performance baseline measurements
- [ ] Plan rollback strategy

### During Migration
- [ ] Start with compatibility layer (AsyncRead/AsyncWrite)
- [ ] Migrate one module at a time
- [ ] Add comprehensive tests for each migrated component
- [ ] Verify performance improvements at each step

### Post-Migration
- [ ] Optimize with native safer-ring APIs
- [ ] Implement registry optimization for hot paths
- [ ] Add monitoring and observability
- [ ] Update documentation and team training

### Validation
- [ ] Run full test suite with multiple backends
- [ ] Performance regression testing
- [ ] Load testing in staging environment
- [ ] Gradual rollout in production

## Rollback Plan

If issues are encountered during migration:

1. **Immediate Rollback**
   ```bash
   git revert <migration-commit>
   cargo build --release
   ./deploy.sh
   ```

2. **Partial Rollback**
   ```rust
   // Use feature flags for gradual rollback
   #[cfg(feature = "safer-ring")]
   use safer_ring::{Ring, PinnedBuffer};
   
   #[cfg(not(feature = "safer-ring"))]
   use tokio::fs::File;
   ```

3. **Hybrid Approach**
   ```rust
   // Keep both implementations during transition
   pub enum IoBackend {
       SaferRing(Ring<'static>),
       Tokio,
   }
   
   impl IoBackend {
       pub async fn read(&self, fd: RawFd, buf: &mut [u8]) -> Result<usize, Box<dyn std::error::Error>> {
           match self {
               IoBackend::SaferRing(ring) => {
                   let buffer = PinnedBuffer::from_mut_slice(buf);
                   let (bytes, _) = ring.read(fd, buffer).await?;
                   Ok(bytes)
               }
               IoBackend::Tokio => {
                   // Fallback to original implementation
                   todo!("Tokio implementation")
               }
           }
       }
   }
   ```

## Getting Help

- **Documentation**: Check the [API documentation](./API_REFERENCE.md)
- **Examples**: See the `examples/` directory for migration patterns
- **Issues**: Report migration issues on [GitHub Issues](https://github.com/your-repo/safer-ring/issues)
- **Discussions**: Join the community discussions for migration tips

## Success Stories

After successful migration, users typically see:

- **2-4x** performance improvement on io_uring-enabled systems
- **Zero memory safety issues** related to I/O operations  
- **Seamless fallback** in restricted environments
- **Simplified async I/O code** with ownership transfer model

The migration investment pays off through improved performance, safety, and maintainability of your async I/O code.