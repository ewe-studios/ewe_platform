/// File descriptor management — wraps any raw fd with readiness tracking.
///
/// Built for our sync, task-driven model on top of our cross-platform
/// poll::Selector (epoll/kqueue/IOCP).
///
/// ## Key Types
///
/// - [`RegisteredFd<T>`] — wraps an IO object, registers it with the poll selector
/// - [`FdRegistration`] — tracks per-fd readiness state as an atomic bitmask
/// - [`ReadyGuard`] — returned by `poll_readable()`/`poll_writable()`, must be
///   explicitly handled to clear readiness
/// - [`Ready`] — bitmask of readiness states (READABLE, WRITABLE, READ_CLOSED, WRITE_CLOSED, ERROR)
///
/// ## Parking a transport task on fd readiness (spec-41 F10 / Decision 00 L2)
///
/// A native leaf task that waits on a real socket **parks** on the reactor rather
/// than re-polling every scheduler turn. Because [`RegisteredFd<T>`] already
/// implements [`foundation_core::valtron::EventReadiness`], the whole path reuses
/// the executor's existing `TaskStatus::Depends(Arc<dyn EventReadiness>)` seam —
/// **there is no `ReadinessSource` trait or global registration slot to build**
/// (the Level-2 sketch in Decision 00 is superseded by this reactor).
///
/// The pattern a transport task follows:
///
/// 1. **Obtain a reactor `Registry`.** Use the process-shared reactor —
///    `let reactor = `[`Reactor::get`]`()?; let registry = reactor.registry();` —
///    so all connections share one selector and `is_ready()` reads a cached
///    atomic instead of issuing a syscall (spec-41 F40, Decision 14 §Scope 3).
///    Passing a registry from your own `Poll` is still honoured: the fd is
///    registered there and readiness is queried against it with a zero-timeout
///    poll. What you pass is what you get.
/// 2. **Register the connection's fd.** The fd comes from `netcap::RawStream`
///    via `AsRawFd` (spec-41 F09 / Decision 12 §12), reachable from above netio:
///    `let fd = Arc::new(`[`RegisteredFd::new`]`(stream, &registry, token)?);`.
/// 3. **Park on it.** When the socket is not ready, the task's
///    [`TaskIterator::next_status`](foundation_core::valtron::TaskIterator::next_status)
///    returns `TaskStatus::Depends(fd.clone() as Arc<dyn EventReadiness>)`; the
///    executor parks the task and re-runs it only when `fd.is_ready(..)` — i.e.
///    when epoll/kqueue signals the fd. Compose with a cancel/stop signal via a
///    composite `EventReadiness` (see `FdMonitorTask`) to also unpark on cancel.
///
/// `foundation_core` never touches the fd or gains an OS dependency: the bridge
/// is realized entirely through the `EventReadiness` impl on `RegisteredFd`.

pub mod guard;
pub mod error;
pub mod reactor;

/// The `CompletionSource` seam — byte acquisition for io_uring completion mode.
pub mod completion;

pub use guard::{MutReadyGuard, ReadyGuard, TryIoError};
pub use error::{FdRegistrationError, RegistrationError};
pub use reactor::{Reactor, SharedReadiness};

#[cfg(all(target_os = "linux", feature = "uring"))]
pub use completion::{Completion, CompletionSource};

use crate::native::poll::{Events, Interest, Token};
use crate::native::poll::sys::RawFd;

use std::io;
use std::os::unix::io::{AsRawFd, RawFd as StdRawFd};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_core::valtron::EventReadiness;

/// Bitmask of readiness and lifecycle states observed on a file descriptor.
///
/// ## Why CLOSED and ERROR flags are needed
///
/// On edge-triggered pollers, when a pipe's write end closes, epoll returns
/// `EPOLLIN | EPOLLHUP`. The `EPOLLIN` sets READABLE, but `EPOLLHUP` means
/// the connection is permanently broken — it will NEVER transition back to
/// "not ready". Without a CLOSED flag, the fd appears permanently readable,
/// causing a busy-wait loop.
///
/// Similarly, `EPOLLERR` can arrive without `EPOLLIN` or `EPOLLOUT`. Without
/// an ERROR flag, the fd would appear "not ready" when it actually has an
/// error condition that needs handling.
///
/// ## Platform mapping
/// | Flag        | epoll                              | kqueue                          |
/// |-------------|-------------------------------------|----------------------------------|
/// | READABLE    | EPOLLIN                             | EVFILT_READ                      |
/// | WRITABLE    | EPOLLOUT                            | EVFILT_WRITE                     |
/// | READ_CLOSED | EPOLLHUP or (EPOLLIN+EPOLLRDHUP)    | EVFILT_READ + EV_EOF             |
/// | WRITE_CLOSED| EPOLLHUP or (EPOLLOUT+EPOLLERR)     | EVFILT_WRITE + EV_EOF            |
/// | ERROR       | EPOLLERR                            | EVFILT_READ/WRITE + EV_EOF+fflags |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready(u8);

impl Ready {
    pub const READABLE:    Ready = Ready(0b00001);
    pub const WRITABLE:    Ready = Ready(0b00010);
    pub const READ_CLOSED: Ready = Ready(0b00100);
    pub const WRITE_CLOSED: Ready = Ready(0b01000);
    pub const ERROR:       Ready = Ready(0b10000);
    pub const EMPTY:       Ready = Ready(0b00000);

    pub fn is_readable(self)     -> bool { self.0 & Self::READABLE.0 != 0 }
    pub fn is_writable(self)     -> bool { self.0 & Self::WRITABLE.0 != 0 }
    pub fn is_read_closed(self)  -> bool { self.0 & Self::READ_CLOSED.0 != 0 }
    pub fn is_write_closed(self) -> bool { self.0 & Self::WRITE_CLOSED.0 != 0 }
    pub fn is_error(self)        -> bool { self.0 & Self::ERROR.0 != 0 }
    pub fn is_empty(self)        -> bool { self.0 == 0 }

    /// Combine readiness states.
    pub fn union(self, other: Ready) -> Ready { Ready(self.0 | other.0) }

    /// Remove specific readiness states.
    pub fn difference(self, other: Ready) -> Ready { Ready(self.0 & !other.0) }

    /// Intersection — readiness flags present in both.
    pub fn intersection(self, other: Ready) -> Ready { Ready(self.0 & other.0) }

    /// Check if this readiness contains all flags in `other`.
    pub fn contains(self, other: Ready) -> bool { self.0 & other.0 == other.0 }

    /// WHY: the shared reactor stores readiness in an `AtomicU8` so the drain
    /// thread can set bits and consumers can clear them without taking a write
    /// lock on the registration map.
    ///
    /// WHAT: the raw bit pattern backing this `Ready`.
    ///
    /// HOW: direct field read; the bit layout is the `Ready::*` constants.
    ///
    /// # Panics
    /// Never panics.
    pub const fn bits(self) -> u8 { self.0 }

    /// WHY: counterpart to [`Ready::bits`] for reading back an atomically
    /// stored readiness set.
    ///
    /// WHAT: rebuild a `Ready` from a raw bit pattern.
    ///
    /// HOW: unknown high bits are masked off, so a corrupted or future-widened
    /// value can never produce a `Ready` claiming flags this build doesn't know.
    ///
    /// # Panics
    /// Never panics.
    pub const fn from_bits(bits: u8) -> Ready { Ready(bits & Self::ALL.0) }

    /// Every readiness flag this build understands.
    pub const ALL: Ready = Ready(0b11111);
}

impl std::fmt::Display for Ready {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return write!(f, "EMPTY");
        }
        let mut first = true;
        for (flag, name) in [
            (Self::READABLE, "READABLE"),
            (Self::WRITABLE, "WRITABLE"),
            (Self::READ_CLOSED, "READ_CLOSED"),
            (Self::WRITE_CLOSED, "WRITE_CLOSED"),
            (Self::ERROR, "ERROR"),
        ] {
            if self.contains(flag) {
                if !first {
                    write!(f, "|")?;
                }
                write!(f, "{name}")?;
                first = false;
            }
        }
        Ok(())
    }
}

/// Result of a readiness poll.
pub enum PollResult<T> {
    /// The fd is ready for I/O — use the guard to perform operations.
    Ready(T),
    /// The fd is not yet ready — caller should yield and retry.
    NotReady,
    /// An error occurred. This includes:
    /// - READ_CLOSED: pipe write end closed, socket peer shutdown (EOF)
    /// - WRITE_CLOSED: pipe read end closed, socket peer shutdown
    /// - ERROR: epoll error condition (EPOLLERR), socket reset
    /// - Deregistered: fd was removed from the poll selector
    Error(io::Error),
}

impl<T: std::fmt::Debug> std::fmt::Debug for PollResult<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollResult::Ready(t) => f.debug_tuple("Ready").field(t).finish(),
            PollResult::NotReady => write!(f, "NotReady"),
            PollResult::Error(e) => f.debug_tuple("Error").field(e).finish(),
        }
    }
}

/// Tracks readiness state for a registered file descriptor.
///
/// # How readiness tracking works:
/// 1. The fd is registered with the poll::Selector via FdRegistration::new()
/// 2. poll_readable()/poll_writable() call poll() with zero timeout to check
///    current readiness state from the selector
/// 3. Events returned are mapped to Ready flags (READABLE, WRITABLE, READ_CLOSED, etc.)
/// 4. The guard's try_io() or clear_ready() resets readiness by re-polling
///
/// This is the critical loop that prevents busy-wait on edge-triggered systems.
#[allow(dead_code)]
pub struct FdRegistration {
    registry: crate::native::poll::Registry,
    token: Token,
    /// Private Poll for readiness queries (legacy — unused when reactor is Some).
    poll: Option<crate::native::poll::Poll>,
    /// Shared reactor (F40). When Some, is_ready() consults cached bits — zero syscalls.
    reactor: Option<Arc<Reactor>>,
    /// Last known readiness state — cached between poll calls.
    readiness: Mutex<Ready>,
    /// Completion-mode read state: buffers taken from the inbox but not yet
    /// fully copied out by [`FdRegistration::read_bytes`] (F43).
    #[cfg(all(target_os = "linux", feature = "uring"))]
    staged: Mutex<completion::StagedReads>,
}

impl FdRegistration {
    /// WHY: the `registry` argument must mean something. An earlier revision
    /// ignored it whenever `Reactor::get()` succeeded — which is always — and
    /// registered into the shared reactor regardless. A caller that built its
    /// own `Poll` and passed its registry got an fd registered somewhere else,
    /// so its own `poll()` returned no events and nothing said why.
    ///
    /// WHAT: register `fd` into the selector `registry` fronts.
    ///
    /// HOW: if `registry` is the shared reactor's registry (same `Arc<Selector>`),
    /// take the reactor path — one process-wide selector, readiness read from a
    /// cached atomic, zero syscalls per check (Decision 14 §Scope 3). Otherwise
    /// honour the caller's registry and keep a private `Poll` alongside it for
    /// zero-timeout readiness queries.
    ///
    /// Pass `Reactor::get()?.registry()` to opt into the shared reactor.
    ///
    /// # Errors
    /// The selector's `io::Error` if registration fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(
        fd: RawFd,
        registry: &crate::native::poll::Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<Self> {
        let shared = Reactor::get()
            .ok()
            .filter(|reactor| registry.same_selector(reactor.registry()));

        let (poll, reactor) = match shared {
            Some(reactor) => {
                reactor.register(fd, token, interest)?;
                (None, Some(reactor))
            }
            None => {
                registry.register_fd(fd, token, interest)?;
                let poll = crate::native::poll::Poll::new()?;
                poll.registry().register_fd(fd, token, interest)?;
                (Some(poll), None)
            }
        };

        Ok(Self {
            registry: registry.clone(),
            token,
            poll,
            reactor,
            readiness: Mutex::new(Ready::EMPTY),
            #[cfg(all(target_os = "linux", feature = "uring"))]
            staged: Mutex::new(completion::StagedReads::default()),
        })
    }

    /// Register into the shared reactor, opting the read path into completion
    /// mode where the backend and the fd allow it.
    ///
    /// See [`RegisteredFd::with_completion`].
    ///
    /// # Errors
    /// `Unsupported` if `registry` is not the shared reactor's — a caller-owned
    /// selector has no completion inbox to pop from. Otherwise the selector's
    /// registration error.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn with_completion(
        fd: RawFd,
        registry: &crate::native::poll::Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<Self> {
        let reactor = Reactor::get()
            .ok()
            .filter(|reactor| registry.same_selector(reactor.registry()))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "completion mode requires the shared reactor's registry \
                     (Reactor::get()?.registry())",
                )
            })?;

        reactor.register_completion(fd, token, interest)?;

        Ok(Self {
            registry: registry.clone(),
            token,
            poll: None,
            reactor: Some(reactor),
            readiness: Mutex::new(Ready::EMPTY),
            staged: Mutex::new(completion::StagedReads::default()),
        })
    }

    /// Poll the selector and update the readiness cache.
    fn query_readiness(&self) -> io::Result<Ready> {
        // F40: shared reactor path — zero syscalls. Return the *actual* latched
        // flags: collapsing them to READABLE|WRITABLE would make READ_CLOSED and
        // ERROR unobservable, so EOF and socket errors would look like readable.
        if let Some(ref reactor) = self.reactor {
            return Ok(reactor.readiness(self.token));
        }

        // Legacy: private poll with zero-timeout syscall.
        let poll = self.poll.as_ref().expect("poll or reactor must be set");
        let mut events = Events::with_capacity(16);
        poll.poll(&mut events, Some(Duration::ZERO))?;

        let mut ready = Ready::EMPTY;
        for event in events.iter() {
            if event.is_readable() {
                ready = ready.union(Ready::READABLE);
            }
            if event.is_writable() {
                ready = ready.union(Ready::WRITABLE);
            }
            if event.is_read_closed() {
                ready = ready.union(Ready::READ_CLOSED);
            }
            if event.is_write_closed() {
                ready = ready.union(Ready::WRITE_CLOSED);
            }
            if event.is_error() {
                ready = ready.union(Ready::ERROR);
            }
        }

        Ok(ready)
    }

    /// Atomically update readiness and return it.
    fn refresh_readiness(&self) -> io::Result<Ready> {
        let ready = self.query_readiness()?;
        *self.readiness.lock().unwrap() = ready;
        Ok(ready)
    }

    /// WHY: the Linux selector is edge-triggered, so the kernel reports each
    /// readiness transition exactly once and the reactor latches it. A consumer
    /// that has drained the fd to `WouldBlock` must clear the latch, otherwise
    /// `is_ready()` stays true and the parked task spins instead of sleeping.
    ///
    /// WHAT: clear the flags in `ready` from this registration's readiness, both
    /// in the local cache and in the shared reactor.
    ///
    /// HOW: masks the bits out of the local `readiness` cache, then clears them
    /// in the reactor entry. On the legacy private-`Poll` path there is no
    /// shared entry, so only the local cache is updated — the next
    /// `query_readiness` re-polls the selector anyway.
    ///
    /// # Panics
    /// Panics if the local readiness lock is poisoned.
    pub fn clear_readiness(&self, ready: Ready) {
        let mut local = self.readiness.lock().expect("readiness lock poisoned");
        *local = local.difference(ready);
        drop(local);

        if let Some(ref reactor) = self.reactor {
            reactor.clear(self.token, ready);
        }
    }

    /// Deregister from the poll selector.
    ///
    /// # Errors
    /// Returns the selector's `io::Error` if deregistration fails.
    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        if let Some(ref reactor) = self.reactor {
            return reactor.deregister(fd, self.token);
        }
        self.registry.deregister_fd(fd)
    }

    /// WHY: this is the opt-in Decision 14 F4 asks of transports — "opt read
    /// paths into inbox-pop". A transport calls this instead of `read(2)` and
    /// gets bytes from whichever source the reactor's backend provides, with no
    /// per-backend branching of its own.
    ///
    /// WHAT: read up to `buf.len()` bytes from `fd`, nonblocking.
    ///
    /// HOW: in io_uring completion mode the kernel has already read the bytes,
    /// so this copies them out of the inbox — **no syscall**. On every other
    /// backend it issues an ordinary `read(2)`. Both report end-of-stream as
    /// `Ok(0)` and "nothing available" as `WouldBlock`, so a caller's loop is
    /// identical either way.
    ///
    /// Zero-copy callers should prefer [`completion::CompletionSource::take_completions`]
    /// and step their decoder straight over `&buf[..]`; this method exists so an
    /// existing `read`-shaped transport can adopt completion mode by changing one
    /// call.
    ///
    /// # Errors
    /// `WouldBlock` when no bytes are available; the socket's error otherwise. A
    /// `WouldBlock` clears the latched readiness, re-arming the park.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn read_bytes(&self, fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
        use completion::CompletionSource;

        if !self.is_completion_source() {
            return Self::read_syscall(fd, buf);
        }

        if buf.is_empty() {
            return Ok(0);
        }

        /// What the head of the staging queue yielded, decided before the queue
        /// is mutated so the borrow of the head buffer has already ended.
        enum Step {
            /// Copied `n` bytes; the head buffer is now exhausted if `true`.
            Copied(usize, bool),
            Eof,
            Failed,
            Empty,
        }

        let mut guard = self.staged.lock().expect("staged reads lock poisoned");

        loop {
            let staged = &mut *guard;

            let step = match staged.queue.front() {
                Some(completion::Completion::Data(provided)) => {
                    let remaining = &provided[staged.offset..];
                    let n = remaining.len().min(buf.len());
                    buf[..n].copy_from_slice(&remaining[..n]);
                    Step::Copied(n, staged.offset + n >= provided.len())
                }
                Some(completion::Completion::Eof) => Step::Eof,
                Some(completion::Completion::Error(_)) => Step::Failed,
                None => Step::Empty,
            };

            match step {
                Step::Copied(n, exhausted) => {
                    staged.offset += n;
                    if exhausted {
                        // Dropping the ProvidedBuf returns its buffer to the pool.
                        staged.queue.pop_front();
                        staged.offset = 0;
                    }
                    return Ok(n);
                }
                Step::Eof => {
                    staged.eof = true;
                    staged.queue.pop_front();
                    return Ok(0);
                }
                Step::Failed => {
                    let Some(completion::Completion::Error(e)) = staged.queue.pop_front() else {
                        unreachable!("front was just matched as Error");
                    };
                    return Err(e);
                }
                Step::Empty => {}
            }

            // Nothing staged. A latched EOF keeps reporting end-of-stream rather
            // than WouldBlock, or the caller would park forever on a dead socket.
            if staged.eof {
                return Ok(0);
            }

            let Some(ref reactor) = self.reactor else {
                return Err(io::Error::new(io::ErrorKind::WouldBlock, "no completions pending"));
            };
            let refill = reactor.take_completions(self.token).unwrap_or_default();
            if refill.is_empty() {
                // Inbox drained: clear the latch so the task parks again. Same
                // contract as a `WouldBlock` from `read(2)` in readiness mode.
                drop(guard);
                self.clear_readiness(Ready::READABLE);
                return Err(io::Error::new(io::ErrorKind::WouldBlock, "no completions pending"));
            }
            staged.queue.extend(refill);
        }
    }

    /// The readiness-mode read: an ordinary nonblocking `read(2)`.
    ///
    /// # Errors
    /// The socket's `io::Error`, including `WouldBlock`.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    fn read_syscall(fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
        // SAFETY: `buf` is a valid writable slice of `buf.len()` bytes and `fd`
        // is a registered, open descriptor.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(n as usize)
    }
}

impl EventReadiness for FdRegistration {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        match self.query_readiness() {
            Ok(r) => !r.is_empty(),
            Err(_) => false,
        }
    }
}

#[cfg(all(target_os = "linux", feature = "uring"))]
impl completion::CompletionSource for FdRegistration {
    fn is_completion_source(&self) -> bool {
        // Must be non-destructive: `take_completions` would drain the inbox and
        // discard the bytes. A pipe on a completion-mode reactor is *not* a
        // completion source — `RECV` is a socket operation, so the selector put
        // it on the poll path and its bytes still need a `read`.
        match self.reactor {
            Some(ref reactor) => reactor.is_recv_token(self.token),
            None => false,
        }
    }

    fn take_completions(&self) -> Vec<completion::Completion> {
        let Some(ref reactor) = self.reactor else {
            return Vec::new();
        };
        let taken = reactor.take_completions(self.token).unwrap_or_default();

        // Draining the inbox is completion mode's "read until WouldBlock": with
        // nothing left, the latched readiness must clear or the task spins.
        if !taken.is_empty() && !reactor.has_completions(self.token) {
            self.clear_readiness(Ready::READABLE);
        }
        taken
    }

    fn has_completions(&self) -> bool {
        match self.reactor {
            Some(ref reactor) => reactor.has_completions(self.token),
            None => false,
        }
    }
}

/// Wraps any type that produces a raw fd, registering it with our poll::Selector
/// and providing readiness polling + guarded I/O operations.
pub struct RegisteredFd<T: AsRawFd> {
    registration: FdRegistration,
    inner: Option<T>,
}

impl<T: AsRawFd + Send + Sync> EventReadiness for RegisteredFd<T> {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.registration.is_ready(dur)
    }
}

/// The completion seam, forwarded from the registration.
///
/// A task parks on `Depends(RegisteredFd)` in both modes; when it wakes it asks
/// here whether its bytes are already in hand, or whether it must `read` them.
#[cfg(all(target_os = "linux", feature = "uring"))]
impl<T: AsRawFd> completion::CompletionSource for RegisteredFd<T> {
    fn is_completion_source(&self) -> bool {
        completion::CompletionSource::is_completion_source(&self.registration)
    }

    fn take_completions(&self) -> Vec<completion::Completion> {
        completion::CompletionSource::take_completions(&self.registration)
    }

    fn has_completions(&self) -> bool {
        completion::CompletionSource::has_completions(&self.registration)
    }
}

impl<T: AsRawFd> RegisteredFd<T> {
    /// Create a new RegisteredFd with default interest (readable + writable).
    ///
    /// The fd is immediately registered with the poll::Selector.
    /// The fd MUST be in nonblocking mode for correct operation.
    pub fn new(inner: T, registry: &crate::native::poll::Registry, token: Token) -> io::Result<Self> {
        Self::with_interest(inner, registry, token, Interest::READABLE | Interest::WRITABLE)
    }

    /// Create with specific interest.
    pub fn with_interest(
        inner: T,
        registry: &crate::native::poll::Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<Self> {
        let fd = inner.as_raw_fd();
        let registration = FdRegistration::new(fd, registry, token, interest)?;

        Ok(Self {
            registration,
            inner: Some(inner),
        })
    }

    /// WHY: the transport-facing opt-in of Decision 14 F4. A transport that will
    /// consume bytes through [`completion::CompletionSource`] declares it at
    /// registration; only then does the kernel start reading the socket for it.
    /// Plain [`RegisteredFd::new`] stays byte-transparent, so nothing that reads
    /// its own fd is disturbed by the reactor's backend.
    ///
    /// WHAT: register `inner` with the shared reactor, opting into completion
    /// mode where the backend and the fd allow it.
    ///
    /// HOW: requires the shared reactor's registry (`Reactor::get()?.registry()`);
    /// a caller-owned registry has no completion inbox. Check
    /// [`completion::CompletionSource::is_completion_source`] afterwards: it is
    /// `false` for non-sockets, listening sockets, and non-completion backends,
    /// and the caller must then use its ordinary read path.
    ///
    /// # Errors
    /// The selector's `io::Error` if registration fails.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn with_completion(
        inner: T,
        registry: &crate::native::poll::Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<Self> {
        let fd = inner.as_raw_fd();
        let registration = FdRegistration::with_completion(fd, registry, token, interest)?;
        Ok(Self { registration, inner: Some(inner) })
    }

    /// Create, returning the original inner value on failure.
    pub fn try_new(
        inner: T,
        registry: &crate::native::poll::Registry,
        token: Token,
    ) -> Result<Self, FdRegistrationError<T>> {
        let fd = inner.as_raw_fd();
        match FdRegistration::new(fd, registry, token, Interest::READABLE | Interest::WRITABLE) {
            Ok(registration) => Ok(Self {
                registration,
                inner: Some(inner),
            }),
            Err(e) => Err(FdRegistrationError::Failed { error: e, inner }),
        }
    }

    /// The underlying registration — used by `ReadyGuard` to clear latched
    /// readiness in the shared reactor when an I/O op reports `WouldBlock`.
    pub(crate) fn registration(&self) -> &FdRegistration {
        &self.registration
    }

    /// WHY: the one call a transport changes to adopt completion mode. In
    /// io_uring completion mode the bytes are copied out of the kernel-filled
    /// inbox with **no syscall**; on every other backend this is `read(2)`.
    ///
    /// WHAT: read up to `buf.len()` bytes, nonblocking.
    ///
    /// HOW: see [`FdRegistration::read_bytes`]. `Ok(0)` is end-of-stream and
    /// `WouldBlock` means "park again", identically on both paths.
    ///
    /// # Errors
    /// `WouldBlock` when nothing is available; the socket's error otherwise.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn read_bytes(&self, buf: &mut [u8]) -> io::Result<usize> {
        let fd = self.inner.as_ref().expect("inner present until into_inner").as_raw_fd();
        self.registration.read_bytes(fd, buf)
    }

    /// Get a shared reference to the inner object.
    pub fn get_ref(&self) -> &T {
        self.inner.as_ref().unwrap()
    }

    /// Get a mutable reference to the inner object.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap()
    }

    /// Deregister and return ownership of the inner object.
    pub fn into_inner(mut self) -> T {
        let fd = self.inner.as_ref().unwrap().as_raw_fd();
        let _ = self.registration.deregister(fd);
        self.inner.take().unwrap()
    }

    /// Poll for read readiness.
    ///
    /// Returns immediately with:
    /// - Ready(guard) if the fd is readable
    /// - NotReady if no data is available
    /// - Error if the fd was deregistered or closed
    ///
    /// The guard must be explicitly handled via try_io(), clear_ready(), or retain_ready().
    pub fn poll_readable(&self) -> PollResult<ReadyGuard<'_, T>> {
        let ready = match self.registration.refresh_readiness() {
            Ok(r) => r,
            Err(e) => return PollResult::Error(e),
        };

        // 1. Check for error condition FIRST
        if ready.is_error() {
            return PollResult::Error(io::Error::new(
                io::ErrorKind::Other,
                "file descriptor has an error condition (EPOLLERR)",
            ));
        }

        // 2. Check for read-closed — peer shut down, pipe broken, EOF
        if ready.is_read_closed() {
            return PollResult::Error(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "read end of file descriptor is closed (EPOLLHUP / broken pipe)",
            ));
        }

        // 3. Check for readability
        if ready.is_readable() {
            return PollResult::Ready(ReadyGuard::new(self, Ready::READABLE));
        }

        // 4. No readiness detected
        PollResult::NotReady
    }

    /// Poll for write readiness.
    pub fn poll_writable(&self) -> PollResult<ReadyGuard<'_, T>> {
        let ready = match self.registration.refresh_readiness() {
            Ok(r) => r,
            Err(e) => return PollResult::Error(e),
        };

        if ready.is_error() {
            return PollResult::Error(io::Error::new(
                io::ErrorKind::Other,
                "file descriptor has an error condition",
            ));
        }

        if ready.is_write_closed() {
            return PollResult::Error(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "write end of file descriptor is closed",
            ));
        }

        if ready.is_writable() {
            return PollResult::Ready(ReadyGuard::new(self, Ready::WRITABLE));
        }

        PollResult::NotReady
    }

    /// Poll for readiness with the given interest flags.
    ///
    /// Checks the readiness bitmask for any of the requested flags
    /// and returns a guard for the first matching readiness state.
    pub fn poll_ready(&self, interest: Interest) -> PollResult<ReadyGuard<'_, T>> {
        let ready = match self.registration.refresh_readiness() {
            Ok(r) => r,
            Err(e) => return PollResult::Error(e),
        };

        // Check error first
        if ready.is_error() {
            return PollResult::Error(io::Error::new(
                io::ErrorKind::Other,
                "file descriptor has an error condition",
            ));
        }

        // Check closed states
        if interest.is_readable() && ready.is_read_closed() {
            return PollResult::Error(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "read end of file descriptor is closed",
            ));
        }

        if interest.is_writable() && ready.is_write_closed() {
            return PollResult::Error(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "write end of file descriptor is closed",
            ));
        }

        // Check readable
        if interest.is_readable() && ready.is_readable() {
            return PollResult::Ready(ReadyGuard::new(self, Ready::READABLE));
        }

        // Check writable
        if interest.is_writable() && ready.is_writable() {
            return PollResult::Ready(ReadyGuard::new(self, Ready::WRITABLE));
        }

        PollResult::NotReady
    }
}

impl<T: AsRawFd> AsRawFd for RegisteredFd<T> {
    fn as_raw_fd(&self) -> StdRawFd {
        self.inner.as_ref().unwrap().as_raw_fd()
    }
}

#[cfg(unix)]
impl<T: std::os::unix::io::AsFd + AsRawFd> std::os::unix::io::AsFd for RegisteredFd<T> {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        self.inner.as_ref().unwrap().as_fd()
    }
}

impl<T: AsRawFd> Drop for RegisteredFd<T> {
    fn drop(&mut self) {
        // Deregister from the poll selector so we don't get stale events.
        // This is critical: if we drop without deregistering, the selector
        // may still have this fd registered and will return events for it,
        // causing use-after-free on the readiness AtomicU8.
        if let Some(ref inner) = self.inner {
            let fd = inner.as_raw_fd();
            if let Err(e) = self.registration.deregister(fd) {
                // Log but don't panic — the fd might already be closed
                tracing::warn!("RegisteredFd: failed to deregister on drop: {}", e);
            }
        }
    }
}
