/// The `poll` module provides an I/O readiness selector extracted from mio patterns.
///
/// It wraps the OS-native readiness mechanism:
/// - **Linux**: `epoll` — `epoll_create1`, `epoll_ctl`, `epoll_wait`
/// - **macOS/BSD**: `kqueue` — `kqueue()`, `kevent()`
/// - **Windows**: `IOCP` — `CreateIoCompletionPort`, `GetQueuedCompletionStatus`
///
/// # Example
///
/// ```no_run
/// use foundation_nativeapis::{Poll, Events, Token, Interest};
/// use std::time::Duration;
///
/// let poll = Poll::new().unwrap();
/// let registry = poll.registry();
///
/// // Register a file descriptor (e.g., inotify fd, socket fd)
/// // registry.register(&mut source_fd, Token(0), Interest::READABLE).unwrap();
///
/// // Poll for readiness
/// let mut events = Events::with_capacity(16);
/// poll.poll(&mut events, Some(Duration::from_millis(100))).unwrap();
///
/// for event in events.iter() {
///     println!("Token {:?}: readable={}, writable={}",
///         event.token(), event.is_readable(), event.is_writable());
/// }
/// ```

pub mod event;
pub use event::{Events, Source};

pub mod sys;
pub use sys::SourceFd;

pub mod backend;
pub use backend::{Backend, BackendPreference, SelectionError};

/// The functional io_uring capability probe (Decision 14 OQ#14.3).
#[cfg(all(target_os = "linux", feature = "uring"))]
pub mod probe;
#[cfg(all(target_os = "linux", feature = "uring"))]
pub use probe::{ProbeError, UringCapabilities};

use std::io;
use std::sync::Arc;
use std::time::Duration;

/// An identifier for a registered source.
///
/// The user chooses the token value. When `poll()` returns an event,
/// the event's token tells you which fd became ready.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Token(pub usize);

/// The interest to track for a registered source.
///
/// Can be combined: `Interest::READABLE | Interest::WRITABLE`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Interest {
    bits: u8,
}

impl Interest {
    /// Track when the source is readable.
    pub const READABLE: Interest = Interest { bits: 0b01 };
    /// Track when the source is writable.
    pub const WRITABLE: Interest = Interest { bits: 0b10 };

    /// Returns `true` if readable interest is set.
    #[inline]
    pub const fn is_readable(self) -> bool {
        self.bits & Self::READABLE.bits != 0
    }

    /// Returns `true` if writable interest is set.
    #[inline]
    pub const fn is_writable(self) -> bool {
        self.bits & Self::WRITABLE.bits != 0
    }

    /// Combine interests.
    #[inline]
    pub const fn union(self, other: Interest) -> Interest {
        Interest {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOr for Interest {
    type Output = Interest;

    fn bitor(self, rhs: Interest) -> Interest {
        self.union(rhs)
    }
}

/// The I/O readiness poller.
///
/// Wraps the platform-specific selector (epoll fd / kqueue fd / IOCP handle)
/// and provides a cross-platform `poll()` method.
pub struct Poll {
    selector: Arc<sys::Selector>,
}

impl std::fmt::Debug for Poll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Poll").field("backend", &self.backend()).finish()
    }
}

impl Poll {
    /// Create a new `Poll` instance.
    ///
    /// Creates the platform-specific selector:
    /// - Linux: `epoll_create1(EPOLL_CLOEXEC)`
    /// - macOS/BSD: `kqueue()`
    /// - Windows: `CreateIoCompletionPort`
    pub fn new() -> io::Result<Self> {
        // Construct the selector and wrap it here rather than asking the
        // selector to hand back a `Registry`: on Linux both `epoll::Selector`
        // and `uring::Selector` are compiled, so only `Poll` knows which one
        // `Registry` is parameterised over.
        let selector = Arc::new(sys::Selector::new()?);
        Ok(Self { selector })
    }

    /// WHY: Decision 14 OQ#14.3 makes an explicit backend request a
    /// *requirement*. A deployment that asks for io_uring and silently gets
    /// epoll discovers it from latency graphs months later.
    ///
    /// WHAT: create a `Poll` on a specific backend preference.
    ///
    /// HOW: [`BackendPreference::Auto`] walks the probe ladder
    /// (uring-completion → uring-readiness → epoll) and logs the choice.
    /// [`BackendPreference::Uring`] fails hard if the probe says io_uring is
    /// unusable. [`BackendPreference::Epoll`] skips the probe entirely.
    ///
    /// # Errors
    /// `io::ErrorKind::Unsupported` carrying the concrete probe failure when an
    /// explicitly requested backend is unavailable, or the platform selector's
    /// own `io::Error`.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(target_os = "linux")]
    pub fn with_preference(preference: BackendPreference) -> io::Result<Self> {
        let selector = Arc::new(sys::Selector::with_preference(preference)?);
        Ok(Self { selector })
    }

    /// WHY: observability — operators need to see which backend a process
    /// actually landed on, not which one it was configured to prefer.
    ///
    /// WHAT: the backend this `Poll` is driving.
    ///
    /// HOW: reported by the runtime dispatch selector on Linux; a constant on
    /// platforms with a single backend.
    ///
    /// # Panics
    /// Never panics.
    pub fn backend(&self) -> Backend {
        #[cfg(target_os = "linux")]
        {
            self.selector.backend()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Backend::Kqueue
        }
    }

    /// WHY: in completion mode the kernel has already read the bytes; the
    /// transport pops them here rather than issuing `read(2)` (Decision 14 F4).
    ///
    /// WHAT: drain everything the kernel delivered for `token`.
    ///
    /// HOW: forwards to the selector. `None` on every backend without an inbox,
    /// which tells the caller to use its ordinary read path.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn take_completions(
        &self,
        token: Token,
    ) -> Option<Vec<sys::unix::selector::uring_completion::Completion>> {
        self.selector.take_completions(token)
    }

    /// Whether `token` has kernel-delivered bytes waiting.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn has_completions(&self, token: Token) -> bool {
        self.selector.has_completions(token)
    }

    /// Whether `token`'s bytes arrive as completions rather than needing a read.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn is_recv_token(&self, token: Token) -> bool {
        self.selector.is_recv_token(token)
    }

    /// Submit an `IORING_OP_SEND` for `token`/`fd` on the completion backend (F49).
    ///
    /// # Errors
    /// `WouldBlock` when the send pool is exhausted; `Unsupported` off the
    /// completion backend; the kernel's submission error otherwise.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn submit_send(&self, token: Token, fd: sys::RawFd, data: &[u8]) -> io::Result<usize> {
        self.selector.submit_send(token, fd, data)
    }

    /// Submit a zero-copy `IORING_OP_SEND_ZC` for `token`/`fd` (F50 Part C).
    ///
    /// # Errors
    /// `WouldBlock` when the send pool is exhausted; `Unsupported` off the
    /// completion backend; the kernel's submission error otherwise.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn submit_send_zc(&self, token: Token, fd: sys::RawFd, data: &[u8]) -> io::Result<usize> {
        self.selector.submit_send_zc(token, fd, data)
    }

    /// Zero-copy send from an owned [`ProvidedBuf`] — the `ProvidedBuf`-direct
    /// handoff (F50 Part C tail). No pool copy. Only the completion backend.
    ///
    /// # Errors
    /// The kernel's submission error; `Unsupported` off the completion backend.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn submit_send_zc_direct(
        &self,
        token: Token,
        fd: sys::RawFd,
        buf: sys::unix::selector::bufring::ProvidedBuf,
    ) -> io::Result<usize> {
        self.selector.submit_send_zc_direct(token, fd, buf)
    }

    /// Take finished sends for `token`. Empty off the completion backend.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn take_send_completions(
        &self,
        token: Token,
    ) -> Vec<sys::unix::selector::uring_completion::SendCompletion> {
        self.selector.take_send_completions(token)
    }

    /// Whether finished sends are waiting for `token`.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn has_send_completions(&self, token: Token) -> bool {
        self.selector.has_send_completions(token)
    }

    /// Whether `token` has an unfinished SEND in flight.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn has_pending_sends(&self, token: Token) -> bool {
        self.selector.has_pending_sends(token)
    }

    /// Register `fd`, opting its read path into completion mode where possible.
    ///
    /// Returns whether the kernel will now read this fd for you. See
    /// [`sys::unix::selector::dispatch::Selector::register_recv_fd`].
    ///
    /// # Errors
    /// Propagates the selector's registration error.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub fn register_recv_fd(
        &self,
        fd: sys::RawFd,
        token: Token,
        interest: Interest,
    ) -> io::Result<bool> {
        self.selector.register_recv_fd(fd, token, interest)
    }

    /// Returns a [`Registry`] for registering/deregistering sources.
    pub fn registry(&self) -> Registry {
        Registry {
            selector: self.selector.clone(),
        }
    }

    /// Wait for readiness events on registered sources.
    ///
    /// Blocks until at least one event occurs or the `timeout` expires.
    /// Returns `Ok(())` in both cases — check `events.len()` to see if
    /// any events were returned.
    ///
    /// # Arguments
    /// * `events` — container to write events into (must have capacity)
    /// * `timeout` — how long to block. `None` = block indefinitely.
    ///
    /// # Errors
    /// Returns an I/O error if the underlying syscall fails (e.g., interrupted).
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.selector.poll(events, timeout)
    }
}

/// Registration handle — used to add, modify, and remove source interests.
///
/// Each `Registry` is tied to its parent `Poll` instance. Multiple threads
/// can hold clones of the same `Registry` and register/deregister
/// sources concurrently.
#[derive(Clone)]
pub struct Registry {
    selector: Arc<sys::Selector>,
}

impl Registry {
    /// WHY: `FdRegistration` must honour the registry it is handed. It decides
    /// between the shared-reactor path (cached readiness, zero syscalls per
    /// check) and a caller-owned selector by asking whether the two registries
    /// front the *same* selector — not by silently preferring the singleton.
    ///
    /// WHAT: whether `self` and `other` register into the same selector.
    ///
    /// HOW: pointer equality of the `Arc<Selector>` both hold.
    ///
    /// # Panics
    /// Never panics.
    pub fn same_selector(&self, other: &Registry) -> bool {
        Arc::ptr_eq(&self.selector, &other.selector)
    }

    /// Register a source with the selector.
    ///
    /// # Arguments
    /// * `source` — the type to register (must implement [`event::Source`](event::Source))
    /// * `token` — identifier for this source, returned in events
    /// * `interest` — what readiness to track (READABLE, WRITABLE, or both)
    pub fn register(
        &self,
        source: &mut impl event::Source,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        source.register(self, token, interest)
    }

    /// Re-register a source with a new token and/or interest.
    pub fn reregister(
        &self,
        source: &mut impl event::Source,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        source.reregister(self, token, interest)
    }

    /// Deregister a source from the selector.
    pub fn deregister(&self, source: &mut impl event::Source) -> io::Result<()> {
        source.deregister(self)
    }

    /// Register a raw file descriptor.
    ///
    /// This is a convenience wrapper around `SourceFd` registration.
    /// The fd must already be in nonblocking mode.
    /// The fd is NOT closed when deregistered — ownership is not transferred.
    pub fn register_fd(&self, fd: sys::RawFd, token: Token, interest: Interest) -> io::Result<()> {
        self.selector.register_fd(fd, token, interest)
    }

    /// Re-register a raw file descriptor with a new token and/or interest.
    pub fn reregister_fd(&self, fd: sys::RawFd, token: Token, interest: Interest) -> io::Result<()> {
        self.selector.reregister_fd(fd, token, interest)
    }

    /// Deregister a raw file descriptor.
    pub fn deregister_fd(&self, fd: sys::RawFd) -> io::Result<()> {
        self.selector.deregister_fd(fd)
    }

    /// Register an EVFILT_VNODE filter for file watching (macOS/BSD only).
    /// On Linux/Windows, returns Unsupported.
    pub fn register_vnode(&self, fd: sys::RawFd, token: Token) -> io::Result<()> {
        self.selector.register_vnode(fd, token)
    }

    /// Deregister an EVFILT_VNODE filter (macOS/BSD only).
    pub fn deregister_vnode(&self, fd: sys::RawFd) -> io::Result<()> {
        self.selector.deregister_vnode(fd)
    }

    /// Access the internal selector (for platform-specific operations).
#[allow(dead_code)]
    pub(crate) fn selector(&self) -> &sys::Selector {
        &self.selector
    }
}

/// Waker — unblocks `poll()` from another thread.
///
/// When you call `wake()`, the next `poll()` call will return immediately
/// with an event for the waker's token. This is useful for shutdown signals
/// or other cross-thread notifications.
pub struct Waker {
    selector: Arc<sys::Selector>,
    token: Token,
}

impl Waker {
    /// Create a new waker associated with the given token.
    pub fn new(registry: &Registry, token: Token) -> io::Result<Self> {
        let selector = registry.selector.clone();
        selector.register_waker(token)?;
        Ok(Self { selector, token })
    }

    /// Wake the poll selector. The next `poll()` call will return
    /// immediately with an event for the waker's token.
    pub fn wake(&self) -> io::Result<()> {
        self.selector.wake(self.token)
    }
}
