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

### Benchmark Framework

Comprehensive performance measurement and comparison system:

```rust
pub struct BenchmarkSuite {
    system_info: SystemInfo,
    file_benchmarks: FileBenchmarkRunner,
    network_benchmarks: NetworkBenchmarkRunner,
    statistical_analyzer: StatisticalAnalyzer,
}

pub trait BenchmarkRunner {
    fn run_safer_ring_baseline(&self) -> BenchmarkResults;
    fn run_raw_io_uring_comparison(&self) -> BenchmarkResults;
    fn analyze_performance_difference(&self) -> PerformanceDelta;
}
```

**Key Features:**
- Automated system information collection for reproducible results
- Statistical analysis with confidence intervals and significance testing
- Integration with existing example applications as performance baselines
- Detailed reporting of overhead percentages and throughput comparisons

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

### Comparative Benchmark Testing

Systematic performance comparison testing to validate zero-cost abstraction claims:

1. **Benchmark Harness**: Automated framework for running identical workloads against safer-ring and raw io_uring implementations
2. **Statistical Validation**: Multiple runs with proper statistical analysis to ensure reliable performance measurements
3. **Environmental Documentation**: Comprehensive system information logging to enable reproducible results
4. **Regression Detection**: Continuous integration benchmarks to detect performance regressions in safety abstractions

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

### Comparative Performance Benchmarks

**Design Rationale**: To provide developers with concrete performance data when evaluating safer-ring, we need comprehensive benchmarks that compare safer-ring implementations against equivalent raw io_uring code. This addresses the critical question of "what is the cost of safety?"

**Benchmark Architecture**:
```
┌─────────────────────────────────────────────────────────────────┐
│                    Benchmark Framework                          │
│                                                                 │
│  ┌─────────────────┐    ┌─────────────────┐    ┌──────────────┐ │
│  │  System Info    │    │  File I/O       │    │  Network I/O │ │
│  │  Collection     │    │  Benchmarks     │    │  Benchmarks  │ │
│  │                 │    │                 │    │              │ │
│  │ - Kernel Ver    │    │ - safer-ring    │    │ - safer-ring │ │
│  │ - CPU Info      │    │ - raw io_uring  │    │ - raw io_uring│ │
│  │ - Memory Info   │    │ - Statistical   │    │ - Statistical│ │
│  │ - I/O Scheduler │    │   Analysis      │    │   Analysis   │ │
│  └─────────────────┘    └─────────────────┘    └──────────────┘ │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │              Statistical Analysis Engine                    │ │
│  │                                                             │ │
│  │ - Confidence Intervals (95% default)                       │ │
│  │ - Significance Testing (t-test, Mann-Whitney U)            │ │
│  │ - Overhead Percentage Calculation                          │ │
│  │ - Throughput Comparison with Error Bars                    │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Benchmark Implementation Strategy**:

1. **Baseline Implementations**: Use existing examples as safer-ring baselines:
   - `examples/file_copy.rs` for file I/O performance comparison
   - `examples/echo_server_main.rs` for network I/O performance comparison

2. **Raw io_uring Equivalents**: Create functionally equivalent implementations using raw io_uring:
   - Direct io_uring syscalls without safety abstractions
   - Manual buffer management and lifetime tracking
   - Identical I/O patterns and buffer sizes

3. **System Information Collection**:
   ```rust
   pub struct SystemInfo {
       pub kernel_version: String,
       pub cpu_model: String,
       pub memory_total: u64,
       pub io_scheduler: String,
       pub numa_nodes: u32,
   }
   ```

4. **Statistical Analysis Framework**:
   ```rust
   pub struct BenchmarkResult {
       pub safer_ring_stats: PerformanceStats,
       pub raw_io_uring_stats: PerformanceStats,
       pub overhead_percentage: f64,
       pub confidence_interval: (f64, f64),
       pub p_value: f64,
       pub is_significant: bool,
   }
   
   pub struct PerformanceStats {
       pub mean_latency: Duration,
       pub throughput_mbps: f64,
       pub p95_latency: Duration,
       pub p99_latency: Duration,
       pub std_deviation: f64,
   }
   ```

**Key Design Decisions**:

- **Statistical Rigor**: Use proper statistical methods (t-tests, confidence intervals) rather than simple averages to ensure meaningful comparisons
- **Environmental Consistency**: Collect and log system information to ensure reproducible results across different environments
- **Multiple Metrics**: Track both latency and throughput to provide comprehensive performance picture
- **Significance Testing**: Only report performance differences that are statistically significant to avoid misleading conclusions
- **Automated Analysis**: Provide clear, actionable output that developers can use for decision-making

**Output Format**:
```
Safer-Ring Performance Comparison Report
========================================
System: Linux 6.1.0, Intel Xeon E5-2680 v4, 64GB RAM
I/O Scheduler: mq-deadline, NUMA Nodes: 2

File Copy Benchmark (1GB file, 64KB buffers):
  safer-ring:    1,247 MB/s ± 23 MB/s (95% CI)
  raw io_uring:  1,289 MB/s ± 19 MB/s (95% CI)
  Overhead:      3.3% ± 2.1% (p=0.023, significant)

Network Echo Benchmark (10K concurrent connections):
  safer-ring:    847,231 req/s ± 12,445 req/s (95% CI)
  raw io_uring:  851,203 req/s ± 11,892 req/s (95% CI)
  Overhead:      0.5% ± 1.8% (p=0.412, not significant)
```

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

## Progressive Enhancement Architecture

### Design Philosophy

Safer-ring implements a **progressive enhancement strategy** where the library's core API works on the minimum supported kernel (Linux 5.1), and newer features are automatically leveraged when available or exposed through new methods with graceful fallbacks.

**Key Principles:**
- **Runtime Feature Detection**: Detect kernel capabilities at Ring initialization
- **Transparent Optimization**: Automatically use faster implementations when available
- **Graceful Degradation**: Provide fallbacks or clear error messages for unsupported features
- **API Consistency**: Maintain the same safety guarantees regardless of underlying implementation

### Feature Detection Framework

```rust
pub struct FeatureDetector {
    kernel_version: KernelVersion,
    features: AvailableFeatures,
}

pub struct AvailableFeatures {
    pub multi_shot_accept: bool,        // Linux 5.19+
    pub multi_shot_recv: bool,          // Linux 6.0+
    pub registered_ring_fd: bool,       // Linux 6.3+
    pub send_zc: bool,                  // Linux 6.0+
    pub recv_multishot: bool,           // Linux 6.0+
    pub msg_ring: bool,                 // Linux 6.11+
    pub buffer_rings: bool,             // Linux 5.19+
    pub file_allocate: bool,            // Linux 5.19+
}

impl FeatureDetector {
    pub fn new() -> Result<Self> {
        let kernel_version = Self::detect_kernel_version()?;
        let features = Self::probe_features(&kernel_version);
        Ok(Self { kernel_version, features })
    }
    
    fn probe_features(kernel_version: &KernelVersion) -> AvailableFeatures {
        AvailableFeatures {
            multi_shot_accept: kernel_version >= &KernelVersion::new(5, 19, 0),
            multi_shot_recv: kernel_version >= &KernelVersion::new(6, 0, 0),
            registered_ring_fd: kernel_version >= &KernelVersion::new(6, 3, 0),
            send_zc: kernel_version >= &KernelVersion::new(6, 0, 0),
            recv_multishot: kernel_version >= &KernelVersion::new(6, 0, 0),
            msg_ring: kernel_version >= &KernelVersion::new(6, 11, 0),
            buffer_rings: kernel_version >= &KernelVersion::new(5, 19, 0),
            file_allocate: kernel_version >= &KernelVersion::new(5, 19, 0),
        }
    }
}
```

### Backend Abstraction Layer

The progressive enhancement is implemented through a backend abstraction that allows different implementations based on available features:

```rust
pub trait Backend {
    fn submit_operation(&mut self, op: &Operation) -> Result<()>;
    fn try_complete(&mut self) -> Result<Option<Completion>>;
    
    // Progressive enhancement methods
    fn submit_accept_multi(&mut self, fd: RawFd, user_data: u64) -> Result<()>;
    fn submit_recv_multi(&mut self, fd: RawFd, buf: &mut [u8], user_data: u64) -> Result<()>;
    fn register_ring_fd(&mut self) -> Result<Option<RawFd>>;
    fn submit_send_zc(&mut self, fd: RawFd, buf: &[u8], user_data: u64) -> Result<()>;
}

pub struct IoUringBackend {
    ring: io_uring::IoUring,
    features: Arc<AvailableFeatures>,
    registered_ring_fd: Option<RawFd>,
}

impl Backend for IoUringBackend {
    fn submit_accept_multi(&mut self, fd: RawFd, user_data: u64) -> Result<()> {
        if self.features.multi_shot_accept {
            // Use native multi-shot accept opcode
            let entry = opcode::AcceptMulti::new(types::Fd(fd))
                .build()
                .user_data(user_data);
            unsafe { self.ring.submission().push(&entry)? };
            Ok(())
        } else {
            Err(SaferRingError::UnsupportedFeature)
        }
    }
    
    fn register_ring_fd(&mut self) -> Result<Option<RawFd>> {
        if self.features.registered_ring_fd {
            // Register the ring fd for improved performance
            let ring_fd = self.ring.as_raw_fd();
            // Implementation would register the fd and return it
            self.registered_ring_fd = Some(ring_fd);
            Ok(Some(ring_fd))
        } else {
            Ok(None) // Feature not available, continue without registration
        }
    }
}
```

### Enhanced Ring API with Progressive Features

The Ring API provides both automatic optimization and explicit feature access:

```rust
impl<'ring> Ring<'ring> {
    pub fn new(entries: u32) -> Result<Self> {
        let detector = FeatureDetector::new()?;
        let backend = IoUringBackend::new(entries, detector.features.clone())?;
        
        let mut ring = Self {
            backend,
            features: detector.features,
            operations: RefCell::new(OperationTracker::new()),
        };
        
        // Automatically register ring fd if available
        if let Ok(Some(_)) = ring.backend.register_ring_fd() {
            log::info!("Using registered ring fd for improved performance");
        }
        
        Ok(ring)
    }
    
    /// Accept multiple connections efficiently
    /// Uses multi-shot accept on Linux 5.19+, falls back to loop on older kernels
    pub async fn accept_multiple(&mut self, fd: RawFd, max_connections: usize) -> Result<Vec<RawFd>> {
        if self.features.multi_shot_accept {
            // High-performance native implementation
            self.accept_multiple_native(fd, max_connections).await
        } else {
            // Userspace fallback for older kernels
            self.accept_multiple_fallback(fd, max_connections).await
        }
    }
    
    /// Zero-copy send (Linux 6.0+)
    /// Returns UnsupportedFeature error on older kernels
    pub async fn send_zero_copy(&mut self, fd: RawFd, buffer: &[u8]) -> Result<usize> {
        if self.features.send_zc {
            let op_id = self.next_operation_id();
            self.backend.submit_send_zc(fd, buffer, op_id)?;
            self.wait_for_completion(op_id).await
        } else {
            Err(SaferRingError::UnsupportedFeature)
        }
    }
    
    /// Get available features for this ring
    pub fn features(&self) -> &AvailableFeatures {
        &self.features
    }
}
```

### Configuration and User Control

Users can control feature usage through configuration:

```rust
pub struct AdvancedConfig {
    pub enable_multi_shot: bool,
    pub enable_zero_copy: bool,
    pub enable_registered_fds: bool,
    pub enable_buffer_rings: bool,
    pub fallback_strategy: FallbackStrategy,
}

pub enum FallbackStrategy {
    /// Use userspace fallback implementations
    Graceful,
    /// Return UnsupportedFeature errors
    Strict,
    /// Log warnings and use fallbacks
    Warn,
}

impl Ring<'_> {
    pub fn with_config(entries: u32, config: AdvancedConfig) -> Result<Self> {
        let mut ring = Self::new(entries)?;
        
        // Apply user preferences
        if !config.enable_multi_shot {
            ring.features.multi_shot_accept = false;
            ring.features.multi_shot_recv = false;
        }
        
        ring.fallback_strategy = config.fallback_strategy;
        Ok(ring)
    }
}
```

### Example: Multi-Shot Accept Implementation

Here's how the multi-shot accept feature would be implemented with progressive enhancement:

```rust
impl<'ring> Ring<'ring> {
    async fn accept_multiple_native(&mut self, fd: RawFd, max_connections: usize) -> Result<Vec<RawFd>> {
        let op_id = self.next_operation_id();
        self.backend.submit_accept_multi(fd, op_id)?;
        
        let mut connections = Vec::new();
        
        // Multi-shot operations can return multiple completions
        while connections.len() < max_connections {
            match self.try_complete_operation(op_id).await? {
                Some(completion) => {
                    if completion.result >= 0 {
                        connections.push(completion.result as RawFd);
                    } else {
                        break; // No more connections available
                    }
                }
                None => break,
            }
        }
        
        Ok(connections)
    }
    
    async fn accept_multiple_fallback(&mut self, fd: RawFd, max_connections: usize) -> Result<Vec<RawFd>> {
        let mut connections = Vec::new();
        
        for _ in 0..max_connections {
            // Use existing single accept with timeout to avoid blocking
            match tokio::time::timeout(
                Duration::from_millis(10),
                self.accept(fd)
            ).await {
                Ok(Ok(client_fd)) => connections.push(client_fd),
                _ => break, // Timeout or error, no more connections
            }
        }
        
        Ok(connections)
    }
}
```

### Error Handling for Unsupported Features

```rust
#[derive(Debug, Error)]
pub enum SaferRingError {
    // ... existing variants
    
    #[error("Feature not supported on this kernel version. Minimum required: {required}, current: {current}")]
    UnsupportedFeature { required: String, current: String },
    
    #[error("Feature disabled by configuration")]
    FeatureDisabled,
}
```

### Testing Strategy for Progressive Enhancement

1. **Multi-Kernel Testing**: Test on various kernel versions to ensure fallbacks work
2. **Feature Toggle Testing**: Test with features artificially disabled
3. **Performance Regression Testing**: Ensure new features don't break existing performance
4. **Compatibility Testing**: Verify older applications continue to work

## Platform Considerations

### Linux Kernel Versions

- **Minimum**: Linux 5.1 (basic io_uring support)
- **Recommended**: Linux 5.19+ (buffer rings, multi-shot operations)
- **Optimal**: Linux 6.0+ (latest performance improvements)

**Feature Timeline:**
- **5.1**: Basic io_uring support
- **5.19**: Multi-shot accept, buffer rings, file allocation
- **6.0**: Multi-shot recv, zero-copy send
- **6.3**: Registered ring fd
- **6.11**: MSG_RING support

### Architecture Support

- **Primary**: x86_64, aarch64
- **Secondary**: Other architectures supported by io_uring
- **Testing**: Comprehensive testing on all supported platforms

### Integration Points

1. **Tokio Runtime**: Seamless integration with Tokio's async ecosystem
2. **async-std**: Compatible with async-std runtime
3. **Custom Runtimes**: Generic Future implementation works with any runtime
4. **Blocking Code**: Bridge functions for integration with synchronous code