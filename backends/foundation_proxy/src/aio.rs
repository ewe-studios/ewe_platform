//! Async I/O building blocks for the proxy's non-HTTP relays.
//!
//! WHY: the raw TCP tunnel, the protocol-upgrade (WebSocket) splice, the raw TCP
//! and UDP passthrough listeners and the control socket used a thread per
//! connection and slept in a loop to poll. A thread per connection does not
//! scale, and a sleep wastes a core's time slice and delays every wake by the
//! sleep.
//!
//! WHAT: [`AsyncFd`] (a non-blocking fd that parks on the shared reactor),
//! [`splice`] (two-way copy between two connections), [`dial_tcp`] (a
//! non-blocking connect) and [`wrap_tcp`].
//!
//! HOW: every wait parks the task on a waker. A kernel socket parks on the
//! shared reactor ([`FdReadiness`]); the writer parks through
//! [`ResponseWriter`] (feature 61); a transport that cannot report readiness
//! (a completion socket) parks on the valtron timer for one
//! `poll_fallback_interval`. No call sleeps a thread.

use std::future::poll_fn;
use std::io::{self, Read};
use std::net::{SocketAddr, TcpStream};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::sleep_async;
use foundation_http::native::writer::{ResponseWriter, WriterConfig};
use foundation_iogate::{FdReadiness, ServerIo};
use foundation_netio::netcap::{PendingConnect, RawStream};
use foundation_netio::shared::client::body_reader::ReadWakeSource;
use foundation_netio::shared::client::{DnsResolver, SystemDnsResolver};
use futures_util::future::{select, Either};

/// A non-blocking fd that parks on the shared reactor.
///
/// The caller sets the fd non-blocking before [`AsyncFd::new`]. The fd must stay
/// open while this lives.
pub struct AsyncFd<T: AsRawFd> {
    io: T,
    ready: FdReadiness,
    write_armed: AtomicBool,
}

impl<T: AsRawFd> AsyncFd<T> {
    /// Register `io` with the reactor for read readiness.
    ///
    /// # Errors
    /// The reactor could not start or refused the fd.
    pub fn new(io: T) -> io::Result<Self> {
        let ready = FdReadiness::register(io.as_raw_fd())?;
        Ok(Self {
            io,
            ready,
            write_armed: AtomicBool::new(false),
        })
    }

    /// The wrapped value.
    pub fn get_ref(&self) -> &T {
        &self.io
    }

    /// Run `op` until it does not return `WouldBlock`, parking on read
    /// readiness between tries.
    ///
    /// # Errors
    /// Any error `op` returns other than `WouldBlock` and `Interrupted`.
    pub async fn read_with<R>(&self, mut op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        poll_fn(|cx| self.poll_op(cx, false, &mut op)).await
    }

    /// Like [`Self::read_with`], parking on write readiness.
    ///
    /// # Errors
    /// Any error `op` returns other than `WouldBlock` and `Interrupted`.
    pub async fn write_with<R>(&self, mut op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        poll_fn(|cx| self.poll_op(cx, true, &mut op)).await
    }

    fn poll_op<R>(
        &self,
        cx: &mut Context<'_>,
        write: bool,
        op: &mut impl FnMut(&T) -> io::Result<R>,
    ) -> Poll<io::Result<R>> {
        loop {
            match op(&self.io) {
                Ok(value) => {
                    if write && self.write_armed.swap(false, Ordering::SeqCst) {
                        // An idle socket is always writable: stop watching it.
                        let _ = self.ready.set_writable(false);
                    }
                    return Poll::Ready(Ok(value));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
            }
            // Would block: arm the waker, then try once more so an edge between
            // the first try and the arming is not lost.
            if write && !self.write_armed.swap(true, Ordering::SeqCst) {
                if let Err(e) = self.ready.set_writable(true) {
                    return Poll::Ready(Err(e));
                }
            }
            self.ready.clear();
            let registered = self.ready.register_waker(cx.waker());
            match op(&self.io) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if !registered {
                        // The reactor no longer holds this fd, so it can never
                        // wake the task. Fail the poll instead of parking
                        // forever (issue #430).
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "socket is no longer registered with the reactor",
                        )));
                    }
                    return Poll::Pending;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                other => return Poll::Ready(other),
            }
        }
    }
}

/// Wrap a connected `TcpStream` as a connection the splice and the response
/// writer use. The stream is set non-blocking.
///
/// # Errors
/// The socket mode or address could not be read.
pub fn wrap_tcp(stream: TcpStream) -> io::Result<SharedByteBufferStream<RawStream>> {
    stream.set_nonblocking(true)?;
    let raw = RawStream::from_tcp(stream).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(SharedByteBufferStream::rwrite(raw))
}

/// Resolve `authority` (`host:port`) and connect to it without blocking a
/// thread.
///
/// WHAT: the lookup runs through the async [`SystemDnsResolver`]. The connect is
/// a non-blocking `connect(2)` that parks on write readiness for every
/// `io_mode`. A `Std` dial wraps the connected socket as it is; the `io_uring`
/// modes then hand the connected socket to
/// `foundation_iogate::adopt_connected`, which registers it with the reactor.
/// The wait for the connect to finish ends after `connect_timeout`.
///
/// # Errors
/// The name did not resolve, the connect failed, or it did not finish within
/// `connect_timeout` (`TimedOut`).
pub async fn dial_tcp(
    authority: &str,
    io_mode: ServerIo,
    connect_timeout: Duration,
) -> io::Result<SharedByteBufferStream<RawStream>> {
    let (host, port) = authority
        .rsplit_once(':')
        .and_then(|(h, p)| {
            p.parse::<u16>()
                .ok()
                .map(|p| (h.trim_matches(['[', ']']), p))
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bad authority {authority}"),
            )
        })?;
    let addrs = SystemDnsResolver
        .resolve(host, port)
        .await
        .map_err(|e| io::Error::other(format!("resolve {authority}: {e}")))?;
    let addr: SocketAddr = addrs.into_iter().next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no address for {authority}"),
        )
    })?;

    let pending = PendingConnect::start(addr)?;
    let ready = FdReadiness::register(pending.as_raw_fd())?;
    ready.set_writable(true)?;
    // Park until the socket turns writable, then read `SO_ERROR`. The waker is
    // armed before the zero-timeout `poll`, so a connect that finishes between
    // the two still wakes this task.
    let fd = pending.as_raw_fd();
    let writable = poll_fn(|cx| {
        ready.clear();
        if !ready.register_waker(cx.waker()) {
            // The reactor no longer holds this fd, so it can never wake the
            // task. Without this check the wait could only end at the connect
            // timeout, and a connected socket would report a timeout (#430).
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "connect socket is no longer registered with the reactor",
            )));
        }
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: `pfd` is one valid `pollfd`; a zero timeout never blocks.
        let n = unsafe { libc::poll(&raw mut pfd, 1, 0) };
        if n != 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    });
    match select(Box::pin(writable), Box::pin(sleep_async(connect_timeout))).await {
        Either::Left((done, _)) => done?,
        Either::Right(((), _)) => {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("connect to {authority} timed out after {connect_timeout:?}"),
            ));
        }
    }
    drop(ready);
    let tcp = pending.check()?;
    if io_mode == ServerIo::Std {
        return wrap_tcp(tcp);
    }
    let conn = foundation_iogate::adopt_connected(tcp, addr, io_mode)?;
    let raw = RawStream::from_connection(conn).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(SharedByteBufferStream::rwrite(raw))
}

/// Read once from `src`: returns the bytes read (0 at end of stream). A read
/// with nothing to take parks on `wake`, or on the timer when `wake` is `None`.
async fn read_some(
    src: &mut SharedByteBufferStream<RawStream>,
    buf: &mut [u8],
    wake: Option<&ReadWakeSource>,
    fallback: Duration,
) -> io::Result<usize> {
    loop {
        match src.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        let Some(wake) = wake else {
            sleep_async(fallback).await;
            continue;
        };
        // Arm the waker, then read once more before parking, so no wake is lost.
        let armed = poll_fn(|cx| {
            wake(cx.waker());
            match src.read(buf) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
                other => Poll::Ready(other),
            }
        })
        .await;
        match armed {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

/// Copy `src` to `dst` until `src` ends or either side fails.
async fn pump(
    mut src: SharedByteBufferStream<RawStream>,
    dst: SharedByteBufferStream<RawStream>,
    config: WriterConfig,
) -> io::Result<()> {
    let wake = src.with_inner_ref(RawStream::read_wake_source);
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = read_some(
            &mut src,
            &mut buf,
            wake.as_ref(),
            config.poll_fallback_interval,
        )
        .await?;
        if n == 0 {
            return Ok(());
        }
        // A fresh writer per chunk: its send timeout counts from this write, not
        // from the last time this direction moved a byte (a tunnel can idle).
        let mut writer = ResponseWriter::new(dst.clone(), config);
        writer.write_all(&buf[..n]).await?;
        writer.flush().await?;
    }
}

/// Relay bytes both ways between `a` and `b` until either direction ends (end
/// of stream or an error), then close both. Reads park on read readiness and
/// writes park on write readiness, so a quiet tunnel costs no thread and no
/// timer.
pub async fn splice(
    a: SharedByteBufferStream<RawStream>,
    b: SharedByteBufferStream<RawStream>,
    config: WriterConfig,
) {
    let a_to_b = Box::pin(pump(a.clone(), b.clone(), config));
    let b_to_a = Box::pin(pump(b, a, config));
    let (finished, outcome) = match select(a_to_b, b_to_a).await {
        Either::Left((r, _)) => ("client to backend", r),
        Either::Right((r, _)) => ("backend to client", r),
    };
    if let Err(e) = outcome {
        tracing::debug!("splice ended ({finished}): {e}");
    }
}
