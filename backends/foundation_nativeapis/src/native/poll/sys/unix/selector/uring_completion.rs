//! Linux io_uring completion-mode selector (F43 — Decision 14 F4).
//!
//! WHY: readiness mode still pays one `read(2)` per wakeup. Completion mode
//! hands the kernel a pool of buffers, arms a multishot `RECV`, and the kernel
//! delivers *bytes* in the completion queue. The read syscall leaves the hot
//! path — the acceptance criterion of this feature.
//!
//! WHAT: a `Selector` sibling of `uring.rs` that speaks the same internal
//! interface (so `Poll`/`Registry`/`Reactor` are unchanged) and additionally
//! exposes [`Selector::take_completions`], the inbox a transport pops bytes
//! from instead of calling `read`.
//!
//! HOW: at registration, sockets get `IORING_OP_RECV` with
//! `IORING_RECV_MULTISHOT` + `BUFFER_SELECT` against a registered buffer ring;
//! everything else gets multishot `POLL_ADD`, exactly as in readiness mode.
//! `poll()` drains the completion queue, converts recv CQEs into `ProvidedBuf`s
//! queued per token, and still emits an ordinary READABLE `Event` so the reactor
//! wakes the parked task the same way it always has.
//!
//! Kernel requirement: Linux ≥ 5.19 (buffer rings) and ≥ 6.0 (multishot recv).
//! The F42 probe gates this; below the matrix the ladder selects readiness mode.
//!
//! ## Not every fd is a socket
//!
//! `RECV` is a socket operation. Pipes, eventfds and inotify fds return
//! `-ENOTSOCK`, so they must stay on `POLL_ADD`. The selector decides per fd
//! with one `getsockopt(SO_TYPE)` at registration and remembers the answer.
//! This is why completion mode is not a blanket replacement: it is a read-path
//! optimisation for sockets, and everything else keeps working unchanged.
//!
//! ## Buffer starvation
//!
//! When consumers hold every buffer, the kernel completes with `-ENOBUFS` and
//! drops the multishot registration. Re-arming right then would spin against an
//! empty pool. The token is instead marked starved and re-armed on the first
//! `poll()` after [`BufRing::recycle_count`] moves — i.e. after a consumer has
//! actually freed a buffer.
//!
//! ## Locking
//!
//! Same rule as `uring.rs`: `poll()` holds no queue lock while it blocks.
//! Lock order where several are taken: `entries` → `starved` → `sq`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use io_uring::types::{SubmitArgs, Timespec};
use io_uring::{cqueue, opcode, types, IoUring};

use super::super::super::super::event::{Event, Events};
use super::bufring::{BufRing, ProvidedBuf};
use crate::native::fd::send_pool::{SendBuf, SendPool};
use crate::native::poll::{Interest, Token};

/// The raw fd type on Linux.
pub type RawFd = std::os::unix::io::RawFd;

/// `user_data` marker for SQEs whose completions carry nothing we act on.
const TRACKING_USER_DATA: u64 = u64::MAX;

/// Tag bit distinguishing a `POLL_ADD` completion from a `RECV` completion.
///
/// A token may have both armed at once (a socket registered readable+writable),
/// so the two ops cannot share a bare token as `user_data`.
const POLL_TAG: u64 = 1 << 62;

/// Tag bit marking an `IORING_OP_SEND` completion (F49). Unlike RECV/POLL, a
/// SEND is one-shot and per-write, so its `user_data` carries a unique send id
/// (below this bit) rather than a token — the in-flight map resolves it to the
/// owning token and buffer.
const SEND_TAG: u64 = 1 << 61;

/// Tokens (and send ids) must fit below the tag bits.
const MAX_TOKEN: u64 = SEND_TAG - 1;

/// Buffer group id for this selector's ring.
const BGID: u16 = 0;

/// Buffers in the pool.
const RING_ENTRIES: u16 = 256;

/// Bytes per buffer. Sized to hold a typical MTU-bounded read without splitting
/// a frame across two completions more often than necessary; the resumable
/// decoder (Decision 12 §11) handles splits regardless.
const BUF_SIZE: usize = 16 * 1024;

/// One completed read, or the terminal condition that ended the stream.
#[derive(Debug)]
pub enum Completion {
    /// Bytes the kernel already read. Dropping returns the buffer to the pool.
    Data(ProvidedBuf),
    /// The peer closed; no further data will arrive.
    Eof,
    /// The recv failed. The multishot registration has ended.
    Error(io::Error),
}

/// One completed `IORING_OP_SEND` (F49): the bytes the kernel placed on the
/// wire, or the error that ended the send. The owned buffer has already been
/// recycled to the [`SendPool`] by the time this is delivered.
#[derive(Debug)]
pub enum SendCompletion {
    /// `n` bytes were placed on the wire.
    Written(usize),
    /// The send failed; the buffer was consumed and recycled.
    Error(io::Error),
}

/// How a registered fd is being watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// A socket: multishot `RECV` into the buffer ring.
    Recv,
    /// Anything else: multishot `POLL_ADD`.
    Poll,
}

/// Per-fd tracking entry.
#[derive(Debug, Clone, Copy)]
struct FdEntry {
    fd: RawFd,
    interest: Interest,
    mode: Mode,
}

/// io_uring completion-mode selector.
pub struct Selector {
    ring: IoUring,
    sq: Mutex<()>,
    cq: Mutex<()>,
    bufring: Arc<BufRing>,
    entries: Mutex<HashMap<Token, FdEntry>>,
    /// Per-token inbox of kernel-filled buffers, drained by `take_completions`.
    inboxes: Mutex<HashMap<Token, VecDeque<Completion>>>,
    /// Tokens whose multishot recv ended with `-ENOBUFS`, awaiting a free buffer.
    starved: Mutex<HashSet<Token>>,
    /// `BufRing::recycle_count` as of the last starvation sweep.
    last_recycles: AtomicU64,
    /// User-filled buffer pool backing `IORING_OP_SEND` (F49). The SEND mirror of
    /// `bufring`: it owns the bytes a write submits until the send's CQE arrives.
    send_pool: Arc<SendPool>,
    /// SEND buffers whose CQE has not yet arrived, keyed by send id. Holding the
    /// `SendBuf` here keeps its bytes alive (and unmoved) for the kernel; the CQE
    /// removes and drops it, recycling the buffer.
    in_flight_sends: Mutex<HashMap<u64, (Token, SendBuf)>>,
    /// Monotonic send-id source; the low bits become each SEND's `user_data`.
    next_send_id: AtomicU64,
    /// Per-token mailbox of finished sends, drained by `take_send_completions`.
    send_mailboxes: Mutex<HashMap<Token, VecDeque<SendCompletion>>>,
    /// SQEs pushed since construction.
    ///
    /// Decision 14 OQ#14.1 predicted that completion mode would make submission
    /// queue contention real, because "per-op submissions" would put every read
    /// through the SQ lock. Multishot `RECV` falsifies that: one SQE arms an fd
    /// for its whole lifetime. This counter is the evidence — see
    /// `submissions_are_per_registration_not_per_read`.
    submissions: AtomicU64,
    waker_fd: Mutex<Option<OwnedFd>>,
    waker_token: Mutex<Option<Token>>,
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("uring_completion::Selector")
            .field("ring_fd", &self.ring.as_raw_fd())
            .field("registered", &self.entries.lock().map(|e| e.len()).unwrap_or(0))
            .field("bufring", &self.bufring)
            .finish()
    }
}

/// Is `fd` a socket? `RECV` only works on sockets.
fn is_socket(fd: RawFd) -> bool {
    sockopt(fd, libc::SO_TYPE).is_some()
}

/// Is `fd` a listening socket? A listener has no bytes to receive — only
/// connections to `accept()` — so `RECV` on it is meaningless.
fn is_listening(fd: RawFd) -> bool {
    sockopt(fd, libc::SO_ACCEPTCONN).is_some_and(|v| v != 0)
}

/// Read one `SOL_SOCKET` integer option, or `None` if `fd` is not a socket.
fn sockopt(fd: RawFd, name: libc::c_int) -> Option<libc::c_int> {
    let mut value: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: `value`/`len` are valid out-params of the expected sizes.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            name,
            std::ptr::from_mut(&mut value).cast(),
            &mut len,
        )
    };
    (rc == 0).then_some(value)
}

impl Selector {
    const DEFAULT_ENTRIES: u32 = 256;

    /// WHY: completion mode needs a ring *and* a registered buffer pool before
    /// it can accept a single registration.
    ///
    /// WHAT: create the io_uring instance and register its buffer ring.
    ///
    /// HOW: `io_uring_setup`, then `IORING_REGISTER_PBUF_RING` and publish every
    /// buffer. Both steps are screened by the F42 probe before the ladder picks
    /// this backend, so a failure here means the host changed under us.
    ///
    /// # Errors
    /// The kernel's error from `io_uring_setup` or the buffer-ring registration.
    ///
    /// # Panics
    /// Never panics.
    pub fn new() -> io::Result<Self> {
        Self::with_ring(RING_ENTRIES, BUF_SIZE)
    }

    /// WHY: the buffer pool's shape is the main tuning knob of completion mode,
    /// and starvation behaviour is only reachable with a small pool.
    ///
    /// WHAT: create the selector with an explicit buffer-ring geometry.
    ///
    /// HOW: as [`Selector::new`], with `entries` buffers of `buf_size` bytes.
    ///
    /// # Errors
    /// `InvalidInput` if `entries` is not a power of two in `1..=32768`, or the
    /// kernel's error from setup or buffer-ring registration.
    ///
    /// # Panics
    /// Never panics.
    pub fn with_ring(entries: u16, buf_size: usize) -> io::Result<Self> {
        let ring = IoUring::new(Self::DEFAULT_ENTRIES)?;
        let bufring = Arc::new(BufRing::new(entries, buf_size, BGID)?);
        bufring.register(&ring.submitter())?;

        Ok(Self {
            ring,
            sq: Mutex::new(()),
            cq: Mutex::new(()),
            bufring,
            entries: Mutex::new(HashMap::new()),
            inboxes: Mutex::new(HashMap::new()),
            starved: Mutex::new(HashSet::new()),
            last_recycles: AtomicU64::new(0),
            send_pool: SendPool::new(buf_size, entries as usize),
            in_flight_sends: Mutex::new(HashMap::new()),
            next_send_id: AtomicU64::new(0),
            send_mailboxes: Mutex::new(HashMap::new()),
            submissions: AtomicU64::new(0),
            waker_fd: Mutex::new(None),
            waker_token: Mutex::new(None),
        })
    }

    /// WHY: the whole point of this backend — the transport pops bytes the
    /// kernel already read, instead of issuing `read(2)`.
    ///
    /// WHAT: drain and return everything queued for `token`.
    ///
    /// HOW: swaps the token's inbox out under the inbox lock. Each
    /// [`Completion::Data`] owns its buffer until dropped, at which point the
    /// buffer returns to the pool.
    ///
    /// # Panics
    /// Panics if the inbox lock is poisoned.
    pub fn take_completions(&self, token: Token) -> Vec<Completion> {
        let mut inboxes = self.inboxes.lock().expect("uring inbox lock poisoned");
        match inboxes.get_mut(&token) {
            Some(queue) => queue.drain(..).collect(),
            None => Vec::new(),
        }
    }

    /// Whether `token` has completions waiting.
    ///
    /// # Panics
    /// Panics if the inbox lock is poisoned.
    pub fn has_completions(&self, token: Token) -> bool {
        self.inboxes
            .lock()
            .expect("uring inbox lock poisoned")
            .get(&token)
            .is_some_and(|q| !q.is_empty())
    }

    /// WHY: a transport needs to know which read path it is on *without*
    /// consuming anything. Asking by calling `take_completions` and checking for
    /// `Some` would drain the inbox and lose the bytes.
    ///
    /// WHAT: whether `token` is registered on the multishot-recv path, i.e. its
    /// bytes arrive as completions rather than needing a `read(2)`.
    ///
    /// HOW: sockets take [`Mode::Recv`]; pipes, eventfds and other non-sockets
    /// take [`Mode::Poll`] because `RECV` is a socket operation.
    ///
    /// # Panics
    /// Panics if the entries lock is poisoned.
    pub fn is_recv_token(&self, token: Token) -> bool {
        self.entries
            .lock()
            .expect("uring entries lock poisoned")
            .get(&token)
            .is_some_and(|e| e.mode == Mode::Recv)
    }

    /// The buffer ring backing this selector's completions.
    pub fn bufring(&self) -> &Arc<BufRing> {
        &self.bufring
    }

    // ── SEND (F49) ────────────────────────────────────────────────────────────

    /// WHY: the SEND mirror of the RECV inbox — a write costs no `write(2)`, the
    /// kernel copies from an owned buffer we hand it and reports completion later.
    ///
    /// WHAT: copy up to one pool buffer of `data` into an owned [`SendBuf`] and
    /// submit an `IORING_OP_SEND` for it on `fd`. Returns how many bytes were
    /// taken (a short write when `data` exceeds the pool buffer size — the caller
    /// resubmits the remainder).
    ///
    /// HOW: check a buffer out of the [`SendPool`] (which parks the caller with
    /// `WouldBlock` when exhausted), record it in the in-flight map keyed by a
    /// unique send id, then push the SEND SQE. The buffer stays owned here — and
    /// its heap allocation unmoved — until the CQE removes and drops it.
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] if the send pool is exhausted; the kernel's
    /// error if the SQE cannot be submitted.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn submit_send(&self, token: Token, fd: RawFd, data: &[u8]) -> io::Result<usize> {
        let buf = self.send_pool.checkout(data)?;
        let n = buf.len();
        if n == 0 {
            return Ok(0);
        }
        let send_id = self.next_send_id.fetch_add(1, Ordering::Relaxed) & (SEND_TAG - 1);
        let user_data = SEND_TAG | send_id;
        // Capture the stable heap pointer before the buffer moves into the map;
        // moving the `SendBuf` moves only the `Vec` header, not its allocation.
        let ptr = buf.as_ptr();
        self.in_flight_sends
            .lock()
            .expect("uring in-flight sends lock poisoned")
            .insert(send_id, (token, buf));

        // SAFETY: `ptr` addresses `n` initialised bytes owned by the `SendBuf`
        // held in `in_flight_sends`; it stays alive and unmoved until this send's
        // CQE removes it in `drain_completions`. The len fits `u32` (pool buffers
        // are 16 KiB).
        let sqe = opcode::Send::new(types::Fd(fd), ptr, n as u32)
            .build()
            .user_data(user_data);
        if let Err(e) = self.push_and_submit(&sqe) {
            // Submission failed: reclaim the buffer so it does not leak from the
            // pool (no CQE will ever arrive for it).
            self.in_flight_sends
                .lock()
                .expect("uring in-flight sends lock poisoned")
                .remove(&send_id);
            return Err(e);
        }
        Ok(n)
    }

    /// Take every finished send for `token` since the last call. A transport
    /// calls this from `flush()`: it sums the byte counts and surfaces any error.
    ///
    /// # Panics
    /// Panics if the send-mailbox lock is poisoned.
    pub fn take_send_completions(&self, token: Token) -> Vec<SendCompletion> {
        let mut mailboxes = self
            .send_mailboxes
            .lock()
            .expect("uring send mailbox lock poisoned");
        match mailboxes.get_mut(&token) {
            Some(queue) => queue.drain(..).collect(),
            None => Vec::new(),
        }
    }

    /// Whether finished sends are waiting for `token`, without taking them.
    ///
    /// # Panics
    /// Panics if the send-mailbox lock is poisoned.
    pub fn has_send_completions(&self, token: Token) -> bool {
        self.send_mailboxes
            .lock()
            .expect("uring send mailbox lock poisoned")
            .get(&token)
            .is_some_and(|q| !q.is_empty())
    }

    /// Number of SEND buffers currently in flight (submitted, CQE not yet seen).
    /// For monitoring the send pool.
    #[must_use]
    pub fn sends_in_flight(&self) -> usize {
        self.in_flight_sends
            .lock()
            .expect("uring in-flight sends lock poisoned")
            .len()
    }

    /// Whether `token` has any SEND whose CQE has not yet arrived. A transport's
    /// `flush()` parks while this is true, so every submitted byte is on the wire
    /// before `flush()` reports success.
    ///
    /// # Panics
    /// Panics if the in-flight lock is poisoned.
    pub fn has_pending_sends(&self, token: Token) -> bool {
        self.in_flight_sends
            .lock()
            .expect("uring in-flight sends lock poisoned")
            .values()
            .any(|(t, _)| *t == token)
    }

    /// WHY: buffer starvation is invisible from the outside — the socket simply
    /// stops delivering — so it needs an observable counter for both operators
    /// and tests.
    ///
    /// WHAT: how many tokens are currently parked waiting for a free buffer.
    ///
    /// HOW: the set the `-ENOBUFS` handler inserts into and `sweep_starved`
    /// drains.
    ///
    /// # Panics
    /// Panics if the starved lock is poisoned.
    pub fn starved_tokens(&self) -> usize {
        self.starved.lock().expect("uring starved lock poisoned").len()
    }

    /// WHY: Decision 14 OQ#14.1 committed to revisiting per-worker rings once
    /// completion mode landed, on the theory that "per-op submission makes SQ
    /// contention real". Whether that theory holds is an empirical question
    /// about this counter.
    ///
    /// WHAT: how many SQEs this selector has pushed since construction.
    ///
    /// HOW: incremented in `push_and_submit`, the single place an SQE enters the
    /// submission queue. With multishot `RECV` this grows with *registrations*,
    /// not with reads.
    ///
    /// # Panics
    /// Never panics.
    pub fn submissions(&self) -> u64 {
        self.submissions.load(Ordering::Relaxed)
    }

    /// The poll mask for an `Interest`, matching `epoll.rs` and `uring.rs`.
    fn poll_mask(interest: Interest) -> u32 {
        let mut mask: u32 = 0;
        if interest.is_readable() {
            mask |= libc::POLLIN as u32;
            mask |= libc::POLLRDHUP as u32;
        }
        if interest.is_writable() {
            mask |= libc::POLLOUT as u32;
        }
        mask
    }

    /// Push one SQE and submit it. Takes only the SQ lock; never blocks.
    fn push_and_submit(&self, sqe: &io_uring::squeue::Entry) -> io::Result<()> {
        self.submissions.fetch_add(1, Ordering::Relaxed);
        let _sq = self.sq.lock().expect("uring sq lock poisoned");
        // SAFETY: the SQ lock makes this the only thread touching the submission
        // queue, and `sqe` is a fully initialised entry.
        unsafe {
            let mut sq = self.ring.submission_shared();
            sq.push(sqe)
                .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "io_uring submission queue full"))?;
            sq.sync();
        }
        self.ring.submitter().submit()?;
        Ok(())
    }

    /// Arm a multishot `RECV` for `fd`, delivering into the buffer ring.
    fn arm_recv(&self, fd: RawFd, token: Token) -> io::Result<()> {
        let sqe = opcode::RecvMulti::new(types::Fd(fd), BGID)
            .build()
            .user_data(token.0 as u64);
        self.push_and_submit(&sqe)
    }

    /// Arm a multishot `POLL_ADD` for `fd`.
    fn arm_poll(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let sqe = opcode::PollAdd::new(types::Fd(fd), Self::poll_mask(interest))
            .multi(true)
            .build()
            .user_data(token.0 as u64 | POLL_TAG);
        self.push_and_submit(&sqe)
    }

    /// Cancel whatever is armed for `token`.
    ///
    /// `ASYNC_CANCEL` rather than `POLL_REMOVE`: the latter only cancels poll
    /// requests, and a `Mode::Recv` token has a multishot `RECV` in flight.
    /// `ASYNC_CANCEL` matches by `user_data` whatever the opcode.
    fn disarm(&self, token: Token, mode: Mode) {
        let user_data = match mode {
            Mode::Recv => token.0 as u64,
            Mode::Poll => token.0 as u64 | POLL_TAG,
        };
        let sqe = opcode::AsyncCancel::new(user_data).build().user_data(TRACKING_USER_DATA);
        if let Err(e) = self.push_and_submit(&sqe) {
            tracing::warn!(token = token.0, error = %e, "io_uring cancel submission failed");
        }
    }

    /// WHY: **byte transparency.** `Poll`/`Registry` callers register an fd and
    /// then read it themselves — `accept()`, `recv_from()`, `read()`. If this
    /// backend armed a multishot `RECV`, the kernel would drain the socket into
    /// our buffer ring and the caller's own read would find nothing. Arming
    /// `RECV` behind a caller's back is not a backend swap, it is a semantic
    /// change, and `Poll::new()` picking this backend would inflict it on every
    /// existing user.
    ///
    /// WHAT: register `fd` for readiness, exactly as the epoll and io_uring
    /// readiness selectors do. No bytes are consumed.
    ///
    /// HOW: multishot `POLL_ADD`. Completion mode is opted into per
    /// registration, via [`Selector::register_recv_fd`] — which is what
    /// Decision 14 F4 means by "transports opt their read path into inbox-pop".
    ///
    /// # Errors
    /// `InvalidInput` if the token collides with the internal tag bits; the
    /// kernel's error if the SQE cannot be submitted.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        self.register_with_mode(fd, token, interest, Mode::Poll).map(|_| ())
    }

    /// WHY: the opt-in half. A transport that will consume through
    /// [`Selector::take_completions`] declares it here, and only then does the
    /// kernel start reading the socket on its behalf.
    ///
    /// WHAT: arm a multishot `RECV` for `fd`, delivering bytes into this
    /// selector's inbox. Returns whether the recv path was actually taken.
    ///
    /// HOW: `RECV` is a connected-socket operation. Three kinds of fd cannot use
    /// it, and each falls back to `POLL_ADD` and returns `false` rather than
    /// failing:
    ///
    /// - non-sockets (pipes, eventfds, inotify): `-ENOTSOCK`
    /// - listening sockets: there are no bytes to receive, only connections to
    ///   `accept()`
    /// - registrations without a readable interest: nothing to receive
    ///
    /// A caller must therefore check the return value (or
    /// [`Selector::is_recv_token`]) before skipping its own `read`.
    ///
    /// # Errors
    /// `InvalidInput` if the token collides with the internal tag bits; the
    /// kernel's error if the SQE cannot be submitted.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn register_recv_fd(
        &self,
        fd: RawFd,
        token: Token,
        interest: Interest,
    ) -> io::Result<bool> {
        let mode = if interest.is_readable() && is_socket(fd) && !is_listening(fd) {
            Mode::Recv
        } else {
            Mode::Poll
        };
        self.register_with_mode(fd, token, interest, mode)
    }

    /// Shared registration body. Returns `true` when the recv path was armed.
    fn register_with_mode(
        &self,
        fd: RawFd,
        token: Token,
        interest: Interest,
        mode: Mode,
    ) -> io::Result<bool> {
        if token.0 as u64 > MAX_TOKEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("token {} exceeds the completion selector's tag space", token.0),
            ));
        }

        let mut entries = self.entries.lock().expect("uring entries lock poisoned");
        match mode {
            Mode::Recv => self.arm_recv(fd, token)?,
            Mode::Poll => self.arm_poll(fd, token, interest)?,
        }
        entries.insert(token, FdEntry { fd, interest, mode });
        drop(entries);

        self.inboxes
            .lock()
            .expect("uring inbox lock poisoned")
            .entry(token)
            .or_default();
        Ok(mode == Mode::Recv)
    }

    /// Re-arm `token` with a new interest.
    ///
    /// # Errors
    /// Propagates [`Selector::register_fd`].
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        self.deregister_fd(fd)?;
        self.register_fd(fd, token, interest)
    }

    /// Cancel `fd`'s registration and drop its inbox.
    ///
    /// Any `ProvidedBuf` still held by a consumer keeps its buffer out of the
    /// pool until dropped; the `Arc<BufRing>` inside it keeps the memory alive.
    ///
    /// # Errors
    /// Never returns `Err`; an unknown fd is a no-op.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        let mut entries = self.entries.lock().expect("uring entries lock poisoned");
        let found = entries.iter().find(|(_, e)| e.fd == fd).map(|(t, e)| (*t, e.mode));
        if let Some((token, mode)) = found {
            self.disarm(token, mode);
            entries.remove(&token);
            drop(entries);

            self.starved.lock().expect("uring starved lock poisoned").remove(&token);
            self.inboxes.lock().expect("uring inbox lock poisoned").remove(&token);
        }
        Ok(())
    }

    /// Register an eventfd that [`Selector::wake`] writes to.
    ///
    /// # Errors
    /// The kernel's error creating or arming the eventfd.
    ///
    /// # Panics
    /// Panics if a waker lock is poisoned.
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        // SAFETY: eventfd(2) with valid flags; immediately wrapped in OwnedFd.
        let efd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };
        // An eventfd is not a socket, so this lands on the poll path.
        self.register_fd(efd.as_raw_fd(), token, Interest::READABLE)?;
        *self.waker_fd.lock().expect("waker_fd lock poisoned") = Some(efd);
        *self.waker_token.lock().expect("waker_token lock poisoned") = Some(token);
        Ok(())
    }

    /// Wake a thread blocked in [`Selector::poll`].
    ///
    /// # Errors
    /// `NotFound` if no waker was registered, or the eventfd write error.
    ///
    /// # Panics
    /// Panics if the waker lock is poisoned.
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let guard = self.waker_fd.lock().expect("waker_fd lock poisoned");
        let Some(ref fd) = *guard else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "io_uring waker not registered"));
        };
        let val: u64 = 1;
        // SAFETY: writing exactly 8 bytes from a valid u64 to an eventfd.
        let r = unsafe {
            libc::write(fd.as_raw_fd(), std::ptr::from_ref(&val).cast(), std::mem::size_of_val(&val))
        };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Drain the waker eventfd's counter.
    ///
    /// # Panics
    /// Panics if the waker lock is poisoned.
    pub fn clear_waker(&self) {
        let guard = self.waker_fd.lock().expect("waker_fd lock poisoned");
        if let Some(ref fd) = *guard {
            let mut val: u64 = 0;
            // SAFETY: reading exactly 8 bytes into a valid u64 from an eventfd.
            let r = unsafe {
                libc::read(fd.as_raw_fd(), std::ptr::from_mut(&mut val).cast(), std::mem::size_of_val(&val))
            };
            if r < 0 {
                let err = io::Error::last_os_error();
                if err.kind() != io::ErrorKind::WouldBlock {
                    tracing::warn!(error = %err, "draining io_uring waker eventfd failed");
                }
            }
        }
    }

    /// `EVFILT_VNODE` has no io_uring equivalent.
    ///
    /// # Errors
    /// Always `Unsupported`.
    pub fn register_vnode(&self, _fd: RawFd, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "EVFILT_VNODE not supported on io_uring"))
    }

    /// No-op counterpart to [`Selector::register_vnode`].
    ///
    /// # Errors
    /// Never returns `Err`.
    pub fn deregister_vnode(&self, _fd: RawFd) -> io::Result<()> {
        Ok(())
    }

    // ── Poll ────────────────────────────────────────────────────────────────

    /// Wait for at least one completion, bounded by `timeout`. Holds no lock.
    fn wait_for_completion(&self, timeout: Option<Duration>) -> io::Result<()> {
        fn is_benign(e: &io::Error) -> bool {
            matches!(
                e.raw_os_error(),
                Some(libc::ETIME) | Some(libc::EINTR) | Some(libc::EBUSY) | Some(libc::EAGAIN)
            )
        }

        let submitter = self.ring.submitter();
        let result = match timeout {
            Some(d) if d.is_zero() => submitter.submit().map(|_| ()),
            Some(d) => {
                let ts = Timespec::new().sec(d.as_secs()).nsec(d.subsec_nanos());
                let args = SubmitArgs::new().timespec(&ts);
                submitter.submit_with_args(1, &args).map(|_| ())
            }
            None => submitter.submit_and_wait(1).map(|_| ()),
        };

        match result {
            Ok(()) => Ok(()),
            Err(ref e) if is_benign(e) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Re-arm any starved tokens, if a buffer has come back since we last looked.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    fn sweep_starved(&self) {
        let recycles = self.bufring.recycle_count();
        if recycles == self.last_recycles.load(Ordering::Acquire) {
            return;
        }
        self.last_recycles.store(recycles, Ordering::Release);

        let mut starved = self.starved.lock().expect("uring starved lock poisoned");
        if starved.is_empty() {
            return;
        }
        let entries = self.entries.lock().expect("uring entries lock poisoned");
        starved.retain(|token| match entries.get(token) {
            Some(entry) => {
                if let Err(e) = self.arm_recv(entry.fd, *token) {
                    tracing::warn!(token = token.0, error = %e, "re-arming starved recv failed");
                    return true; // keep it starved; try again next sweep
                }
                tracing::debug!(token = token.0, "re-armed recv after buffer starvation");
                false
            }
            // Deregistered while starved.
            None => false,
        });
    }

    /// Record one completion for `token`.
    ///
    /// # Panics
    /// Panics if the inbox lock is poisoned.
    fn push_completion(&self, token: Token, completion: Completion) {
        self.inboxes
            .lock()
            .expect("uring inbox lock poisoned")
            .entry(token)
            .or_default()
            .push_back(completion);
    }

    /// Map a poll revents mask onto the epoll bit values `Event` decodes.
    fn epoll_bits_from_revents(mask: u32) -> u32 {
        let mut flags: u32 = 0;
        if mask & (libc::POLLIN as u32) != 0 {
            flags |= libc::EPOLLIN as u32;
        }
        if mask & (libc::POLLOUT as u32) != 0 {
            flags |= libc::EPOLLOUT as u32;
        }
        if mask & (libc::POLLPRI as u32) != 0 {
            flags |= libc::EPOLLPRI as u32;
        }
        if mask & (libc::POLLRDHUP as u32) != 0 {
            flags |= libc::EPOLLRDHUP as u32;
        }
        if mask & (libc::POLLHUP as u32) != 0 {
            flags |= libc::EPOLLHUP as u32;
        }
        if mask & (libc::POLLERR as u32) != 0 {
            flags |= libc::EPOLLERR as u32;
        }
        flags
    }

    /// Drain the completion queue, filling inboxes and accumulating events.
    ///
    /// Returns `(token → epoll event bits, tokens needing a recv re-arm)`.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    fn drain_completions(&self) -> (HashMap<Token, u32>, Vec<Token>) {
        let mut seen: HashMap<Token, u32> = HashMap::new();
        let mut rearm_recv: Vec<Token> = Vec::new();
        let mut rearm_poll: Vec<Token> = Vec::new();

        {
            let _cq = self.cq.lock().expect("uring cq lock poisoned");
            // SAFETY: the CQ lock makes this the only thread touching the
            // completion queue.
            let mut cq = unsafe { self.ring.completion_shared() };
            cq.sync();

            for cqe in &mut cq {
                let user_data = cqe.user_data();
                if user_data == TRACKING_USER_DATA {
                    continue;
                }

                let flags = cqe.flags();
                let result = cqe.result();

                if user_data & POLL_TAG != 0 {
                    // ── multishot POLL_ADD, as in readiness mode ──
                    let token = Token((user_data & MAX_TOKEN) as usize);
                    if result < 0 {
                        continue;
                    }
                    *seen.entry(token).or_insert(0) |= Self::epoll_bits_from_revents(result as u32);
                    if !cqueue::more(flags) {
                        rearm_poll.push(token);
                    }
                    continue;
                }

                if user_data & SEND_TAG != 0 {
                    // ── one-shot IORING_OP_SEND (F49) ──
                    let send_id = user_data & (SEND_TAG - 1);
                    let removed = self
                        .in_flight_sends
                        .lock()
                        .expect("uring in-flight sends lock poisoned")
                        .remove(&send_id);
                    // Dropping the taken `SendBuf` (in `_buf`) recycles it to the pool.
                    if let Some((token, _buf)) = removed {
                        let completion = if result >= 0 {
                            SendCompletion::Written(result as usize)
                        } else {
                            let errno = -result;
                            // Cancelled by deregistration: the buffer is reclaimed,
                            // and the token is gone — nothing to report.
                            if errno == libc::ECANCELED {
                                continue;
                            }
                            SendCompletion::Error(io::Error::from_raw_os_error(errno))
                        };
                        self.send_mailboxes
                            .lock()
                            .expect("uring send mailbox lock poisoned")
                            .entry(token)
                            .or_default()
                            .push_back(completion);
                        // Wake the task parked on `flush()` for this token.
                        *seen.entry(token).or_insert(0) |= libc::EPOLLOUT as u32;
                    }
                    continue;
                }

                // ── multishot RECV: the kernel already read the bytes ──
                let token = Token(user_data as usize);

                if result > 0 {
                    match cqueue::buffer_select(flags) {
                        Some(bid) => {
                            // SAFETY: the kernel just reported `bid` in this CQE, so
                            // it is no longer published and no other ProvidedBuf can
                            // exist for it. `result` is the byte count it wrote.
                            let buf = unsafe {
                                ProvidedBuf::from_completion(
                                    Arc::clone(&self.bufring),
                                    bid,
                                    result as usize,
                                )
                            };
                            self.push_completion(token, Completion::Data(buf));
                            *seen.entry(token).or_insert(0) |= libc::EPOLLIN as u32;
                        }
                        None => {
                            // A recv completion without a buffer id should be
                            // impossible with BUFFER_SELECT; surfacing it beats
                            // silently dropping the bytes.
                            tracing::error!(
                                token = token.0,
                                result,
                                "recv completion carried no buffer id; bytes lost"
                            );
                            self.push_completion(
                                token,
                                Completion::Error(io::Error::other(
                                    "io_uring recv completion carried no buffer id",
                                )),
                            );
                            *seen.entry(token).or_insert(0) |= libc::EPOLLERR as u32;
                        }
                    }
                } else if result == 0 {
                    // Peer closed. Multishot ends here; nothing to re-arm.
                    self.push_completion(token, Completion::Eof);
                    *seen.entry(token).or_insert(0) |=
                        libc::EPOLLIN as u32 | libc::EPOLLRDHUP as u32;
                    continue;
                } else {
                    let errno = -result;
                    match errno {
                        // Pool exhausted: park the token until a buffer returns.
                        libc::ENOBUFS => {
                            self.starved.lock().expect("uring starved lock poisoned").insert(token);
                            tracing::debug!(token = token.0, "recv starved: buffer pool empty");
                            continue;
                        }
                        // Cancelled by deregistration.
                        libc::ECANCELED => continue,
                        _ => {
                            self.push_completion(
                                token,
                                Completion::Error(io::Error::from_raw_os_error(errno)),
                            );
                            *seen.entry(token).or_insert(0) |= libc::EPOLLERR as u32;
                            continue;
                        }
                    }
                }

                if !cqueue::more(flags) {
                    rearm_recv.push(token);
                }
            }
        }

        // Re-arm polls. Lock order: entries → sq.
        if !rearm_poll.is_empty() {
            let entries = self.entries.lock().expect("uring entries lock poisoned");
            for token in &rearm_poll {
                if let Some(entry) = entries.get(token) {
                    if let Err(e) = self.arm_poll(entry.fd, *token, entry.interest) {
                        tracing::warn!(token = token.0, error = %e, "multishot poll re-arm failed");
                    }
                }
            }
        }

        (seen, rearm_recv)
    }

    /// Wait for readiness and fill `events`.
    ///
    /// Bytes are delivered into the per-token inboxes; the `Event`s emitted here
    /// exist so the reactor's wake path is identical to readiness mode.
    ///
    /// # Errors
    /// The kernel's `io_uring_enter` error, other than timeout/interrupt/busy.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.sweep_starved();
        self.wait_for_completion(timeout)?;

        let (seen, rearm_recv) = self.drain_completions();

        if !rearm_recv.is_empty() {
            let entries = self.entries.lock().expect("uring entries lock poisoned");
            for token in &rearm_recv {
                if let Some(entry) = entries.get(token) {
                    if let Err(e) = self.arm_recv(entry.fd, *token) {
                        tracing::warn!(token = token.0, error = %e, "multishot recv re-arm failed");
                    }
                }
            }
        }

        let (ptr, cap) = events.as_mut_ptr_and_cap();
        let mut n = 0;
        for (token, mask) in &seen {
            if *mask == 0 || n >= cap {
                continue;
            }
            // SAFETY: `n < cap`, so `ptr.add(n)` is in bounds, and `set_len(n)`
            // below publishes exactly what was written.
            unsafe { ptr.add(n).write(Event::from_parts(*mask, *token)) };
            n += 1;
        }
        events.set_len(n);

        if let Some(waker_token) = *self.waker_token.lock().expect("waker_token lock poisoned") {
            if seen.contains_key(&waker_token) {
                self.clear_waker();
            }
        }

        Ok(())
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        // Unregister before the ring closes, so the kernel stops referencing the
        // buffer memory that outstanding `ProvidedBuf`s keep alive.
        if let Err(e) = self.bufring.unregister(&self.ring.submitter()) {
            tracing::debug!(error = %e, "unregistering buffer ring during selector drop");
        }
    }
}
