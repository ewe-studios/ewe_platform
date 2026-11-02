//! Raw byte-level passthrough.
//!
//! WHY: Decision 14 §"Protocol inference" — a `tcp://` backend is proxied at the
//! byte level with no HTTP semantics (RDP, VNC, noVNC). The same primitive
//! relays a WebSocket connection once its `101` handshake is done: after the
//! upgrade everything is opaque frames. Both cases are "copy bytes both ways
//! until one side closes".
//!
//! WHAT: [`TcpPassthrough`] (a standalone raw TCP listener that splices every
//! accepted client to a fixed backend authority), [`UdpPassthrough`] (the same
//! for datagrams) and [`splice_bidirectional`] (a blocking two-way copy over
//! any `Read + Write` pair; the proxy itself no longer calls it).
//!
//! HOW: The listeners are futures on the valtron pool. The accept loop parks on
//! the listener's read readiness, each client is its own future that runs
//! [`crate::aio::splice`], and the UDP loop parks on the socket's read readiness.
//! No thread is started per client or per listener, and none sleeps. A loop
//! also wakes on a timer every `shutdown_poll` to see the shutdown signal.
//! [`splice_bidirectional`] keeps its polling loop: it serves generic
//! `Read + Write` pairs such as io_uring completion sockets, and only tests
//! call it (backlog section 9).

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::synca::OnSignal;
use foundation_core::valtron::{from_future, send, sleep_async};
use foundation_http::native::writer::WriterConfig;
use foundation_iogate::ServerIo;
use foundation_netio::shared::client::{DnsResolver, SystemDnsResolver};
use futures_util::future::{select, Either};

use crate::aio::{dial_tcp, splice, wrap_tcp, AsyncFd};

/// Limits and modes for [`TcpPassthrough`] and [`UdpPassthrough`].
#[derive(Debug, Clone, Copy)]
pub struct PassthroughConfig {
    /// How often an idle loop wakes to check for shutdown. A timer wake, not a
    /// thread sleep. Default 50 ms (the control socket's former poll).
    pub shutdown_poll: Duration,
    /// How long the UDP relay waits for the backend to answer one datagram.
    /// Default 5 seconds.
    pub udp_response_timeout: Duration,
    /// How the backend leg is dialed. Default `Std`.
    pub io_mode: ServerIo,
    /// Write limits for the spliced connections (`send_timeout` is nginx's 60 s).
    pub writer: WriterConfig,
}

impl Default for PassthroughConfig {
    fn default() -> Self {
        Self {
            shutdown_poll: Duration::from_millis(50),
            udp_response_timeout: Duration::from_secs(5),
            io_mode: ServerIo::default(),
            writer: WriterConfig::default(),
        }
    }
}

impl PassthroughConfig {
    /// The defaults documented on each field.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::shutdown_poll`].
    #[must_use]
    pub fn with_shutdown_poll(mut self, poll: Duration) -> Self {
        self.shutdown_poll = poll;
        self
    }

    /// Set [`Self::udp_response_timeout`].
    #[must_use]
    pub fn with_udp_response_timeout(mut self, timeout: Duration) -> Self {
        self.udp_response_timeout = timeout;
        self
    }

    /// Set [`Self::io_mode`].
    #[must_use]
    pub fn with_io_mode(mut self, mode: ServerIo) -> Self {
        self.io_mode = mode;
        self
    }

    /// Set [`Self::writer`].
    #[must_use]
    pub fn with_writer(mut self, writer: WriterConfig) -> Self {
        self.writer = writer;
        self
    }
}

/// Wait for `fut`, or give up after `tick`: `None` means the tick came first.
async fn or_tick<T>(fut: impl std::future::Future<Output = T>, tick: Duration) -> Option<T> {
    match select(Box::pin(fut), Box::pin(sleep_async(tick))).await {
        Either::Left((value, _)) => Some(value),
        Either::Right(((), _)) => None,
    }
}

/// Start `task` on the valtron pool and return a channel that gets one message
/// when it ends.
fn spawn_loop(
    task: impl std::future::Future<Output = ()> + Send + 'static,
    done: Sender<()>,
) -> std::io::Result<()> {
    send(from_future(async move {
        task.await;
        let _ = done.send(());
    }))
    .map_err(|e| std::io::Error::other(format!("cannot start the passthrough loop: {e}")))
}

/// Stop a loop: set its signal and wait (bounded) until it has ended and
/// dropped its socket. A blocking wait on the caller's thread.
fn stop_loop(shutdown: &OnSignal, done: &Receiver<()>, poll: Duration) {
    shutdown.turn_on();
    let _ = done.recv_timeout(poll.saturating_mul(4) + Duration::from_secs(1));
}

/// One direction of a bidirectional splice: bytes read from a source and being
/// relayed to a sink, with the backpressure state that keeps the relay correct
/// on both readiness (`Std`) and completion (io_uring `SEND`) sinks.
struct HalfRelay {
    /// Bytes read from the source, awaiting write to the sink.
    pending: Vec<u8>,
    /// How many bytes of `pending` have already been accepted by the sink.
    written: usize,
    /// The sink has an un-acked write. On a completion sink `flush` returns
    /// `WouldBlock` ("send still in flight"); that is not an error, and the
    /// relay must drain it to completion before submitting the next chunk so
    /// separate `IORING_OP_SEND`s cannot land out of order.
    flushing: bool,
}

/// The outcome of pumping one direction for one tick.
enum Pump {
    /// Bytes moved (read, written, or a flush completed) — keep spinning hot.
    Progressed,
    /// Nothing to do this tick (source empty, or a send still draining).
    Idle,
    /// EOF or a hard error — the splice is over.
    Closed,
}

impl HalfRelay {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            written: 0,
            flushing: false,
        }
    }

    /// Move one step of `src → dst`: finish an outstanding flush, drain buffered
    /// bytes, then read more. Ordering is preserved by never writing new bytes
    /// while a previous completion-mode send is still in flight.
    fn pump<S: Read, D: Write>(&mut self, src: &mut S, dst: &mut D, buf: &mut [u8]) -> Pump {
        // 1. A prior write is still draining — finish it before anything else.
        if self.flushing {
            match dst.flush() {
                Ok(()) => self.flushing = false,
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => return Pump::Idle,
                Err(_) => return Pump::Closed,
            }
        }

        // 2. Drain buffered bytes into the sink.
        if self.written < self.pending.len() {
            match dst.write(&self.pending[self.written..]) {
                Ok(0) => return Pump::Closed,
                Ok(n) => {
                    self.written += n;
                    if self.written >= self.pending.len() {
                        self.pending.clear();
                        self.written = 0;
                        // Kick the flush; a completion sink reports the send as
                        // still in flight (`WouldBlock`) — drain it next tick.
                        match dst.flush() {
                            Ok(()) => {}
                            Err(ref e) if e.kind() == ErrorKind::WouldBlock => self.flushing = true,
                            Err(_) => return Pump::Closed,
                        }
                    }
                    return Pump::Progressed;
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => return Pump::Idle,
                Err(_) => return Pump::Closed,
            }
        }

        // 3. Buffer is empty — read the next chunk from the source.
        match src.read(buf) {
            Ok(0) => Pump::Closed,
            Ok(n) => {
                self.pending.extend_from_slice(&buf[..n]);
                Pump::Progressed
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => Pump::Idle,
            Err(_) => Pump::Closed,
        }
    }
}

/// Copy bytes in both directions between `a` and `b` until one side closes.
///
/// WHAT: Reads whatever is available on each side and writes it to the other,
/// looping until an EOF (`Ok(0)`) or a non-`WouldBlock` error on either side.
///
/// # Preconditions
/// Both streams must already be in non-blocking mode; otherwise a read on an
/// idle side blocks the other direction. Callers own that setup because the two
/// stream types differ.
///
/// The sinks may be readiness sockets (`flush` is synchronous) or io_uring
/// completion sockets (`flush` is a barrier that reports `WouldBlock` while a
/// `SEND` is still in flight). Both are handled: a `WouldBlock` from `write` or
/// `flush` is backpressure, not failure.
pub fn splice_bidirectional<A: Read + Write, B: Read + Write>(mut a: A, mut b: B) {
    let mut buf = [0u8; 16 * 1024];
    let mut a_to_b = HalfRelay::new();
    let mut b_to_a = HalfRelay::new();

    loop {
        let ab = a_to_b.pump(&mut a, &mut b, &mut buf);
        let ba = b_to_a.pump(&mut b, &mut a, &mut buf);

        if matches!(ab, Pump::Closed) || matches!(ba, Pump::Closed) {
            break;
        }
        // Neither direction moved: nothing buffered, nothing readable, no send
        // draining — sleep briefly rather than burn the core.
        if matches!(ab, Pump::Idle) && matches!(ba, Pump::Idle) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// A raw TCP passthrough listener: every accepted client is spliced to
/// `backend_authority`.
///
/// WHY: The HTTP front end parses HTTP before dispatch, so a genuinely raw
/// protocol (`tcp://` service) cannot ride it — it needs its own listener that
/// hands the bytes straight to the backend.
///
/// The valtron pool must be initialised before [`TcpPassthrough::start`].
#[derive(Debug)]
pub struct TcpPassthrough {
    local_addr: SocketAddr,
    shutdown: Arc<OnSignal>,
    done: Receiver<()>,
    poll: Duration,
}

impl TcpPassthrough {
    /// Bind `listen_addr` and splice every connection to `backend_authority`,
    /// with the default [`PassthroughConfig`].
    ///
    /// Returns immediately with a handle; the accept loop runs as a future until
    /// [`TcpPassthrough::shutdown`] (or drop).
    ///
    /// # Errors
    /// Returns the bind error if `listen_addr` cannot be bound, or an error if
    /// the loop cannot be started on the pool.
    pub fn start(listen_addr: &str, backend_authority: &str) -> std::io::Result<Self> {
        Self::start_with(listen_addr, backend_authority, PassthroughConfig::default())
    }

    /// Like [`Self::start`], with explicit limits.
    ///
    /// # Errors
    /// As [`Self::start`].
    pub fn start_with(
        listen_addr: &str,
        backend_authority: &str,
        config: PassthroughConfig,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(listen_addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let listener = AsyncFd::new(listener)?;
        let shutdown = Arc::new(OnSignal::new());
        let backend = backend_authority.to_string();
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        spawn_loop(
            accept_loop(listener, backend, Arc::clone(&shutdown), config),
            done_tx,
        )?;

        Ok(Self {
            local_addr,
            shutdown,
            done: done_rx,
            poll: config.shutdown_poll,
        })
    }

    /// The actual bound address (useful when binding to port 0).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Stop accepting. Clients already spliced run until they close.
    pub fn shutdown(self) {
        stop_loop(&self.shutdown, &self.done, self.poll);
    }
}

impl Drop for TcpPassthrough {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

/// Accept loop for [`TcpPassthrough`]: one splice future per client.
async fn accept_loop(
    listener: AsyncFd<TcpListener>,
    backend: String,
    shutdown: Arc<OnSignal>,
    config: PassthroughConfig,
) {
    while !shutdown.probe() {
        let accepted = or_tick(listener.read_with(|l| l.accept()), config.shutdown_poll).await;
        match accepted {
            None => {}
            Some(Ok((client, _addr))) => {
                let backend = backend.clone();
                let task = async move {
                    if let Err(e) = splice_client_to_backend(client, &backend, config).await {
                        tracing::warn!(%backend, "tcp passthrough failed: {e}");
                    }
                };
                if let Err(e) = send(from_future(task)) {
                    tracing::error!("tcp passthrough cannot start a client relay: {e}");
                }
            }
            Some(Err(e)) => {
                tracing::error!("tcp passthrough accept error: {e}");
                // Back off one poll so a persistent error does not spin.
                sleep_async(config.shutdown_poll).await;
            }
        }
    }
}

/// Connect to the backend and splice a single client connection to it.
async fn splice_client_to_backend(
    client: std::net::TcpStream,
    backend_authority: &str,
    config: PassthroughConfig,
) -> std::io::Result<()> {
    let client = wrap_tcp(client)?;
    let backend = dial_tcp(backend_authority, config.io_mode).await?;
    splice(client, backend, config.writer).await;
    Ok(())
}

/// A raw UDP passthrough: datagrams arriving on `listen_addr` are forwarded to
/// `backend_authority` and responses are sent back to the originating client.
///
/// WHY: Decision 17 — `udp://` backends need byte-level passthrough with no HTTP
/// semantics (DNS, QUIC, game servers, syslog, STUN).
///
/// HOW: Binds a single `UdpSocket`. Each datagram is forwarded to the backend;
/// the backend's response is sent back to the client address that sent the
/// original datagram.  Connectionless — one socket serves all clients. The relay
/// is one future that parks on the socket's read readiness.
///
/// The valtron pool must be initialised before [`UdpPassthrough::start`].
#[derive(Debug)]
pub struct UdpPassthrough {
    local_addr: SocketAddr,
    shutdown: Arc<OnSignal>,
    done: Receiver<()>,
    poll: Duration,
}

impl UdpPassthrough {
    /// Bind `listen_addr` and relay every datagram to `backend_authority`, with
    /// the default [`PassthroughConfig`].
    ///
    /// Returns immediately with a handle; the relay runs as a future until
    /// [`UdpPassthrough::shutdown`] (or drop).
    ///
    /// # Errors
    /// Returns the bind error if `listen_addr` cannot be bound, or an error if
    /// the loop cannot be started on the pool.
    pub fn start(listen_addr: &str, backend_authority: &str) -> std::io::Result<Self> {
        Self::start_with(listen_addr, backend_authority, PassthroughConfig::default())
    }

    /// Like [`Self::start`], with explicit limits.
    ///
    /// # Errors
    /// As [`Self::start`].
    pub fn start_with(
        listen_addr: &str,
        backend_authority: &str,
        config: PassthroughConfig,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(listen_addr)?;
        socket.set_nonblocking(true)?;
        let local_addr = socket.local_addr()?;
        let socket = AsyncFd::new(socket)?;
        let shutdown = Arc::new(OnSignal::new());
        let backend = backend_authority.to_string();
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        spawn_loop(
            relay_udp(socket, backend, Arc::clone(&shutdown), config),
            done_tx,
        )?;

        Ok(Self {
            local_addr,
            shutdown,
            done: done_rx,
            poll: config.shutdown_poll,
        })
    }

    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Stop relaying.
    pub fn shutdown(self) {
        stop_loop(&self.shutdown, &self.done, self.poll);
    }
}

impl Drop for UdpPassthrough {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

/// Relay loop: client datagram → backend, response → client.
async fn relay_udp(
    socket: AsyncFd<UdpSocket>,
    backend_authority: String,
    shutdown: Arc<OnSignal>,
    config: PassthroughConfig,
) {
    let backend = match resolve_first(&backend_authority).await {
        Some(addr) => addr,
        None => {
            tracing::error!(backend_addr = %backend_authority, "UDP relay: cannot resolve backend address");
            return;
        }
    };
    let mut buf = vec![0u8; 65535];

    while !shutdown.probe() {
        let received = or_tick(
            socket.read_with(|s| s.recv_from(&mut buf)),
            config.shutdown_poll,
        )
        .await;
        let (n, client_addr) = match received {
            None => continue,
            Some(Ok(pair)) => pair,
            Some(Err(e)) => {
                tracing::error!("UDP relay recv error: {e}");
                return;
            }
        };
        // Forward to the backend.
        if let Err(e) = socket.write_with(|s| s.send_to(&buf[..n], backend)).await {
            tracing::warn!(%backend, "UDP relay send to backend failed: {e}");
            continue;
        }
        // Wait for the response, in slices so shutdown stays responsive.
        let deadline = Instant::now() + config.udp_response_timeout;
        let mut responded = false;
        while !responded && Instant::now() < deadline && !shutdown.probe() {
            let slice = config
                .shutdown_poll
                .min(deadline.saturating_duration_since(Instant::now()));
            match or_tick(socket.read_with(|s| s.recv_from(&mut buf)), slice).await {
                None => {}
                Some(Ok((m, _from))) => {
                    if let Err(e) = socket
                        .write_with(|s| s.send_to(&buf[..m], client_addr))
                        .await
                    {
                        tracing::warn!(%client_addr, "UDP relay send to client failed: {e}");
                    }
                    responded = true;
                }
                Some(Err(e)) => {
                    tracing::warn!("UDP relay recv from backend failed: {e}");
                    break;
                }
            }
        }
        if !responded {
            tracing::trace!(
                "UDP relay: no response from backend within {:?}",
                config.udp_response_timeout
            );
        }
    }
}

/// Resolve the first socket address for `host:port` without blocking a thread.
async fn resolve_first(authority: &str) -> Option<SocketAddr> {
    let (host, port) = authority.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    SystemDnsResolver::default()
        .resolve(host.trim_matches(['[', ']']), port)
        .await
        .ok()?
        .into_iter()
        .next()
}
