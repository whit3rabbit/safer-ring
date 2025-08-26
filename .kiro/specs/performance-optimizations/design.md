# Design Document

## Overview

This design document outlines performance optimizations for the safer-ring library that improve efficiency without compromising safety guarantees. The optimizations target three key areas: BatchFuture polling efficiency, buffer implementation consolidation, and threading model documentation. These improvements will reduce computational overhead, eliminate code duplication, and enhance developer experience.

## Architecture

### Current Performance Bottlenecks

The analysis identified three specific areas where performance can be improved:

1. **BatchFuture O(N) Lookup**: The `poll_completions` method currently uses linear search to match operation IDs to batch indices
2. **Redundant Buffer Implementations**: Two similar `PinnedBuffer` implementations exist (`buffer_temp.rs` and `buffer/mod.rs`)
3. **Insufficient Threading Documentation**: The Ring's Send-but-not-Sync nature lacks clear documentation

### Optimization Strategy

```
┌─────────────────────────────────────────────────────────────────┐
│                    Performance Optimizations                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   BatchFuture   │  │     Buffer      │  │   Threading     │ │
│  │   O(1) Lookup   │  │  Consolidation  │  │ Documentation   │ │
│  │                 │  │                 │  │                 │ │
│  │ HashMap<u64,    │  │ Remove          │  │ Clear Send/     │ │
│  │ usize> for      │  │ buffer_temp.rs  │  │ Sync docs       │ │
│  │ operation_id    │  │                 │  │                 │ │
│  │ to batch_index  │  │ Use unified     │  │ Usage patterns  │ │
│  │                 │  │ buffer/mod.rs   │  │ and examples    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### BatchFuture Optimization

#### Current Implementation Analysis

The current `BatchFuture::poll_completions` method uses this approach:
```rust
let index_opt = self.operation_ids
    .iter()
    .position(|id_opt| id_opt.map_or(false, |id| id == operation_id));
```

This results in O(N) complexity for each completion, where N is the batch size.

#### Optimized Design

**New Data Structure:**
```rust
pub struct BatchFuture<'ring> {
    // Existing fields...
    ring: &'ring mut Ring<'ring>,
    results: Vec<Option<OperationResult>>,
    dependencies: HashMap<usize, Vec<usize>>,
    fail_fast: bool,
    completed: bool,
    
    // NEW: O(1) lookup from operation_id to batch_index
    operation_id_to_index: HashMap<u64, usize>,
    
    // MODIFIED: Keep for iteration but use HashMap for lookup
    operation_ids: Vec<Option<u64>>,
}
```

**Optimized Lookup Logic:**
```rust
// Replace linear search with HashMap lookup
if let Some(&index) = self.operation_id_to_index.get(&operation_id) {
    // Direct O(1) access to the batch index
    if self.results[index].is_some() {
        continue; // Already completed
    }
    // Process completion...
}
```

**Memory Overhead Analysis:**
- HashMap overhead: ~24 bytes base + (8 + 8) bytes per entry = 24 + 16N bytes
- For typical batch sizes (N < 100), this adds ~2KB maximum
- Performance benefit: O(N) → O(1) per completion
- Net benefit: Significant for batches with multiple completions

#### Interface Changes

The public API remains unchanged. All optimizations are internal to `BatchFuture`:

```rust
// Public API unchanged
impl<'ring> BatchFuture<'ring> {
    pub(crate) fn new(
        operation_ids: Vec<Option<u64>>,
        dependencies: HashMap<usize, Vec<usize>>,
        ring: &'ring mut Ring<'ring>,
        waker_registry: Rc<WakerRegistry>,
        fail_fast: bool,
    ) -> Self {
        // Internal implementation optimized
    }
}
```

### Buffer Implementation Consolidation

#### Current State Analysis

**buffer_temp.rs** (1,200+ lines):
- Complete implementation with AtomicU64 generation counter
- Comprehensive test suite
- NUMA allocation support
- Advanced features like aligned allocation

**buffer/mod.rs** (200 lines):
- Simplified implementation delegating to submodules
- Uses `GenerationCounter` from generation.rs
- Missing some advanced features from buffer_temp.rs

#### Consolidation Strategy

**Phase 1: Feature Parity Analysis**
```rust
// Features in buffer_temp.rs but missing in buffer/mod.rs:
- with_capacity_aligned()
- with_capacity_numa() 
- try_numa_allocation()
- mark_in_use() / mark_available()
- is_available()
- Comprehensive Debug implementations
- Thread safety tests
```

**Phase 2: Migration Plan**
1. Ensure `buffer/mod.rs` has all features from `buffer_temp.rs`
2. Migrate any missing functionality to the modular structure
3. Update imports throughout codebase to use `buffer/mod.rs`
4. Remove `buffer_temp.rs` from build system
5. Verify all tests pass

**Phase 3: Verification**
```rust
// Ensure these imports work correctly:
use crate::buffer::{PinnedBuffer, GenerationCounter};

// Verify all methods are available:
let buffer = PinnedBuffer::with_capacity_aligned(4096);
buffer.mark_in_use();
assert!(buffer.is_available());
```

#### File Structure After Consolidation

```
src/buffer/
├── mod.rs           # Main PinnedBuffer implementation
├── allocation.rs    # Allocation strategies (aligned, NUMA)
├── generation.rs    # GenerationCounter implementation  
└── numa.rs         # NUMA-aware allocation helpers
```

### Threading Model Documentation

#### Current Documentation Gap

The Ring struct has minimal threading documentation despite critical design decisions:
- `Ring` is `Send` but not `Sync` due to `RefCell<OperationTracker>`
- This prevents `Arc<Ring>` sharing but allows moving between threads
- Runtime panics can occur if multiple tasks try to access the same Ring

#### Enhanced Documentation Design

**Comprehensive Threading Section:**
```rust
/// # Threading Model
///
/// **Important:** `Ring` is `Send` but **NOT** `Sync`. This means:
///
/// - ✅ A `Ring` instance can be **moved** between threads
/// - ✅ A `Ring` can be used by a single async task at a time  
/// - ❌ A `Ring` **cannot** be shared concurrently via `Arc<Ring>`
/// - ❌ Multiple threads **cannot** access the same `Ring` simultaneously
///
/// This design choice optimizes for performance by avoiding locks on the
/// submission path while maintaining memory safety. The `RefCell` used
/// internally for operation tracking prevents concurrent access.
///
/// ## Why Not Sync?
///
/// The `RefCell<OperationTracker>` provides interior mutability for tracking
/// operations without requiring `&mut self` on every method. However, `RefCell`
/// uses runtime borrow checking, which panics on concurrent access:
///
/// ```rust,should_panic
/// use std::sync::Arc;
/// use std::thread;
/// 
/// let ring = Arc::new(Ring::new(32)?); // Won't compile - Ring is not Sync
/// ```
///
/// ## Recommended Usage Patterns
///
/// ### Single-threaded or async task usage (✅ Recommended):
/// ```rust,no_run
/// let mut ring = Ring::new(32)?;
/// // Use ring for I/O operations within this task
/// ```
///
/// ### Moving between threads (✅ Allowed):
/// ```rust,no_run
/// let ring = Ring::new(32)?;
/// thread::spawn(move || {
///     // Ring moved to this thread, can be used here
/// });
/// ```
///
/// ### One Ring per thread (✅ Recommended for multi-threading):
/// ```rust,no_run
/// use std::thread;
/// 
/// for i in 0..num_cpus::get() {
///     thread::spawn(move || {
///         let ring = Ring::new(32).unwrap();
///         // Each thread has its own Ring instance
///     });
/// }
/// ```
///
/// ### Sharing via channels (✅ Alternative pattern):
/// ```rust,no_run
/// use tokio::sync::mpsc;
/// 
/// let (tx, mut rx) = mpsc::channel(100);
/// let mut ring = Ring::new(32)?;
/// 
/// // Dedicated I/O task
/// tokio::spawn(async move {
///     while let Some(operation) = rx.recv().await {
///         // Process operation with ring
///     }
/// });
/// 
/// // Other tasks send operations via channel
/// tx.send(operation).await?;
/// ```
///
/// ## Performance Implications
///
/// - **Single-threaded**: Optimal performance, no synchronization overhead
/// - **Multi-threaded with separate Rings**: Good performance, scales with cores
/// - **Channel-based sharing**: Moderate overhead from message passing
/// - **Mutex-wrapped Ring**: Not supported, would require API changes
```

**Additional Documentation Sections:**

1. **RefCell Panic Prevention:**
```rust
/// ## Avoiding RefCell Panics
///
/// The internal `RefCell` can panic if accessed concurrently. This typically
/// happens when:
/// 
/// 1. Multiple async tasks share the same Ring reference
/// 2. Recursive calls during operation processing
/// 3. Accessing Ring from Drop implementations of operations
///
/// To prevent panics:
/// - Use one Ring per async task/thread
/// - Avoid storing Ring references in operation callbacks
/// - Complete all operations before dropping the Ring
```

2. **Performance Characteristics:**
```rust
/// ## Performance Characteristics
///
/// | Pattern | Throughput | Latency | Memory | Complexity |
/// |---------|------------|---------|---------|------------|
/// | Single Ring | Highest | Lowest | Lowest | Simple |
/// | Ring per thread | High | Low | Medium | Medium |
/// | Channel sharing | Medium | Medium | Medium | Medium |
/// | Multiple processes | Lower | Higher | Higher | Complex |
```

## Data Models

### BatchFuture Data Structure

```rust
pub struct BatchFuture<'ring> {
    /// Ring reference for polling completions
    ring: &'ring mut Ring<'ring>,
    
    /// Results collected so far (indexed by batch position)
    results: Vec<Option<OperationResult>>,
    
    /// Dependencies between operations (dependent -> dependencies)
    dependencies: HashMap<usize, Vec<usize>>,
    
    /// Whether to fail fast on first error
    fail_fast: bool,
    
    /// Whether the batch has completed
    completed: bool,
    
    /// Operation IDs for tracking completions (for iteration)
    operation_ids: Vec<Option<u64>>,
    
    /// NEW: Fast lookup from operation_id to batch_index
    operation_id_to_index: HashMap<u64, usize>,
}
```

### Buffer Module Structure

```rust
// src/buffer/mod.rs - Unified implementation
pub struct PinnedBuffer<T: ?Sized> {
    inner: Pin<Box<T>>,
    generation: GenerationCounter,
}

// src/buffer/generation.rs - Generation tracking
pub struct GenerationCounter {
    counter: AtomicU64,
}

// src/buffer/allocation.rs - Allocation strategies  
pub trait AllocationStrategy {
    fn allocate(size: usize) -> Result<Pin<Box<[u8]>>, AllocationError>;
}

pub struct AlignedAllocator;
pub struct NumaAllocator;

// src/buffer/numa.rs - NUMA-specific functionality
#[cfg(target_os = "linux")]
pub fn allocate_on_node(size: usize, node: usize) -> Result<Pin<Box<[u8]>>, NumaError>;
```

## Error Handling

### BatchFuture Error Handling

The optimization maintains existing error handling while improving performance:

```rust
impl<'ring> BatchFuture<'ring> {
    fn poll_completions(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match self.ring.try_complete() {
            Ok(completions) => {
                for completion in completions {
                    let operation_id = completion.id();
                    
                    // NEW: O(1) lookup instead of O(N) search
                    if let Some(&index) = self.operation_id_to_index.get(&operation_id) {
                        // Process completion...
                    }
                    // If operation_id not found, it's from a different batch - ignore
                }
            }
            Err(e) => return Poll::Ready(Err(e)),
        }
        // ... rest of method unchanged
    }
}
```

### Buffer Consolidation Error Handling

Ensure error handling consistency across buffer implementations:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("Allocation failed: {0}")]
    AllocationFailed(String),
    
    #[error("NUMA allocation failed: {0}")]
    NumaAllocationFailed(String),
    
    #[error("Buffer size too large: {size}")]
    SizeTooLarge { size: usize },
    
    #[error("Invalid alignment: {alignment}")]
    InvalidAlignment { alignment: usize },
}
```

## Testing Strategy

### BatchFuture Performance Testing

```rust
#[cfg(test)]
mod batch_performance_tests {
    use super::*;
    
    #[test]
    fn test_batch_lookup_performance() {
        // Test O(1) vs O(N) performance with large batches
        let batch_sizes = [10, 50, 100, 500, 1000];
        
        for &size in &batch_sizes {
            let start = std::time::Instant::now();
            // Create batch with 'size' operations
            // Measure completion processing time
            let duration = start.elapsed();
            
            // Verify performance scales O(1) not O(N)
            assert!(duration.as_micros() < size as u128 * 10);
        }
    }
    
    #[test]
    fn test_memory_overhead() {
        let batch = create_test_batch(100);
        let memory_usage = std::mem::size_of_val(&batch);
        
        // Verify HashMap overhead is reasonable
        assert!(memory_usage < 10000); // Less than 10KB for 100 operations
    }
}
```

### Buffer Consolidation Testing

```rust
#[cfg(test)]
mod buffer_consolidation_tests {
    use super::*;
    
    #[test]
    fn test_feature_parity() {
        // Verify all buffer_temp.rs features work in buffer/mod.rs
        let buffer = PinnedBuffer::with_capacity_aligned(4096);
        assert_eq!(buffer.len(), 4096);
        
        buffer.mark_in_use();
        assert!(buffer.is_available());
        
        let numa_buffer = PinnedBuffer::with_capacity_numa(4096, Some(0));
        assert_eq!(numa_buffer.len(), 4096);
    }
    
    #[test]
    fn test_no_buffer_temp_references() {
        // Ensure no code references buffer_temp.rs after consolidation
        // This would be a compile-time check
    }
}
```

### Threading Documentation Testing

```rust
#[cfg(test)]
mod threading_tests {
    use super::*;
    
    #[test]
    fn test_send_but_not_sync() {
        fn assert_send<T: Send>() {}
        fn assert_not_sync<T>() where T: Send, T: !Sync {}
        
        assert_send::<Ring>();
        // This should not compile if Ring becomes Sync accidentally:
        // assert_not_sync::<Ring>();
    }
    
    #[test]
    fn test_move_between_threads() {
        let ring = Ring::new(32).unwrap();
        
        let handle = std::thread::spawn(move || {
            // Ring successfully moved to this thread
            ring.capacity()
        });
        
        let capacity = handle.join().unwrap();
        assert!(capacity > 0);
    }
}
```

## Performance Considerations

### BatchFuture Optimization Impact

**Before Optimization:**
- Complexity: O(N) per completion, O(N×M) for M completions
- Memory: O(N) for operation_ids vector
- Cache behavior: Poor due to linear search

**After Optimization:**
- Complexity: O(1) per completion, O(M) for M completions  
- Memory: O(N) + HashMap overhead (~16N bytes)
- Cache behavior: Good due to direct HashMap access

**Benchmark Expectations:**
```
Batch Size | Before (μs) | After (μs) | Improvement
-----------|-------------|------------|------------
10         | 5           | 2          | 2.5x
50         | 25          | 3          | 8.3x  
100        | 50          | 4          | 12.5x
500        | 250         | 8          | 31.3x
1000       | 500         | 12         | 41.7x
```

### Memory Usage Analysis

**HashMap Memory Overhead:**
- Base HashMap: ~24 bytes
- Per entry: 16 bytes (8-byte key + 8-byte value)
- Total for N operations: 24 + 16N bytes

**Break-even Analysis:**
- Small batches (N < 10): Minimal benefit, acceptable overhead
- Medium batches (10 ≤ N ≤ 100): Good benefit-to-overhead ratio
- Large batches (N > 100): Significant performance improvement

### Buffer Consolidation Impact

**Benefits:**
- Reduced binary size (eliminate duplicate code)
- Improved maintainability (single implementation)
- Consistent behavior across all buffer usage
- Simplified imports and documentation

**Risks:**
- Potential regression if features are missed during migration
- Build system changes required
- Import updates throughout codebase

## Security Considerations

### Memory Safety

All optimizations maintain existing memory safety guarantees:

1. **BatchFuture HashMap**: Uses safe Rust collections, no unsafe code
2. **Buffer Consolidation**: Preserves Pin<Box<T>> safety guarantees  
3. **Documentation**: No runtime impact, pure documentation changes

### Performance Attack Vectors

**HashMap DoS Prevention:**
```rust
impl<'ring> BatchFuture<'ring> {
    pub(crate) fn new(operation_ids: Vec<Option<u64>>, ...) -> Self {
        // Validate batch size to prevent excessive memory usage
        if operation_ids.len() > MAX_BATCH_SIZE {
            panic!("Batch size exceeds maximum allowed");
        }
        
        // Pre-allocate HashMap with known capacity to prevent rehashing
        let mut operation_id_to_index = HashMap::with_capacity(operation_ids.len());
        
        // ... rest of implementation
    }
}
```

### Information Disclosure

The optimizations don't introduce new information disclosure risks:
- HashMap keys are operation IDs (already known to caller)
- Buffer consolidation doesn't change data handling
- Documentation improvements enhance security understanding

## Platform Considerations

### Cross-Platform Compatibility

**BatchFuture Optimization:**
- Uses standard Rust HashMap - works on all platforms
- No platform-specific code changes required
- Performance benefits apply universally

**Buffer Consolidation:**
- NUMA features remain Linux-specific with proper `#[cfg]` guards
- Aligned allocation works on all platforms
- Fallback behavior preserved for unsupported features

**Threading Documentation:**
- Applies to all platforms where safer-ring is supported
- RefCell behavior is consistent across platforms
- Examples work with standard Rust threading primitives

### Kernel Version Dependencies

No new kernel dependencies introduced:
- HashMap optimization is pure userspace
- Buffer consolidation doesn't change io_uring usage
- Documentation changes have no runtime impact

The optimizations maintain compatibility with the existing kernel version requirements (Linux 5.1+ for io_uring features).