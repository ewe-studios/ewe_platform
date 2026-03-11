# Bytes Fundamentals

**Purpose**: Understand the internal workings of `bytes::Bytes` and `BytesMut` to inform our custom implementation and buffer pooling strategy for WebSocket Phase 3.

**Date**: 2026-03-11

---

## Table of Contents

1. [Overview](#1-overview)
2. [bytes::Bytes Internal Structure](#2-bytesbytes-internal-structure)
3. [bytes::BytesMut Internal Structure](#3-bytesbytesmut-internal-structure)
4. [Arc-Based Sharing Patterns](#4-arc-based-sharing-patterns)
5. [no_std Considerations](#5-no_std-considerations)
6. [Design Decisions for Our Implementation](#6-design-decisions-for-our-implementation)
7. [Reference Implementation](#7-reference-implementation)

---

## 1. Overview

### Why This Matters for WebSocket

WebSocket frame processing requires efficient buffer management:

- **Zero-copy**: Avoid copying payload data between decode and application
- **Shared buffers**: Multiple readers (frame decoder, message assembler) access same data
- **Pooling**: Reuse allocated memory to reduce allocations in high-throughput scenarios
- **no_std**: Must work without `std::alloc` in embedded/Wasm environments

The `bytes` crate (version 1.x) provides battle-tested patterns we can learn from and adapt.

### Key Concepts

| Concept | Description | WebSocket Use Case |
|---------|-------------|-------------------|
| **Reference counting** | `Arc` tracks shared buffer ownership | Multiple frames reference same underlying buffer |
| **Slicing without copy** | Pointer/length adjustments, not data copy | Extract frame payload without allocation |
| **Buffer pooling** | Reuse allocated `Vec<u8>` capacity | Reduce syscalls for repeated frame reads |
| **Split capabilities** | Separate read/write handles | Full-duplex WebSocket communication |

---

## 2. bytes::Bytes Internal Structure

### Memory Layout

```
┌─────────────────────────────────────────────────────────────┐
│                     Shared Buffer                           │
│  ┌─────────────┬─────────────┬─────────────────────────┐   │
│  │   AtomicArc │     Len     │    Data [u8; N]         │   │
│  │  (refcount) │  (usize)    │                         │   │
│  └─────────────┴─────────────┴─────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
         ▲                    ▲                    ▲
         │                    │                    │
         │                    │                    └─ Actual bytes
         │                    │
    Atomic refcount      Total length
```

### Bytes Struct (Simplified)

```rust
pub struct Bytes {
    ptr: *const u8,           // Pointer to start of THIS slice
    len: usize,               // Length of THIS slice
    data: AtomicArc<Shared>,  // Reference-counted shared state
}

struct Shared {
    len: usize,               // Total buffer length
    buf: *mut u8,             // Pointer to start of ALLOCATION
    // Followed by atomic refcount and optional drop fn
}
```

### Key Operations

#### Clone (Zero-Copy)

```rust
impl Clone for Bytes {
    fn clone(&self) -> Self {
        // Increment refcount only - no data copy
        self.data.ref_increment();
        Bytes {
            ptr: self.ptr,
            len: self.len,
            data: self.data.clone(),
        }
    }
}
```

**Cost**: O(1) - atomic increment only

#### Slice (Zero-Copy)

```rust
impl Bytes {
    pub fn slice(&self, range: Range<usize>) -> Self {
        // Adjust pointer and length, share same backing buffer
        Bytes {
            ptr: unsafe { self.ptr.add(range.start) },
            len: range.end - range.start,
            data: self.data.clone(),
        }
    }
}
```

**Cost**: O(1) - pointer arithmetic + refcount increment

#### Drop

```rust
impl Drop for Bytes {
    fn drop(&mut self) {
        if self.data.ref_decrement() == 0 {
            // Last reference - free the buffer
            unsafe {
                dealloc(self.data.buf, self.data.layout);
            }
        }
    }
}
```

### Design Insights

1. **Pointer to middle**: `ptr` can point anywhere in buffer, enabling zero-copy slices
2. **Shared metadata**: `Shared` struct lives in allocation, tracked by `Arc`
3. **Atomic refcount**: Thread-safe sharing without locks
4. **No copy on clone/slice**: All operations are O(1) pointer math

---

## 3. bytes::BytesMut Internal Structure

### Purpose

`BytesMut` is a **mutable** buffer that can grow. When shared, it converts to `Bytes`.

### Memory Layout

```
┌─────────────────────────────────────────────────────────────┐
│                   BytesMut Buffer                           │
│  ┌──────────┬──────────┬──────────┬─────────────────────┐  │
│  │  Position│  Capacity│   Len    │   Data [u8; CAP]    │  │
│  │  (usize) │  (usize) │  (usize) │                     │  │
│  └──────────┴──────────┴──────────┴─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### BytesMut Struct (Simplified)

```rust
pub struct BytesMut {
    ptr: *mut u8,      // Pointer to current position
    len: usize,        // Initialized data length
    cap: usize,        // Total capacity
    data: Option<Arc<Shared>>,  // None = unique ownership
}
```

### Key Operations

#### Write (Extend)

```rust
impl BufMut for BytesMut {
    fn put_slice(&mut self, src: &[u8]) {
        // Ensure capacity (may reallocate)
        self.reserve(src.len());

        // Copy data
        unsafe {
            ptr::copy_nonoverlapping(
                src.as_ptr(),
                self.ptr.add(self.len),
                src.len(),
            );
        }
        self.len += src.len();
    }
}
```

#### Freeze (Convert to Bytes)

```rust
impl BytesMut {
    pub fn freeze(self) -> Bytes {
        // Consume BytesMut, return immutable Bytes
        Bytes {
            ptr: self.ptr,
            len: self.len,
            data: self.data.unwrap(),
        }
        // BytesMut is consumed - no more mutations
    }
}
```

#### Split

```rust
impl BytesMut {
    pub fn split_off(&mut self, at: usize) -> Self {
        let new_cap = self.len - at;
        let new_buf = BytesMut {
            ptr: unsafe { self.ptr.add(at) },
            len: new_cap,
            cap: self.cap - at,
            data: self.data.clone(),
        };
        self.len = at;
        new_buf
    }
}
```

### Design Insights

1. **Unique vs Shared**: `data: None` means unique ownership (can grow in-place)
2. **Copy-on-write**: First clone converts to shared `Bytes`
3. **Split without copy**: Like `slice`, but returns mutable `BytesMut`
4. **Capacity tracking**: `cap` tracks allocation size, `len` tracks initialized data

---

## 4. Arc-Based Sharing Patterns

### Pattern 1: Simple Shared Buffer

```rust
use std::sync::Arc;

struct SharedBuffer {
    data: Vec<u8>,
}

#[derive(Clone)]
pub struct SharedBytes {
    buffer: Arc<SharedBuffer>,
    offset: usize,
    len: usize,
}

impl SharedBytes {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            buffer: Arc::new(SharedBuffer { data }),
            offset: 0,
            len: Arc::new(SharedBuffer { data }).data.len(),
        }
    }

    pub fn slice(&self, range: Range<usize>) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
            offset: self.offset + range.start,
            len: range.len(),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buffer.data[self.offset..self.offset + self.len]
    }
}
```

### Pattern 2: Buffer Pool with RAII Return

```rust
use std::sync::Arc;
use concurrent_queue::ConcurrentQueue;

pub struct BytesPool {
    queue: ConcurrentQueue<Vec<u8>>,
    initial_capacity: usize,
}

impl BytesPool {
    pub fn new(initial_capacity: usize) -> Self {
        let queue = ConcurrentQueue::unbounded();
        // Pre-allocate some buffers
        for _ in 0..4 {
            queue.push(Vec::with_capacity(initial_capacity)).ok();
        }
        Self { queue, initial_capacity }
    }

    pub fn acquire(&self) -> PooledBuffer {
        let buffer = self.queue.pop()
            .unwrap_or_else(|_| Vec::with_capacity(self.initial_capacity));
        PooledBuffer {
            buffer,
            pool: self,
        }
    }

    fn return_buffer(&self, mut buffer: Vec<u8>) {
        buffer.clear();
        self.queue.push(buffer).ok();
    }
}

pub struct PooledBuffer<'a> {
    buffer: Vec<u8>,
    pool: &'a BytesPool,
}

impl<'a> Drop for PooledBuffer<'a> {
    fn drop(&mut self) {
        self.pool.return_buffer(std::mem::take(&mut self.buffer));
    }
}

impl<'a> std::ops::Deref for PooledBuffer<'a> {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target { &self.buffer }
}

impl<'a> std::ops::DerefMut for PooledBuffer<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.buffer }
}
```

### Pattern 3: User-Owned Pool (Arc<BytesPool>)

```rust
/// User creates and owns the pool, passes Arc to stream readers.
/// This gives users control over memory allocation strategy.

pub struct SharedByteBufferStream<S> {
    inner: Arc<Mutex<BufferedStream<S>>>,
    pool: Option<Arc<BytesPool>>,  // User-provided pool
}

impl<S: Read> SharedByteBufferStream<S> {
    /// User supplies the buffer - zero-copy into their pool
    pub fn read_into_bytes(
        &self,
        buf: &mut bytes::BytesMut,
        max_len: usize,
    ) -> io::Result<usize> {
        // Read directly into user's BytesMut
        // No intermediate allocation
    }

    /// Pool-based read - uses internal pool if provided
    pub fn read_pooled_buffer(
        &self,
        max_len: usize,
    ) -> io::Result<Option<PooledBuffer>> {
        let mut pool_buf = match self.pool {
            Some(ref pool) => pool.acquire(),
            None => PooledBuffer::owned(Vec::with_capacity(max_len)),
        };
        pool_buf.resize(max_len, 0);
        let n = self.read_exact_or_partial(&mut pool_buf)?;
        pool_buf.truncate(n);
        Ok(Some(pool_buf))
    }
}
```

---

## 5. no_std Considerations

### What We Lose Without std

| Feature | std | no_std (core only) | no_std (with alloc) |
|---------|-----|-------------------|---------------------|
| `Vec<u8>` | ✓ | ✗ | ✓ |
| `Box<T>` | ✓ | ✗ | ✓ |
| `Arc<T>` | ✓ | ✗ | ✓ |
| `Mutex<T>` | ✓ | ✗ | ✓ (with spinlock) |
| `String` | ✓ | ✗ | ✓ |
| Global allocator | ✓ | ✗ | Must be provided |
| Panics | ✓ | ✓ | ✓ |

### no_std Requirements

For `foundation_nostd` or `foundation_core` in no_std mode:

```rust
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
```

### Custom Allocator Hook

```rust
// In no_std, user must provide global allocator:

#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator;

struct MyAllocator;

impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Custom allocation logic
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Custom deallocation logic
    }
}
```

### Our Design Target

Support both modes:

```rust
#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;

#[cfg(not(feature = "std"))]
extern crate alloc;
```

---

## 6. Design Decisions for Our Implementation

### Decision 1: Use bytes Crate Directly

**Pros:**
- Battle-tested, widely used (Tokio, Hyper, Tower depend on it)
- Zero-copy by design
- no_std compatible (with `alloc` feature)
- Well-documented

**Cons:**
- External dependency (but already used in ecosystem)
- May have features we don't need

**Decision**: Add `bytes = "1.5"` to `foundation_core/Cargo.toml`

### Decision 2: User-Owned Pool Pattern

Users create and own `Arc<BytesPool>`, pass to readers:

```rust
let pool = Arc::new(BytesPool::new(8192));
let stream = SharedByteBufferStream::with_pool(inner, Some(pool.clone()));
```

**Why:**
- Users control memory strategy
- No hidden allocations
- Easy to test with mock pools
- Consistent with project's explicit-resource pattern

### Decision 3: RAII Buffer Return

`PooledBuffer` automatically returns to pool on drop:

```rust
pub struct PooledBuffer {
    buffer: Vec<u8>,
    pool: Arc<BytesPool>,
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        self.pool.return_buffer(std::mem::take(&mut self.buffer));
    }
}
```

**Why:**
- Impossible to leak buffers (forgetting pool membership)
- Clear ownership semantics
- No manual `return_to_pool()` calls

### Decision 4: Read Methods

```rust
// User supplies buffer (most flexible)
fn read_into_bytes(&self, buf: &mut BytesMut, max_len: usize) -> io::Result<usize>;

// Pool-based read (convenient)
fn read_pooled_buffer(&self, max_len: usize) -> io::Result<Option<PooledBuffer>>;

// Exact read with pool (for known-length frames)
fn read_exact_into_bytes(&self, len: usize) -> io::Result<BytesMut>;
```

---

## 7. Reference Implementation

### BytesPool

```rust
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;

/// A pool of reusable `Vec<u8>` buffers.
///
/// # WHY
///
/// WebSocket frame reading allocates a buffer for each read. For high-throughput
/// applications, this causes significant allocation pressure. Buffer pooling reuses
/// allocated capacity, reducing syscalls and memory fragmentation.
///
/// # WHAT
///
/// `BytesPool` maintains a concurrent queue of pre-allocated `Vec<u8>` buffers.
/// Users acquire buffers with `acquire()` and buffers are automatically returned
/// on drop via `PooledBuffer`.
///
/// # HOW
///
/// Uses `ConcurrentQueue` for lock-free multi-threaded access. Pre-allocates
/// buffers on creation. Buffers are cleared (but capacity retained) on return.
///
/// # Thread Safety
///
/// `BytesPool` is `Send + Sync`. Cloning produces a new `Arc` reference.
pub struct BytesPool {
    queue: ConcurrentQueue<Vec<u8>>,
    capacity: usize,
    stats: PoolStats,
}

struct PoolStats {
    allocations: AtomicUsize,
    pool_hits: AtomicUsize,
}

impl BytesPool {
    /// Create a new pool with pre-allocated buffers.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Capacity of each buffer in bytes
    /// * `prealloc` - Number of buffers to pre-allocate
    pub fn new(capacity: usize, prealloc: usize) -> Self {
        let queue = ConcurrentQueue::unbounded();
        for _ in 0..prealloc {
            queue.push(Vec::with_capacity(capacity)).ok();
        }
        Self {
            queue,
            capacity,
            stats: PoolStats {
                allocations: AtomicUsize::new(0),
                pool_hits: AtomicUsize::new(0),
            },
        }
    }

    /// Acquire a buffer from the pool.
    ///
    /// Returns a `PooledBuffer` that will automatically return the buffer
    /// to the pool on drop.
    pub fn acquire(self: &Arc<Self>) -> PooledBuffer {
        match self.queue.pop() {
            Ok(mut buf) => {
                self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
                buf.clear();
                PooledBuffer::new(buf, self.clone())
            }
            Err(_) => {
                self.stats.allocations.fetch_add(1, Ordering::Relaxed);
                PooledBuffer::new(Vec::with_capacity(self.capacity), self.clone())
            }
        }
    }

    /// Return statistics about pool usage.
    pub fn stats(&self) -> PoolStatsSnapshot {
        PoolStatsSnapshot {
            allocations: self.stats.allocations.load(Ordering::Relaxed),
            pool_hits: self.stats.pool_hits.load(Ordering::Relaxed),
        }
    }
}

pub struct PoolStatsSnapshot {
    pub allocations: usize,
    pub pool_hits: usize,
}

impl PoolStatsSnapshot {
    /// Hit ratio: pool_hits / (allocations + pool_hits)
    pub fn hit_ratio(&self) -> f64 {
        let total = self.allocations + self.pool_hits;
        if total == 0 { return 1.0; }
        self.pool_hits as f64 / total as f64
    }
}
```

### PooledBuffer

```rust
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// RAII wrapper for a pooled buffer.
///
/// # WHY
///
/// Ensures buffers are always returned to the pool, even on panic or early return.
/// Prevents memory leaks from forgotten `return_to_pool()` calls.
///
/// # WHAT
///
/// Wraps a `Vec<u8>` with a reference to its pool. On drop, clears and returns
/// the buffer to the pool's queue.
///
/// # Deref Behavior
///
/// Implements `Deref<Target = Vec<u8>>` and `DerefMut` for transparent access.
pub struct PooledBuffer {
    buffer: Vec<u8>,
    pool: Arc<BytesPool>,
}

impl PooledBuffer {
    fn new(buffer: Vec<u8>, pool: Arc<BytesPool>) -> Self {
        Self { buffer, pool }
    }

    /// Create an owned buffer (not from pool).
    ///
    /// Useful for callers who want pool-like API without pooling.
    pub fn owned(buffer: Vec<u8>) -> Self {
        // Creates a dummy pool for API consistency
        Self {
            buffer,
            pool: Arc::new(BytesPool::new(buffer.capacity(), 0)),
        }
    }
}

impl Deref for PooledBuffer {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target { &self.buffer }
}

impl DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.buffer }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        // Return buffer to pool (ignore errors - queue might be dropped)
        self.buffer.clear();
        self.pool.queue.push(std::mem::take(&mut self.buffer)).ok();
    }
}
```

### Stream Extension

```rust
use bytes::BytesMut;

impl<S: Read> SharedByteBufferStream<S> {
    /// Read into a user-supplied `BytesMut`.
    ///
    /// # WHY
    ///
    /// Allows zero-copy reads when caller owns the buffer. No intermediate
    /// allocation - data goes directly into caller's buffer.
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to read into (must have capacity)
    /// * `max_len` - Maximum bytes to read
    ///
    /// # Returns
    ///
    /// Number of bytes read (0 = EOF, >0 = data available)
    pub fn read_into_bytes(
        &self,
        buf: &mut BytesMut,
        max_len: usize,
    ) -> io::Result<usize> {
        let available = buf.spare_capacity_mut();
        let to_read = max_len.min(available.len());
        let slice = &mut available[..to_read];

        let ptr = slice.as_mut_ptr() as *mut u8;
        let n = unsafe {
            use std::io::Read;
            let mut reader = self.inner.lock().unwrap();
            reader.read(std::slice::from_raw_parts_mut(ptr, to_read))?
        };

        unsafe {
            buf.set_len(buf.len() + n);
        }
        Ok(n)
    }

    /// Read exactly `len` bytes into a pooled buffer.
    ///
    /// # Panics
    ///
    /// Panics if EOF is reached before `len` bytes.
    pub fn read_exact_into_bytes(&self, len: usize) -> io::Result<BytesMut> {
        let mut buf = BytesMut::with_capacity(len);
        buf.resize(len, 0);

        {
            use std::io::Read;
            let mut reader = self.inner.lock().unwrap();
            reader.read_exact(&mut buf)?;
        }

        Ok(buf)
    }
}
```

---

## Summary

| Concept | Takeaway |
|---------|----------|
| `bytes::Bytes` | Pointer + length + Arc - zero-copy clone/slice |
| `bytes::BytesMut` | Mutable buffer, converts to `Bytes` on share |
| Buffer pooling | `ConcurrentQueue<Vec<u8>>` with RAII return |
| User-owned pool | `Arc<BytesPool>` passed to readers |
| no_std | Use `alloc` crate, avoid `std::sync` |
| Our design | Use `bytes` crate + custom pool |

---

_Next: Implement `BytesPool` and `PooledBuffer` in `foundation_core/src/io/buffer_pool.rs`_
