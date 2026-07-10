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

/// Copy bytes in both directions between `a` and `b` until one side closes.
///
/// WHAT: Reads whatever is available on each side and writes it to the other,
/// looping until an EOF (`Ok(0)`) or a non-`WouldBlock` error on either side.
///
/// # Preconditions
/// Both streams must already be in non-blocking mode; otherwise a read on an
/// idle side blocks the other direction. Callers own that setup because the two
/// stream types differ.
pub fn splice_bidirectional<A: Read + Write, B: Read + Write>(mut a: A, mut b: B) {
    let mut buf = [0u8; 16 * 1024];
    loop {
        let mut progressed = false;

        match a.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if b.write_all(&buf[..n]).and_then(|()| b.flush()).is_err() {
                    break;
                }
                progressed = true;
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        match b.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if a.write_all(&buf[..n]).and_then(|()| a.flush()).is_err() {
                    break;
                }
                progressed = true;
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        if !progressed {
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

// ═══════════════════════════════════════════════════════════════════════════════
// UdpPassthrough — raw UDP datagram relay (Decision 17)
// ═══════════════════════════════════════════════════════════════════════════════

/// A raw UDP passthrough: datagrams arriving on `listen_addr` are forwarded to
/// `backend_authority` and responses are sent back to the originating client.
///
/// WHY: Decision 17 — `udp://` backends need byte-level passthrough with no HTTP
/// semantics (DNS, QUIC, game servers, syslog, STUN).
///
/// HOW: Binds a single `UdpSocket`. Each datagram is forwarded to the backend;
/// the backend's response is sent back to the client address. Connectionless —
/// one socket serves all clients. The relay runs on a dedicated OS thread.
#[derive(Debug)]
pub struct UdpPassthrough {
    local_addr: std::net::SocketAddr,
    shutdown: Arc<OnSignal>,
    relay_thread: Option<JoinHandle<()>>,
}

/// How long the relay waits for a backend response before giving up on that
/// datagram. The relay checks the shutdown signal on every poll cycle (5ms
/// sleep), so shutdown is never delayed by more than 5ms.
const BACKEND_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

impl UdpPassthrough {
    /// Bind `listen_addr` and relay every datagram to `backend_authority`.
    ///
    /// Returns immediately; the relay loop runs on its own thread until
    /// [`UdpPassthrough::shutdown`] (or drop).
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

/// Single-threaded relay: recv client datagram → send to backend → recv
/// response → send to client. Stays non-blocking so shutdown is responsive.
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
                if let Err(e) = socket.send_to(&buf[..n], backend) {
                    tracing::warn!(%backend, "UDP relay send to backend failed: {e}");
                    continue;
                }
                // Poll backend for response — non-blocking spin with short
                // sleeps, shutdown check on every iteration.
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
