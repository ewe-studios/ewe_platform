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

impl Poll {
    /// Create a new `Poll` instance.
    ///
    /// Creates the platform-specific selector:
    /// - Linux: `epoll_create1(EPOLL_CLOEXEC)`
    /// - macOS/BSD: `kqueue()`
    /// - Windows: `CreateIoCompletionPort`
    pub fn new() -> io::Result<Self> {
        let (selector, _) = sys::Selector::new_with_registry()?;
        Ok(Self { selector })
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
