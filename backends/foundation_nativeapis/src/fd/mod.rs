/// File descriptor management — wraps any raw fd with readiness tracking.
///
/// Adapted from tokio's `AsyncFd`, but works with our sync, task-driven model
/// and our cross-platform poll::Selector (epoll/kqueue/IOCP).
///
/// ## Key Types
///
/// - [`RegisteredFd<T>`] — wraps an IO object, registers it with the poll selector
/// - [`FdRegistration`] — tracks per-fd readiness state as an atomic bitmask
/// - [`ReadyGuard`] — returned by `poll_readable()`/`poll_writable()`, must be
///   explicitly handled to clear readiness
/// - [`Ready`] — bitmask of readiness states (READABLE, WRITABLE, READ_CLOSED, WRITE_CLOSED, ERROR)

pub mod guard;
pub mod error;

pub use guard::{MutReadyGuard, ReadyGuard, TryIoError};
pub use error::{FdRegistrationError, RegistrationError};

use crate::poll::{Events, Interest, Registry, Token};
use crate::poll::sys::RawFd;

use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd as StdRawFd};
use std::sync::Mutex;
use std::time::Duration;

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
pub struct FdRegistration {
    registry: crate::poll::Registry,
    token: Token,
    /// Mutex-protected Poll instance for readiness queries.
    poll: crate::poll::Poll,
    /// Last known readiness state — cached between poll calls.
    readiness: Mutex<Ready>,
}

impl FdRegistration {
    /// Create a new FdRegistration for the given raw fd.
    ///
    /// The fd is immediately registered with the poll::Selector.
    /// The fd MUST be in nonblocking mode for correct operation.
    pub fn new(
        fd: RawFd,
        registry: &crate::poll::Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<Self> {
        registry.register_fd(fd, token, interest)?;

        // Create a new Poll instance and register the fd with it
        let poll = crate::poll::Poll::new()?;
        poll.registry().register_fd(fd, token, interest)?;

        Ok(Self {
            registry: registry.clone(),
            token,
            poll,
            readiness: Mutex::new(Ready::EMPTY),
        })
    }

    /// Poll the selector and update the readiness cache.
    /// Returns the updated readiness bitmask.
    fn query_readiness(&self) -> io::Result<Ready> {
        let mut events = Events::with_capacity(16);
        self.poll.poll(&mut events, Some(Duration::ZERO))?;

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

    /// Deregister from the poll selector.
    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        self.registry.deregister_fd(fd)
    }
}

/// Wraps any type that produces a raw fd, registering it with our poll::Selector
/// and providing readiness polling + guarded I/O operations.
///
/// Adapted from tokio's AsyncFd, but works with our sync, task-driven model.
pub struct RegisteredFd<T: AsRawFd> {
    registration: FdRegistration,
    inner: Option<T>,
}

impl<T: AsRawFd> RegisteredFd<T> {
    /// Create a new RegisteredFd with default interest (readable + writable).
    ///
    /// The fd is immediately registered with the poll::Selector.
    /// The fd MUST be in nonblocking mode for correct operation.
    pub fn new(inner: T, registry: &crate::poll::Registry, token: Token) -> io::Result<Self> {
        Self::with_interest(inner, registry, token, Interest::READABLE | Interest::WRITABLE)
    }

    /// Create with specific interest.
    pub fn with_interest(
        inner: T,
        registry: &crate::poll::Registry,
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

    /// Create, returning the original inner value on failure.
    pub fn try_new(
        inner: T,
        registry: &crate::poll::Registry,
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
