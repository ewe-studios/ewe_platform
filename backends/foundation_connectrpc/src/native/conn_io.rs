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
//! WHAT: [`ConnIo`] — a per-request driver. [`ConnIo::ready`] is a future that
//! resolves once the socket is readable or writable (or the wait times out);
//! [`ConnIo::block_on`] runs the dispatch future to completion on the calling
//! thread, which `Serve::serve` (a synchronous boundary) already owns.
//!
//! HOW: when the dispatch future is pending, `block_on` sleeps in one `poll(2)`
//! over the connection's fd (for the directions some future is waiting on)
//! and a wake socket pair. Wakers write a byte to the pair, so a wake from any
//! thread — a pipe the handler awaits, a cancel — ends the `poll` at once. The
//! earliest wait deadline bounds the `poll` timeout. Nothing polls on a timer.
//!
//! Transports without a kernel fd that reports readiness (the WireGuard
//! overlay, `io_uring` completion sockets, openssl/native-tls) cannot be waited
//! on here: a wait on them fails with `Unsupported`, which fails the RPC
//! instead of spinning. Over rustls the fd is polled in the direction of the
//! application's read or write; a TLS session that needs the other direction
//! (a key update mid-stream) waits until the deadline.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::native::readiness::IoInterest;
use foundation_netio::netcap::RawStream;

/// Futures waiting on the connection, by direction.
#[derive(Default)]
struct Waiters {
    read: Vec<(Waker, Instant)>,
    write: Vec<(Waker, Instant)>,
}

impl Waiters {
    fn list(&mut self, interest: IoInterest) -> &mut Vec<(Waker, Instant)> {
        match interest {
            IoInterest::Readable => &mut self.read,
            IoInterest::Writable => &mut self.write,
        }
    }

    fn earliest_deadline(&self) -> Option<Instant> {
        self.read
            .iter()
            .chain(self.write.iter())
            .map(|(_, deadline)| *deadline)
            .min()
    }
}

/// Drives one request's dispatch future on the calling thread and lets it wait
/// for the connection's socket. See the module docs.
pub(crate) struct ConnIo {
    /// The connection's socket, when it is a kernel fd that reports readiness.
    #[cfg(unix)]
    fd: Option<std::os::unix::io::RawFd>,
    #[cfg(unix)]
    wake_rx: std::os::unix::net::UnixStream,
    #[cfg(unix)]
    wake_tx: Arc<WakeTx>,
    waiters: Mutex<Waiters>,
}

impl ConnIo {
    /// A driver for `conn`.
    ///
    /// # Errors
    /// The wake socket pair could not be created.
    pub(crate) fn new(conn: &SharedByteBufferStream<RawStream>) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;

            let fd = conn.with_inner_ref(|raw: &RawStream| {
                let fd = raw.as_raw_fd();
                // `readiness` is `None` for transports whose bytes do not come
                // from fd readability (completion sockets, openssl); the overlay
                // has no kernel fd at all (-1).
                (fd >= 0 && raw.readiness(IoInterest::Readable).is_some()).then_some(fd)
            });
            let (wake_rx, wake_tx) = std::os::unix::net::UnixStream::pair()?;
            wake_rx.set_nonblocking(true)?;
            wake_tx.set_nonblocking(true)?;
            Ok(Self {
                fd,
                wake_rx,
                wake_tx: Arc::new(WakeTx(wake_tx)),
                waiters: Mutex::new(Waiters::default()),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = conn;
            Ok(Self {
                waiters: Mutex::new(Waiters::default()),
            })
        }
    }

    /// Resolves once the connection is ready in `interest`'s direction (or has
    /// hung up or failed, so the next read or write sees why). Fails with
    /// `TimedOut` when that takes longer than `timeout`, and with
    /// `Unsupported` on a transport that cannot be waited on.
    pub(crate) fn ready(&self, interest: IoInterest, timeout: Duration) -> IoReady<'_> {
        IoReady {
            io: self,
            interest,
            timeout,
            deadline: None,
        }
    }

    fn waiters(&self) -> std::sync::MutexGuard<'_, Waiters> {
        // The lists hold no invariant a panic could break.
        self.waiters.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn register(&self, interest: IoInterest, waker: &Waker, deadline: Instant) {
        let mut waiters = self.waiters();
        let list = waiters.list(interest);
        if let Some(entry) = list.iter_mut().find(|(w, _)| w.will_wake(waker)) {
            entry.1 = deadline;
        } else {
            list.push((waker.clone(), deadline));
        }
    }
}

#[cfg(unix)]
impl ConnIo {
    /// Whether the socket is ready in `interest`'s direction right now.
    fn ready_now(&self, interest: IoInterest) -> io::Result<bool> {
        let Some(fd) = self.fd else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this transport cannot report socket readiness, so the RPC cannot wait for the peer",
            ));
        };
        let mut fds = [libc::pollfd {
            fd,
            events: poll_events(interest),
            revents: 0,
        }];
        Ok(poll_fds(&mut fds, 0)? > 0)
    }

    /// Run `future` to completion on this thread, sleeping in `poll(2)` while it
    /// is pending.
    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        let waker = Waker::from(self.wake_tx.clone());
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
                return value;
            }
            self.wait();
        }
    }

    /// Sleep until a waker fires, the socket is ready for a waiting future, or
    /// the earliest wait deadline passes; then wake the futures that can move.
    fn wait(&self) {
        use std::io::Read;

        let (events, deadline) = {
            let waiters = self.waiters();
            let mut events = 0;
            if !waiters.read.is_empty() {
                events |= poll_events(IoInterest::Readable);
            }
            if !waiters.write.is_empty() {
                events |= poll_events(IoInterest::Writable);
            }
            (events, waiters.earliest_deadline())
        };

        let timeout_ms = deadline.map_or(-1, |deadline| {
            let left = deadline.saturating_duration_since(Instant::now());
            // Round up so the wake lands at or after the deadline.
            i32::try_from(left.as_millis() + 1).unwrap_or(i32::MAX)
        });

        let mut fds = [
            libc::pollfd {
                fd: std::os::unix::io::AsRawFd::as_raw_fd(&self.wake_rx),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.fd.unwrap_or(-1),
                events,
                revents: 0,
            },
        ];
        let count = if events != 0 && self.fd.is_some() {
            2
        } else {
            1
        };
        if let Err(e) = poll_fds(&mut fds[..count], timeout_ms) {
            // Wake everyone: each waiting future re-checks and reports the
            // socket's own error rather than sleeping on a broken poll.
            tracing::warn!(error = %e, "poll(2) on the connection failed; waking all waiters");
            let mut waiters = self.waiters();
            let Waiters { read, write } = &mut *waiters;
            for (waker, _) in read.drain(..).chain(write.drain(..)) {
                waker.wake();
            }
            return;
        }

        // Drain the wake pair; a wake that lands after this still leaves a byte.
        let mut sink = [0u8; 64];
        while matches!((&self.wake_rx).read(&mut sink), Ok(n) if n > 0) {}

        let fired = fds[1].revents;
        let gone = libc::POLLERR | libc::POLLHUP | libc::POLLNVAL;
        let now = Instant::now();
        let mut waiters = self.waiters();
        let read_fired = count == 2 && fired & (libc::POLLIN | gone) != 0;
        let write_fired = count == 2 && fired & (libc::POLLOUT | gone) != 0;
        wake_list(&mut waiters.read, read_fired, now);
        wake_list(&mut waiters.write, write_fired, now);
    }
}

#[cfg(not(unix))]
impl ConnIo {
    fn ready_now(&self, _interest: IoInterest) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "waiting on a connection is only supported on unix",
        ))
    }

    /// Run `future` to completion on this thread, parking it while pending.
    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        struct ThreadWaker(std::thread::Thread);
        impl Wake for ThreadWaker {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }
}

/// Wake (and forget) the waiters whose socket direction fired or whose
/// deadline has passed; keep the rest.
#[cfg(unix)]
fn wake_list(list: &mut Vec<(Waker, Instant)>, fired: bool, now: Instant) {
    list.retain(|(waker, deadline)| {
        if fired || *deadline <= now {
            waker.wake_by_ref();
            false
        } else {
            true
        }
    });
}

#[cfg(unix)]
fn poll_events(interest: IoInterest) -> libc::c_short {
    match interest {
        IoInterest::Readable => libc::POLLIN,
        IoInterest::Writable => libc::POLLOUT,
    }
}

/// `poll(2)` over `fds`, retrying on `EINTR`. Returns how many fds have events.
#[cfg(unix)]
fn poll_fds(fds: &mut [libc::pollfd], timeout_ms: i32) -> io::Result<usize> {
    loop {
        let len = libc::nfds_t::try_from(fds.len()).unwrap_or(libc::nfds_t::MAX);
        // SAFETY: `fds` is a valid slice of initialised `pollfd`s and `len` is
        // its length.
        let rc = unsafe { libc::poll(fds.as_mut_ptr(), len, timeout_ms) };
        if rc >= 0 {
            return Ok(usize::try_from(rc).unwrap_or(0));
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

/// The waker of [`ConnIo::block_on`]: one byte on the wake pair ends its `poll`.
#[cfg(unix)]
struct WakeTx(std::os::unix::net::UnixStream);

#[cfg(unix)]
impl Wake for WakeTx {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        use std::io::Write;
        match (&self.0).write(&[1]) {
            // A full pair already holds a pending wake.
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => tracing::warn!(error = %e, "could not wake the connection driver"),
        }
    }
}

/// See [`ConnIo::ready`].
pub(crate) struct IoReady<'a> {
    io: &'a ConnIo,
    interest: IoInterest,
    timeout: Duration,
    /// Set on the first poll, so the timeout counts from when the wait starts.
    deadline: Option<Instant>,
}

impl Future for IoReady<'_> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let timeout = self.timeout;
        let deadline = *self
            .deadline
            .get_or_insert_with(|| Instant::now() + timeout);
        match self.io.ready_now(self.interest) {
            Ok(true) => return Poll::Ready(Ok(())),
            Ok(false) => {}
            Err(e) => return Poll::Ready(Err(e)),
        }
        if Instant::now() >= deadline {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "the connection did not become {} within {timeout:?}",
                    self.interest
                ),
            )));
        }
        self.io.register(self.interest, cx.waker(), deadline);
        Poll::Pending
    }
}
