//! quinn-proto 0.11 driver — valtron TaskIterator (F33). No loops.
//! One I/O op per poll.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};
use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use quinn_proto::{
    ClientConfig, ConnectionHandle, DatagramEvent, Endpoint, EndpointConfig, TransportConfig,
    VarInt,
};
use quinn_proto::crypto::rustls::QuicClientConfig;

// ── Event ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum QuicEvent {
    Connected,
    StreamOpened { id: u64 },
    StreamData { id: u64, data: Bytes, fin: bool },
    ConnectionClosed { reason: String },
}

// ── Phase ────────────────────────────────────────────────────────────────────

enum Phase { Connecting, Open, Closed }

// ── rustls config ────────────────────────────────────────────────────────────

fn quic_client_config() -> io::Result<ClientConfig> {
    let rustls_cfg = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipCertVerification))
        .with_no_client_auth();
    let quic_crypto = QuicClientConfig::try_from(rustls_cfg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("{e}")))?;
    let mut transport = TransportConfig::default();
    transport.max_idle_timeout(Some(quinn_proto::IdleTimeout::from(VarInt::from_u32(10_000))));
    let mut cfg = ClientConfig::new(Arc::new(quic_crypto));
    cfg.transport_config(Arc::new(transport));
    Ok(cfg)
}

#[derive(Debug)]
struct SkipCertVerification;

impl rustls::client::danger::ServerCertVerifier for SkipCertVerification {
    fn verify_server_cert(
        &self, _: &rustls::pki_types::CertificateDer<'_>, _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![rustls::SignatureScheme::RSA_PKCS1_SHA256,
             rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
             rustls::SignatureScheme::ED25519]
    }
}

// ── QuicDriver ───────────────────────────────────────────────────────────────

/// One poll = one transmit drain OR one read OR one event. Parks via
/// `Depends(fd_readiness)` on idle when a readiness signal was injected
/// by the caller (F38 pattern); falls back to `Delayed` otherwise.
pub struct QuicDriver {
    phase: Phase,
    endpoint: Endpoint,
    conn: Option<(ConnectionHandle, quinn_proto::Connection)>,
    socket: UdpSocket,
    peer: SocketAddr,
    scratch: Vec<u8>,
    tx_buf: Vec<u8>,
    inbound: Vec<QuicEvent>,
    /// Caller-injected fd readiness signal (F38). When `Some`, the idle
    /// branch returns `TaskStatus::Depends(fd)` — parking on the reactor
    /// instead of polling via `Delayed`.
    fd_readiness: Option<Arc<dyn foundation_core::valtron::EventReadiness + Send + Sync>>,
    idle: Duration,
}

impl QuicDriver {
    pub fn connect(addr: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        socket.connect(addr)?;
        let mut ep = Endpoint::new(
            Arc::new(EndpointConfig::default()),
            None, // server_config: None (client-only)
            true,  // allow_mtud
            None,  // rng_seed
        );
        let cfg = quic_client_config()?;
        let now = Instant::now();
        let (ch, conn) = ep.connect(now, cfg, addr, &addr.ip().to_string())
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, format!("{e}")))?;
        Ok(Self {
            phase: Phase::Connecting, endpoint: ep, conn: Some((ch, conn)),
            socket, peer: addr, scratch: Vec::with_capacity(1500),
            tx_buf: Vec::with_capacity(1500), inbound: Vec::new(),
            fd_readiness: None, idle: Duration::from_millis(1),
        })
    }

    /// Inject an fd readiness signal. The caller wraps the UDP socket fd in
    /// `SharedReadiness` (F40) or `TimerReadiness` (F38 fallback) and passes
    /// it here. On idle, the task returns `TaskStatus::Depends(fd)` — parking
    /// on the reactor — instead of polling with `Delayed`.
    pub fn with_fd_readiness(mut self, fd: Arc<dyn foundation_core::valtron::EventReadiness + Send + Sync>) -> Self {
        self.fd_readiness = Some(fd);
        self
    }
}

impl TaskIterator for QuicDriver {
    type Ready = QuicEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let (ref mut ch, ref mut conn) = self.conn.as_mut()?;
        let now = Instant::now();

        // 1. Drain ONE transmit from connection.
        self.tx_buf.clear();
        match conn.poll_transmit(now, 1, &mut self.tx_buf) {
            Some(t) => {
                let _ = self.socket.send(&self.tx_buf[..t.size.min(self.tx_buf.len())]);
                return Some(TaskStatus::Pending(()));
            }
            None => {}
        }

        // 2. Read ONE UDP datagram → feed to endpoint.
        let mut buf = [0u8; 65535];
        match self.socket.recv(&mut buf) {
            Ok(n) => {
                let data = BytesMut::from(&buf[..n]);
                self.scratch.clear();
                match self.endpoint.handle(now, self.peer, None, None, data, &mut self.scratch) {
                    Some(DatagramEvent::ConnectionEvent(ev_ch, event)) => {
                        if ev_ch.0 == ch.0 {
                            conn.handle_event(event);
                        }
                    }
                    Some(DatagramEvent::Response(t)) => {
                        let _ = self.socket.send(
                            &self.scratch[..t.size.min(self.scratch.len())]
                        );
                    }
                    _ => {}
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => {
                self.phase = Phase::Closed; self.conn = None; return None;
            }
        }

        // 3. Process ONE connection event.
        match conn.poll() {
            Some(quinn_proto::Event::Connected) => {
                self.phase = Phase::Open;
                return Some(TaskStatus::Ready(QuicEvent::Connected));
            }
            Some(quinn_proto::Event::Stream(quinn_proto::StreamEvent::Readable { id })) => {
                let data = {
                    let mut stream = conn.recv_stream(id);
                    let chunks_result = stream.read(true);
                    match chunks_result {
                        Ok(mut chunks) => {
                            let mut buf = Vec::new();
                            while let Ok(Some(chunk)) = chunks.next(usize::MAX) {
                                buf.extend_from_slice(&chunk.bytes);
                            }
                            if !buf.is_empty() { Some(Bytes::from(buf)) } else { None }
                        }
                        Err(_) => None,
                    }
                };
                if let Some(bytes) = data {
                    return Some(TaskStatus::Ready(QuicEvent::StreamData {
                        id: id.index(), data: bytes, fin: false,
                    }));
                }
            }
            Some(quinn_proto::Event::ConnectionLost { reason }) => {
                self.phase = Phase::Closed; self.conn = None;
                return Some(TaskStatus::Ready(QuicEvent::ConnectionClosed {
                    reason: format!("{reason}"),
                }));
            }
            _ => {}
        }

        // 4. Deliver queued events, or park on fd readiness.
        if let Some(e) = self.inbound.pop() {
            return Some(TaskStatus::Ready(e));
        }
        if matches!(self.phase, Phase::Closed) {
            self.conn = None;
            return None;
        }
        match &self.fd_readiness {
            Some(fd) => Some(TaskStatus::Depends(Arc::clone(fd))),
            None => Some(TaskStatus::Delayed(self.idle)),
        }
    }
}
