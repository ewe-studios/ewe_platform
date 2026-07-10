//! The valtron driver for `quinn-proto` (F33).
//!
//! WHY: `quinn-proto` is sans-IO. It never touches a socket and never sleeps: it
//! is a state machine that consumes datagrams and timeouts and produces datagrams
//! and events. Someone has to do the I/O. That someone is a valtron task, not a
//! tokio reactor — which is the whole reason we do not use `quinn` or `h3`.
//!
//! WHAT: [`QuicDriver`] — a `TaskIterator` owning one `quinn_proto::Endpoint`, one
//! UDP socket, and the connections that endpoint routes for. Drives handshakes,
//! transmits, timeouts, and stream events, and hands out [`QuinnConnection`]s.
//!
//! HOW: one poll does a bounded amount of work and returns. Idle polls park via
//! `TaskStatus::Depends(fd_readiness)` on the native reactor (Decisions 00/14)
//! when the caller injects a readiness signal, so an idle QUIC connection costs
//! nothing; without one it falls back to `Delayed`.
//!
//! ## Why the endpoint is here and not in `ConnState`
//!
//! A single `Endpoint` routes datagrams for **many** connections and answers
//! stateless retries and version negotiation. It cannot be per-connection state.
//! Per-connection state ([`ConnState`]) is the `Connection` state machine, the
//! peer address, and the accept queues; the driver owns the endpoint and the
//! socket and pumps every connection it routes.
//!
//! ## Timeouts are not optional
//!
//! `quinn_proto::Connection::poll_timeout()` reports when the state machine next
//! needs attention — loss detection, idle timeout, keep-alive. A driver that never
//! calls `handle_timeout` looks fine on a healthy link and silently stalls on a
//! lossy one, because retransmissions never fire.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::BytesMut;
use foundation_core::valtron::{BoxedSendExecutionAction, EventReadinessPtr, TaskIterator, TaskStatus};
use quinn_proto::{
    ClientConfig, ConnectionHandle, DatagramEvent, Endpoint, EndpointConfig, ServerConfig,
    TransportConfig, VarInt,
};

use super::quinn_impl::QuinnConnection;
use super::state::{ConnState, SharedConn};
use super::traits::QuicConnError;

/// Maximum UDP datagram we will read in one go.
const MAX_DATAGRAM: usize = 65_535;

/// Fallback poll interval when the caller injected no readiness signal.
const IDLE_POLL: Duration = Duration::from_millis(1);

/// What the driver surfaces to its caller.
#[derive(Debug)]
pub enum QuicEvent {
    /// A connection completed its handshake and is ready for streams.
    ///
    /// For a server this is also the accept signal — take the connection with
    /// [`QuicDriver::take_accepted`].
    Connected,
    /// A connection ended.
    ConnectionClosed {
        /// Human-readable reason, for logs.
        reason: String,
    },
}

/// Drives one `quinn_proto::Endpoint` and every connection it routes.
pub struct QuicDriver {
    endpoint: Endpoint,
    socket: Arc<UdpSocket>,
    /// Every connection this endpoint routes for.
    conns: HashMap<ConnectionHandle, SharedConn>,
    /// Connections that finished their handshake and have not been taken yet.
    accepted: Arc<Mutex<VecDeque<QuinnConnection>>>,
    /// Scratch buffer for endpoint responses (retry, version negotiation).
    scratch: Vec<u8>,
    /// Scratch buffer for connection transmits.
    tx_buf: Vec<u8>,
    /// Caller-injected fd readiness. When `Some`, an idle poll returns
    /// `TaskStatus::Depends(fd)` — parking on the reactor rather than spinning.
    fd_readiness: Option<EventReadinessPtr>,
    idle: Duration,
    /// A server driver keeps listening with no connections; a client driver ends.
    is_server: bool,
    /// Set when the driver has nothing left to drive.
    done: bool,
}

impl std::fmt::Debug for QuicDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuicDriver")
            .field("connections", &self.conns.len())
            .field("parks_on_reactor", &self.fd_readiness.is_some())
            .finish()
    }
}

impl QuicDriver {
    fn new(endpoint: Endpoint, socket: UdpSocket, is_server: bool) -> Self {
        Self {
            endpoint,
            socket: Arc::new(socket),
            conns: HashMap::new(),
            accepted: Arc::new(Mutex::new(VecDeque::new())),
            scratch: Vec::with_capacity(MAX_DATAGRAM),
            tx_buf: Vec::with_capacity(MAX_DATAGRAM),
            fd_readiness: None,
            idle: IDLE_POLL,
            is_server,
            done: false,
        }
    }

    /// WHY: a client needs a connection before it can drive one.
    ///
    /// WHAT: bind an ephemeral UDP socket, start a QUIC handshake to `addr`, and
    /// return the driver plus a handle to the (still handshaking) connection.
    ///
    /// HOW: `server_name` is what the certificate is validated against.
    ///
    /// # Errors
    /// Socket bind/connect failures, or a rejected client config.
    ///
    /// # Panics
    /// Never panics.
    pub fn connect(addr: SocketAddr, cfg: ClientConfig, server_name: &str) -> io::Result<(Self, QuinnConnection)> {
        let socket = UdpSocket::bind(if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
        socket.set_nonblocking(true)?;

        let endpoint = Endpoint::new(Arc::new(EndpointConfig::default()), None, true, None);
        let mut driver = Self::new(endpoint, socket, false);

        let now = Instant::now();
        let (handle, conn) = driver
            .endpoint
            .connect(now, cfg, addr, server_name)
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))?;

        let state = driver.insert_conn(handle, conn, addr);
        Ok((driver, QuinnConnection::new(state)))
    }

    /// WHY: F33's acceptance criterion is connect **and accept**.
    ///
    /// WHAT: bind a UDP socket and drive a server endpoint that accepts inbound
    /// QUIC connections.
    ///
    /// HOW: connections appear via [`QuicDriver::take_accepted`] once their
    /// handshake completes; the driver emits [`QuicEvent::Connected`] at the same
    /// moment.
    ///
    /// # Errors
    /// Socket bind failure.
    ///
    /// # Panics
    /// Never panics.
    pub fn server(addr: SocketAddr, cfg: ServerConfig) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        let endpoint = Endpoint::new(
            Arc::new(EndpointConfig::default()),
            Some(Arc::new(cfg)),
            true,
            None,
        );
        Ok(Self::new(endpoint, socket, true))
    }

    /// The address this driver's socket is bound to.
    ///
    /// # Errors
    /// The socket's `io::Error`.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// WHY: a server hands accepted connections to whatever will serve them.
    ///
    /// WHAT: take the next connection whose handshake has completed.
    ///
    /// HOW: the driver pushes here on `Event::Connected`. Returns `None` when none
    /// are waiting.
    ///
    /// # Panics
    /// Never panics; a poisoned queue yields `None`.
    pub fn take_accepted(&self) -> Option<QuinnConnection> {
        self.accepted.lock().ok()?.pop_front()
    }

    /// A shared handle to the accept queue, so a caller can drain it from another
    /// task while the driver runs.
    ///
    /// # Panics
    /// Never panics.
    pub fn accept_queue(&self) -> Arc<Mutex<VecDeque<QuinnConnection>>> {
        Arc::clone(&self.accepted)
    }

    /// WHY: an idle QUIC connection should cost nothing. Wrap the UDP socket's fd
    /// in `RegisteredFd`/`SharedReadiness` (Decision 14) and inject it here; idle
    /// polls then park on the reactor instead of spinning on `Delayed`.
    ///
    /// WHAT: attach a readiness signal for the driver's socket.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn with_fd_readiness(mut self, fd: EventReadinessPtr) -> Self {
        self.fd_readiness = Some(fd);
        self
    }

    /// The raw UDP socket, so the caller can register it with the reactor.
    #[must_use]
    pub fn socket(&self) -> Arc<UdpSocket> {
        Arc::clone(&self.socket)
    }

    fn insert_conn(
        &mut self,
        handle: ConnectionHandle,
        conn: quinn_proto::Connection,
        peer: SocketAddr,
    ) -> SharedConn {
        let state = Arc::new(Mutex::new(ConnState {
            handle,
            conn,
            peer,
            inbound_bidi: VecDeque::new(),
            inbound_uni: VecDeque::new(),
            closed: None,
        }));
        self.conns.insert(handle, Arc::clone(&state));
        state
    }

    /// Read one datagram, if any, and route it. Returns whether work was done.
    fn pump_recv(&mut self, now: Instant) -> bool {
        let mut buf = [0u8; MAX_DATAGRAM];
        let (n, from) = match self.socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return false,
            Err(e) => {
                tracing::debug!(error = %e, "QUIC socket recv failed");
                return false;
            }
        };

        let data = BytesMut::from(&buf[..n]);
        self.scratch.clear();
        let local_ip = self.socket.local_addr().ok().map(|a| a.ip());

        let Some(event) = self.endpoint.handle(now, from, local_ip, None, data, &mut self.scratch)
        else {
            return true;
        };

        match event {
            DatagramEvent::ConnectionEvent(handle, event) => {
                if let Some(state) = self.conns.get(&handle) {
                    if let Ok(mut guard) = state.lock() {
                        guard.conn.handle_event(event);
                    }
                }
            }
            // A stateless reply (retry / version negotiation). Send it verbatim.
            DatagramEvent::Response(transmit) => {
                let len = transmit.size.min(self.scratch.len());
                let _ = self.socket.send_to(&self.scratch[..len], transmit.destination);
            }
            DatagramEvent::NewConnection(incoming) => {
                self.scratch.clear();
                match self.endpoint.accept(incoming, now, &mut self.scratch, None) {
                    Ok((handle, conn)) => {
                        tracing::debug!(?handle, %from, "QUIC connection accepted");
                        self.insert_conn(handle, conn, from);
                    }
                    Err(err) => {
                        // The endpoint may want to send a rejection datagram.
                        if let Some(transmit) = err.response {
                            let len = transmit.size.min(self.scratch.len());
                            let _ = self.socket.send_to(&self.scratch[..len], transmit.destination);
                        }
                        tracing::debug!(error = %err.cause, "QUIC accept rejected");
                    }
                }
            }
        }
        true
    }

    /// Drain every connection's pending transmits. Returns whether work was done.
    fn pump_transmit(&mut self, now: Instant) -> bool {
        let mut worked = false;
        for state in self.conns.values() {
            let Ok(mut guard) = state.lock() else { continue };
            let peer = guard.peer;
            // Bounded: one transmit per connection per poll keeps the task fair.
            self.tx_buf.clear();
            if let Some(transmit) = guard.conn.poll_transmit(now, 1, &mut self.tx_buf) {
                let len = transmit.size.min(self.tx_buf.len());
                let dest = if transmit.destination.port() == 0 { peer } else { transmit.destination };
                let _ = self.socket.send_to(&self.tx_buf[..len], dest);
                worked = true;
            }
        }
        worked
    }

    /// Fire any connection timers that are due. Returns whether work was done.
    ///
    /// Without this, loss detection and idle timeouts never run: a lossy link
    /// stalls forever because nothing retransmits.
    fn pump_timeouts(&mut self, now: Instant) -> bool {
        let mut worked = false;
        for state in self.conns.values() {
            let Ok(mut guard) = state.lock() else { continue };
            if guard.conn.poll_timeout().is_some_and(|deadline| deadline <= now) {
                guard.conn.handle_timeout(now);
                worked = true;
            }
        }
        worked
    }

    /// Process one event from each connection. Returns an event to surface, if any.
    fn pump_events(&mut self) -> Option<QuicEvent> {
        let mut surfaced = None;
        let mut closed_handles = Vec::new();

        for (handle, state) in &self.conns {
            let Ok(mut guard) = state.lock() else { continue };

            while let Some(event) = guard.conn.poll() {
                match event {
                    quinn_proto::Event::Connected => {
                        // For a server this is the accept point.
                        if let Ok(mut queue) = self.accepted.lock() {
                            queue.push_back(QuinnConnection::new(Arc::clone(state)));
                        }
                        surfaced.get_or_insert(QuicEvent::Connected);
                    }
                    quinn_proto::Event::Stream(quinn_proto::StreamEvent::Opened { dir }) => {
                        // Drain every newly-opened stream of this direction into
                        // the accept queue; `Opened` fires once for a batch.
                        while let Some(id) = guard.conn.streams().accept(dir) {
                            guard.push_inbound(dir, id.into());
                        }
                    }
                    quinn_proto::Event::ConnectionLost { reason } => {
                        guard.closed = Some(conn_error_from(&reason));
                        closed_handles.push(*handle);
                        surfaced.get_or_insert(QuicEvent::ConnectionClosed {
                            reason: reason.to_string(),
                        });
                    }
                    // Readable/Writable/Finished/Stopped are observed by the stream
                    // handles themselves on their next call; nothing to do here.
                    _ => {}
                }
            }
        }

        for handle in closed_handles {
            self.conns.remove(&handle);
        }
        surfaced
    }
}

/// Map quinn's connection-loss reason onto our protocol vocabulary.
fn conn_error_from(reason: &quinn_proto::ConnectionError) -> QuicConnError {
    use quinn_proto::ConnectionError;
    match reason {
        ConnectionError::ApplicationClosed(close) => {
            QuicConnError::ApplicationClose { code: close.error_code.into_inner() }
        }
        ConnectionError::TimedOut => QuicConnError::Timeout,
        other => QuicConnError::Internal(other.to_string()),
    }
}

impl TaskIterator for QuicDriver {
    type Ready = QuicEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.done {
            return None;
        }

        let now = Instant::now();

        // Ordering matters: feed the state machine before asking it for output.
        let recv = self.pump_recv(now);
        let timers = self.pump_timeouts(now);
        let event = self.pump_events();
        let tx = self.pump_transmit(now);

        if let Some(event) = event {
            return Some(TaskStatus::Ready(event));
        }
        if recv || timers || tx {
            return Some(TaskStatus::Pending(()));
        }

        // A client driver with no connections left has nothing to do. A server
        // driver keeps listening for new ones.
        if self.conns.is_empty() && !self.is_server {
            self.done = true;
            return None;
        }

        match &self.fd_readiness {
            Some(fd) => Some(TaskStatus::Depends(Arc::clone(fd))),
            None => Some(TaskStatus::Delayed(self.idle)),
        }
    }
}

// ── Configuration helpers ────────────────────────────────────────────────────

/// Build a client config that trusts the given DER certificate.
///
/// # Errors
/// A rejected rustls configuration.
///
/// # Panics
/// Never panics.
pub fn client_config_trusting(cert_der: &[u8]) -> io::Result<ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(cert_der.to_vec()))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let rustls_cfg = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    quic_client_config(rustls_cfg)
}

fn quic_client_config(rustls_cfg: rustls::ClientConfig) -> io::Result<ClientConfig> {
    let crypto = quinn_proto::crypto::rustls::QuicClientConfig::try_from(rustls_cfg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let mut transport = TransportConfig::default();
    transport.max_idle_timeout(Some(quinn_proto::IdleTimeout::from(VarInt::from_u32(10_000))));

    let mut cfg = ClientConfig::new(Arc::new(crypto));
    cfg.transport_config(Arc::new(transport));
    Ok(cfg)
}

/// Build a server config from a DER certificate chain and private key.
///
/// # Errors
/// A rejected rustls configuration or key.
///
/// # Panics
/// Never panics.
pub fn server_config_from_der(
    cert_chain: Vec<Vec<u8>>,
    key_der: Vec<u8>,
) -> io::Result<ServerConfig> {
    let chain = cert_chain
        .into_iter()
        .map(rustls::pki_types::CertificateDer::from)
        .collect();
    let key = rustls::pki_types::PrivateKeyDer::try_from(key_der)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let mut cfg = ServerConfig::with_single_cert(chain, key)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let mut transport = TransportConfig::default();
    transport.max_idle_timeout(Some(quinn_proto::IdleTimeout::from(VarInt::from_u32(10_000))));
    // HTTP/3 needs both directions; allow a healthy number of concurrent streams.
    transport.max_concurrent_bidi_streams(VarInt::from_u32(100));
    transport.max_concurrent_uni_streams(VarInt::from_u32(100));
    cfg.transport_config(Arc::new(transport));

    Ok(cfg)
}

/// WHY: a QUIC server needs a certificate, and every caller — tests included —
/// otherwise reimplements PEM parsing. Glue belongs in the library, not in each
/// test.
///
/// WHAT: build a [`ServerConfig`] from PEM-encoded certificate chain and private
/// key bytes.
///
/// HOW: `rustls_pemfile` decodes the PEM into DER, then
/// [`server_config_from_der`] does the rest.
///
/// # Errors
/// Malformed PEM, an empty certificate chain, a missing private key, or a
/// rustls configuration rejection.
///
/// # Panics
/// Never panics.
pub fn server_config_from_pem(cert_pem: &[u8], key_pem: &[u8]) -> io::Result<ServerConfig> {
    let mut cert_reader = std::io::BufReader::new(cert_pem);
    let chain: Vec<Vec<u8>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
        .into_iter()
        .map(|der| der.to_vec())
        .collect();

    if chain.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no certificates found in PEM",
        ));
    }

    let mut key_reader = std::io::BufReader::new(key_pem);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no private key found in PEM"))?;

    server_config_from_der(chain, key.secret_der().to_vec())
}

/// Build a client config that trusts the PEM-encoded certificate.
///
/// # Errors
/// Malformed PEM, an empty chain, or a rustls configuration rejection.
///
/// # Panics
/// Never panics.
pub fn client_config_trusting_pem(cert_pem: &[u8]) -> io::Result<ClientConfig> {
    let mut reader = std::io::BufReader::new(cert_pem);
    let first = rustls_pemfile::certs(&mut reader)
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no certificates found in PEM"))?
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    client_config_trusting(&first)
}
