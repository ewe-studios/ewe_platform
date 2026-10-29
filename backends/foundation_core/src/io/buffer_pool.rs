//! Buffer pooling for zero-copy WebSocket frame processing.
//!
//! # WHY
//!
//! WebSocket frame reading allocates a buffer for each read. For high-throughput
//! applications, this causes significant allocation pressure and memory fragmentation.
//! Buffer pooling reuses allocated capacity, reducing syscalls and improving performance.
//!
//! # WHAT
//!
//! `BytesPool` maintains a concurrent queue of pre-allocated `Vec<u8>` buffers.
//! Users acquire buffers with `acquire()` and buffers are automatically returned
//! on drop via `PooledBuffer`.
//!
//! # HOW
//!
//! Uses `ConcurrentQueue` for lock-free multi-threaded access. Pre-allocates
//! buffers on creation. Buffers are cleared (but capacity retained) on return.
//!
//! # Thread Safety
//!
//! `BytesPool` is `Send + Sync`. Cloning produces a new `Arc` reference.

use concurrent_queue::ConcurrentQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Limits on a [`BytesPool`]. Limits are configuration, not constants
/// (CLAUDE.md rule 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytesPoolConfig {
    max_pooled_buffers: usize,
    max_buffer_size: usize,
}

impl BytesPoolConfig {
    /// Default for [`Self::max_pooled_buffers`]: 64 buffers. At 64 KiB each that
    /// is at most 4 MiB of retained memory, enough for bursty concurrent readers
    /// without holding an unbounded number of idle buffers.
    pub const DEFAULT_MAX_POOLED_BUFFERS: usize = 64;

    /// Default for [`Self::max_buffer_size`]: 64 KiB. This is the upper end of
    /// Go's `bufio` default buffer range (4 KiB to 64 KiB); a single pooled read
    /// buffer rarely needs more, and a larger allocation is not worth pooling.
    pub const DEFAULT_MAX_BUFFER_SIZE: usize = 64 * 1024;

    /// A config with the default limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_pooled_buffers: Self::DEFAULT_MAX_POOLED_BUFFERS,
            max_buffer_size: Self::DEFAULT_MAX_BUFFER_SIZE,
        }
    }

    /// The most buffers the pool keeps. A buffer returned past this count is
    /// dropped (deallocated) instead of queued.
    #[must_use]
    pub fn with_max_pooled_buffers(mut self, count: usize) -> Self {
        self.max_pooled_buffers = count;
        self
    }

    /// The largest buffer the pool keeps. A returned buffer larger than this is
    /// dropped instead of queued.
    #[must_use]
    pub fn with_max_buffer_size(mut self, bytes: usize) -> Self {
        self.max_buffer_size = bytes;
        self
    }

    /// The configured maximum number of pooled buffers.
    #[must_use]
    pub fn max_pooled_buffers(&self) -> usize {
        self.max_pooled_buffers
    }

    /// The configured maximum buffer size in bytes.
    #[must_use]
    pub fn max_buffer_size(&self) -> usize {
        self.max_buffer_size
    }
}

impl Default for BytesPoolConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// A pool of reusable `Vec<u8>` buffers.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use foundation_core::io::buffer_pool::BytesPool;
///
/// // Create pool with 8KB buffers, pre-allocate 4
/// let pool = Arc::new(BytesPool::new(8192, 4));
///
/// // Acquire a buffer (automatically returned on drop)
/// let buf = pool.acquire();
/// // ... use buf ...
/// // buf.drop() automatically returns buffer to pool
/// ```
pub struct BytesPool {
    pub(crate) queue: ConcurrentQueue<Vec<u8>>,
    default_capacity: usize,
    max_buffer_size: usize,
    stats: PoolStats,
}

struct PoolStats {
    allocations: AtomicUsize,
    pool_hits: AtomicUsize,
}

impl BytesPool {
    /// Create a new pool with pre-allocated buffers and the default limits.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Capacity of each buffer in bytes
    /// * `prealloc` - Number of buffers to pre-allocate
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use foundation_core::io::buffer_pool::BytesPool;
    ///
    /// let pool = Arc::new(BytesPool::new(8192, 4));
    /// ```
    #[must_use]
    pub fn new(capacity: usize, prealloc: usize) -> Self {
        Self::with_config(capacity, prealloc, BytesPoolConfig::default())
    }

    /// Create a new pool with explicit limits.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Capacity of each buffer in bytes
    /// * `prealloc` - Number of buffers to pre-allocate
    /// * `config` - The pool limits
    #[must_use]
    pub fn with_config(capacity: usize, prealloc: usize, config: BytesPoolConfig) -> Self {
        let queue = ConcurrentQueue::bounded(config.max_pooled_buffers);
        // Pre-allocation is best-effort: past the queue bound the extra buffers
        // are dropped rather than panicking.
        for _ in 0..prealloc {
            queue.push(Vec::with_capacity(capacity)).ok();
        }
        Self {
            queue,
            default_capacity: capacity,
            max_buffer_size: config.max_buffer_size,
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
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use foundation_core::io::buffer_pool::BytesPool;
    ///
    /// let pool = Arc::new(BytesPool::new(8192, 4));
    /// let mut buf = pool.acquire();
    /// buf.extend_from_slice(b"hello");
    /// // buf automatically returned to pool when dropped
    /// ```
    #[must_use]
    pub fn acquire(self: &Arc<Self>) -> PooledBuffer {
        if let Ok(mut buf) = self.queue.pop() {
            self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
            buf.clear();
            PooledBuffer::new(buf, Some(self.clone()))
        } else {
            self.stats.allocations.fetch_add(1, Ordering::Relaxed);
            PooledBuffer::new(Vec::with_capacity(self.default_capacity), Some(self.clone()))
        }
    }

    /// Acquire a buffer with specific capacity (not pool default).
    ///
    /// If the pooled buffer has insufficient capacity, a new buffer is allocated
    /// and the pooled buffer is returned to the queue. A pooled buffer counts as
    /// one hit only when it is used as-is; a too-small buffer counts as one
    /// miss plus one allocation.
    ///
    /// Useful when you know the exact size needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use foundation_core::io::buffer_pool::BytesPool;
    ///
    /// let pool = Arc::new(BytesPool::new(8192, 4));
    /// let mut buf = pool.acquire_with_capacity(256);
    /// ```
    #[must_use]
    pub fn acquire_with_capacity(self: &Arc<Self>, capacity: usize) -> PooledBuffer {
        if let Ok(mut buf) = self.queue.pop() {
            if buf.capacity() >= capacity {
                // Pooled buffer is large enough: use it as-is.
                buf.clear();
                self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
                PooledBuffer::new(buf, Some(self.clone()))
            } else {
                // Pooled buffer too small: return it and allocate a larger one.
                self.queue.push(buf).ok();
                self.stats.allocations.fetch_add(1, Ordering::Relaxed);
                PooledBuffer::new(Vec::with_capacity(capacity), Some(self.clone()))
            }
        } else {
            self.stats.allocations.fetch_add(1, Ordering::Relaxed);
            PooledBuffer::new(Vec::with_capacity(capacity), Some(self.clone()))
        }
    }

    /// Return statistics about pool usage.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use foundation_core::io::buffer_pool::BytesPool;
    ///
    /// let pool = Arc::new(BytesPool::new(8192, 4));
    /// let stats = pool.stats();
    /// println!("Hit ratio: {}", stats.hit_ratio());
    /// ```
    #[must_use]
    pub fn stats(&self) -> PoolStatsSnapshot {
        PoolStatsSnapshot {
            allocations: self.stats.allocations.load(Ordering::Relaxed),
            pool_hits: self.stats.pool_hits.load(Ordering::Relaxed),
        }
    }

    /// Get the default buffer capacity in bytes.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.default_capacity
    }

    /// The largest buffer the pool keeps (bytes).
    #[must_use]
    pub fn max_buffer_size(&self) -> usize {
        self.max_buffer_size
    }

    /// The most buffers the pool keeps.
    #[must_use]
    pub fn max_pooled_buffers(&self) -> usize {
        self.queue.capacity().unwrap_or(usize::MAX)
    }
}

/// Statistics snapshot for a `BytesPool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolStatsSnapshot {
    /// Number of times a new buffer was allocated (pool miss)
    pub allocations: usize,
    /// Number of times a buffer was reused from the pool (pool hit)
    pub pool_hits: usize,
}

impl PoolStatsSnapshot {
    /// Hit ratio: `pool_hits` / (`allocations` + `pool_hits`)
    ///
    /// Returns 1.0 if no operations have occurred.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_ratio(&self) -> f64 {
        let total = self.allocations + self.pool_hits;
        if total == 0 {
            return 1.0;
        }
        self.pool_hits as f64 / total as f64
    }
}

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
    pool: Option<Arc<BytesPool>>,
}

impl PooledBuffer {
    fn new(buffer: Vec<u8>, pool: Option<Arc<BytesPool>>) -> Self {
        Self { buffer, pool }
    }

    /// Create an owned buffer (not from pool).
    ///
    /// Useful for callers who want pool-like API without pooling.
    /// Buffer is NOT returned to any pool on drop.
    #[must_use]
    pub fn owned(buffer: Vec<u8>) -> Self {
        Self { buffer, pool: None }
    }

    /// Get the pool statistics associated with this buffer.
    ///
    /// An owned buffer has no pool, so it returns an empty snapshot.
    #[must_use]
    pub fn pool_stats(&self) -> PoolStatsSnapshot {
        match &self.pool {
            Some(pool) => pool.stats(),
            None => PoolStatsSnapshot::default(),
        }
    }
}

impl std::fmt::Debug for PooledBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledBuffer")
            .field("len", &self.buffer.len())
            .field("capacity", &self.buffer.capacity())
            .finish()
    }
}

impl std::ops::Deref for PooledBuffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl std::ops::DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        let Some(pool) = self.pool.take() else {
            return;
        };
        // Return buffer to pool (ignore errors - queue might be dropped).
        self.buffer.clear();
        // Drop buffers over the size cap instead of pooling them.
        if self.buffer.capacity() > pool.max_buffer_size {
            return;
        }
        pool.queue.push(std::mem::take(&mut self.buffer)).ok();
    }
}

impl AsRef<[u8]> for PooledBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.buffer
    }
}

impl AsMut<[u8]> for PooledBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
}
