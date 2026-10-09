//! Registered provided-buffer ring (F43 — Decision 14 F4).
//!
//! WHY: in readiness mode the kernel tells us a socket is readable and we then
//! issue `read(2)` — one syscall per wakeup, plus a copy into a buffer we chose.
//! In completion mode we hand the kernel a *pool* of buffers up front; it picks
//! one, fills it, and the CQE tells us which. The read syscall disappears from
//! the hot path entirely.
//!
//! WHAT: [`BufRing`] owns the registered ring and its backing buffers.
//! [`ProvidedBuf`] is one kernel-filled buffer, on loan to the consumer, which
//! returns it to the ring when dropped.
//!
//! HOW: the ring is an array of `io_uring_buf` entries in page-aligned memory,
//! registered with `IORING_REGISTER_PBUF_RING` (kernel ≥ 5.19). Buffers are
//! published by writing an entry at `tail & mask` and then releasing a new
//! `tail`. The kernel consumes from `head`, and a CQE carrying
//! `IORING_CQE_F_BUFFER` names the buffer id it used.
//!
//! ## The ownership protocol
//!
//! A buffer is in exactly one of three states, and the type system enforces the
//! transitions:
//!
//! ```text
//!   published ──kernel fills──> owned by ProvidedBuf ──drop──> published
//!      ▲                                                          │
//!      └──────────────────────────────────────────────────────────┘
//! ```
//!
//! While a `ProvidedBuf` exists, its buffer id is *not* in the ring, so the
//! kernel cannot write to it — which is what makes handing out `&[u8]` sound.
//! Dropping the `ProvidedBuf` republishes the id. Leaking one (via
//! `mem::forget`) leaks a buffer from the pool, degrading throughput until the
//! pool empties; it is not unsound. An empty pool surfaces as `-ENOBUFS` on the
//! next completion, which the selector handles by re-arming.
//!
//! ## The `resv` field is the tail
//!
//! `io_uring_buf_ring` is a union: the first entry's trailing `u16` *is* the
//! ring's `tail`. Writing `resv` on entry 0 would clobber it. Every write here
//! therefore touches `addr`/`len`/`bid` only, exactly as `io_uring_buf_ring_add`
//! does in liburing.

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::io;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::Mutex;

use io_uring::Submitter;

/// Page size assumed for the ring allocation. The kernel maps the pages
/// containing the ring, so its address must be page-aligned.
const PAGE: usize = 4096;

/// Byte offset of the `tail` field within the ring, i.e. entry 0's `resv`.
const TAIL_OFFSET: usize = 14;

/// Kernel ABI: one entry of a provided-buffer ring. 16 bytes, stable layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct IoUringBuf {
    addr: u64,
    len: u32,
    bid: u16,
    resv: u16,
}

/// A registered ring of provided buffers, plus the memory they live in.
pub struct BufRing {
    /// Page-aligned array of `entries` [`IoUringBuf`] records.
    ring: NonNull<u8>,
    ring_layout: Layout,
    /// `entries * buf_size` bytes of buffer space, indexed by buffer id.
    buffers: NonNull<u8>,
    buffers_layout: Layout,
    entries: u16,
    /// `entries - 1`; `entries` is always a power of two.
    mask: u16,
    buf_size: usize,
    bgid: u16,
    /// Producer-side tail. The kernel reads the published tail from the ring;
    /// this mutex serialises the read-modify-write of (entry, tail) across the
    /// threads that drop `ProvidedBuf`s.
    tail: Mutex<u16>,
    /// Monotonic count of buffers returned to the pool.
    ///
    /// When the pool empties the kernel answers `-ENOBUFS` and drops the
    /// multishot registration. Re-arming immediately would spin, because the
    /// pool is still empty. The selector instead parks the token and re-arms it
    /// once this counter moves, which is precisely "a consumer freed a buffer".
    recycles: AtomicU64,
}

// SAFETY: the raw pointers are unique owning handles to allocations that live
// for the lifetime of the `BufRing`. All mutation of the ring's entries and
// tail happens under `tail`'s mutex, and buffer bytes are only ever accessed
// through a `ProvidedBuf`, which holds exclusive rights to its buffer id.
unsafe impl Send for BufRing {}
unsafe impl Sync for BufRing {}

impl std::fmt::Debug for BufRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufRing")
            .field("bgid", &self.bgid)
            .field("entries", &self.entries)
            .field("buf_size", &self.buf_size)
            .finish()
    }
}

impl BufRing {
    /// WHY: the kernel needs a pool of buffers before it can fill any.
    ///
    /// WHAT: allocate a ring of `entries` buffers of `buf_size` bytes each.
    ///
    /// HOW: two allocations — the page-aligned entry array the kernel maps, and
    /// the buffer space it writes into. Nothing is registered or published yet;
    /// see [`BufRing::register`].
    ///
    /// # Errors
    /// `io::ErrorKind::InvalidInput` if `entries` is zero, not a power of two,
    /// or above the kernel's 32768 limit; `OutOfMemory` if either allocation
    /// fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(entries: u16, buf_size: usize, bgid: u16) -> io::Result<Self> {
        if entries == 0 || !entries.is_power_of_two() || entries > 32768 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("buffer ring entries must be a power of two in 1..=32768, got {entries}"),
            ));
        }
        if buf_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "buffer size must be non-zero",
            ));
        }

        let ring_bytes = entries as usize * std::mem::size_of::<IoUringBuf>();
        let ring_layout = Layout::from_size_align(ring_bytes.max(PAGE), PAGE)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        // SAFETY: `ring_layout` has non-zero size.
        let ring = NonNull::new(unsafe { alloc_zeroed(ring_layout) })
            .ok_or_else(|| io::Error::new(io::ErrorKind::OutOfMemory, "buffer ring allocation"))?;

        let buffers_bytes = entries as usize * buf_size;
        let buffers_layout = Layout::from_size_align(buffers_bytes, PAGE)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        // SAFETY: `buffers_layout` has non-zero size.
        let buffers = match NonNull::new(unsafe { alloc_zeroed(buffers_layout) }) {
            Some(p) => p,
            None => {
                // SAFETY: `ring` came from `alloc_zeroed` with `ring_layout`.
                unsafe { dealloc(ring.as_ptr(), ring_layout) };
                return Err(io::Error::new(io::ErrorKind::OutOfMemory, "buffer space allocation"));
            }
        };

        Ok(Self {
            ring,
            ring_layout,
            buffers,
            buffers_layout,
            entries,
            mask: entries - 1,
            buf_size,
            bgid,
            tail: Mutex::new(0),
            recycles: AtomicU64::new(0),
        })
    }

    /// The buffer group id this ring is registered under.
    pub fn bgid(&self) -> u16 {
        self.bgid
    }

    /// How many buffers the pool holds.
    pub fn entries(&self) -> u16 {
        self.entries
    }

    /// The size of each buffer.
    pub fn buf_size(&self) -> usize {
        self.buf_size
    }

    /// WHY: the kernel can only select from a ring it knows about.
    ///
    /// WHAT: register this ring with `submitter`'s io_uring instance and publish
    /// every buffer into it.
    ///
    /// HOW: `IORING_REGISTER_PBUF_RING`, then publish all `entries` buffer ids.
    ///
    /// # Errors
    /// The kernel's error from `io_uring_register`; notably `EINVAL` on kernels
    /// before 5.19, which the F42 probe already screens for.
    ///
    /// # Panics
    /// Panics if the tail mutex is poisoned.
    pub fn register(&self, submitter: &Submitter<'_>) -> io::Result<()> {
        // SAFETY: `self.ring` is a page-aligned allocation of `entries`
        // `IoUringBuf` records, and it stays live until `BufRing` is dropped —
        // which unregisters first.
        unsafe {
            submitter.register_buf_ring_with_flags(
                self.ring.as_ptr() as u64,
                self.entries,
                self.bgid,
                0,
            )?;
        }
        self.publish_all();
        Ok(())
    }

    /// Remove this ring from the kernel.
    ///
    /// # Errors
    /// The kernel's error from `io_uring_register`.
    pub fn unregister(&self, submitter: &Submitter<'_>) -> io::Result<()> {
        submitter.unregister_buf_ring(self.bgid)
    }

    /// Publish every buffer id into the ring, making the whole pool available.
    ///
    /// # Panics
    /// Panics if the tail mutex is poisoned.
    fn publish_all(&self) {
        let mut tail = self.tail.lock().expect("bufring tail poisoned");
        for bid in 0..self.entries {
            // SAFETY: `bid < entries`, and `tail` is held, so this is the only
            // writer of the ring's entries and tail.
            unsafe { self.write_entry(*tail, bid) };
            *tail = tail.wrapping_add(1);
        }
        // SAFETY: `tail` is held; publishing with Release pairs with the
        // kernel's acquire of the tail.
        unsafe { self.store_tail(*tail) };
    }

    /// WHY: a buffer the consumer has finished with must go back into the pool,
    /// or the pool drains and the kernel starts answering `-ENOBUFS`.
    ///
    /// WHAT: republish buffer `bid`.
    ///
    /// HOW: write the entry at `tail & mask`, then release the incremented
    /// tail. Called from [`ProvidedBuf::drop`], on whatever thread finished with
    /// the bytes.
    ///
    /// # Panics
    /// Panics if the tail mutex is poisoned.
    pub fn recycle(&self, bid: u16) {
        debug_assert!(bid < self.entries, "buffer id {bid} out of range");
        let mut tail = self.tail.lock().expect("bufring tail poisoned");
        // SAFETY: `bid < entries` and the tail mutex is held.
        unsafe {
            self.write_entry(*tail, bid);
            *tail = tail.wrapping_add(1);
            self.store_tail(*tail);
        }
        drop(tail);
        self.recycles.fetch_add(1, Ordering::Release);
    }

    /// How many buffers have been returned to the pool since it was created.
    ///
    /// The selector watches this to know when a starved token is worth
    /// re-arming: a change means at least one buffer is available again.
    ///
    /// # Panics
    /// Never panics.
    pub fn recycle_count(&self) -> u64 {
        self.recycles.load(Ordering::Acquire)
    }

    /// Pointer to buffer `bid`'s bytes.
    ///
    /// # Safety
    /// `bid` must be less than `entries`, and the caller must hold exclusive
    /// rights to that buffer id (i.e. it is not currently published).
    unsafe fn buf_ptr(&self, bid: u16) -> *const u8 {
        self.buffers.as_ptr().add(bid as usize * self.buf_size)
    }

    /// Write the ring entry at slot `tail & mask` describing buffer `bid`.
    ///
    /// # Safety
    /// The caller must hold the tail mutex, and `bid` must be in range. Only
    /// `addr`/`len`/`bid` are written: entry 0's `resv` field aliases the ring's
    /// `tail`, and writing it would corrupt the ring.
    unsafe fn write_entry(&self, tail: u16, bid: u16) {
        let slot = (tail & self.mask) as usize;
        let entry = self.ring.as_ptr().cast::<IoUringBuf>().add(slot);
        (&raw mut (*entry).addr).write(self.buf_ptr(bid) as u64);
        (&raw mut (*entry).len).write(self.buf_size as u32);
        (&raw mut (*entry).bid).write(bid);
    }

    /// Publish `tail` to the kernel.
    ///
    /// # Safety
    /// The caller must hold the tail mutex and have written every entry up to
    /// `tail` first; the `Release` store is what makes those writes visible.
    unsafe fn store_tail(&self, tail: u16) {
        let ptr = self.ring.as_ptr().add(TAIL_OFFSET).cast::<AtomicU16>();
        (*ptr).store(tail, Ordering::Release);
    }

    /// The tail the kernel currently sees. Test/diagnostic use.
    ///
    /// # Panics
    /// Never panics.
    pub fn published_tail(&self) -> u16 {
        // SAFETY: `TAIL_OFFSET` is within the ring allocation and the field is
        // only ever accessed atomically.
        unsafe {
            let ptr = self.ring.as_ptr().add(TAIL_OFFSET).cast::<AtomicU16>();
            (*ptr).load(Ordering::Acquire)
        }
    }
}

impl Drop for BufRing {
    fn drop(&mut self) {
        // Unregistration is the selector's job: it owns the `IoUring` and drops
        // it before releasing its `Arc<BufRing>`. By the time the last `Arc`
        // goes away the kernel no longer references this memory.
        //
        // SAFETY: both pointers came from `alloc_zeroed` with these exact
        // layouts and are not used again.
        unsafe {
            dealloc(self.ring.as_ptr(), self.ring_layout);
            dealloc(self.buffers.as_ptr(), self.buffers_layout);
        }
    }
}

/// One kernel-filled buffer, on loan from a [`BufRing`].
///
/// Deref gives the `len` bytes the kernel wrote. Dropping returns the buffer to
/// the pool. Hold it only as long as the bytes are needed: every outstanding
/// `ProvidedBuf` is one fewer buffer the kernel can fill.
pub struct ProvidedBuf {
    ring: std::sync::Arc<BufRing>,
    bid: u16,
    len: usize,
}

impl ProvidedBuf {
    /// Claim buffer `bid` holding `len` kernel-written bytes.
    ///
    /// # Safety
    /// The kernel must have just reported this `bid` in a CQE (so it is no
    /// longer published), `len` must be the byte count that CQE reported, and no
    /// other `ProvidedBuf` may exist for the same `bid`.
    pub(crate) unsafe fn from_completion(ring: std::sync::Arc<BufRing>, bid: u16, len: usize) -> Self {
        debug_assert!(bid < ring.entries, "buffer id {bid} out of range");
        debug_assert!(len <= ring.buf_size, "completion length {len} exceeds buffer size");
        Self { ring, bid, len }
    }

    /// The buffer id, for diagnostics.
    pub fn id(&self) -> u16 {
        self.bid
    }
}

impl std::ops::Deref for ProvidedBuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // SAFETY: while this `ProvidedBuf` lives, `bid` is not published, so the
        // kernel cannot write to it. The kernel wrote exactly `len` bytes there
        // before reporting the completion this buffer came from.
        unsafe { std::slice::from_raw_parts(self.ring.buf_ptr(self.bid), self.len) }
    }
}

impl AsRef<[u8]> for ProvidedBuf {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl std::fmt::Debug for ProvidedBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvidedBuf")
            .field("bid", &self.bid)
            .field("len", &self.len)
            .finish()
    }
}

impl Drop for ProvidedBuf {
    fn drop(&mut self) {
        self.ring.recycle(self.bid);
    }
}
