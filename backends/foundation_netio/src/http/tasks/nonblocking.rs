//! Shared pieces for driving an HTTP/1.1 exchange on a non-blocking socket.
//!
//! WHY: every client task that touches the socket (sending the request head, the
//! `Expect: 100-continue` wait, writing the body, reading the response head,
//! streaming the response body) does the same three things. It moves bytes until
//! the socket would block. It parks on readiness instead of blocking a valtron
//! worker. It enforces an inactivity deadline where the kernel used to enforce
//! `SO_RCVTIMEO`. Keeping that logic in one place keeps the semantics identical
//! across tasks.
//!
//! WHAT: [`IdleDeadline`] (a re-armable inactivity deadline), [`PendingBytes`] (a
//! write buffer that survives `WouldBlock` without losing track of what was
//! accepted), [`flush`], and [`park`] (readiness, or deadline, or "this
//! `WouldBlock` is terminal").
//!
//! HOW: parking returns `TaskStatus::Depends(io_or_deadline(..))`. The executor
//! keeps the task off the worker until the socket turns ready or the deadline
//! passes, whichever comes first.

use std::io::Write;
use std::time::{Duration, Instant};

use foundation_core::valtron::EventReadinessPtr;

use crate::http::HttpClientConnection;
use crate::native::readiness::{io_or_deadline, IoInterest};

/// An inactivity deadline: armed when a task first parks, cleared when bytes move.
///
/// This mirrors `SO_RCVTIMEO`/`SO_SNDTIMEO`, which bound each wait and not the
/// whole exchange. A slow but steady peer never trips it. A silent one does.
#[derive(Debug, Clone, Copy)]
pub struct IdleDeadline {
    timeout: Duration,
    deadline: Option<Instant>,
}

impl IdleDeadline {
    /// A deadline `timeout` after the first park with no progress.
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            deadline: None,
        }
    }

    /// Arm the deadline if it is not armed yet, and return it.
    pub fn arm(&mut self) -> Instant {
        *self
            .deadline
            .get_or_insert_with(|| Instant::now() + self.timeout)
    }

    /// Bytes moved: the next wait gets a fresh budget.
    pub fn progress(&mut self) {
        self.deadline = None;
    }

    /// Whether an armed deadline has passed.
    #[must_use]
    pub fn expired(&self) -> bool {
        self.deadline.is_some_and(|d| Instant::now() >= d)
    }

    /// The configured inactivity budget.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// What a task should do after an I/O call returned `WouldBlock`.
pub enum Park {
    /// Return `TaskStatus::Depends(signal)`; the executor wakes the task on
    /// readiness or at the deadline.
    On(EventReadinessPtr),
    /// The deadline passed with no progress: fail with a timeout.
    Expired,
    /// The socket is blocking (its own read timeout already elapsed) or cannot
    /// report readiness: the `WouldBlock` is terminal.
    Unsupported,
}

/// Decide how to wait after a `WouldBlock` on `conn` in the `interest` direction.
#[must_use]
pub fn park(conn: &HttpClientConnection, interest: IoInterest, idle: &mut IdleDeadline) -> Park {
    if idle.expired() {
        return Park::Expired;
    }
    match conn.park_readiness(interest) {
        Some(io) => Park::On(io_or_deadline(io, idle.arm())),
        None => Park::Unsupported,
    }
}

/// Outcome of a non-blocking write or flush step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoStep {
    /// Everything was written (or flushed).
    Done,
    /// The socket would block. Park on writability and call again.
    Blocked,
}

/// Bytes waiting to be written, with how many the socket has accepted so far.
///
/// WHY: `write_all` cannot be used on a non-blocking socket. When it hits
/// `WouldBlock` partway through, it has already handed some bytes over and does
/// not say how many, so a retry would duplicate or drop data. This tracks the
/// offset across `WouldBlock`s.
#[derive(Debug, Default)]
pub struct PendingBytes {
    bytes: Vec<u8>,
    written: usize,
}

impl PendingBytes {
    /// Queue `bytes` for writing.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, written: 0 }
    }

    /// Whether every queued byte has been accepted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.written >= self.bytes.len()
    }

    /// Write as much as the socket accepts. Returns how many bytes moved and
    /// whether it finished.
    ///
    /// # Errors
    /// Any write error other than `WouldBlock`/`Interrupted`. `Ok(0)` from the
    /// writer means the socket can take no more, reported as `WriteZero`.
    pub fn write_to<W: Write>(&mut self, writer: &mut W) -> std::io::Result<(usize, IoStep)> {
        let mut moved = 0;
        while !self.is_empty() {
            match writer.write(&self.bytes[self.written..]) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "the socket accepted no bytes",
                    ));
                }
                Ok(n) => {
                    self.written += n;
                    moved += n;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Ok((moved, IoStep::Blocked));
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok((moved, IoStep::Done))
    }
}

/// Flush buffered output. On `WouldBlock` the buffered bytes are kept and the
/// caller parks on writability.
///
/// # Errors
/// Any flush error other than `WouldBlock`/`Interrupted`.
pub fn flush<W: Write>(writer: &mut W) -> std::io::Result<IoStep> {
    loop {
        match writer.flush() {
            Ok(()) => return Ok(IoStep::Done),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(IoStep::Blocked),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}
