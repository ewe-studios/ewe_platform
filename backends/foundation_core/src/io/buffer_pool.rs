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
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use foundation_core::io::buffer_pool::BytesPool;
    ///
    /// let pool = Arc::new(BytesPool::new(8192, 4));
    /// ```
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

    /// Acquire a buffer with specific capacity (not pool default).
    ///
    /// If the pooled buffer has insufficient capacity, a new buffer is allocated
    /// and the pooled buffer is returned to the queue.
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
        match self.queue.pop() {
            Ok(mut buf) => {
                self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
                buf.clear();
                // Ensure capacity is at least the requested amount
                if buf.capacity() < capacity {
                    // Pooled buffer too small - return it to pool and allocate new one
                    self.queue.push(buf).ok();
                    self.stats.allocations.fetch_add(1, Ordering::Relaxed);
                    PooledBuffer::new(Vec::with_capacity(capacity), self.clone())
                } else {
                    PooledBuffer::new(buf, self.clone())
                }
            }
            Err(_) => {
                self.stats.allocations.fetch_add(1, Ordering::Relaxed);
                PooledBuffer::new(Vec::with_capacity(capacity), self.clone())
            }
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

    /// Get the default buffer capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Statistics snapshot for a `BytesPool`.
#[derive(Debug, Clone, Copy)]
pub struct PoolStatsSnapshot {
    /// Number of times a new buffer was allocated (pool miss)
    pub allocations: usize,
    /// Number of times a buffer was reused from the pool (pool hit)
    pub pool_hits: usize,
}

impl PoolStatsSnapshot {
    /// Hit ratio: pool_hits / (allocations + pool_hits)
    ///
    /// Returns 1.0 if no operations have occurred.
    #[must_use]
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
    pool: Arc<BytesPool>,
}

impl PooledBuffer {
    fn new(buffer: Vec<u8>, pool: Arc<BytesPool>) -> Self {
        Self { buffer, pool }
    }

    /// Create an owned buffer (not from pool).
    ///
    /// Useful for callers who want pool-like API without pooling.
    /// Buffer is NOT returned to any pool on drop.
    #[must_use]
    pub fn owned(buffer: Vec<u8>) -> Self {
        // Create a dummy pool with the buffer's capacity
        let capacity = buffer.capacity();
        Self {
            buffer,
            pool: Arc::new(BytesPool::new(capacity, 0)),
        }
    }

    /// Get the pool statistics associated with this buffer.
    #[must_use]
    pub fn pool_stats(&self) -> PoolStatsSnapshot {
        self.pool.stats()
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
        // Return buffer to pool (ignore errors - queue might be dropped)
        self.buffer.clear();
        self.pool.queue.push(std::mem::take(&mut self.buffer)).ok();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// WHY: Pool should reuse buffers to reduce allocations
    /// WHAT: Acquiring and dropping buffers should return them to pool
    fn test_pool_reuses_buffers() {
        let pool = Arc::new(BytesPool::new(1024, 2));

        // Initial stats - no operations yet
        let stats = pool.stats();
        assert_eq!(stats.allocations, 0);
        assert_eq!(stats.pool_hits, 0);

        // Acquire first buffer (should come from pre-allocated)
        let buf1 = pool.acquire();
        let stats1 = pool.stats();
        assert_eq!(stats1.pool_hits, 1);
        assert_eq!(stats1.allocations, 0);

        // Acquire second buffer (should come from pre-allocated)
        let _buf2 = pool.acquire();
        let stats2 = pool.stats();
        assert_eq!(stats2.pool_hits, 2);
        assert_eq!(stats2.allocations, 0);

        // Acquire third buffer (pool empty, must allocate)
        let _buf3 = pool.acquire();
        let stats3 = pool.stats();
        assert_eq!(stats3.pool_hits, 2);
        assert_eq!(stats3.allocations, 1);

        // Drop buf1 - returns to pool
        drop(buf1);

        // Acquire fourth buffer (should reuse returned buffer)
        let _buf4 = pool.acquire();
        let stats4 = pool.stats();
        assert_eq!(stats4.pool_hits, 3);
        assert_eq!(stats4.allocations, 1);
    }

    #[test]
    /// WHY: Buffers should be cleared when returned to pool
    /// WHAT: Data from previous use should not leak
    fn test_buffer_cleared_on_return() {
        let pool = Arc::new(BytesPool::new(1024, 1));

        // Acquire, write data, drop
        {
            let mut buf = pool.acquire();
            buf.extend_from_slice(b"hello world");
            assert_eq!(&buf[..], b"hello world");
        }

        // Acquire again - should be cleared
        let buf2 = pool.acquire();
        assert!(buf2.is_empty());
        assert_eq!(buf2.capacity(), 1024); // Capacity retained
    }

    #[test]
    /// WHY: Pool should handle concurrent access safely
    /// WHAT: Multiple threads can acquire/return buffers
    fn test_concurrent_access() {
        use std::thread;

        let pool = Arc::new(BytesPool::new(1024, 10));
        let mut handles = vec![];

        for i in 0..10 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                let mut buf = pool_clone.acquire();
                buf.extend_from_slice(format!("thread {}", i).as_bytes());
                // Buf returned on drop
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All buffers should be returned
        let stats = pool.stats();
        assert_eq!(stats.pool_hits, 10);
    }

    #[test]
    /// WHY: Hit ratio should calculate correctly
    /// WHAT: hit_ratio = pool_hits / (allocations + pool_hits)
    fn test_hit_ratio() {
        let pool = Arc::new(BytesPool::new(1024, 1));

        let buf1 = pool.acquire(); // hit from pre-allocated
        drop(buf1); // return to pool

        let _buf2 = pool.acquire(); // hit from returned buf1
        let _buf3 = pool.acquire(); // allocation (pool empty)
        let _buf4 = pool.acquire(); // allocation

        let stats = pool.stats();
        assert_eq!(stats.pool_hits, 2);
        assert_eq!(stats.allocations, 2);
        assert!((stats.hit_ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    /// WHY: Empty pool should have 1.0 hit ratio (no operations)
    /// WHAT: Default hit_ratio is 1.0
    fn test_hit_ratio_empty() {
        let pool = Arc::new(BytesPool::new(1024, 0));
        let stats = pool.stats();
        assert_eq!(stats.hit_ratio(), 1.0);
    }

    #[test]
    /// WHY: PooledBuffer should deref to Vec<u8> transparently
    /// WHAT: All Vec methods should work
    fn test_pooled_buffer_deref() {
        let pool = Arc::new(BytesPool::new(1024, 1));
        let mut buf = pool.acquire();

        // Test DerefMut
        buf.push(42);
        buf.extend_from_slice(&[1, 2, 3]);

        // Test Deref
        assert_eq!(buf.len(), 4);
        assert_eq!(buf[0], 42);
    }

    #[test]
    /// WHY: Owned buffer should not use pool
    /// WHAT: No pool stats should change
    fn test_owned_buffer() {
        let pool = Arc::new(BytesPool::new(1024, 1));
        let stats_before = pool.stats();

        let mut owned = PooledBuffer::owned(Vec::with_capacity(512));
        owned.push(1);

        let stats_after = pool.stats();
        // Stats should be unchanged
        assert_eq!(stats_before.allocations, stats_after.allocations);
        assert_eq!(stats_before.pool_hits, stats_after.pool_hits);
    }

    #[test]
    /// WHY: acquire_with_capacity should respect requested size
    /// WHAT: Buffer should have at least requested capacity
    fn test_acquire_with_capacity() {
        let pool = Arc::new(BytesPool::new(1024, 1));

        let buf = pool.acquire_with_capacity(4096);
        assert!(buf.capacity() >= 4096);
    }
}
