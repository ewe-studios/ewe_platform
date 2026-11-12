//! Waiting on an HTTP/1.1 connection from inside the dispatch future — native
//! only.
//!
//! WHY: the HTTP/1.1 server hands [`ConnectRpcServe`](super::server::ConnectRpcServe)
//! a non-blocking socket. A read of a body the peer has not sent yet, and a
//! write to a peer that has stopped reading, both fail with `WouldBlock`.
//! That is backpressure, not an error: the RPC must wait for the socket. It
//! must wait without blocking the thread inside a poll (the handler, the
//! protocol tasks and the other half of the exchange share this future) and
//! without spinning or sleeping.
//!
//! WHAT: [`ConnIo`] — a per-request driver shared by [`IoReady`] (wait for the
//! socket to become readable or writable, or time out), [`Hangup`] (wait for
//! the peer to close or reset the connection) and the call's deadline timer.
//! Each future owns an `Arc<ConnIo>`, so the connection task that polls them
//! stays a plain `Future`.
//!
//! HOW: a future that would block registers its waker and returns `Pending`.
//! A kernel-fd transport (TCP, Unix, rustls) parks on the shared reactor
//! through [`FdReadiness`]; the overlay has no fd and parks on a one-shot wake
//! callback its driver fires. A deadline parks on valtron's process-wide timer
//! thread. Nothing blocks, spins or sleeps.
//!
//! A hangup is the peer shutting down its side (`POLLRDHUP` on Linux/Android,
//! `POLLHUP` elsewhere) or the socket failing (`POLLHUP`/`POLLERR`). Like Go's
//! `net/http`, which cancels a request's context when its background read sees
//! EOF, and nginx, which ends a request whose client closed the connection, a
//! peer that half-closes counts as gone. Data the peer sends ahead (a pipelined
//! request) is not a hangup and is left unread — the close bit only latches
//! once that data is read, so a pipelined request is not mistaken for a close.
//!
//! Transports are waited on through `RawStream::conn_readiness` (Feature 06):
//! a kernel-fd transport (TCP, Unix, rustls) is polled with the direction the
//! TLS session needs (`wants_write` first, then "still handshaking"), not the
//! direction the application asked for; the WireGuard overlay has no fd and is
//! woken by its driver through a wake callback. A transport that still cannot
//! report readiness (a completion socket, openssl/native-tls) parks on the
//! valtron timer and re-polls at the configured poll-fallback interval instead
//! of failing with `Unsupported` or spinning.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::timer::{self, Sleep};
#[cfg(unix)]
use foundation_netio::native::connection::{ConnReadiness, ConnWaker};
use foundation_netio::native::readiness::IoInterest;
use foundation_netio::netcap::RawStream;
#[cfg(unix)]
use foundation_reactor::FdReadiness;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

/// Drives one request's waits on the connection's socket. See the module docs.
#[cfg(unix)]
pub(crate) struct ConnIo {
    /// How to wait for this connection's readiness, when the transport reports
    /// it (kernel fd, rustls, the overlay). `None` is a completion socket or an
    /// openssl/native-tls stream, which park on the valtron timer instead
    /// ([`Self::poll_fallback_interval`]).
    readiness: Option<ConnReadiness>,
    /// The shared-reactor registration for a kernel-fd transport; `None` for
    /// the overlay (woken by callbacks) and for the poll-fallback transports.
    fd_ready: Option<FdReadiness>,
    /// The raw fd to poll for a peer close/error when `readiness` is `None`
    /// (the poll-fallback transports both expose one). `None` for the overlay.
    fallback_fd: Option<RawFd>,
    /// How often the poll-fallback transports re-probe. See
    /// [`ServeTimeouts::poll_fallback_interval`](super::server::ServeTimeouts).
    poll_fallback_interval: Duration,
}

#[cfg(not(unix))]
pub(crate) struct ConnIo {
    _priv: (),
}

impl ConnIo {
    /// A driver for `conn`, with `poll_fallback_interval` as the re-probe period
    /// for transports that cannot report readiness (see the module docs).
    ///
    /// # Errors
    /// The reactor could not register the connection's fd.
    #[cfg(unix)]
    pub(crate) fn new(
        conn: &SharedByteBufferStream<RawStream>,
        poll_fallback_interval: Duration,
    ) -> io::Result<Self> {
        let readiness = conn.with_inner_ref(|raw: &RawStream| raw.conn_readiness());
        let fd_ready = match &readiness {
            Some(ConnReadiness::Fd(fd, _)) => Some(FdReadiness::register(*fd)?),
            _ => None,
        };
        // The poll-fallback transports expose a raw fd (openssl/native-tls wrap a
        // TCP socket; the completion socket has one too), even though they do not
        // report readiness. Keep it so `gone_now` can watch for a close/error.
        let fallback_fd = if readiness.is_none() {
            let fd = conn.with_inner_ref(|raw: &RawStream| raw.as_raw_fd());
            (fd >= 0).then_some(fd)
        } else {
            None
        };
        Ok(Self {
            readiness,
            fd_ready,
            fallback_fd,
            poll_fallback_interval,
        })
    }

    #[cfg(not(unix))]
    pub(crate) fn new(
        _conn: &SharedByteBufferStream<RawStream>,
        _poll_fallback_interval: Duration,
    ) -> io::Result<Self> {
        Ok(Self { _priv: () })
    }

    /// Wait for the connection to become ready in `interest`'s direction (or
    /// hang up, so the next read or write sees why), or for `timeout` to pass.
    #[must_use]
    pub(crate) fn ready(
        self: &Arc<Self>,
        interest: IoInterest,
        timeout: Duration,
        ctx: &foundation_context::Ctx,
    ) -> IoReady {
        IoReady::new(Arc::clone(self), interest, timeout, ctx)
    }

    /// Wait for the peer to close or reset the connection (see the module docs
    /// for what counts). Fails with `Unsupported` on a transport that cannot
    /// report it, so the caller can tell "never" from "not yet".
    #[must_use]
    pub(crate) fn hangup(self: &Arc<Self>, ctx: foundation_context::Ctx) -> Hangup {
        Hangup::new(Arc::clone(self), ctx)
    }
}

#[cfg(unix)]
impl ConnIo {
    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "this transport cannot report socket readiness, so the RPC cannot wait for the peer",
        )
    }

    /// Whether this transport reports readiness (`Some`) or parks on the valtron
    /// timer and re-polls (completion socket, openssl/native-tls).
    fn uses_poll_fallback(&self) -> bool {
        self.readiness.is_none()
    }

    /// The re-probe period for poll-fallback transports. See
    /// [`ServeTimeouts::poll_fallback_interval`](super::server::ServeTimeouts).
    fn poll_fallback_interval(&self) -> Duration {
        self.poll_fallback_interval
    }

    /// Whether the connection is ready in `interest`'s direction right now.
    /// A poll-fallback transport answers "not ready yet" — its wait future
    /// re-polls through the valtron timer instead of erroring.
    fn ready_now(&self, interest: IoInterest) -> io::Result<bool> {
        match &self.readiness {
            Some(r) => r.ready_now(interest),
            None => Ok(false),
        }
    }

    /// Whether the peer has closed or reset the connection right now. A
    /// poll-fallback transport probes its raw fd for a close/error.
    fn gone_now(&self) -> io::Result<bool> {
        match &self.fd_ready {
            Some(r) => {
                let ready = r.ready();
                Ok(ready.is_read_closed() || ready.is_write_closed() || ready.is_error())
            }
            None => match self.fallback_fd {
                Some(fd) => Self::fd_gone(fd),
                None => Err(Self::unsupported()),
            },
        }
    }

    /// Probe `fd` with `poll(2)` for a close (`POLLHUP`, `POLLRDHUP` on Linux) or
    /// an error (`POLLERR`, `POLLNVAL`). The probe is non-blocking (timeout 0), so
    /// it never blocks the poll of the calling future. An interrupted probe
    /// answers "not gone yet" and is re-probed later.
    fn fd_gone(fd: RawFd) -> io::Result<bool> {
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // `POLLRDHUP` separates a half-close (FIN) from a full close on Linux;
        // other platforms report a full close as `POLLHUP`. The flag must be
        // requested in `events` for `poll` to report it.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            pfd.events |= libc::POLLRDHUP;
        }
        // SAFETY: `pfd` is one valid pollfd and outlives the call.
        let rc = unsafe { libc::poll(&raw mut pfd, 1, 0) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                return Ok(false);
            }
            return Err(err);
        }
        let revents = pfd.revents;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let closed =
            (revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL | libc::POLLRDHUP)) != 0;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let closed = (revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL)) != 0;
        Ok(closed)
    }

    /// Register `waker` so the connection wakes it when `interest` becomes
    /// ready. A kernel fd is watched in the TLS-corrected direction; the
    /// overlay registers a one-shot wake callback; a poll-fallback transport
    /// has nothing to register — its wait future re-arms the valtron timer.
    fn register(&self, interest: IoInterest, waker: &Waker) -> io::Result<()> {
        match &self.readiness {
            Some(ConnReadiness::Fd(_, _)) => {
                let fd_ready = self.fd_ready.as_ref().ok_or_else(Self::unsupported)?;
                let (_, corrected) = self
                    .readiness
                    .as_ref()
                    .and_then(|r| r.poll_fd(interest))
                    .ok_or_else(Self::unsupported)?;
                fd_ready.set_writable(corrected == IoInterest::Writable)?;
                if !fd_ready.register_waker(waker) {
                    // The reactor no longer holds this fd, so it can never wake
                    // the task. Fail the poll instead of parking forever.
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "socket is no longer registered with the reactor",
                    ));
                }
                Ok(())
            }
            Some(ConnReadiness::Overlay(_)) => {
                let waker = waker.clone();
                let cb: ConnWaker = Arc::new(move || waker.wake_by_ref());
                self.readiness
                    .as_ref()
                    .expect("matched an overlay")
                    .register_waker(interest, cb);
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Register `waker` for a hangup. Only a kernel fd can report one (its read
    /// registration latches the close); a poll-fallback transport re-polls its
    /// fd through the valtron timer; the overlay has nothing to watch.
    fn register_hangup(&self, waker: &Waker) -> io::Result<()> {
        match &self.fd_ready {
            Some(r) => {
                if !r.register_waker(waker) {
                    // As in `register`: a deregistered fd never wakes the task.
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "socket is no longer registered with the reactor",
                    ));
                }
                Ok(())
            }
            None => match self.fallback_fd {
                Some(_) => Ok(()),
                None => Err(Self::unsupported()),
            },
        }
    }
}

#[cfg(not(unix))]
impl ConnIo {
    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "waiting on a connection is only supported on unix",
        )
    }

    fn uses_poll_fallback(&self) -> bool {
        false
    }

    fn poll_fallback_interval(&self) -> Duration {
        Duration::ZERO
    }

    fn ready_now(&self, _interest: IoInterest) -> io::Result<bool> {
        Err(Self::unsupported())
    }

    fn gone_now(&self) -> io::Result<bool> {
        Err(Self::unsupported())
    }

    fn register(&self, _interest: IoInterest, _waker: &Waker) -> io::Result<()> {
        Err(Self::unsupported())
    }

    fn register_hangup(&self, _waker: &Waker) -> io::Result<()> {
        Err(Self::unsupported())
    }
}

/// See [`ConnIo::ready`].
pub(crate) struct IoReady {
    io: Arc<ConnIo>,
    interest: IoInterest,
    timeout: Duration,
    /// The valtron timer that wakes this future at the deadline.
    timer: Option<Sleep>,
    /// The context whose clock measures the timers of this future.
    ctx: foundation_context::Ctx,
    /// The valtron timer that re-polls a poll-fallback transport at
    /// [`ConnIo::poll_fallback_interval`]. `None` when not poll-fallback.
    fallback: Option<Sleep>,
}

impl IoReady {
    pub(crate) fn new(
        io: Arc<ConnIo>,
        interest: IoInterest,
        timeout: Duration,
        ctx: &foundation_context::Ctx,
    ) -> Self {
        let deadline = ctx.clock().now() + timeout;
        Self {
            io,
            interest,
            timeout,
            timer: Some(timer::sleep_until(deadline, ctx)),
            ctx: ctx.clone(),
            fallback: None,
        }
    }

    fn timed_out(timeout: Duration, interest: IoInterest) -> io::Error {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("the connection did not become {interest} within {timeout:?}"),
        )
    }
}

impl Future for IoReady {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this.io.ready_now(this.interest) {
            Ok(true) => return Poll::Ready(Ok(())),
            Ok(false) => {}
            Err(e) => return Poll::Ready(Err(e)),
        }
        if this.io.uses_poll_fallback() {
            // Park on the valtron timer for one poll-fallback interval, then
            // return `Ok` so the caller retries its read or write. The retry is
            // paced at the configured interval; the wake is a timer wake, never
            // a spin.
            if this.fallback.is_none() {
                this.fallback = Some(timer::sleep_for(
                    this.io.poll_fallback_interval(),
                    &this.ctx,
                ));
            }
            if let Some(fallback) = this.fallback.as_mut() {
                if Pin::new(fallback).poll(cx).is_ready() {
                    return Poll::Ready(Ok(()));
                }
            }
        } else if let Err(e) = this.io.register(this.interest, cx.waker()) {
            return Poll::Ready(Err(e));
        }
        // The timer wakes this future at the deadline. A `Ready` here means the
        // deadline passed in the window between the probes above.
        if let Some(timer) = this.timer.as_mut() {
            if let Poll::Ready(()) = Pin::new(timer).poll(cx) {
                return Poll::Ready(Err(Self::timed_out(this.timeout, this.interest)));
            }
        }
        Poll::Pending
    }
}

/// See [`ConnIo::hangup`].
pub(crate) struct Hangup {
    io: Arc<ConnIo>,
    /// The valtron timer that re-polls a poll-fallback transport's fd at
    /// [`ConnIo::poll_fallback_interval`]. `None` when not poll-fallback.
    fallback: Option<Sleep>,
    /// The context whose clock times the poll-fallback timer.
    ctx: foundation_context::Ctx,
}

impl Hangup {
    pub(crate) fn new(io: Arc<ConnIo>, ctx: foundation_context::Ctx) -> Self {
        Self {
            io,
            fallback: None,
            ctx,
        }
    }
}

impl Future for Hangup {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this.io.gone_now() {
            Ok(true) => Poll::Ready(Ok(())),
            Ok(false) => {
                if this.io.uses_poll_fallback() {
                    // Park on the valtron timer for one poll-fallback interval,
                    // then re-probe `gone_now`. The wake is a timer wake, never
                    // a spin.
                    if this.fallback.is_none() {
                        this.fallback = Some(timer::sleep_for(
                            this.io.poll_fallback_interval(),
                            &this.ctx,
                        ));
                    }
                    if let Some(fallback) = this.fallback.as_mut() {
                        if Pin::new(fallback).poll(cx).is_ready() {
                            this.fallback = None;
                            cx.waker().wake_by_ref();
                        }
                    }
                    Poll::Pending
                } else {
                    match this.io.register_hangup(cx.waker()) {
                        Ok(()) => Poll::Pending,
                        Err(e) => Poll::Ready(Err(e)),
                    }
                }
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
