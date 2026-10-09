//! Raw byte-level passthrough.
//!
//! WHY: Decision 14 §"Protocol inference" — a `tcp://` backend is proxied at the
//! byte level with no HTTP semantics (RDP, VNC, noVNC). The same primitive
//! relays a WebSocket connection once its `101` handshake is done: after the
//! upgrade everything is opaque frames. Both cases are "copy bytes both ways
//! until one side closes".
//!
//! WHAT: [`splice_bidirectional`] (the generic two-way copy used by the
//! WebSocket relay) and [`TcpPassthrough`] (a standalone raw TCP listener that
//! splices every accepted client to a fixed backend authority).
//!
//! HOW: One poll loop on a single thread moves data in both directions over
//! non-blocking sockets, sleeping briefly only when neither side had data. EOF
//! or a hard error on either side ends the splice.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use foundation_core::synca::OnSignal;

/// How long the UDP relay waits for a backend response before giving up on that
/// datagram.  The relay checks the shutdown signal on every poll cycle, so
/// shutdown is never delayed by more than the poll sleep (5ms).
const BACKEND_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

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
        Self { pending: Vec::new(), written: 0, flushing: false }
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
#[derive(Debug)]
pub struct TcpPassthrough {
    local_addr: std::net::SocketAddr,
    shutdown: Arc<OnSignal>,
    accept_thread: Option<JoinHandle<()>>,
}

impl TcpPassthrough {
    /// Bind `listen_addr` and splice every connection to `backend_authority`.
    ///
    /// Returns immediately with a handle; the accept loop runs on its own
    /// thread until [`TcpPassthrough::shutdown`] (or drop).
    ///
    /// # Errors
    /// Returns the bind error if `listen_addr` cannot be bound.
    pub fn start(listen_addr: &str, backend_authority: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(listen_addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let shutdown = Arc::new(OnSignal::new());
        let backend = backend_authority.to_string();

        let loop_shutdown = Arc::clone(&shutdown);
        let accept_thread = std::thread::spawn(move || {
            accept_loop(&listener, &backend, &loop_shutdown);
        });

        Ok(Self {
            local_addr,
            shutdown,
            accept_thread: Some(accept_thread),
        })
    }

    /// The actual bound address (useful when binding to port 0).
    #[must_use]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.local_addr
    }

    /// Stop accepting and join the accept thread.
    pub fn shutdown(mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TcpPassthrough {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Accept loop for [`TcpPassthrough`]: one splice thread per client.
fn accept_loop(listener: &TcpListener, backend_authority: &str, shutdown: &Arc<OnSignal>) {
    loop {
        if shutdown.probe() {
            return;
        }
        match listener.accept() {
            Ok((client, _addr)) => {
                let backend = backend_authority.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = splice_client_to_backend(client, &backend) {
                        tracing::warn!(%backend, "tcp passthrough failed: {e}");
                    }
                });
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                tracing::error!("tcp passthrough accept error: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Connect to the backend and splice a single client connection to it.
fn splice_client_to_backend(client: TcpStream, backend_authority: &str) -> std::io::Result<()> {
    let backend = TcpStream::connect(backend_authority)?;
    client.set_nonblocking(true)?;
    backend.set_nonblocking(true)?;
    splice_bidirectional(client, backend);
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
/// original datagram.  Connectionless — one socket serves all clients.
#[derive(Debug)]
pub struct UdpPassthrough {
    local_addr: std::net::SocketAddr,
    shutdown: Arc<OnSignal>,
    relay_thread: Option<JoinHandle<()>>,
}

impl UdpPassthrough {
    /// Bind `listen_addr` and relay every datagram to `backend_authority`.
    ///
    /// Returns immediately with a handle; the relay loop runs on its own thread
    /// until [`UdpPassthrough::shutdown`] (or drop).
    ///
    /// # Errors
    /// Returns the bind error if `listen_addr` cannot be bound.
    pub fn start(listen_addr: &str, backend_authority: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(listen_addr)?;
        socket.set_nonblocking(true)?;
        let local_addr = socket.local_addr()?;
        let shutdown = Arc::new(OnSignal::new());
        let backend = backend_authority.to_string();

        let loop_shutdown = Arc::clone(&shutdown);
        let relay_thread = std::thread::spawn(move || {
            relay_udp(socket, &backend, &loop_shutdown);
        });

        Ok(Self {
            local_addr,
            shutdown,
            relay_thread: Some(relay_thread),
        })
    }

    #[must_use]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.local_addr
    }

    pub fn shutdown(mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.relay_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for UdpPassthrough {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.relay_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Single-threaded relay loop: client datagram → backend, response → client.
fn relay_udp(socket: UdpSocket, backend_addr: &str, shutdown: &Arc<OnSignal>) {
    let mut buf = [0u8; 65535];
    let backend = match backend_addr.to_socket_addrs_first() {
        Some(addr) => addr,
        None => {
            tracing::error!(%backend_addr, "UDP relay: cannot resolve backend address");
            return;
        }
    };

    loop {
        if shutdown.probe() {
            return;
        }
        match socket.recv_from(&mut buf) {
            Ok((n, client_addr)) => {
                // Forward to backend.
                if let Err(e) = socket.send_to(&buf[..n], backend) {
                    tracing::warn!(%backend, "UDP relay send to backend failed: {e}");
                    continue;
                }
                // Poll for a response while keeping shutdown responsive.
                let deadline = Instant::now() + BACKEND_RESPONSE_TIMEOUT;
                let mut responded = false;
                while Instant::now() < deadline {
                    if shutdown.probe() {
                        return;
                    }
                    match socket.recv_from(&mut buf) {
                        Ok((m, _from)) => {
                            if let Err(e) = socket.send_to(&buf[..m], client_addr) {
                                tracing::warn!(%client_addr, "UDP relay send to client failed: {e}");
                            }
                            responded = true;
                            break;
                        }
                        Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => {
                            tracing::warn!("UDP relay recv from backend failed: {e}");
                            break;
                        }
                    }
                }
                if !responded {
                    tracing::trace!("UDP relay: no response from backend within {:?}", BACKEND_RESPONSE_TIMEOUT);
                }
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => {
                tracing::error!("UDP relay recv error: {e}");
                break;
            }
        }
    }
}

/// Minimal helper: resolve the first socket address for `host:port`.
trait FirstSocketAddr {
    fn to_socket_addrs_first(&self) -> Option<std::net::SocketAddr>;
}

impl FirstSocketAddr for str {
    fn to_socket_addrs_first(&self) -> Option<std::net::SocketAddr> {
        use std::net::ToSocketAddrs;
        self.to_socket_addrs().ok().and_then(|mut it| it.next())
    }
}
