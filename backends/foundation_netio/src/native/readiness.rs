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

use foundation_core::valtron::{AnyReadiness, EventReadiness, EventReadinessPtr};

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
        // A zero timeout makes the call return immediately.
        let rc = unsafe { libc::poll(&raw mut pfd, 1, 0) };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                return Ok(false);
            }
            return Err(err);
        }
        let woke = events | libc::POLLERR | libc::POLLHUP | libc::POLLNVAL;
        Ok(rc > 0 && (pfd.revents & woke) != 0)
    }
}

#[cfg(unix)]
impl EventReadiness for SocketReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        match self.poll_now() {
            Ok(ready) => ready,
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
#[derive(Debug, Clone, Copy)]
pub struct DeadlineReadiness {
    deadline: Instant,
}

impl DeadlineReadiness {
    /// Ready at `deadline`.
    #[must_use]
    pub fn at(deadline: Instant) -> Self {
        Self { deadline }
    }

    /// Ready `timeout` from now.
    #[must_use]
    pub fn after(timeout: Duration) -> Self {
        Self::at(Instant::now() + timeout)
    }

    /// The instant this signal becomes ready.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Whether the deadline has passed.
    #[must_use]
    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

impl EventReadiness for DeadlineReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.expired()
    }
}

/// Compose an I/O readiness signal with a deadline: ready when either is.
#[must_use]
pub fn io_or_deadline(io: EventReadinessPtr, deadline: Instant) -> EventReadinessPtr {
    Arc::new(AnyReadiness::either(
        io,
        Arc::new(DeadlineReadiness::at(deadline)),
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::io::AsRawFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn readable_only_after_peer_writes() {
        let (a, mut b) = UnixStream::pair().expect("socket pair");
        let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Readable);
        assert!(!signal.is_ready(None), "nothing written yet");

        b.write_all(b"x").expect("peer write");
        assert!(signal.is_ready(None), "peer wrote a byte");
    }

    #[test]
    fn hang_up_counts_as_readable() {
        let (a, b) = UnixStream::pair().expect("socket pair");
        let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Readable);
        assert!(!signal.is_ready(None));

        drop(b);
        assert!(signal.is_ready(None), "EOF must wake a parked reader");
    }

    #[test]
    fn fresh_socket_is_writable() {
        let (a, _b) = UnixStream::pair().expect("socket pair");
        let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Writable);
        assert!(signal.is_ready(None));
    }

    #[test]
    fn full_send_buffer_is_not_writable() {
        let (a, _b) = UnixStream::pair().expect("socket pair");
        a.set_nonblocking(true).expect("nonblocking");
        let mut writer = &a;
        let chunk = vec![0u8; 64 * 1024];
        loop {
            match writer.write(&chunk) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("unexpected write error: {e}"),
            }
        }
        let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Writable);
        assert!(
            !signal.is_ready(None),
            "the peer never read, so the buffer is full"
        );
    }

    #[test]
    fn deadline_fires_and_composes() {
        let (a, _b) = UnixStream::pair().expect("socket pair");
        let io: EventReadinessPtr =
            Arc::new(SocketReadiness::new(a.as_raw_fd(), IoInterest::Readable));

        let later = io_or_deadline(io.clone(), Instant::now() + Duration::from_secs(60));
        assert!(
            !later.is_ready(None),
            "neither the socket nor the deadline is ready"
        );

        let past = io_or_deadline(io, Instant::now());
        assert!(past.is_ready(None), "a passed deadline wakes the task");
        assert!(!DeadlineReadiness::after(Duration::from_secs(60)).expired());
    }
}
