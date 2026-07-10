//! The `CompletionSource` seam (F43 — Decision 14 F4).
//!
//! WHY: Decision 14's seam analysis is the load-bearing argument for this
//! feature living here and not in valtron. Valtron's contract — a task returns
//! `Depends(source)`, the reactor calls `wake(token)`, the task is re-polled —
//! is **byte-blind and identical in both modes**: it cannot tell a readiness bit
//! from a completed recv. The entire delta is (a) ring and buffer lifecycle,
//! which lives in `foundation_nativeapis`, and (b) byte acquisition in the
//! transport task: `stream.read()` versus popping a kernel-filled buffer.
//!
//! WHAT: [`CompletionSource`] is that second half — the inbox a transport pops
//! instead of calling `read(2)`, sitting next to `EventReadiness` rather than
//! replacing it. A task still parks on `Depends(RegisteredFd)`; when it wakes it
//! asks *this* trait for its bytes.
//!
//! HOW: only the io_uring completion backend has an inbox. Everything else
//! reports [`CompletionSource::is_completion_source`] as `false`, and a
//! transport branches on that **once** at setup, not per read:
//!
//! ```ignore
//! if fd.is_completion_source() {
//!     for completion in fd.take_completions() {
//!         match completion {
//!             Completion::Data(buf) => decoder.step(&mut &buf[..])?,  // no syscall
//!             Completion::Eof => break,
//!             Completion::Error(e) => return Err(e),
//!         }
//!     }
//! } else {
//!     let n = fd.get_ref().read(&mut buf)?;                            // read(2)
//!     decoder.step(&mut &buf[..n])?;
//! }
//! ```
//!
//! The Decision 12 §11 `IncrementalDecoder` is unchanged in both arms: its
//! `step(&mut impl Read)` takes a byte-slice cursor as happily as a socket, and
//! its accumulating buffer already handles frames spanning two completions.
//!
//! ## The buffer ownership protocol
//!
//! [`Completion::Data`] owns its buffer. Dropping it returns that buffer to the
//! kernel's pool. Hold one only as long as the bytes are needed: every
//! outstanding buffer is one the kernel cannot fill, and an empty pool stalls
//! the socket until a buffer comes back. Copy out of it (as the decoder does)
//! and drop it.

#[cfg(all(target_os = "linux", feature = "uring"))]
pub use crate::native::poll::sys::unix::selector::uring_completion::Completion;

/// A source of bytes the kernel has already read.
///
/// Implemented by `RegisteredFd` and `FdRegistration`. On every backend except
/// io_uring completion mode this is inert: [`Self::is_completion_source`]
/// returns `false` and [`Self::take_completions`] returns nothing, because the
/// bytes are still in the socket and the caller must `read` them.
#[cfg(all(target_os = "linux", feature = "uring"))]
pub trait CompletionSource {
    /// WHY: a transport must know, once, which read path it is on. Asking per
    /// read would be a branch on every byte and would invite the two paths to
    /// drift apart.
    ///
    /// WHAT: whether this fd delivers bytes through [`Self::take_completions`]
    /// rather than through `read(2)`.
    ///
    /// HOW: true only when the reactor selected the io_uring completion backend
    /// *and* this fd took the recv path (sockets do; pipes and eventfds do not,
    /// because `RECV` is a socket operation).
    ///
    /// # Panics
    /// Never panics.
    fn is_completion_source(&self) -> bool;

    /// WHY: this is the read, and it costs no syscall — the kernel performed it
    /// when the data arrived.
    ///
    /// WHAT: take everything the kernel has delivered since the last call.
    ///
    /// HOW: drains the reactor's per-token inbox. Empty when there is nothing
    /// pending, and always empty on a non-completion backend. Consuming the last
    /// completion clears the fd's latched readiness, so the task parks again —
    /// the completion-mode analogue of reading until `WouldBlock`.
    ///
    /// # Panics
    /// Never panics.
    fn take_completions(&self) -> Vec<Completion>;

    /// Whether bytes are waiting, without taking them.
    ///
    /// # Panics
    /// Never panics.
    fn has_completions(&self) -> bool;
}

/// Completion buffers taken from the inbox but not yet fully copied out.
///
/// WHY: a `read`-shaped API hands the caller a fixed-size buffer, but a
/// completion carries whatever the kernel read — possibly more. The remainder
/// must survive until the next call, and the `ProvidedBuf` holding it must stay
/// alive (dropping it would return the bytes to the kernel's pool unread).
///
/// WHAT: the FIFO of untaken completions plus how far into the head one the
/// caller has read.
///
/// HOW: `read_bytes` copies from the head, advancing `offset`; when the head is
/// exhausted it is dropped — recycling that buffer — and the next one becomes
/// the head. `eof` latches so every read after the peer closed keeps returning
/// `Ok(0)` rather than `WouldBlock`.
#[cfg(all(target_os = "linux", feature = "uring"))]
#[derive(Debug, Default)]
pub(crate) struct StagedReads {
    pub(crate) queue: std::collections::VecDeque<Completion>,
    pub(crate) offset: usize,
    pub(crate) eof: bool,
}
