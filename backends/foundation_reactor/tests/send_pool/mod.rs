//! `SendPool` buffer ownership tests (moved from `src/send_pool.rs`).

use std::io;

use foundation_reactor::send_pool::SendPool;

#[test]
fn checkout_fills_up_to_buf_size_and_reports_taken_len() {
    let pool = SendPool::new(4, 2);
    let buf = pool.checkout(b"hello").expect("checkout");
    // Only buf_size bytes are taken; the caller sends the rest separately.
    assert_eq!(buf.len(), 4);
    assert_eq!(buf.as_bytes(), b"hell");
}

#[test]
fn checkout_smaller_than_buf_size_takes_all() {
    let pool = SendPool::new(16, 2);
    let buf = pool.checkout(b"hi").expect("checkout");
    assert_eq!(buf.as_bytes(), b"hi");
    assert!(!buf.is_empty());
}

#[test]
fn drop_recycles_buffer_without_growing_the_pool() {
    let pool = SendPool::new(8, 1);
    {
        let _b = pool.checkout(b"first").expect("checkout");
        assert_eq!(pool.allocated(), 1);
    }
    // The buffer returned on drop; a second checkout reuses it, no new alloc.
    let _b = pool.checkout(b"second").expect("checkout reuse");
    assert_eq!(pool.allocated(), 1);
}

#[test]
fn exhaustion_returns_wouldblock_then_unblocks_on_return() {
    let pool = SendPool::new(8, 1);
    let held = pool.checkout(b"one").expect("first checkout");
    // At cap with the only buffer held out: the next checkout parks.
    let err = pool.checkout(b"two").expect_err("should be WouldBlock");
    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
    // Returning the buffer unblocks the next writer.
    drop(held);
    let _b = pool.checkout(b"two").expect("checkout after return");
}

#[test]
fn allocates_up_to_cap_across_concurrent_holders() {
    let pool = SendPool::new(8, 3);
    let a = pool.checkout(b"a").expect("a");
    let b = pool.checkout(b"b").expect("b");
    let c = pool.checkout(b"c").expect("c");
    assert_eq!(pool.allocated(), 3);
    assert!(pool.checkout(b"d").is_err());
    drop((a, b, c));
    // All three recycled; still capped at 3 allocations total.
    let _reuse = pool.checkout(b"reuse").expect("reuse");
    assert_eq!(pool.allocated(), 3);
}
