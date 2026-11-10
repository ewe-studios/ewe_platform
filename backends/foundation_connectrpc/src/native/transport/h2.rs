//! Native HTTP/2 `Transport` implementation (Feature 30).
//!
//! WHY: `Transport` byte-level client-seam contract for HTTP/2 cleartext (h2c
//! prior-knowledge). `open()` starts a non-blocking TCP dial and spawns a
//! valtron `TaskIterator` pump; the pump settles the dial and negotiates the h2
//! handshake, and `open()` returns its `TransportStream` at once.
//!
//! WHAT: [`H2Transport`] wraps nothing — it is a stateless factory. The pump
//! ([`H2Pump`]) is a [`TaskIterator`] that owns a non-blocking `TcpStream` and an
//! [`H2Channel`]. Each turn: read fd bytes → feed channel → step → drain output →
//! write fd. When there is nothing to do it parks on `TaskStatus::Depends`: the
//! socket's reactor readiness, the request-body queue and its own deadline. The
//! valtron executor then stops polling it until a real event arrives.
//!
//! HOW: Pattern matches `H1Transport` — `valtron::send(pump)` spawns the task
//! on the pool; the caller gets `TransportStream { send_body, head, recv_body }`
//! and drains response bytes from the pipe handles.
//!
//! `TaskStatus::Delayed` remains only as the fallback for a platform or a
//! process where `FdReadiness::register` fails, so the pump still makes
//! progress without a reactor.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::valtron::{
    self, Ctx, Pipe, PipeReceiver, PipeSender, TaskIterator, TaskStatus, TryRecvError,
};
#[cfg(unix)]
use foundation_core::valtron::{AnyReadiness, EventReadiness, EventReadinessPtr, ReadinessWaker};
use foundation_netio::http2::channel::{H2Channel, H2StreamEvent};
use foundation_netio::http2::connection::H2Request;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, Status,
};
#[cfg(unix)]
use foundation_reactor::FdReadiness;

use crate::shared::transport::{
    body_stream_from_pipe, BodyStream, HeadStream, Transport, TransportCapabilities,
    TransportError, TransportStream,
};

/// Fallback re-poll interval for a pump with no reactor registration. The
/// normal path parks on reactor readiness; see [`H2Pump::park`].
const POLL_DELAY: Duration = Duration::from_millis(10);

/// The number of [`H2Pump`] turns in this process. Read by
/// [`h2_pump_poll_count`].
static H2_PUMP_POLLS: AtomicU64 = AtomicU64::new(0);

/// The number of [`H2Pump`] poll turns since process start.
///
/// `#[doc(hidden)]`: this exists so the parking regression test can prove that
/// the pump waits on reactor readiness instead of polling every [`POLL_DELAY`].
#[doc(hidden)]
#[must_use]
pub fn h2_pump_poll_count() -> u64 {
    H2_PUMP_POLLS.load(Ordering::Relaxed)
}

/// The response head as the pump publishes it: the head, or the pre-head
/// failure (h2 handshake error or handshake timeout) that stopped the call.
type HeadItem = Result<(Status, SimpleHeaders), TransportError>;

// ── H2Socket — the byte stream a pump drives ───────────────────────────────

/// The socket an [`H2Pump`] reads/writes — TCP (`host:port` from the request
/// URI) or a Unix domain socket (fixed path supplied at transport build time).
/// Both sides of the enum expose the same non-blocking I/O surface the pump
/// needs, so the pump body is socket-agnostic.
enum H2Socket {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
    /// A connect in flight on a non-blocking socket (feature 127).
    ///
    /// The pump settles it from `SO_ERROR` in [`PumpPhase::Connecting`] and
    /// then replaces the variant with `Tcp` or `Unix`. The slot is `None` only
    /// for the moment between taking the socket out and filling the final
    /// variant.
    Connecting {
        socket: Option<socket2::Socket>,
        /// A Unix address to retry `connect` on. `None` for TCP, whose connect
        /// completes in the background without a retry.
        ///
        /// A non-blocking connect to a Unix listener whose backlog is full
        /// returns `EAGAIN` and does not queue the connection kernel-side, so
        /// the pump must call `connect` again.
        #[cfg(unix)]
        retry_addr: Option<socket2::SockAddr>,
    },
}

impl H2Socket {
    /// Create a non-blocking socket and start a connect to `addr`.
    ///
    /// Returns the socket and whether the connect is still in flight. `Ok(())`
    /// from `connect` means a loopback peer accepted at once; an `EINPROGRESS`
    /// (or `WouldBlock`) error means the pump must wait for write readiness.
    /// Any other error is returned at once, so `open` still reports a refusal
    /// that the kernel makes synchronously.
    fn connect_tcp(addr: SocketAddr) -> io::Result<(Self, bool)> {
        let socket = socket2::Socket::new(
            socket2::Domain::for_address(addr),
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )?;
        socket.set_nonblocking(true)?;
        match socket.connect(&addr.into()) {
            Ok(()) => {
                let stream: TcpStream = socket.into();
                Ok((Self::Tcp(stream), false))
            }
            Err(err) if connect_in_progress(&err) => Ok((
                Self::Connecting {
                    socket: Some(socket),
                    #[cfg(unix)]
                    retry_addr: None,
                },
                true,
            )),
            Err(err) => Err(err),
        }
    }

    /// Create a non-blocking Unix socket and start a connect to `path`.
    ///
    /// The same contract as [`connect_tcp`](Self::connect_tcp): a retryable
    /// error means the pump waits for write readiness. It never blocks the
    /// calling worker, so a slow or backlogged listener cannot stall a valtron
    /// worker.
    #[cfg(unix)]
    fn connect_unix(path: &std::path::Path) -> io::Result<(Self, bool)> {
        use std::os::fd::OwnedFd;

        let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        socket.set_nonblocking(true)?;
        let addr = socket2::SockAddr::unix(path)?;
        match socket.connect(&addr) {
            Ok(()) => {
                let stream = std::os::unix::net::UnixStream::from(OwnedFd::from(socket));
                Ok((Self::Unix(stream), false))
            }
            Err(err) if connect_in_progress(&err) => Ok((
                Self::Connecting {
                    socket: Some(socket),
                    retry_addr: Some(addr),
                },
                true,
            )),
            Err(err) => Err(err),
        }
    }

    /// Settle an in-flight connect.
    ///
    /// `Ok(true)` means the connection is up, `Ok(false)` means it is still in
    /// flight, and `Err` is the dial failure. Only a `None` `SO_ERROR` plus a
    /// successful `peer_addr` means connected: while the connect is in flight
    /// `peer_addr` fails with `ENOTCONN`, which is not an error.
    fn connect_state(&mut self) -> io::Result<bool> {
        let Self::Connecting {
            socket,
            #[cfg(unix)]
            retry_addr,
        } = self
        else {
            return Ok(true);
        };
        let Some(socket) = socket else {
            return Ok(true);
        };
        if socket.peer_addr().is_ok() {
            return Ok(true);
        }
        if let Some(err) = socket.take_error()? {
            // A Unix connect that the listener's backlog refused is retried
            // below, not failed.
            #[cfg(unix)]
            let retry = retry_addr.is_some() && connect_in_progress(&err);
            #[cfg(not(unix))]
            let retry = false;
            if !retry {
                return Err(err);
            }
        }
        #[cfg(unix)]
        if let Some(addr) = retry_addr {
            match socket.connect(addr) {
                Ok(()) => return Ok(true),
                Err(ref e) if connect_in_progress(e) => return Ok(false),
                Err(ref e) if e.raw_os_error() == Some(libc::EISCONN) => return Ok(true),
                Err(ref e) if e.raw_os_error() == Some(libc::EALREADY) => return Ok(false),
                Err(e) => return Err(e),
            }
        }
        Ok(socket.peer_addr().is_ok())
    }

    /// Move a connected socket from the `Connecting` state into `Tcp` or
    /// `Unix`.
    fn finish_connect(&mut self) {
        let Self::Connecting { socket, .. } = self else {
            return;
        };
        let Some(socket) = socket.take() else {
            return;
        };
        #[cfg(unix)]
        if matches!(
            self,
            Self::Connecting {
                retry_addr: Some(_),
                ..
            }
        ) {
            use std::os::fd::OwnedFd;
            *self = Self::Unix(std::os::unix::net::UnixStream::from(OwnedFd::from(socket)));
            return;
        }
        let stream: TcpStream = socket.into();
        *self = Self::Tcp(stream);
    }

    /// The raw fd to register with the reactor, if the socket has one.
    #[cfg(unix)]
    fn raw_fd(&self) -> Option<RawFd> {
        match self {
            Self::Tcp(s) => Some(s.as_raw_fd()),
            Self::Unix(s) => Some(s.as_raw_fd()),
            Self::Connecting {
                socket: Some(s), ..
            } => Some(s.as_raw_fd()),
            Self::Connecting { socket: None, .. } => None,
        }
    }

    /// Whether a Unix connect is waiting for a retry.
    ///
    /// A non-blocking connect that the listener's backlog refused is not
    /// queued kernel-side, so no readiness edge announces when the retry can
    /// succeed. The pump then retries on [`POLL_DELAY`] instead of parking on
    /// the socket.
    #[cfg(unix)]
    fn retries_connect(&self) -> bool {
        matches!(
            self,
            Self::Connecting {
                retry_addr: Some(_),
                ..
            }
        )
    }
}

/// Whether a failed `connect` means "still in flight" rather than a failure.
fn connect_in_progress(err: &io::Error) -> bool {
    #[cfg(unix)]
    if err.raw_os_error() == Some(libc::EINPROGRESS) {
        return true;
    }
    err.kind() == io::ErrorKind::WouldBlock
}

impl Read for H2Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.read(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
            // The pump settles the connect before it reads, so this arm is
            // only reachable if that order changes. Report "try again" rather
            // than block or panic.
            Self::Connecting { .. } => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "h2 socket: the connect is still in flight",
            )),
        }
    }
}

impl Write for H2Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.write(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.write(buf),
            Self::Connecting { .. } => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "h2 socket: the connect is still in flight",
            )),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Tcp(s) => s.flush(),
            #[cfg(unix)]
            Self::Unix(s) => s.flush(),
            Self::Connecting { .. } => Ok(()),
        }
    }
}

/// Register `stream` with the shared reactor for read readiness.
///
/// `None` when the reactor cannot watch it (a failed registration, or a
/// platform without a reactor); the pump then polls on [`POLL_DELAY`].
#[cfg(unix)]
fn register_socket_readiness(stream: &H2Socket) -> Option<Arc<FdReadiness>> {
    let Some(fd) = stream.raw_fd() else {
        tracing::debug!(poll_delay = ?POLL_DELAY, "h2 pump: socket has no fd to watch; polling instead");
        return None;
    };
    match FdReadiness::register(fd) {
        Ok(readiness) => Some(Arc::new(readiness)),
        Err(e) => {
            tracing::debug!(
                error = %e,
                poll_delay = ?POLL_DELAY,
                "h2 pump: cannot watch the socket on the reactor; polling instead"
            );
            None
        }
    }
}

// ── H2Transport ────────────────────────────────────────────────────────────

/// HTTP/2 cleartext (h2c prior-knowledge) transport — one connection per call.
///
/// Dials TCP by default (`host:port` from the request URI). Build with
/// [`unix`](Self::unix) to dial a Unix domain socket instead — the request
/// URI's authority is then only used for the `:authority` pseudo-header
/// (gRPC servers on Unix sockets ignore it).
///
/// `open()` never waits on the peer: it starts a non-blocking dial and hands
/// the socket to the pump task, which settles the dial and runs the h2
/// handshake (preface + SETTINGS exchange) ahead of the request. A dial failure
/// therefore arrives as `Err(TransportError::Connect)` on the response head,
/// not as an `Err` from `open()`. `open()` is called from async code running on
/// a valtron worker, and the peer is often served by the same pool (an
/// in-process server, or a proxy and its backend). A dial or a handshake that
/// blocked the worker could starve the very task that answers it and deadlock
/// the pool. This holds for both the TCP and the Unix domain socket paths.
#[derive(Clone)]
pub struct H2Transport {
    /// When set, every `open()` dials this Unix socket instead of TCP.
    #[cfg(unix)]
    unix_path: Option<std::path::PathBuf>,
    /// Longest the dial may take. `open()` returns at once; the pump enforces
    /// this budget while the connect is in flight and reports a failure on the
    /// response head. See [`Self::with_connect_timeout`].
    connect_timeout: Duration,
    /// Longest the call waits for the peer's h2 SETTINGS. See
    /// [`Self::with_handshake_timeout`].
    handshake_timeout: Duration,
    /// Longest the pump waits for the next bytes from the peer once the request
    /// is out. `Duration::ZERO` disables the check (the default). See
    /// [`Self::with_read_timeout`].
    read_timeout: Duration,
}

impl Default for H2Transport {
    fn default() -> Self {
        Self {
            #[cfg(unix)]
            unix_path: None,
            connect_timeout: Self::DEFAULT_CONNECT_TIMEOUT,
            handshake_timeout: Self::DEFAULT_HANDSHAKE_TIMEOUT,
            read_timeout: Self::DEFAULT_READ_TIMEOUT,
        }
    }
}

impl H2Transport {
    /// Default for [`Self::with_connect_timeout`]: 10 s, the dial budget this
    /// transport has always used.
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

    /// Default for [`Self::with_handshake_timeout`]: 10 s, the same budget Go's
    /// `net/http` `DefaultTransport` gives the comparable step
    /// (`TLSHandshakeTimeout`): the connection is up but the peer has not
    /// finished its preamble.
    pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

    /// Default for [`Self::with_read_timeout`]: `Duration::ZERO`, which disables
    /// the check. A disabled read timeout keeps this transport's old behaviour,
    /// where a peer that stops answering leaves the call hanging on the peer's
    /// own idle timeout.
    pub const DEFAULT_READ_TIMEOUT: Duration = Duration::ZERO;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An `H2Transport` that dials the given Unix domain socket for every
    /// call — h2c-over-Unix, the shape `buildkitd` listens on by default
    /// (`unix:///run/buildkit/buildkitd.sock`).
    #[cfg(unix)]
    #[must_use]
    pub fn unix(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            unix_path: Some(path.into()),
            ..Self::default()
        }
    }

    /// Set how long the dial may take before the call fails with
    /// [`TransportError::Connect`] on its response head. `open()` does not wait.
    /// Default [`Self::DEFAULT_CONNECT_TIMEOUT`].
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set how long a call waits for the peer's h2 SETTINGS after the dial. A
    /// peer that accepts the connection but never answers the preface fails the
    /// call with [`TransportError::Timeout`] on its response head instead of
    /// hanging it. Default [`Self::DEFAULT_HANDSHAKE_TIMEOUT`].
    #[must_use]
    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }

    /// Set how long the pump waits for the next bytes from the peer once the
    /// request is out. A peer that accepts the connection but then sends nothing
    /// fails the call with [`TransportError::Timeout`] instead of hanging it on
    /// the peer's own idle timeout. `Duration::ZERO` (the default) disables the
    /// check. See [`Self::DEFAULT_READ_TIMEOUT`].
    #[must_use]
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Start the dial for one call without waiting for the peer.
    ///
    /// Returns the socket and whether the connect is still in flight. Both the
    /// TCP and the Unix path create a non-blocking socket and start
    /// `connect(2)`, so an in-flight connect costs no worker and the pump
    /// enforces [`Self::DEFAULT_CONNECT_TIMEOUT`].
    fn start_dial(&self, host: &str, port: u16) -> Result<(H2Socket, bool), TransportError> {
        #[cfg(unix)]
        if let Some(path) = &self.unix_path {
            return H2Socket::connect_unix(path).map_err(|e| TransportError::Connect(Arc::new(e)));
        }
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| TransportError::Connect(Arc::new(e)))?;
        H2Socket::connect_tcp(addr).map_err(|e| TransportError::Connect(Arc::new(e)))
    }
}

impl Transport for H2Transport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            request_streaming: true,
            // The pump streams request DATA frames while concurrently draining
            // response frames, so a bidi call makes progress in both directions.
            full_duplex: true,
            h2_trailers: true,
            http_versions: &[Proto::HTTP20],
            // One TCP connection per call — h2 stream multiplexing across calls
            // is not wired up on the client side yet.
            multiplexed: false,
        }
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let host = request
            .request_uri
            .host_str()
            .unwrap_or_else(|| "localhost".into());
        let port = request.request_uri.port_or_default();
        let path = request.request_uri.path().to_string();
        let method = request.method.clone();
        let req_headers = request.headers.clone();

        // ── Connect ──────────────────────────────────────────────────────────
        // The dial does not block the worker (feature 127). See
        // [`Self::start_dial`]. Only a failure the kernel reports synchronously
        // (a bad address or an immediate refusal) is `Err` here; an in-flight
        // connect is settled by the pump's `Connecting` phase.
        let (stream, in_flight) = self.start_dial(&host, port)?;

        // ── h2 handshake ───────────────────────────────────────────────────────
        // Not run here. The pump runs it (`PumpPhase::Handshaking`) without
        // blocking a worker and reports a failure on the head stream. Waiting
        // for the peer's SETTINGS on this thread deadlocked the pool whenever
        // every worker was blocked here on a peer that needed a worker to answer.
        let channel = H2Channel::new(false); // is_server = false (client)

        // ── Pipes ────────────────────────────────────────────────────────────
        let (send_tx, send_rx) = Pipe::<Bytes>::new();
        let (head_tx, head_rx) = Pipe::<HeadItem>::new();
        let (body_tx, body_rx) = Pipe::<Bytes>::new();
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);

        // ── Park signals ─────────────────────────────────────────────────────
        // Register before the pump's first turn, so a connect that completes
        // early cannot be missed. The socket is non-blocking, so the reactor
        // can watch it now. While the connect is in flight the fd is armed
        // WRITABLE (its edge is the completion); the pump disarms it when the
        // connection resolves. Otherwise WRITABLE starts off, and `write_out`
        // turns it on only while output is queued.
        #[cfg(unix)]
        let socket = register_socket_readiness(&stream);
        #[cfg(unix)]
        let mut watching_writes = false;
        #[cfg(unix)]
        if in_flight {
            if let Some(socket) = &socket {
                match socket.set_writable(true) {
                    Ok(()) => watching_writes = true,
                    Err(e) => {
                        tracing::debug!(
                            error = %e,
                            "h2 pump: cannot arm the in-flight connect for write readiness"
                        );
                    }
                }
            }
        }
        #[cfg(unix)]
        let body_queued: EventReadinessPtr = Arc::new(send_rx.readiness());

        // The pump starts both budgets on its first poll, from the engine
        // clock in its `Ctx`: the connect budget first, then the handshake
        // budget when the connection is up.
        // ── Spawn the valtron pump ────────────────────────────────────────────
        let pump = H2Pump {
            #[cfg(unix)]
            socket,
            #[cfg(unix)]
            watching_writes,
            stream,
            channel,
            state: if in_flight {
                PumpPhase::Connecting
            } else {
                PumpPhase::Handshaking
            },
            opened: None,
            connecting_timeout: self.connect_timeout,
            in_flight,
            handshake_deadline: None,
            handshake_timeout: self.handshake_timeout,
            read_timeout: self.read_timeout,
            last_progress: None,
            ctx: None,
            pending_out: Vec::new(),
            send_rx,
            head_tx,
            body_tx,
            #[cfg(unix)]
            body_queued,
            polls: &H2_PUMP_POLLS,
            trailer_tx,
            method: method.to_string(),
            host,
            port,
            path,
            req_headers,
            request_sent: false,
            response_head_sent: false,
            stream_id: 0,
            body_complete: false,
            request_ended: false,
            pending_body: VecDeque::new(),
            response_ended: false,
        };

        valtron::send(pump)
            .map_err(|e| TransportError::Connect(Arc::new(io::Error::other(e.to_string()))))?;

        // The head pipe carries `Result`s so a handshake failure reaches the
        // caller as `Err(TransportError)`, as it did when `open()` returned it.
        let head: HeadStream = Box::pin(futures::stream::unfold(head_rx, |rx| async move {
            rx.receive().await.map(|item| (item, rx))
        }));
        let recv_body: BodyStream = body_stream_from_pipe(body_rx);

        Ok(TransportStream {
            send_body: Arc::new(send_tx),
            head,
            recv_body,
            trailers: trailer_rx,
        })
    }
}

// ── H2Pump — valtron TaskIterator ──────────────────────────────────────────

/// Ready once `at` has passed. It has no waker: the executor sleeps until the
/// instant it reports in [`EventReadiness::deadline`] and then runs the task.
/// Built per park, because the idle deadline moves with `last_progress`.
#[cfg(unix)]
struct DeadlineReadiness {
    at: Instant,
    ctx: Ctx,
}

#[cfg(unix)]
impl DeadlineReadiness {
    fn at(at: Instant, ctx: Ctx) -> Self {
        Self { at, ctx }
    }
}

#[cfg(unix)]
impl EventReadiness for DeadlineReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.ctx.clock().now() >= self.at
    }

    // `foundation_compact::Instant` re-exports `std::time::Instant` on native,
    // so the trait's return type is this one.
    fn deadline(&self) -> Option<Instant> {
        Some(self.at)
    }

    /// Nothing signals it: the executor wakes at [`Self::deadline`].
    fn register(&self, _waker: &ReadinessWaker) -> bool {
        true
    }
}

enum PumpPhase {
    /// A non-blocking TCP connect is in flight. The pump settles it from
    /// `SO_ERROR` before it reads or writes (feature 127).
    Connecting,
    /// Exchanging the connection preface and SETTINGS with the peer.
    Handshaking,
    SendingRequest,
    WaitingResponse,
    Draining,
    Done,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "the flags are independent protocol state"
)]
struct H2Pump {
    /// The socket's reactor registration. READABLE is always watched; WRITABLE
    /// is added only while `pending_out` holds bytes, because an idle socket is
    /// always writable and would keep the reactor reporting it.
    #[cfg(unix)]
    socket: Option<Arc<FdReadiness>>,
    /// The current WRITABLE interest, so an unchanged value costs no selector
    /// re-registration.
    #[cfg(unix)]
    watching_writes: bool,
    stream: H2Socket,
    channel: H2Channel,
    state: PumpPhase,
    /// The instant of the first poll, set from the engine clock.
    opened: Option<Instant>,
    /// How long `Connecting` may take, from the first poll.
    connecting_timeout: Duration,
    /// Whether the dial was still in flight when `open` returned.
    in_flight: bool,
    /// When `Handshaking` gives up on the peer's SETTINGS. It starts when the
    /// connect resolves, not at `open`, so a slow dial cannot spend it.
    handshake_deadline: Option<Instant>,
    /// The SETTINGS budget, kept so the deadline can start when the connect
    /// resolves.
    handshake_timeout: Duration,
    /// Longest the pump waits for the next bytes from the peer once the request
    /// is out. `Duration::ZERO` disables the check.
    read_timeout: Duration,
    /// When the peer last sent bytes or the socket last took our bytes. The
    /// read timeout measures from here.
    last_progress: Option<Instant>,
    /// The context, resolved on the first poll. Its clock is the engine clock.
    ctx: Option<Ctx>,
    /// Bytes drained from `channel` that the non-blocking socket has not taken
    /// yet. Written ahead of any later output so frames are never split or lost.
    pending_out: Vec<u8>,
    send_rx: PipeReceiver<Bytes>,
    /// Ready when a request body chunk is queued in `send_rx`. Included in a
    /// park only while the request direction is still open: a closed queue is
    /// always ready and would spin the task.
    #[cfg(unix)]
    body_queued: EventReadinessPtr,
    /// Counts this pump's poll turns for [`h2_pump_poll_count`].
    polls: &'static AtomicU64,
    head_tx: PipeSender<HeadItem>,
    body_tx: PipeSender<Bytes>,
    trailer_tx: PipeSender<SimpleHeaders>,
    method: String,
    host: String,
    port: u16,
    path: String,
    req_headers: SimpleHeaders,
    request_sent: bool,
    response_head_sent: bool,
    /// The stream this request occupies, known once HEADERS has been sent.
    stream_id: u32,
    /// `send_rx` has closed: the request body is complete.
    body_complete: bool,
    /// Set once the terminating `DATA(END_STREAM)` has gone out.
    request_ended: bool,
    /// Response chunks the body pipe was too full to accept. They must be
    /// delivered, in order, ahead of any later chunk — `try_send` dropping a
    /// `Full` chunk on the floor silently truncates the response body.
    pending_body: VecDeque<Bytes>,
    /// The peer signalled `END_STREAM`; close `body_tx` once `pending_body` drains.
    response_ended: bool,
}

impl H2Pump {
    /// Forget the socket's latched readiness, so the edge-triggered reactor
    /// re-arms for the next event.
    ///
    /// Call it once at the start of a turn, before the read loop. Clearing
    /// *before* an attempt can only cause a harmless spurious wake; clearing
    /// *after* a `WouldBlock` could erase an edge that arrived during the
    /// attempt, and that wake would be lost.
    #[cfg(unix)]
    fn clear_socket(&self) {
        if let Some(socket) = &self.socket {
            socket.clear();
        }
    }

    #[cfg(not(unix))]
    fn clear_socket(&self) {}

    /// Watch or stop watching the socket for WRITABLE.
    ///
    /// `write_out` turns it on when a write comes up short (the socket is full)
    /// and off once `pending_out` drains. An idle socket is always writable, so
    /// arming it earlier would latch a spurious wake on every turn.
    #[cfg(unix)]
    fn set_writable(&mut self, watch: bool) {
        if self.watching_writes == watch {
            return;
        }
        if let Some(socket) = &self.socket {
            match socket.set_writable(watch) {
                Ok(()) => self.watching_writes = watch,
                Err(e) => {
                    tracing::debug!(error = %e, "h2 pump: cannot change socket write interest");
                }
            }
        }
    }

    #[cfg(not(unix))]
    fn set_writable(&mut self, _watch: bool) {}

    /// The instant the pump must wake on its own, if any.
    ///
    /// While the connect is in flight it is the connect deadline, and during
    /// the handshake it is the handshake deadline. Once the request is out and
    /// a read timeout is configured, it is `last_progress + read_timeout`.
    #[cfg(unix)]
    fn next_deadline(&self) -> Option<Instant> {
        if matches!(self.state, PumpPhase::Connecting) {
            self.opened.map(|opened| opened + self.connecting_timeout)
        } else if matches!(self.state, PumpPhase::Handshaking) {
            self.handshake_deadline
                .or_else(|| self.opened.map(|opened| opened + self.handshake_timeout))
        } else if !self.read_timeout.is_zero() && self.request_sent {
            self.last_progress
                .map(|progress| progress + self.read_timeout)
        } else {
            None
        }
    }

    /// The wait for a turn with nothing to do right now.
    ///
    /// Parks on the socket's reactor readiness, the request-body queue while
    /// the request direction is open, the response pipe while it holds chunks
    /// the consumer has not taken (or while the consumer may still be there),
    /// and the pump's own deadline. Without a reactor registration it falls
    /// back to [`POLL_DELAY`].
    #[cfg(unix)]
    fn park(&self) -> TaskStatus<(), ()> {
        let Some(socket) = &self.socket else {
            return TaskStatus::Delayed(POLL_DELAY);
        };
        // A Unix connect the listener refused is retried on a short delay: no
        // readiness edge announces when the backlog has room again.
        if matches!(self.state, PumpPhase::Connecting) && self.stream.retries_connect() {
            return TaskStatus::Delayed(POLL_DELAY);
        }
        let mut children: Vec<EventReadinessPtr> = Vec::with_capacity(4);
        children.push(Arc::clone(socket) as EventReadinessPtr);
        // A closed queue is always ready: include it only while the request
        // direction can still receive a chunk, or the task would spin.
        if self.request_sent && !self.request_ended {
            children.push(Arc::clone(&self.body_queued));
        }
        // The response pipe is full and holds chunks. The consumer's next pop
        // frees a slot and wakes this task through the pipe vacancy signal.
        if !self.pending_body.is_empty() {
            children.push(Arc::new(self.body_tx.vacancy()) as EventReadinessPtr);
        } else if !self.response_ended {
            // A cancelled call drops its response receiver. Without this, a
            // parked pump never notices and never resets the peer's stream.
            // The vacancy signal already covers the close when it is present.
            // This signal is close-only, so a consumer pop does not wake the
            // pump for nothing.
            children.push(Arc::new(self.body_tx.closed_readiness()) as EventReadinessPtr);
        }
        // Likewise before the head arrives: the pump checks `head_tx.is_closed`
        // each turn, so a dropped head receiver must wake it.
        if !self.response_head_sent {
            children.push(Arc::new(self.head_tx.closed_readiness()) as EventReadinessPtr);
        }
        if let Some(deadline) = self.next_deadline() {
            children
                .push(Arc::new(DeadlineReadiness::at(deadline, self.engine_ctx()))
                    as EventReadinessPtr);
        }
        TaskStatus::Depends(Arc::new(AnyReadiness::new(children)))
    }

    /// The context, resolved on the first poll.
    ///
    /// # Panics
    /// Panics when `next_status` has not run yet.
    fn engine_ctx(&self) -> Ctx {
        self.ctx
            .as_ref()
            .expect("the engine context is resolved on the first poll")
            .clone()
    }

    /// No reactor off unix: keep the fixed re-poll interval.
    #[cfg(not(unix))]
    fn park(&self) -> TaskStatus<(), ()> {
        TaskStatus::Delayed(POLL_DELAY)
    }

    /// Forward any newly-available request body chunks, and terminate the request
    /// direction once `send_rx` closes. Returns `false` on a fatal channel error.
    ///
    /// Called from every phase: a server may answer while the client is still
    /// sending (bidi), so the request direction cannot stall once we move on to
    /// reading the response.
    fn pump_request_body(&mut self) -> bool {
        if !self.request_sent || self.request_ended {
            return true;
        }

        loop {
            match self.send_rx.try_recv() {
                Ok(chunk) => {
                    if !chunk.is_empty()
                        && self
                            .channel
                            .send_data_frame(self.stream_id, &chunk, false)
                            .is_err()
                    {
                        return false;
                    }
                }
                Err(TryRecvError::Closed) => {
                    self.body_complete = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        if self.body_complete && !self.request_ended {
            if self
                .channel
                .send_data_frame(self.stream_id, &[], true)
                .is_err()
            {
                return false;
            }
            self.request_ended = true;
        }
        true
    }

    /// Push queued response chunks into `body_tx`, respecting its capacity.
    /// Returns `false` if the consumer hung up.
    fn flush_response_body(&mut self) -> bool {
        while let Some(chunk) = self.pending_body.front() {
            match self.body_tx.try_send(chunk.clone()) {
                Ok(()) => {
                    self.pending_body.pop_front();
                }
                // Consumer is behind. Keep the chunk and retry next poll.
                Err(e) if e.is_full() => return true,
                Err(_) => return false,
            }
        }
        if self.response_ended {
            let _ = self.body_tx.close();
        }
        true
    }

    /// Tell the peer to stop this stream: send `RST_STREAM(CANCEL)` and flush it.
    ///
    /// Used when the response consumer goes away mid-call. Dropping the socket
    /// would also stop the flow, but an explicit reset is the protocol-correct
    /// abort and reaches a peer whose server may not treat a bare FIN as an
    /// error — `foundation_http`'s h2 server peeks for whole frames and reads
    /// EOF as `WouldBlock`, so it would otherwise keep streaming into a socket
    /// whose reader is gone.
    fn abort_peer_stream(&mut self) {
        if !self.request_sent {
            return;
        }
        self.channel.send_rst_stream(self.stream_id);
        // Best effort: the call is being torn down either way, and a socket
        // that cannot take the reset is closed when the pump drops it.
        if let Err(e) = self.write_out() {
            tracing::debug!(err = %e, "h2 pump: could not send RST_STREAM");
        }
    }

    /// Queue the channel's new output behind any unwritten bytes and write as
    /// much as the non-blocking socket takes.
    ///
    /// Returns `Ok(true)` once everything is written, `Ok(false)` when the socket
    /// is full (the rest stays in `pending_out` for the next poll), or the write
    /// error. A bare `write` that ignored its count, or dropped the drained
    /// bytes on `WouldBlock`, would cut h2 frames and corrupt the connection.
    fn write_out(&mut self) -> io::Result<bool> {
        let out = self.channel.drain_output();
        if !out.is_empty() {
            self.pending_out.extend_from_slice(&out);
        }
        while !self.pending_out.is_empty() {
            match self.stream.write(&self.pending_out) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "h2 socket accepted no bytes",
                    ))
                }
                Ok(n) => {
                    self.pending_out.drain(..n);
                    self.last_progress = Some(self.engine_ctx().clock().now());
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // The socket is full. Arm WRITABLE now: the drain back to
                    // writable is the edge that must wake the parked task.
                    self.set_writable(true);
                    return Ok(false);
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        self.stream.flush()?;
        self.set_writable(false);
        Ok(true)
    }

    /// End the call before a response head: publish `err` on the head stream
    /// (so the caller sees why, as when `open()` returned it) and close the
    /// response pipes.
    fn fail_before_head(&mut self, err: TransportError) {
        let _ = self.head_tx.try_send(Err(err));
        let _ = self.head_tx.close();
        let _ = self.body_tx.close();
        let _ = self.trailer_tx.close();
    }
}

impl TaskIterator for H2Pump {
    type Ready = ();
    type Pending = ();

    #[allow(
        clippy::too_many_lines,
        reason = "one poll step covers every protocol phase in order"
    )]
    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<(), ()>> {
        self.polls.fetch_add(1, Ordering::Relaxed);
        let now = ctx.clock().now();
        self.ctx.get_or_insert_with(|| ctx.clone());
        let opened = *self.opened.get_or_insert(now);
        let connecting_deadline = opened + self.connecting_timeout;
        let handshake_deadline = self.handshake_deadline.unwrap_or_else(|| {
            if self.in_flight {
                connecting_deadline + self.handshake_timeout
            } else {
                opened + self.handshake_timeout
            }
        });
        self.handshake_deadline = Some(handshake_deadline);

        // Re-arm the socket's edge-triggered registration once, before this
        // turn reads it. Clearing before an attempt can only cause a spurious
        // wake; clearing after a `WouldBlock` could erase a real edge.
        self.clear_socket();

        loop {
            // ── Idle read timeout ──────────────────────────────────────
            // Once the request is out, the peer must keep making progress. A
            // peer that accepts the connection but then sends nothing trips this
            // before its own (often 60 s+) idle timeout. The connect and
            // handshake phases have their own deadlines, so they are skipped.
            if !self.read_timeout.is_zero()
                && !matches!(self.state, PumpPhase::Connecting | PumpPhase::Handshaking)
                && self.last_progress.is_some_and(|progress| {
                    now.saturating_duration_since(progress) >= self.read_timeout
                })
            {
                if self.response_head_sent {
                    // The head is already out. End the response stream here;
                    // there is no head pipe left to publish the error on.
                    let _ = self.body_tx.close();
                    let _ = self.trailer_tx.close();
                    return None;
                }
                self.fail_before_head(TransportError::Timeout);
                return None;
            }

            // ── Settle the non-blocking connect ─────────────────────────
            // `open` started the dial without waiting (feature 127). Check
            // `SO_ERROR` before any read: an in-flight socket answers a read
            // with `ENOTCONN`, not `WouldBlock`.
            if matches!(self.state, PumpPhase::Connecting) {
                // The caller dropped the call while the dial was in flight.
                if self.head_tx.is_closed() {
                    let _ = self.body_tx.close();
                    let _ = self.trailer_tx.close();
                    return None;
                }
                match self.stream.connect_state() {
                    Ok(true) => {
                        self.stream.finish_connect();
                        // A connected socket is writable at once; stop watching.
                        self.set_writable(false);
                        // The SETTINGS exchange starts now, not at `open`. A
                        // slow dial must not spend the handshake budget.
                        self.handshake_deadline = Some(now + self.handshake_timeout);
                        self.last_progress = Some(now);
                        self.state = PumpPhase::Handshaking;
                    }
                    Ok(false) => {
                        if now >= connecting_deadline {
                            tracing::warn!(
                                host = %self.host,
                                port = self.port,
                                "h2 pump: peer did not accept before the connect timeout"
                            );
                            // Use the platform's timeout error so the call
                            // sees the same `ConnectError` text that a
                            // blocking `connect_timeout` produced.
                            self.fail_before_head(TransportError::Connect(Arc::new(
                                io::Error::from(io::ErrorKind::TimedOut),
                            )));
                            return None;
                        }
                        return Some(self.park());
                    }
                    Err(e) => {
                        self.fail_before_head(TransportError::Connect(Arc::new(e)));
                        return None;
                    }
                }
            }

            // ── Read from socket, feed to channel ──────────────────────
            let mut buf = [0u8; 8192];
            let mut read_some = false;
            match self.stream.read(&mut buf) {
                Ok(0) if matches!(self.state, PumpPhase::Handshaking) => {
                    self.fail_before_head(TransportError::Connect(Arc::new(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "connection closed during handshake",
                    ))));
                    return None;
                }
                Ok(0) => {
                    // Connection closed
                    tracing::debug!(
                        phase = match self.state {
                            PumpPhase::Connecting => "Connecting",
                            PumpPhase::Handshaking => "Handshaking",
                            PumpPhase::SendingRequest => "SendingRequest",
                            PumpPhase::WaitingResponse => "WaitingResponse",
                            PumpPhase::Draining => "Draining",
                            PumpPhase::Done => "Done",
                        },
                        "h2 pump: connection closed by peer"
                    );
                    let _ = self.head_tx.close();
                    let _ = self.body_tx.close();
                    return None;
                }
                Ok(n) => {
                    self.channel.feed_input(&buf[..n]);
                    self.last_progress = Some(now);
                    read_some = true;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No data — continue to process channel
                }
                Err(e) if matches!(self.state, PumpPhase::Handshaking) => {
                    self.fail_before_head(TransportError::Connect(Arc::new(e)));
                    return None;
                }
                Err(e) => {
                    tracing::warn!(err = %e, "h2 pump: socket read error — exiting");
                    let _ = self.head_tx.close();
                    let _ = self.body_tx.close();
                    return None;
                }
            }

            match self.state {
                PumpPhase::Connecting => {
                    // The branch above resolves the dial or ends the task, so
                    // this arm is not reached in practice. Park rather than
                    // spin if a future change breaks that order.
                    return Some(self.park());
                }

                PumpPhase::Handshaking => {
                    // The caller dropped the call before the connection was up.
                    if self.head_tx.is_closed() {
                        let _ = self.body_tx.close();
                        let _ = self.trailer_tx.close();
                        return None;
                    }
                    match self.channel.client_handshake_step() {
                        // Done: fall through to the shared flush below, and the
                        // next loop turn sends the request.
                        Ok(()) => self.state = PumpPhase::SendingRequest,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            if let Err(e) = self.write_out() {
                                self.fail_before_head(TransportError::Connect(Arc::new(e)));
                                return None;
                            }
                            if now >= handshake_deadline {
                                tracing::warn!(
                                    host = %self.host,
                                    port = self.port,
                                    "h2 pump: peer sent no SETTINGS before the handshake timeout"
                                );
                                self.fail_before_head(TransportError::Timeout);
                                return None;
                            }
                            // More bytes may already be waiting: read them
                            // before parking, so no latched edge is lost.
                            if read_some {
                                continue;
                            }
                            return Some(self.park());
                        }
                        Err(e) => {
                            self.fail_before_head(TransportError::Connect(Arc::new(e)));
                            return None;
                        }
                    }
                }

                PumpPhase::SendingRequest => {
                    // ── Collect whatever body is available right now ───
                    // `Empty` means "nothing buffered yet", NOT end-of-body. Only
                    // `Closed` ends the request.
                    let mut chunks: Vec<Bytes> = Vec::new();
                    loop {
                        match self.send_rx.try_recv() {
                            Ok(b) => chunks.push(b),
                            Err(TryRecvError::Closed) => {
                                self.body_complete = true;
                                break;
                            }
                            Err(TryRecvError::Empty) => break,
                        }
                    }

                    // ── HEADERS, on the first poll ────────────────────
                    // Emitted immediately rather than waiting for the body to
                    // complete: a bidi caller keeps `send_rx` open while awaiting
                    // responses, so waiting here would deadlock. END_STREAM rides
                    // the HEADERS only when we already know there is no body.
                    if !self.request_sent {
                        let no_body = self.body_complete && chunks.is_empty();
                        let authority = Bytes::from(format!("{}:{}", self.host, self.port));

                        let mut h2_headers = Vec::new();
                        for (k, vals) in &self.req_headers {
                            for v in vals {
                                h2_headers.push((
                                    Bytes::copy_from_slice(k.to_string().as_bytes()),
                                    Bytes::copy_from_slice(v.as_bytes()),
                                ));
                            }
                        }

                        let req = H2Request {
                            method: Bytes::copy_from_slice(self.method.to_uppercase().as_bytes()),
                            scheme: Bytes::from_static(b"http"),
                            authority,
                            path: Bytes::copy_from_slice(self.path.as_bytes()),
                            headers: h2_headers,
                            // Body frames are streamed separately below, so the
                            // HEADERS never carries one. `end_stream` here means
                            // "no body at all" — `send_request` puts END_STREAM on
                            // HEADERS exactly when `body.is_none() && end_stream`.
                            body: None,
                            end_stream: no_body,
                        };

                        match self.channel.send_request(&req) {
                            Ok(sid) => {
                                self.stream_id = sid;
                                self.request_sent = true;
                                self.request_ended = no_body;
                            }
                            Err(_e) => {
                                let _ = self
                                    .head_tx
                                    .try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
                                let _ = self.head_tx.close();
                                let _ = self.body_tx.close();
                                return None;
                            }
                        }
                    }

                    // ── Stream body chunks as they arrive ─────────────
                    if self.request_sent && !self.request_ended {
                        for chunk in chunks {
                            if chunk.is_empty() {
                                continue;
                            }
                            if self
                                .channel
                                .send_data_frame(self.stream_id, &chunk, false)
                                .is_err()
                            {
                                let _ = self.head_tx.close();
                                let _ = self.body_tx.close();
                                return None;
                            }
                        }

                        // `send_rx` closed: terminate the request direction. The
                        // frame may be empty — END_STREAM is what matters.
                        if self.body_complete {
                            if self
                                .channel
                                .send_data_frame(self.stream_id, &[], true)
                                .is_err()
                            {
                                let _ = self.head_tx.close();
                                let _ = self.body_tx.close();
                                return None;
                            }
                            self.request_ended = true;
                        }
                    }

                    // Flush output
                    match self.write_out() {
                        Ok(true) => {}
                        // The socket is full. Drain anything already readable
                        // first, then park on writability (armed by `write_out`).
                        Ok(false) => {
                            if read_some {
                                continue;
                            }
                            return Some(self.park());
                        }
                        Err(_e) => {
                            let _ = self
                                .head_tx
                                .try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
                            let _ = self.head_tx.close();
                            let _ = self.body_tx.close();
                            return None;
                        }
                    }

                    // Move on once the head is out. The remaining body chunks (if
                    // any) keep streaming from the WaitingResponse/Draining
                    // phases, so a server that answers mid-request still works.
                    if self.request_sent {
                        self.state = PumpPhase::WaitingResponse;
                    } else {
                        if read_some {
                            continue;
                        }
                        return Some(self.park());
                    }
                }

                PumpPhase::WaitingResponse => {
                    // The response consumer went away (it dropped the head/body
                    // pipes, e.g. on cancellation) before the head arrived. Nothing
                    // will ever read this response — close up rather than leak the
                    // socket waiting on a peer whose reader is gone.
                    if self.head_tx.is_closed() {
                        self.abort_peer_stream();
                        let _ = self.body_tx.close();
                        let _ = self.trailer_tx.close();
                        return None;
                    }
                    // Keep feeding the request: a bidi server may not answer until
                    // it has seen some (or all) of the request body.
                    if !self.pump_request_body() {
                        let _ = self.head_tx.close();
                        let _ = self.body_tx.close();
                        return None;
                    }

                    // ── Try to read a response ─────────────────────────
                    match self.channel.recv_response() {
                        Ok(Some((_sid, resp))) => {
                            // Extract pseudo-header :status from response headers
                            let mut status = Status::OK;
                            let mut resp_headers = SimpleHeaders::new();

                            for (name, value) in &resp.headers {
                                let n = String::from_utf8_lossy(name);
                                if n == ":status" {
                                    if let Ok(code) =
                                        String::from_utf8_lossy(value).trim().parse::<u16>()
                                    {
                                        status = status_from_u16(code);
                                    }
                                } else {
                                    resp_headers
                                        .entry(SimpleHeader::from(n.to_string()))
                                        .or_default()
                                        .push(String::from_utf8_lossy(value).to_string());
                                }
                            }

                            let _ = self.head_tx.try_send(Ok((status, resp_headers)));
                            let _ = self.head_tx.close();
                            self.response_head_sent = true;

                            // Queue body if present; `flush_response_body` honours
                            // the pipe's capacity rather than dropping on Full.
                            if let Some(body) = &resp.body {
                                if !body.is_empty() {
                                    self.pending_body.push_back(body.clone());
                                }
                            }
                            if resp.end_stream {
                                self.response_ended = true;
                            }
                            if !self.flush_response_body() {
                                return None;
                            }

                            if self.response_ended && self.pending_body.is_empty() {
                                self.state = PumpPhase::Done;
                                return Some(TaskStatus::Pending(()));
                            }

                            self.state = PumpPhase::Draining;
                        }
                        Ok(None) => {
                            // GOAWAY or connection closed
                            let _ = self
                                .head_tx
                                .try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
                            let _ = self.head_tx.close();
                            let _ = self.body_tx.close();
                            return None;
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // Need more data — flush writes, then park on the
                            // socket. Drain any bytes still in the kernel first.
                            if self.write_out().is_err() {
                                let _ = self.head_tx.close();
                                let _ = self.body_tx.close();
                                return None;
                            }
                            if read_some {
                                continue;
                            }
                            return Some(self.park());
                        }
                        Err(_e) => {
                            let _ = self
                                .head_tx
                                .try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
                            let _ = self.head_tx.close();
                            let _ = self.body_tx.close();
                            return None;
                        }
                    }
                }

                PumpPhase::Draining => {
                    // The request direction may still be open (bidi).
                    if !self.pump_request_body() {
                        let _ = self.body_tx.close();
                        return None;
                    }

                    // The response consumer went away mid-body (dropped
                    // `recv_body`, e.g. on cancellation) while nothing was buffered
                    // to reveal it through `flush_response_body`. Close up rather
                    // than leak the socket. `response_ended` is still false, so the
                    // pump has not closed `body_tx` itself — a closed pipe here
                    // means the receiver is gone.
                    if self.body_tx.is_closed() && !self.response_ended {
                        self.abort_peer_stream();
                        let _ = self.trailer_tx.close();
                        return None;
                    }

                    // Retry anything the consumer was too slow to take. Only pull
                    // a new frame once the backlog has drained, so ordering holds.
                    if !self.flush_response_body() {
                        return None;
                    }
                    if !self.pending_body.is_empty() {
                        // The response pipe is full. Read the rest of what the
                        // kernel has before parking on the pipe's vacancy.
                        if read_some {
                            continue;
                        }
                        return Some(self.park());
                    }
                    if self.response_ended {
                        self.state = PumpPhase::Done;
                        return Some(TaskStatus::Pending(()));
                    }

                    match self.channel.recv_stream_event() {
                        Ok(Some((
                            _sid,
                            H2StreamEvent::Data {
                                data,
                                end_stream: end,
                                ..
                            },
                        ))) => {
                            if !data.is_empty() {
                                self.pending_body.push_back(data);
                            }
                            if end {
                                self.response_ended = true;
                            }
                            if !self.flush_response_body() {
                                return None;
                            }
                            if self.response_ended && self.pending_body.is_empty() {
                                self.state = PumpPhase::Done;
                                return Some(TaskStatus::Pending(()));
                            }
                        }
                        Ok(Some((_sid, H2StreamEvent::Trailers { headers, .. }))) => {
                            // Decode trailing headers and push them into the
                            // trailer pipe so the caller can read grpc-status etc.
                            let mut trailers = SimpleHeaders::new();
                            for (name, value) in &headers {
                                let n = String::from_utf8_lossy(name);
                                let v = String::from_utf8_lossy(value);
                                // Skip pseudo-headers (:status etc.).
                                if !n.starts_with(':') {
                                    trailers
                                        .entry(SimpleHeader::from(n.to_string()))
                                        .or_default()
                                        .push(v.to_string());
                                }
                            }
                            let _ = self.trailer_tx.try_send(trailers);
                            let _ = self.trailer_tx.close();
                            // Trailers carry END_STREAM, so the body is also done.
                            self.response_ended = true;
                            if !self.flush_response_body() {
                                return None;
                            }
                            if self.pending_body.is_empty() {
                                self.state = PumpPhase::Done;
                                return Some(TaskStatus::Pending(()));
                            }
                        }
                        Ok(None) => {
                            let _ = self.body_tx.close();
                            let _ = self.trailer_tx.close();
                            self.state = PumpPhase::Done;
                            return Some(TaskStatus::Pending(()));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            if self.write_out().is_err() {
                                let _ = self.body_tx.close();
                                let _ = self.trailer_tx.close();
                                return None;
                            }
                            if read_some {
                                continue;
                            }
                            return Some(self.park());
                        }
                        Err(_e) => {
                            let _ = self.body_tx.close();
                            let _ = self.trailer_tx.close();
                            return None;
                        }
                    }
                }

                PumpPhase::Done => {
                    // The response direction is finished, and `Done` is terminal.
                    //
                    // Closing `body_tx` is what tells the consumer end-of-stream.
                    // Leaving it to the sender's eventual drop parks the reader
                    // until the *peer's* idle timeout closes the socket — a 60s
                    // stall on a call that completed in milliseconds.
                    //
                    // Returning `None` completes the task. `TaskStatus::Ready`
                    // only yields a value, so the executor polls again, lands on
                    // `Done` again, and spins: nobody consumes this task's value
                    // (`valtron::send` is fire-and-forget), so it never ended.
                    let _ = self.body_tx.close();
                    let _ = self.head_tx.close();
                    let _ = self.trailer_tx.close();
                    return None;
                }
            }

            // Flush output
            match self.write_out() {
                Ok(true) => {}
                Ok(false) => {
                    if read_some {
                        continue;
                    }
                    return Some(self.park());
                }
                Err(_e) => {
                    let _ = self.head_tx.close();
                    let _ = self.body_tx.close();
                    return None;
                }
            }
        }
    }
}

fn status_from_u16(code: u16) -> Status {
    match code {
        200 => Status::OK,
        201 => Status::Created,
        204 => Status::NoContent,
        400 => Status::BadRequest,
        401 => Status::Unauthorized,
        403 => Status::Forbidden,
        404 => Status::NotFound,
        502 => Status::BadGateway,
        503 => Status::ServiceUnavailable,
        _ => Status::InternalServerError,
    }
}
