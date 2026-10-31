use std::sync::Arc;

use foundation_core::io::buffer_pool::{BytesPool, BytesPoolConfig, PoolStatsSnapshot, PooledBuffer};

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

/// WHY: Feature 09 bounds pooled buffer size. A returned buffer larger than
/// `max_buffer_size` must be dropped, not pooled, so a pool never retains one
/// giant buffer.
/// WHAT: Grow a pooled buffer past the cap and drop it; the pool stays empty.
#[test]
fn bytes_pool_drops_oversized_buffers() {
    let config = BytesPoolConfig::new().with_max_buffer_size(128);
    let pool = Arc::new(BytesPool::with_config(64, 1, config));

    // Pre-allocated buffer is a hit; grow it past the 128-byte cap.
    let mut buf = pool.acquire();
    assert_eq!(pool.stats().pool_hits, 1);
    buf.resize(256, 0);
    drop(buf);

    // The oversized buffer was dropped, so the pool is empty: the next acquire
    // is an allocation, not a hit.
    let _buf2 = pool.acquire();
    let stats = pool.stats();
    assert_eq!(stats.pool_hits, 1, "oversized buffer must not be pooled");
    assert_eq!(stats.allocations, 1, "re-acquire after a drop is an allocation");
}

/// WHY: Feature 09 caps how many buffers a pool keeps. A buffer returned past
/// `max_pooled_buffers` must be dropped, not queued.
/// WHAT: Return 4 buffers into a pool capped at 2; only 2 come back as hits.
#[test]
fn bytes_pool_caps_pooled_count() {
    let config = BytesPoolConfig::new().with_max_pooled_buffers(2);
    let pool = Arc::new(BytesPool::with_config(64, 0, config));

    // Allocate and return 4 buffers (all allocations; no pre-alloc).
    let bufs: Vec<_> = (0..4).map(|_| pool.acquire()).collect();
    drop(bufs);

    // Acquire 4 more. Only the 2 buffers that fit the cap are hits.
    let _b1 = pool.acquire();
    let _b2 = pool.acquire();
    let _b3 = pool.acquire();
    let _b4 = pool.acquire();

    let stats = pool.stats();
    assert_eq!(stats.pool_hits, 2, "only 2 buffers may be pooled");
    assert_eq!(stats.allocations, 6, "4 originals + 2 re-allocations");
}

/// WHY: Feature 09 fixes `acquire_with_capacity` double-counting a too-small
/// pooled buffer as both a hit and an allocation. One acquire counts one
/// outcome: a hit only when the pooled buffer is used as-is, otherwise a miss
/// plus one allocation.
#[test]
fn bytes_pool_counts_one_outcome_per_acquire() {
    // Too-small: the pooled buffer is returned and a larger one is allocated.
    // This is one miss + one allocation, NOT a hit + an allocation.
    let pool = Arc::new(BytesPool::with_config(64, 1, BytesPoolConfig::new()));
    let buf = pool.acquire_with_capacity(4096);
    assert!(buf.capacity() >= 4096);
    let stats = pool.stats();
    assert_eq!(stats.pool_hits, 0, "too-small buffer must not count as a hit");
    assert_eq!(stats.allocations, 1, "too-small buffer is one allocation");

    // Large-enough: the pooled buffer is used as-is. One hit, no allocation.
    let pool = Arc::new(BytesPool::with_config(1024, 1, BytesPoolConfig::new()));
    let buf = pool.acquire_with_capacity(256);
    assert!(buf.capacity() >= 256);
    let stats = pool.stats();
    assert_eq!(stats.pool_hits, 1, "large-enough buffer is one hit");
    assert_eq!(stats.allocations, 0, "a hit allocates nothing");
}

/// WHY: Feature 09 makes `PooledBuffer::owned` hold no pool. It must report an
/// empty stats snapshot and return nothing on drop.
#[test]
fn owned_buffer_has_no_pool() {
    let owned = PooledBuffer::owned(vec![1, 2, 3]);
    let stats = owned.pool_stats();
    assert_eq!(stats, PoolStatsSnapshot::default());
    assert_eq!(stats.allocations, 0);
    assert_eq!(stats.pool_hits, 0);
}

/// WHY: Feature 09 adds `Debug` to `PooledBuffer` that shows length and
/// capacity, never the payload bytes (which may be a multi-GB body).
#[test]
fn pooled_buffer_debug_shows_len_and_capacity() {
    let pool = Arc::new(BytesPool::new(1024, 1));
    let mut buf = pool.acquire();
    buf.extend_from_slice(b"hello");

    let debug = format!("{buf:?}");
    assert!(debug.contains("len: 5"), "debug shows length, got {debug}");
    assert!(
        debug.contains("capacity: 1024"),
        "debug shows capacity, got {debug}"
    );
    assert!(!debug.contains("hello"), "debug must not dump bytes, got {debug}");
}
