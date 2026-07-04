//! Buffer pool — the write-path ownership model (Decision 06 §Buffer Pool / RS6).
//!
//! WHY: connect-go uses `sync.Pool`; ours is **per worker thread**. A worker is
//! one OS thread of the valtron pool driving many tasks one at a time, so a
//! thread-local pool is uncontended by construction — no `Mutex`. Frames that
//! must outlive a poll (parked in a pipe) are `freeze`d into a `Bytes` whose
//! owner returns the allocation to its **origin** pool on final drop (usually on
//! another thread), via a lock-free return lane.
//!
//! WHAT: [`BufferPool`] (LIFO hot path + bounded return lane), the private
//! `PooledFrame` owner behind `Bytes::from_owner`, and the [`with_worker_buffer`]
//! / [`worker_freeze`] thread-local accessors.
//!
//! HOW: the hot path is a plain local `Vec<Vec<u8>>` (zero sync); the only
//! synchronization is a lock-free push on a frozen frame's final drop plus an
//! occasional lane drain in [`get`](BufferPool::get).
//!
//! **Invariants (Decision 06, normative):** a pooled buffer never crosses a yield
//! point as a borrow (acquire → fill → `put`/`freeze` within one poll); anything
//! entering a pipe owns its memory (`Bytes`) — a frozen frame satisfies this.

use std::cell::RefCell;
use std::mem;
use std::sync::Arc;

use bytes::Bytes;
use concurrent_queue::ConcurrentQueue;

/// Depth of the lock-free return lane; overflow frames are simply freed so an
/// idle/dead thread can never hoard memory.
const RETURN_LANE_DEPTH: usize = 256;
/// Largest buffer capacity that is recycled (matching connect-go's 8 MiB cap).
const MAX_RECYCLE: usize = 8 * 1024 * 1024;
/// Initial capacity for freshly-allocated scratch buffers.
const INITIAL_CAPACITY: usize = 512;

/// A per-worker recycled buffer pool (Decision 06).
pub struct BufferPool {
    /// Recycled buffers (LIFO, hot path — zero sync).
    pool: Vec<Vec<u8>>,
    /// Lock-free lane where final drops of frozen frames (from any thread) return.
    return_lane: Arc<ConcurrentQueue<Vec<u8>>>,
    initial_capacity: usize,
    max_recycle_size: usize,
}

impl BufferPool {
    /// Create an empty pool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pool: Vec::new(),
            return_lane: Arc::new(ConcurrentQueue::bounded(RETURN_LANE_DEPTH)),
            initial_capacity: INITIAL_CAPACITY,
            max_recycle_size: MAX_RECYCLE,
        }
    }

    /// Acquire a cleared buffer: pop the local LIFO; when empty, drain the return
    /// lane; otherwise allocate.
    #[must_use]
    pub fn get(&mut self) -> Vec<u8> {
        if let Some(mut buf) = self.pool.pop() {
            buf.clear();
            return buf;
        }
        // Local free-list empty — pull home any frames returned by other threads.
        while let Ok(buf) = self.return_lane.pop() {
            self.pool.push(buf);
        }
        match self.pool.pop() {
            Some(mut buf) => {
                buf.clear();
                buf
            }
            None => Vec::with_capacity(self.initial_capacity),
        }
    }

    /// Return a scratch buffer consumed within the same poll to the local LIFO.
    /// Oversized buffers are freed rather than recycled.
    pub fn put(&mut self, mut buf: Vec<u8>) {
        if buf.capacity() <= self.max_recycle_size {
            buf.clear();
            self.pool.push(buf);
        }
    }

    /// Convert filled scratch into a pipe-safe [`Bytes`] that **recycles on final
    /// drop** — the "convert" arm of invariant 1. Clones/slices only bump the
    /// refcount; on the last drop the allocation returns to *this* pool's lane.
    #[must_use]
    pub fn freeze(&self, buf: Vec<u8>) -> Bytes {
        Bytes::from_owner(PooledFrame {
            buf,
            home: Arc::clone(&self.return_lane),
        })
    }

    /// Number of frames waiting in the return lane (observability / tests).
    #[must_use]
    pub fn pending_returns(&self) -> usize {
        self.return_lane.len()
    }

    /// Number of buffers in the local free-list (observability / tests).
    #[must_use]
    pub fn free_list_len(&self) -> usize {
        self.pool.len()
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Owner behind [`Bytes::from_owner`]: returns its allocation to the **origin**
/// pool's return lane on final drop. Invisible to consumers — the pipe, facade,
/// and transport see a normal `Bytes`.
struct PooledFrame {
    buf: Vec<u8>,
    home: Arc<ConcurrentQueue<Vec<u8>>>,
}

impl AsRef<[u8]> for PooledFrame {
    fn as_ref(&self) -> &[u8] {
        &self.buf
    }
}

impl Drop for PooledFrame {
    fn drop(&mut self) {
        let mut buf = mem::take(&mut self.buf);
        // Empty (already-taken) or oversized buffers are freed; the bounded lane
        // drops overflow so a dead thread cannot hoard memory.
        if buf.capacity() != 0 && buf.capacity() <= MAX_RECYCLE {
            buf.clear();
            let _ = self.home.push(buf);
        }
    }
}

thread_local! {
    /// The current worker thread's buffer pool. A thread hosts many tasks but
    /// polls them serially, so this is uncontended (RS6).
    static WORKER_POOL: RefCell<BufferPool> = RefCell::new(BufferPool::new());
}

/// Borrow scratch from the current worker's pool for the duration of `f`, then
/// return it (Decision 06). Use for buffers consumed within one poll; a buffer
/// that must outlive the poll goes through [`worker_freeze`] instead.
pub fn with_worker_buffer<R>(f: impl FnOnce(&mut Vec<u8>) -> R) -> R {
    WORKER_POOL.with(|cell| {
        let mut pool = cell.borrow_mut();
        let mut buf = pool.get();
        let out = f(&mut buf);
        pool.put(buf);
        out
    })
}

/// Fill a buffer from the current worker's pool and freeze it into a pipe-safe
/// [`Bytes`] whose allocation returns to **this** thread's pool on final drop
/// (Decision 06 origin-return, even when the last reference drops elsewhere).
pub fn worker_freeze(fill: impl FnOnce(&mut Vec<u8>)) -> Bytes {
    WORKER_POOL.with(|cell| {
        let mut pool = cell.borrow_mut();
        let mut buf = pool.get();
        fill(&mut buf);
        pool.freeze(buf)
    })
}
