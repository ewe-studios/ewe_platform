//! User-filled buffer pool for `IORING_OP_SEND` (F49 — Decision 14 F4 SEND half).
//!
//! WHY: `IORING_OP_SEND` reads from a buffer *we* own and must not touch, move,
//! or free until the completion says the bytes are on the wire. A `&[u8]`
//! borrowed from a transport's response buffer satisfies `write(2)` — which
//! copies synchronously and returns — but not an async kernel send: the borrow
//! ends long before the CQE. So the write path needs owned buffers with stable
//! addresses, checked out for the kernel's duration and returned when it is done.
//!
//! WHAT: [`SendPool`] owns a set of fixed-size byte buffers. [`SendBuf`] is one
//! buffer checked out and filled with the caller's bytes; while it is alive the
//! kernel may read from it, and dropping it returns the buffer to the pool.
//!
//! HOW: this is the SEND mirror of the RECV [`BufRing`](super::super::poll::sys::unix::selector::bufring).
//! The crucial difference in direction: RECV buffers are *kernel-filled* (the
//! kernel writes, we read), so they live in a registered `PBUF_RING`; SEND
//! buffers are *user-filled* (we write, the kernel reads), so they are plain
//! owned allocations behind a free list. No kernel registration is needed — the
//! address is handed to each SQE directly.
//!
//! ## The ownership protocol
//!
//! ```text
//!   free ──checkout+fill──> owned by SendBuf ──kernel reads, CQE, drop──> free
//!    ▲                                                                      │
//!    └──────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! A [`SendBuf`]'s backing `Vec<u8>` never reallocates while the buffer is
//! outstanding (it is filled once, up to `buf_size`, at checkout and never
//! grown), so its data pointer is stable for the SQE's lifetime. Dropping the
//! `SendBuf` returns the allocation to the free list for reuse. Leaking one (via
//! `mem::forget`) permanently shrinks the pool by one buffer — a throughput
//! degradation, not unsoundness, exactly as leaking a `ProvidedBuf` does on the
//! RECV side. If the pool is at its `max_bufs` cap and no buffer is free,
//! [`SendPool::checkout`] returns [`io::ErrorKind::WouldBlock`] so the caller
//! parks until an in-flight SEND completes and returns one.

use std::collections::VecDeque;
use std::io;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A pool of owned, fixed-size buffers for `IORING_OP_SEND`.
///
/// See the module docs for the ownership protocol. Cheap to clone via [`Arc`];
/// every [`SendBuf`] holds an `Arc` back to its pool so it can recycle itself on
/// drop.
#[derive(Debug)]
pub struct SendPool {
    /// Capacity of each buffer. A write larger than this is split across several
    /// SENDs by the caller (one `SendBuf` per chunk).
    buf_size: usize,
    /// Hard cap on live buffers. Beyond it, [`Self::checkout`] returns
    /// `WouldBlock` until a buffer returns.
    max_bufs: usize,
    /// Idle buffers available for reuse. Kept at capacity (never reallocated).
    free: Mutex<VecDeque<Vec<u8>>>,
    /// Count of buffers the pool has handed out and not yet reclaimed, plus those
    /// sitting in `free`. Bounded by `max_bufs`.
    allocated: AtomicUsize,
}

impl SendPool {
    /// A pool of at most `max_bufs` buffers, each `buf_size` bytes.
    ///
    /// # Panics
    /// Panics if `buf_size` or `max_bufs` is zero — a pool that can hold nothing
    /// would deadlock every writer.
    #[must_use]
    pub fn new(buf_size: usize, max_bufs: usize) -> Arc<Self> {
        assert!(buf_size > 0, "SendPool buf_size must be non-zero");
        assert!(max_bufs > 0, "SendPool max_bufs must be non-zero");
        Arc::new(Self {
            buf_size,
            max_bufs,
            free: Mutex::new(VecDeque::new()),
            allocated: AtomicUsize::new(0),
        })
    }

    /// Check out a buffer and fill it with up to `buf_size` bytes from `data`.
    ///
    /// Returns a [`SendBuf`] holding `min(data.len(), buf_size)` bytes; the
    /// caller submits the SEND from it and keeps it alive until the CQE. When a
    /// write exceeds `buf_size`, the caller checks out again for the remainder
    /// (the returned buffer's [`SendBuf::len`] reports how much was taken).
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] if the pool is at `max_bufs` and no buffer
    /// is free — the caller should park until an in-flight SEND returns one.
    pub fn checkout(self: &Arc<Self>, data: &[u8]) -> io::Result<SendBuf> {
        let mut buf = self.take_free()?;
        let take = data.len().min(self.buf_size);
        buf.clear();
        buf.extend_from_slice(&data[..take]);
        Ok(SendBuf {
            buf,
            pool: Arc::clone(self),
        })
    }

    /// Pop a free buffer, or allocate a fresh one while under `max_bufs`.
    fn take_free(&self) -> io::Result<Vec<u8>> {
        if let Some(buf) = self.free.lock().expect("SendPool free poisoned").pop_front() {
            return Ok(buf);
        }
        // No idle buffer — allocate if we have not hit the cap. A CAS loop keeps
        // the cap exact under concurrent writers.
        let mut current = self.allocated.load(Ordering::Acquire);
        loop {
            if current >= self.max_bufs {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "SendPool at capacity; park until an in-flight SEND returns a buffer",
                ));
            }
            match self.allocated.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(Vec::with_capacity(self.buf_size)),
                Err(actual) => current = actual,
            }
        }
    }

    /// Return a buffer to the free list (called from [`SendBuf`]'s drop).
    fn checkin(&self, mut buf: Vec<u8>) {
        buf.clear();
        self.free.lock().expect("SendPool free poisoned").push_back(buf);
    }

    /// Number of buffers currently allocated (live + idle). For monitoring pool
    /// exhaustion and buffer leaks.
    #[must_use]
    pub fn allocated(&self) -> usize {
        self.allocated.load(Ordering::Acquire)
    }

    /// Per-buffer capacity.
    #[must_use]
    pub fn buf_size(&self) -> usize {
        self.buf_size
    }
}

/// An owned buffer checked out from a [`SendPool`], filled with the bytes of one
/// SEND. Its data pointer is stable for the SQE's lifetime; dropping it returns
/// the allocation to the pool.
///
/// The `SendBuf` must outlive the `IORING_OP_SEND` it backs — hold it until the
/// send's CQE arrives, then drop it.
#[derive(Debug)]
pub struct SendBuf {
    buf: Vec<u8>,
    pool: Arc<SendPool>,
}

impl SendBuf {
    /// The filled bytes the kernel will place on the wire.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// A stable pointer to the filled bytes, for an `IORING_OP_SEND` SQE.
    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }

    /// Number of bytes taken from the caller's data (== the SEND length).
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether the buffer is empty (a zero-length send — the caller should not
    /// submit one).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

impl Deref for SendBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.buf
    }
}

impl Drop for SendBuf {
    fn drop(&mut self) {
        self.pool.checkin(std::mem::take(&mut self.buf));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
