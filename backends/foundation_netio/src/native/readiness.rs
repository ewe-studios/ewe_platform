//! Socket readiness signals for parking valtron tasks on non-blocking sockets.
//!
//! WHY: the HTTP/1.1 client used blocking sockets with `SO_RCVTIMEO`. A read
//! that waited for the server held a valtron task worker for the whole wait. When
//! the server ran on the same pool (in-process tests, proxies, sidecars), that
//! wait could starve the very handler that would answer it. The canonical case is
//! the `Expect: 100-continue` probe, which sat out its full 3s timeout because
//! the server's handler never got a worker to send `100 Continue` on.
//!
//! WHAT: [`SocketReadiness`] is an [`EventReadiness`] that reports whether a
//! socket fd is readable or writable. [`DeadlineReadiness`] reports whether a
//! deadline has passed. A task that hits `WouldBlock` returns
//! `TaskStatus::Depends` on one of these, usually both composed with
//! [`AnyReadiness`], and the executor parks it until the socket is ready or the
//! deadline passes.
//!
//! HOW: `is_ready` runs `poll(2)` on the single fd with a zero timeout. It never
//! blocks, which matters because the executor calls `is_ready` from its scheduling
//! loop (and passes a `dur` hint that this impl deliberately ignores). This is
//! the same `Depends(EventReadiness)` mechanism the server uses with
//! `foundation_nativeapis`' `RegisteredFd`. That reactor sits above netio in the
//! crate graph (`nativeapis -> foundation_db -> netio`), so netio cannot register
//! with it. Netio instead answers readiness from the fd itself, which needs no
//! reactor at all.
//!
//! The fd is borrowed, not owned. A task only parks on a connection it still
//! holds, so the fd stays open for as long as the signal can be polled.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_context::Ctx;
#[cfg(unix)]
use foundation_core::valtron::{AnyReadiness, EventReadiness, EventReadinessPtr, ReadinessWaker};

/// Which direction of I/O a parked task is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoInterest {
    /// Wake when the socket has bytes to read, or has hung up or errored.
    Readable,
    /// Wake when the socket can accept more bytes, or has hung up or errored.
    Writable,
}

impl core::fmt::Display for IoInterest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Readable => f.write_str("readable"),
            Self::Writable => f.write_str("writable"),
        }
    }
}

/// Readiness of one socket fd in one direction, answered with a zero-timeout
/// `poll(2)`.
///
/// Hang-up and error conditions count as ready: the parked task must wake up
/// and see the EOF or error on its next read or write. Otherwise it would sleep
/// forever on a dead socket.
#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
pub struct SocketReadiness {
    fd: std::os::unix::io::RawFd,
    interest: IoInterest,
}

#[cfg(unix)]
impl SocketReadiness {
    /// Build a readiness signal for `fd` in the `interest` direction.
    ///
    /// The caller must keep the fd open for as long as the signal may be polled.
    /// A parked client task satisfies this because it owns the connection.
    #[must_use]
    pub fn new(fd: std::os::unix::io::RawFd, interest: IoInterest) -> Self {
        Self { fd, interest }
    }

    /// The direction this signal watches.
    #[must_use]
    pub fn interest(&self) -> IoInterest {
        self.interest
    }

    /// Poll the fd once without blocking.
    ///
    /// # Errors
    /// The `poll(2)` error, other than `EINTR` (which reports "not ready").
    pub fn poll_now(&self) -> std::io::Result<bool> {
        self.poll_timeout(Some(Duration::ZERO))
    }

    /// Poll the fd once, waiting at most `timeout` for readiness.
    ///
    /// `None` waits without a limit. This blocks the calling thread, so only a
    /// synchronous boundary calls it. A task parks on the `EventReadiness`
    /// implementation instead.
    ///
    /// # Errors
    /// The `poll(2)` error.
    pub fn poll_timeout(&self, timeout: Option<Duration>) -> std::io::Result<bool> {
        let timeout_ms = match timeout {
            None => -1,
            Some(dur) => {
                // Round up: a sub-millisecond wait must still wait once.
                let millis = dur.as_nanos().div_ceil(1_000_000).min(i32::MAX as u128);
                i32::try_from(millis).unwrap_or(i32::MAX)
            }
        };
        let events = match self.interest {
            IoInterest::Readable => libc::POLLIN,
            IoInterest::Writable => libc::POLLOUT,
        };
        let mut pfd = libc::pollfd {
            fd: self.fd,
            events,
            revents: 0,
        };
        // SAFETY: `pfd` is a valid, initialised `pollfd` and we pass a count of 1.
        let rc = unsafe { libc::poll(&raw mut pfd, 1, timeout_ms) };
        if rc < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let woke = events | libc::POLLERR | libc::POLLHUP | libc::POLLNVAL;
        Ok(rc > 0 && (pfd.revents & woke) != 0)
    }

    /// Wait for readiness until `deadline` (or without a limit when it is
    /// `None`), blocking the calling thread. Returns `false` when the deadline
    /// passed first.
    ///
    /// WHY: the synchronous constructors keep their signature for sync callers
    /// (feature 04 §2.3). They drive the same non-blocking state machine and
    /// wait here instead of calling a blocking socket operation.
    ///
    /// # Errors
    /// The `poll(2)` error.
    pub fn wait_blocking(&self, deadline: Option<Instant>, ctx: &Ctx) -> std::io::Result<bool> {
        loop {
            let remaining = match deadline {
                None => None,
                Some(deadline) => {
                    let now = ctx.clock().now();
                    if now >= deadline {
                        return Ok(false);
                    }
                    Some(deadline - now)
                }
            };
            match self.poll_timeout(remaining) {
                Ok(ready) => return Ok(ready),
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                Err(err) => return Err(err),
            }
        }
    }
}

#[cfg(unix)]
impl EventReadiness for SocketReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        match self.poll_now() {
            Ok(ready) => ready,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => false,
            Err(err) => {
                // An fd we cannot poll will not become pollable later. Report it
                // ready so the task wakes, retries its I/O, and gets the real error.
                tracing::warn!(fd = self.fd, ?err, "poll(2) failed; waking parked task");
                true
            }
        }
    }
}

/// Ready once a fixed point in time has passed.
///
/// WHY: a blocking socket enforced its timeouts in the kernel (`SO_RCVTIMEO`).
/// A non-blocking socket never times out on its own, so every wait carries a
/// deadline instead. It is composed with the socket signal via [`AnyReadiness`],
/// so the task wakes on whichever comes first.
pub struct DeadlineReadiness {
    deadline: Instant,
    /// The engine clock that decides when the deadline passes.
    ctx: Ctx,
}

impl core::fmt::Debug for DeadlineReadiness {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DeadlineReadiness")
            .field("deadline", &self.deadline)
            .finish_non_exhaustive()
    }
}

impl Clone for DeadlineReadiness {
    fn clone(&self) -> Self {
        Self {
            deadline: self.deadline,
            ctx: self.ctx.clone(),
        }
    }
}

impl DeadlineReadiness {
    /// Ready at `deadline`, on the clock of `ctx`.
    #[must_use]
    pub fn at(deadline: Instant, ctx: Ctx) -> Self {
        Self { deadline, ctx }
    }

    /// Ready `timeout` from now, on the clock of `ctx`.
    #[must_use]
    pub fn after(timeout: Duration, ctx: Ctx) -> Self {
        let deadline = ctx.clock().now() + timeout;
        Self::at(deadline, ctx)
    }

    /// The instant this signal becomes ready.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Whether the deadline has passed.
    #[must_use]
    pub fn expired(&self) -> bool {
        self.ctx.clock().now() >= self.deadline
    }
}

impl EventReadiness for DeadlineReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.expired()
    }

    fn deadline(&self) -> Option<Instant> {
        Some(self.deadline)
    }

    fn register(&self, _waker: &ReadinessWaker) -> bool {
        true
    }
}

/// Compose an I/O readiness signal with a deadline: ready when either is.
#[must_use]
pub fn io_or_deadline(io: EventReadinessPtr, deadline: Instant, ctx: Ctx) -> EventReadinessPtr {
    Arc::new(AnyReadiness::either(
        io,
        Arc::new(DeadlineReadiness::at(deadline, ctx)),
    ))
}

/// Drive a TLS handshake to completion on a non-blocking socket, blocking the
/// calling thread until it finishes or the deadline passes.
///
/// WHY: the synchronous constructors keep their signatures (feature 04 §2.3),
/// but they must not start a blocking handshake call. They start the handshake
/// non-blocking and wait here on the same readiness signal a valtron task parks
/// on.
///
/// WHAT: calls `drive` until it succeeds. A `WouldBlock` result asks
/// `poll_target` for the fd and direction the TLS session needs, then waits for
/// it.
///
/// HOW: the deadline comes from the clock of the caller's `ctx`, not from the
/// process clock in the logic below.
///
/// # Errors
/// The handshake error, `TimedOut` when the deadline passes first, or
/// `Unsupported` when the backend cannot report readiness.
#[cfg(unix)]
pub fn complete_handshake_blocking(
    mut drive: impl FnMut() -> std::io::Result<()>,
    mut poll_target: impl FnMut() -> Option<(std::os::unix::io::RawFd, IoInterest)>,
    timeout: Option<Duration>,
    ctx: &Ctx,
) -> std::io::Result<()> {
    let deadline = timeout.map(|timeout| ctx.clock().now() + timeout);
    loop {
        match drive() {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                let Some((fd, interest)) = poll_target() else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "the TLS backend cannot report handshake readiness",
                    ));
                };
                if !SocketReadiness::new(fd, interest).wait_blocking(deadline, ctx)? {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "TLS handshake timed out",
                    ));
                }
            }
            Err(err) => return Err(err),
        }
    }
}
