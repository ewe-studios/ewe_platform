//! Linux io_uring readiness selector (F41 — Decision 14 §Scope 1).
//!
//! WHY: epoll costs one syscall per readiness wait and cannot batch. io_uring
//! submits a **multishot** `IORING_OP_POLL_ADD` once per fd at registration and
//! then delivers every subsequent readiness transition as a completion queue
//! entry, so the steady state is one `io_uring_enter` per *batch* of ready fds
//! rather than per wait. It slots behind the existing `Poll`/`Registry` API, so
//! no caller changes.
//!
//! WHAT: a `Selector` sibling of `epoll.rs`/`kqueue.rs` — same internal
//! interface (`register_fd`/`reregister_fd`/`deregister_fd`/`poll`), same
//! `Events` output, same edge-triggered semantics.
//!
//! HOW: `PollAdd::multi(true)` per fd, keyed by `Token` in `user_data`. `poll()`
//! waits for at least one CQE (bounded by the caller's timeout via
//! `submit_with_args` + `Timespec`), drains the completion queue, and maps each
//! CQE's poll revents mask onto the epoll bit values `Event` reads.
//!
//! Kernel requirement: Linux ≥ 5.13 (multishot poll); `submit_with_args` needs
//! the `EXT_ARG` feature (≥ 5.11).
//!
//! ## Locking: never hold a queue lock across the wait
//!
//! `IoUring` is `Send + Sync`; its submission and completion queues are not
//! internally synchronised, so each gets its own lock and the rule is that
//! **`poll()` holds neither while it blocks**. An earlier revision took a single
//! `Mutex<IoUring>` and called `submit_and_wait(1)` while holding it, which
//! deadlocked on the first registration: the drain thread parked in
//! `io_cqring_wait` holding the mutex, and every `register_fd` blocked forever
//! trying to push its SQE. `Submitter::submit*` only issues `io_uring_enter` and
//! is safe to call concurrently with pushes, so the wait takes no lock at all.
//!
//! Lock order, where two are taken: `entries` → `sq`. Never the reverse.

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::Mutex;
use std::time::Duration;

use io_uring::types::{SubmitArgs, Timespec};
use io_uring::{cqueue, opcode, types, IoUring};

use super::super::super::super::event::{Event, Events};
use crate::native::poll::{Interest, Token};

/// The raw fd type on Linux.
pub type RawFd = std::os::unix::io::RawFd;

/// `user_data` marker for SQEs whose completions carry no readiness (e.g.
/// `POLL_REMOVE` acknowledgements). Drained and discarded.
const TRACKING_USER_DATA: u64 = u64::MAX;

/// Per-fd tracking entry.
#[derive(Debug, Clone, Copy)]
struct FdEntry {
    fd: RawFd,
    interest: Interest,
}

/// io_uring-based selector.
pub struct Selector {
    /// The ring. `IoUring` is `Send + Sync`; the queues below are what need
    /// serialising, not the ring itself.
    ring: IoUring,
    /// Guards `submission_shared()` — only one thread may touch the SQ.
    sq: Mutex<()>,
    /// Guards `completion_shared()` — only one thread may touch the CQ.
    cq: Mutex<()>,
    /// Token → fd tracking, for multishot re-arm and deregistration.
    entries: Mutex<HashMap<Token, FdEntry>>,
    waker_fd: Mutex<Option<OwnedFd>>,
    waker_token: Mutex<Option<Token>>,
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("uring::Selector")
            .field("ring_fd", &self.ring.as_raw_fd())
            .field("registered", &self.entries.lock().map(|e| e.len()).unwrap_or(0))
            .finish()
    }
}

impl Selector {
    const DEFAULT_ENTRIES: u32 = 256;

    /// WHY: one ring per selector, mirroring one epoll fd per selector.
    ///
    /// WHAT: create the io_uring instance backing this selector.
    ///
    /// HOW: `IoUring::new` performs `io_uring_setup`, which is also the first
    /// tier of the Decision 14 OQ#14.3 probe: it fails on `CONFIG_IO_URING=n`
    /// and on hosts with `kernel.io_uring_disabled` set.
    ///
    /// # Errors
    /// Returns the `io::Error` from `io_uring_setup` when io_uring is
    /// unavailable or administratively disabled.
    ///
    /// # Panics
    /// Never panics.
    pub fn new() -> io::Result<Self> {
        let ring = IoUring::new(Self::DEFAULT_ENTRIES)?;
        Ok(Self {
            ring,
            sq: Mutex::new(()),
            cq: Mutex::new(()),
            entries: Mutex::new(HashMap::new()),
            waker_fd: Mutex::new(None),
            waker_token: Mutex::new(None),
        })
    }

    /// The poll mask for an `Interest`, matching what `epoll.rs` requests.
    ///
    /// `POLLERR`/`POLLHUP` are always reported by the kernel and need not be
    /// requested. `POLLRDHUP` must be requested explicitly, exactly as
    /// `epoll_events_from_interest` does, or peer-shutdown detection would
    /// differ between the two backends.
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

    /// Push one SQE and submit it.
    ///
    /// Takes only the SQ lock, and never blocks: `submit()` issues a
    /// non-waiting `io_uring_enter`.
    fn push_and_submit(&self, sqe: &io_uring::squeue::Entry) -> io::Result<()> {
        let _sq = self.sq.lock().expect("uring sq lock poisoned");

        // SAFETY: the SQ lock makes this the only thread touching the
        // submission queue, and `sqe` is a fully initialised entry.
        unsafe {
            let mut sq = self.ring.submission_shared();
            sq.push(sqe)
                .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "io_uring submission queue full"))?;
            sq.sync();
        }

        self.ring.submitter().submit()?;
        Ok(())
    }

    /// Submit a multishot `POLL_ADD` for the given fd.
    fn arm_poll(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let sqe = opcode::PollAdd::new(types::Fd(fd), Self::poll_mask(interest))
            .multi(true)
            .build()
            .user_data(token.0 as u64);
        self.push_and_submit(&sqe)
    }

    /// Submit a `POLL_REMOVE` cancelling the multishot poll for `token`.
    fn disarm_poll(&self, token: Token) {
        let sqe = opcode::PollRemove::new(token.0 as u64)
            .build()
            .user_data(TRACKING_USER_DATA);
        if let Err(e) = self.push_and_submit(&sqe) {
            tracing::warn!(token = token.0, error = %e, "io_uring POLL_REMOVE submission failed");
        }
    }

    /// Register `fd` under `token`, arming a multishot poll for `interest`.
    ///
    /// # Errors
    /// Returns an `io::Error` if the SQE cannot be submitted.
    ///
    /// # Panics
    /// Panics if the entries lock is poisoned.
    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut entries = self.entries.lock().expect("uring entries lock poisoned");
        self.arm_poll(fd, token, interest)?;
        entries.insert(token, FdEntry { fd, interest });
        Ok(())
    }

    /// Re-arm `token` with a new interest.
    ///
    /// # Errors
    /// Returns an `io::Error` if the replacement SQE cannot be submitted.
    ///
    /// # Panics
    /// Panics if the entries lock is poisoned.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut entries = self.entries.lock().expect("uring entries lock poisoned");
        self.disarm_poll(token);
        entries.remove(&token);
        self.arm_poll(fd, token, interest)?;
        entries.insert(token, FdEntry { fd, interest });
        Ok(())
    }

    /// Cancel the multishot poll registered for `fd`, if any.
    ///
    /// # Errors
    /// Never returns `Err`; an unknown fd is a no-op, matching `epoll`'s
    /// `ENOENT` being treated as success by the reactor.
    ///
    /// # Panics
    /// Panics if the entries lock is poisoned.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        let mut entries = self.entries.lock().expect("uring entries lock poisoned");
        let token = entries.iter().find(|(_, e)| e.fd == fd).map(|(t, _)| *t);
        if let Some(token) = token {
            self.disarm_poll(token);
            entries.remove(&token);
        }
        Ok(())
    }

    /// Register an eventfd that [`Selector::wake`] writes to.
    ///
    /// # Errors
    /// Returns an `io::Error` if the eventfd cannot be created or armed.
    ///
    /// # Panics
    /// Panics if the waker locks are poisoned.
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        // SAFETY: eventfd(2) with valid flags; the returned fd is immediately
        // wrapped in OwnedFd, which owns and closes it.
        let efd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };
        self.register_fd(efd.as_raw_fd(), token, Interest::READABLE)?;
        *self.waker_fd.lock().expect("waker_fd lock poisoned") = Some(efd);
        *self.waker_token.lock().expect("waker_token lock poisoned") = Some(token);
        Ok(())
    }

    /// Wake a thread blocked in [`Selector::poll`].
    ///
    /// # Errors
    /// Returns `io::ErrorKind::NotFound` if no waker was registered, or the
    /// write error from the eventfd.
    ///
    /// # Panics
    /// Panics if the waker lock is poisoned.
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let guard = self.waker_fd.lock().expect("waker_fd lock poisoned");
        let Some(ref fd) = *guard else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "io_uring waker not registered"));
        };
        let val: u64 = 1;
        // SAFETY: writing exactly 8 bytes from a valid u64 to an eventfd, which
        // is the only accepted write size.
        let r = unsafe {
            libc::write(fd.as_raw_fd(), std::ptr::from_ref(&val).cast(), std::mem::size_of_val(&val))
        };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Drain the waker eventfd's counter so it stops reporting readable.
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

    /// `EVFILT_VNODE` is a kqueue concept with no io_uring equivalent.
    ///
    /// # Errors
    /// Always returns `io::ErrorKind::Unsupported`.
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

    /// Wait for at least one completion, bounded by `timeout`.
    ///
    /// Holds **no** lock: `Submitter` issues `io_uring_enter` directly, so a
    /// concurrent `register_fd` can push its SQE while this thread is parked in
    /// the kernel.
    fn wait_for_completion(&self, timeout: Option<Duration>) -> io::Result<()> {
        /// Kernel conditions that mean "no completion this round", not failure.
        fn is_benign(e: &io::Error) -> bool {
            matches!(
                e.raw_os_error(),
                Some(libc::ETIME) | Some(libc::EINTR) | Some(libc::EBUSY) | Some(libc::EAGAIN)
            )
        }

        let submitter = self.ring.submitter();
        let result = match timeout {
            // Zero timeout: submit whatever is queued, never block.
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

    /// Drain the completion queue into `(token → revents)` plus the set of
    /// tokens whose multishot registration ended and must be re-armed.
    ///
    /// # Panics
    /// Panics if the CQ lock is poisoned.
    fn drain_completions(&self) -> (HashMap<Token, u32>, Vec<Token>) {
        let mut seen: HashMap<Token, u32> = HashMap::new();
        let mut needs_rearm: Vec<Token> = Vec::new();

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
            let token = Token(user_data as usize);
            let result = cqe.result();

            // A cancelled or failed poll posts a negative result. It also ends
            // the multishot registration, but the fd is being deregistered, so
            // there is nothing to re-arm.
            if result < 0 {
                continue;
            }

            *seen.entry(token).or_insert(0) |= result as u32;

            // IORING_CQE_F_MORE clear means the kernel dropped the multishot
            // registration; without a fresh POLL_ADD this fd goes deaf.
            if !cqueue::more(cqe.flags()) {
                needs_rearm.push(token);
            }
        }

        (seen, needs_rearm)
    }

    /// Map a poll revents mask onto the epoll bit values `Event` decodes.
    ///
    /// The `POLL*` and `EPOLL*` constants coincide numerically on Linux, but
    /// the translation is written out so the intent survives a constant change.
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

    /// Wait for readiness and fill `events`.
    ///
    /// # Errors
    /// Returns an `io::Error` if `io_uring_enter` fails for a reason other than
    /// timeout, interruption, or a busy ring.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.wait_for_completion(timeout)?;

        let (seen, needs_rearm) = self.drain_completions();

        // Re-arm multishot polls the kernel dropped. Lock order: entries → sq.
        if !needs_rearm.is_empty() {
            let entries = self.entries.lock().expect("uring entries lock poisoned");
            for token in &needs_rearm {
                if let Some(entry) = entries.get(token) {
                    if let Err(e) = self.arm_poll(entry.fd, *token, entry.interest) {
                        tracing::warn!(token = token.0, error = %e, "io_uring multishot re-arm failed");
                    }
                }
            }
        }

        let (ptr, cap) = events.as_mut_ptr_and_cap();
        let mut n = 0;
        for (token, mask) in &seen {
            let flags = Self::epoll_bits_from_revents(*mask);
            if flags == 0 || n >= cap {
                continue;
            }
            // SAFETY: `n < cap`, so `ptr.add(n)` is within the buffer's
            // allocation, and `Events::set_len(n)` below publishes exactly the
            // entries written here.
            unsafe { ptr.add(n).write(Event::from_parts(flags, *token)) };
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
