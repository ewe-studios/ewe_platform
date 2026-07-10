//! Runtime selector dispatch for Linux (F42 — Decision 14 Scope §2).
//!
//! WHY: Decision 14 OQ#14.3 requires the backend to be chosen by a *functional
//! probe at runtime*, with epoll as the automatic fallback. Before F42 the
//! choice was a `#[cfg(feature = "uring")]`, which compiled epoll out of the
//! binary entirely — so there was nothing to fall back to, an explicit
//! `IOUring` request could not fail with a probe reason, and `Auto` could not
//! surface a choice it never made.
//!
//! WHAT: [`Selector`] — an enum over the concrete Linux selectors, forwarding
//! the internal selector interface to whichever the ladder chose.
//!
//! HOW: [`Selector::new`] resolves [`BackendPreference::Auto`] through
//! [`backend::select`] and constructs the matching selector.
//! [`Selector::with_preference`] does the same for an explicit request, where a
//! probe failure is a hard error rather than a demotion.
//!
//! The dispatch is a `match` on a two-variant enum, monomorphic per call site
//! after inlining. Readiness checks do not reach it at all — they read the
//! reactor's cached bits — so it sits only on register/deregister/poll.

use std::io;
use std::time::Duration;

use super::super::super::super::event::Events;
use super::super::super::super::{backend, Backend, BackendPreference};
use super::epoll;
#[cfg(feature = "uring")]
use super::uring;
use crate::native::poll::{Interest, Token};

/// The raw fd type on Linux.
pub type RawFd = std::os::unix::io::RawFd;

/// A Linux selector, chosen at runtime.
#[derive(Debug)]
pub enum Selector {
    /// `epoll`, the portable Linux fallback.
    Epoll(epoll::Selector),
    /// `io_uring` in readiness mode (multishot poll).
    #[cfg(feature = "uring")]
    Uring(uring::Selector),
}

/// Forward one method to whichever selector this is.
macro_rules! dispatch {
    ($self:expr, |$sel:ident| $body:expr) => {
        match $self {
            Selector::Epoll($sel) => $body,
            #[cfg(feature = "uring")]
            Selector::Uring($sel) => $body,
        }
    };
}

impl Selector {
    /// WHY: `Poll::new()` wants the best backend this kernel supports, and
    /// wants to be told which one that turned out to be.
    ///
    /// WHAT: construct the selector chosen by the `Auto` ladder.
    ///
    /// HOW: delegates to [`Selector::with_preference`] with
    /// [`BackendPreference::Auto`], which never fails on Linux.
    ///
    /// # Errors
    /// Returns an `io::Error` if the chosen selector cannot be constructed
    /// (e.g. `epoll_create1` fails under an fd limit).
    ///
    /// # Panics
    /// Never panics.
    pub fn new() -> io::Result<Self> {
        Self::with_preference(BackendPreference::Auto)
    }

    /// WHY: an explicit `BackendPreference::Uring` is a requirement, not a
    /// hint — a perf-critical deployment silently running on epoll is the
    /// failure mode Decision 14 OQ#14.3 exists to prevent.
    ///
    /// WHAT: construct the selector for `preference`, honouring the ladder.
    ///
    /// HOW: [`backend::select`] runs the functional probe (once per call) and
    /// returns the [`Backend`] to build, or a `SelectionError` naming the
    /// concrete probe failure.
    ///
    /// # Errors
    /// Returns `io::ErrorKind::Unsupported` carrying the probe detail when an
    /// explicitly requested backend is unavailable, or the selector's own
    /// `io::Error` if construction fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn with_preference(preference: BackendPreference) -> io::Result<Self> {
        let chosen = backend::select(preference)?;
        Self::with_backend(chosen)
    }

    /// Construct the selector for an already-resolved backend.
    ///
    /// # Errors
    /// Returns an `io::Error` if the selector cannot be constructed, or
    /// `Unsupported` if `chosen` names a backend absent from this build.
    ///
    /// # Panics
    /// Never panics.
    pub fn with_backend(chosen: Backend) -> io::Result<Self> {
        match chosen {
            Backend::Epoll => Ok(Selector::Epoll(epoll::Selector::new()?)),

            #[cfg(feature = "uring")]
            Backend::UringReadiness => Ok(Selector::Uring(uring::Selector::new()?)),

            // The probe can report the completion tier as available before F43
            // ships the selector for it; `backend::select` will not choose it
            // while `COMPLETION_IMPLEMENTED` is false, so reaching here means a
            // caller named it directly.
            #[cfg(feature = "uring")]
            Backend::UringCompletion => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring completion mode is not implemented in this build (feature 43)",
            )),

            #[cfg(not(feature = "uring"))]
            Backend::UringReadiness | Backend::UringCompletion => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring support is not compiled into this build (enable the `uring` feature)",
            )),

            Backend::Kqueue => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kqueue is not a Linux backend",
            )),
        }
    }

    /// Which backend this selector is.
    ///
    /// # Panics
    /// Never panics.
    pub fn backend(&self) -> Backend {
        match self {
            Selector::Epoll(_) => Backend::Epoll,
            #[cfg(feature = "uring")]
            Selector::Uring(_) => Backend::UringReadiness,
        }
    }

    /// Register `fd` under `token` for `interest`.
    ///
    /// # Errors
    /// Propagates the underlying selector's registration error.
    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        dispatch!(self, |s| s.register_fd(fd, token, interest))
    }

    /// Re-register `fd` with a new token or interest.
    ///
    /// # Errors
    /// Propagates the underlying selector's error.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        dispatch!(self, |s| s.reregister_fd(fd, token, interest))
    }

    /// Deregister `fd`.
    ///
    /// # Errors
    /// Propagates the underlying selector's error.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        dispatch!(self, |s| s.deregister_fd(fd))
    }

    /// Register the wakeup eventfd under `token`.
    ///
    /// # Errors
    /// Propagates the underlying selector's error.
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        dispatch!(self, |s| s.register_waker(token))
    }

    /// Wake a thread blocked in [`Selector::poll`].
    ///
    /// # Errors
    /// Propagates the underlying selector's error.
    pub fn wake(&self, token: Token) -> io::Result<()> {
        dispatch!(self, |s| s.wake(token))
    }

    /// Drain the wakeup eventfd.
    pub fn clear_waker(&self) {
        dispatch!(self, |s| s.clear_waker());
    }

    /// Register a vnode watch. Unsupported on Linux backends.
    ///
    /// # Errors
    /// Always `io::ErrorKind::Unsupported`.
    pub fn register_vnode(&self, fd: RawFd, token: Token) -> io::Result<()> {
        dispatch!(self, |s| s.register_vnode(fd, token))
    }

    /// Deregister a vnode watch. No-op on Linux backends.
    ///
    /// # Errors
    /// Never returns `Err`.
    pub fn deregister_vnode(&self, fd: RawFd) -> io::Result<()> {
        dispatch!(self, |s| s.deregister_vnode(fd))
    }

    /// Wait for readiness, filling `events`.
    ///
    /// # Errors
    /// Propagates the underlying selector's error.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        dispatch!(self, |s| s.poll(events, timeout))
    }
}
