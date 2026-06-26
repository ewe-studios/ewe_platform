use std::sync::Arc;

use foundation_core::io::buffer_pool::{BytesPool, PooledBuffer};

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
