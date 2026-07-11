//! Target-neutral I/O-gate types.
//!
//! WHY: [`ServerIo`] is the operator-facing switch for how a server reads bytes.
//! It is a plain data enum with no reactor dependency, so it is legal on every
//! target (including wasm, where it is inert) and can appear in a server
//! builder's public signature without dragging native-only types into it.
//!
//! WHAT: the [`ServerIo`] enum and its `Default`/`Display` impls.
//!
//! HOW: the native accept path (`crate::native`) maps each variant onto a
//! reactor backend preference and a registration choice; here the enum only
//! names the intent.

use core::fmt;

/// How a server acquires bytes from its accepted sockets.
///
/// WHY: an operator must be able to demand a specific I/O path — and be told at
/// startup when the kernel cannot deliver it — rather than silently getting
/// whatever the reactor happened to initialise with (Feature 48, no-silent-
/// defaults). Keeping [`ServerIo::Std`] the default means nothing changes for
/// any caller who does not ask.
///
/// WHAT: four modes. `Std` never touches the reactor; the other three register
/// the accepted fd with the shared reactor and differ only in whether reads pop
/// the io_uring completion inbox or issue `read(2)`.
///
/// HOW: the native accept helper
/// (`crate::native::accept_connection`) branches on this to build either a plain
/// `Connection::Tcp` or a `Connection::Completion`, and to choose the reactor
/// backend preference when it initialises the shared reactor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServerIo {
    /// No reactor, no registration. `read(2)` on the socket with the classic
    /// blocking / `WouldBlock`-retry loop — today's behaviour, and the default
    /// so existing servers are unaffected.
    #[default]
    Std,

    /// Register with the shared reactor; still `read(2)` on the socket, but park
    /// on `Depends(RegisteredFd)` (epoll, or io_uring multishot poll) instead of
    /// spinning.
    Readiness,

    /// Register with the shared reactor in completion mode: the kernel fills a
    /// buffer ring and `read` pops the inbox with **no syscall**. Falls back to
    /// `read(2)` transparently when the backend is not io_uring.
    Completion,

    /// Ask for the best available path; the choice is surfaced in a log line.
    Auto,
}

impl ServerIo {
    /// Whether this mode registers the accepted fd with the shared reactor.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn uses_reactor(self) -> bool {
        !matches!(self, ServerIo::Std)
    }

    /// Whether this mode opts the fd into the io_uring completion read path.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn is_completion(self) -> bool {
        matches!(self, ServerIo::Completion | ServerIo::Auto)
    }
}

impl fmt::Display for ServerIo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ServerIo::Std => "std",
            ServerIo::Readiness => "readiness",
            ServerIo::Completion => "completion",
            ServerIo::Auto => "auto",
        };
        f.write_str(name)
    }
}
