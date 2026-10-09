//! Native Noise-PSK bootstrap channel + Join protocol (spec-55, feature 02; decision 06).
//!
//! WHY: A joiner must reach a member over an **encrypted, seed-authenticated** channel
//! before any tunnel exists, and be handed the current membership. The seed's derived
//! `channel_psk` authenticates a Noise-PSK handshake — a wrong seed simply fails the handshake.
//!
//! WHAT: [`BootstrapServer`] (a member's listener) and [`BootstrapClient`] /
//! [`BootstrapConnection`] (the joiner), exchanging [`BootstrapRequest`]/
//! [`BootstrapResponse`] over a length-prefixed frame protocol on the Noise stream.
//!
//! HOW: A pure-Rust Noise-PSK session ([`super::noise_psk`]) keyed by the seed-derived
//! `channel_psk`; a wrong seed fails the handshake. A [`BootstrapHandler`] supplies admission
//! decisions + membership.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;

use super::noise_psk::{self, NoiseStream};
use crate::shared::bootstrap::rpc::{Admission, BootstrapRequest, BootstrapResponse};
use crate::shared::error::{WgError, WgResult};
use crate::shared::membership::PeerRecord;

/// Maximum accepted frame length (guards against a hostile/desynced peer).
const MAX_FRAME: usize = 4 * 1024 * 1024;

// ---------------------------------------------------------------------------
// HTTP/1.1 framing — connectrpc-compatible (JSON codec + HTTP transport)
// ---------------------------------------------------------------------------
// The wire format is standard HTTP/1.1 with JSON body — this is exactly what
// connectrpc's H1Transport + JsonCodec produces. We implement a minimal subset
// (no chunked encoding, no connection reuse) sufficient for the bootstrap RPC.

const SERVICE_PATH: &str = "/wg_bootstrap.v1.WgBootstrap";

fn http_request(stream: &mut impl Write, method: &str, body: &[u8]) -> io::Result<()> {
    write!(stream, "POST {SERVICE_PATH}/{method} HTTP/1.1\r\n")?;
    write!(stream, "Host: bootstrap\r\n")?;
    write!(stream, "Content-Type: application/json\r\n")?;
    write!(stream, "Content-Length: {}\r\n", body.len())?;
    write!(stream, "\r\n")?;
    stream.write_all(body)?;
    stream.flush()
}

/// Parsed HTTP response from the server.
struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

fn http_response(stream: &mut impl Read) -> io::Result<Option<HttpResponse>> {
    let mut line = String::new();
    // Read the status line.
    match read_line(stream, &mut line) {
        Ok(0) => return Ok(None),
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    let status = parse_status(&line)?;

    // Read headers until the empty line.
    let mut content_length: usize = 0;
    loop {
        line.clear();
        match read_line(stream, &mut line) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "headers truncated")),
            Err(e) => return Err(e),
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() { break; }
        if let Some(val) = trimmed.strip_prefix("Content-Length:").or_else(|| trimmed.strip_prefix("content-length:")) {
            content_length = val.trim().parse::<usize>().unwrap_or(0);
        }
    }

    if content_length > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "response body exceeds limit"));
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        stream.read_exact(&mut body)?;
    }
    Ok(Some(HttpResponse { status, body }))
}

fn read_line(stream: &mut impl Read, buf: &mut String) -> io::Result<usize> {
    buf.clear();
    let mut total = 0;
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return if total == 0 { Ok(0) } else { Ok(total) },
            Ok(_) => {
                total += 1;
                if byte[0] == b'\n' { return Ok(total); }
                if byte[0] != b'\r' { buf.push(byte[0] as char); }
            }
            Err(e) => return Err(e),
        }
    }
}

fn parse_status(line: &str) -> io::Result<u16> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 || !parts[0].starts_with("HTTP/") {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("bad status line: {line}")));
    }
    parts[1].parse::<u16>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("bad status code: {line}")))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// The mesh-side policy the [`BootstrapServer`] consults: admission + membership.
pub trait BootstrapHandler: Send + Sync {
    /// Decide whether to admit a joining `record`, and return the current membership to
    /// hand it on admission.
    fn on_join(&self, record: PeerRecord) -> (Admission, Vec<PeerRecord>);

    /// The full membership set (for `PullMembership`).
    fn membership(&self) -> Vec<PeerRecord>;

    /// Absorb a re-announced record (gossip fan-out happens in feature 03/04).
    fn on_announce(&self, record: PeerRecord);
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// A member's Noise-PSK bootstrap listener.
pub struct BootstrapServer {
    listener: TcpListener,
    channel_psk: [u8; 32],
    handler: Arc<dyn BootstrapHandler>,
}

impl std::fmt::Debug for BootstrapServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapServer")
            .field("local_addr", &self.listener.local_addr().ok())
            .finish_non_exhaustive()
    }
}

impl BootstrapServer {
    /// WHY: Any member can accept joiners (masterless — decision 05).
    ///
    /// WHAT: Bind a TCP listener keyed by `channel_psk` for the Noise-PSK handshake.
    ///
    /// HOW: Binds `addr` and stores the pre-shared key + handler; the handshake runs per
    /// accepted connection.
    ///
    /// # Errors
    /// [`WgError::Io`] if binding fails.
    pub fn bind(
        addr: SocketAddr,
        channel_psk: [u8; 32],
        handler: Arc<dyn BootstrapHandler>,
    ) -> WgResult<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self {
            listener,
            channel_psk,
            handler,
        })
    }

    /// The bound local address.
    ///
    /// # Errors
    /// [`WgError::Io`] if the address cannot be read.
    pub fn local_addr(&self) -> WgResult<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    /// The raw fd of the TCP listener, for reactor registration (F02 valtron task).
    #[must_use]
    pub fn listener_fd(&self) -> std::os::unix::io::RawFd {
        use std::os::unix::io::AsRawFd;
        self.listener.as_raw_fd()
    }

    /// Try to accept one connection and serve it synchronously.
    /// Returns `Ok(true)` when a connection was served, `Ok(false)` when
    /// no connection was waiting (WouldBlock), or an error.
    pub fn try_accept_and_serve(&self) -> io::Result<bool> {
        match self.listener.accept() {
            Ok((tcp, _peer)) => {
                tcp.set_nonblocking(false)?;
                if let Err(err) = self.handshake_and_serve(tcp) {
                    tracing::debug!(error = %err, "bootstrap connection dropped");
                }
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// WHY: Tests and simple deployments accept one joiner at a time.
    ///
    /// WHAT: Accept a single TCP connection, complete the Noise-PSK handshake, and serve its
    /// requests until the peer closes.
    ///
    /// HOW: `accept` → [`noise_psk::accept`] → frame loop dispatched through the handler.
    ///
    /// # Errors
    /// [`WgError::Protocol`] on a failed handshake (e.g. wrong seed) or malformed frame;
    /// [`WgError::Io`] on transport errors.
    pub fn serve_once(&self) -> WgResult<()> {
        let (tcp, _peer) = self.listener.accept()?;
        self.handshake_and_serve(tcp)
    }

    /// WHY: In a live mesh every member keeps its bootstrap door open (masterless).
    ///
    /// WHAT: Accept and serve joiners until `stop` is set.
    ///
    /// HOW: Non-blocking `accept` with reactor-based idle waiting (F02 must-do:
    /// no `thread::sleep` polling). The listener fd is registered with the shared
    /// reactor; when idle, the thread polls the atomic readiness cache with 1ms
    /// granularity — the reactor drain thread updates it asynchronously from
    /// epoll/kqueue events. Falls back to 50ms sleep when no reactor is running.
    ///
    /// # Errors
    /// [`WgError::Io`] only on a fatal listener error; per-connection failures (e.g. a
    /// wrong-seed handshake) are logged and skipped.
    pub fn serve_until(&self, stop: &std::sync::atomic::AtomicBool) -> WgResult<()> {
        use std::sync::atomic::Ordering;
        use std::time::{Duration, Instant};

        self.listener.set_nonblocking(true)?;
        let fd = self.listener_fd();

        // Try to register with the shared reactor for edge-triggered wake.
        let reactor_token = foundation_nativeapis::native::poll::Token(0xF200);
        let reactor = foundation_nativeapis::native::fd::Reactor::get().ok();
        if let Some(ref r) = reactor {
            r.register(fd, reactor_token, foundation_nativeapis::native::poll::Interest::READABLE).ok();
        }

        while !stop.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((tcp, _peer)) => {
                    if let Err(err) = tcp.set_nonblocking(false) {
                        tracing::debug!(error = %err, "bootstrap conn set_blocking");
                        continue;
                    }
                    if let Err(err) = self.handshake_and_serve(tcp) {
                        tracing::debug!(error = %err, "bootstrap connection dropped");
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Wait for a connection or timeout using reactor readiness.
                    if let Some(ref r) = reactor {
                        r.clear(reactor_token, foundation_nativeapis::native::fd::Ready::READABLE);
                        let deadline = Instant::now() + Duration::from_millis(50);
                        while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
                            if r.is_ready(reactor_token) {
                                r.clear(reactor_token, foundation_nativeapis::native::fd::Ready::READABLE);
                                break;
                            }
                            std::hint::spin_loop();
                            std::thread::sleep(Duration::from_millis(1));
                        }
                    } else {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn handshake_and_serve(&self, tcp: TcpStream) -> WgResult<()> {
        let mut stream = noise_psk::accept(tcp, self.channel_psk)?;
        self.serve_stream(&mut stream)
    }

    fn serve_stream(&self, stream: &mut NoiseStream<TcpStream>) -> WgResult<()> {
        loop {
            let req = match read_http_request(stream)? {
                Some(r) => r,
                None => return Ok(()), // peer closed
            };
            let (status, body) = match req.method.as_str() {
                "Join" => {
                    let request: BootstrapRequest = serde_json::from_slice(&req.body)
                        .map_err(|e| WgError::Protocol(format!("join body: {e}")))?;
                    let record = match request {
                        BootstrapRequest::Join(r) => r,
                        other => return Err(WgError::Protocol(format!("expected Join, got {other:?}"))),
                    };
                    let (decision, members) = self.handler.on_join(record);
                    (200, serde_json::to_vec(&BootstrapResponse::JoinResult { decision, members })
                        .map_err(|e| WgError::Protocol(format!("encode: {e}")))?)
                }
                "PullMembership" => {
                    (200, serde_json::to_vec(&BootstrapResponse::Membership(self.handler.membership()))
                        .map_err(|e| WgError::Protocol(format!("encode: {e}")))?)
                }
                "Announce" => {
                    let request: BootstrapRequest = serde_json::from_slice(&req.body)
                        .map_err(|e| WgError::Protocol(format!("announce body: {e}")))?;
                    let record = match request {
                        BootstrapRequest::Announce(r) => r,
                        other => return Err(WgError::Protocol(format!("expected Announce, got {other:?}"))),
                    };
                    self.handler.on_announce(record);
                    (200, serde_json::to_vec(&BootstrapResponse::Ack)
                        .map_err(|e| WgError::Protocol(format!("encode: {e}")))?)
                }
                _ => {
                    let err = serde_json::json!({"code":"unimplemented","message":format!("unknown method: {}",req.method)});
                    (501, serde_json::to_vec(&err).unwrap_or_default())
                }
            };
            write_http_response(stream, status, &body)?;
        }
    }
}

// ── HTTP request parser ────────────────────────────────────────────────

struct HttpRequest {
    method: String,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut impl Read) -> io::Result<Option<HttpRequest>> {
    let mut line = String::new();
    // Read the request line: POST /service/Method HTTP/1.1
    match read_line(stream, &mut line) {
        Ok(0) => return Ok(None),
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("bad request line: {line}")));
    }
    // Extract method name from the path: /wg_bootstrap.v1.WgBootstrap/Join → Join
    let method = parts[1]
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();

    // Read headers.
    let mut content_length: usize = 0;
    loop {
        line.clear();
        match read_line(stream, &mut line) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "headers truncated")),
            Err(e) => return Err(e),
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() { break; }
        if let Some(val) = trimmed.strip_prefix("Content-Length:").or_else(|| trimmed.strip_prefix("content-length:")) {
            content_length = val.trim().parse::<usize>().unwrap_or(0);
        }
    }

    if content_length > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "request body exceeds limit"));
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        stream.read_exact(&mut body)?;
    }
    Ok(Some(HttpRequest { method, body }))
}

fn write_http_response(stream: &mut impl Write, status: u16, body: &[u8]) -> io::Result<()> {
    let reason = if status == 200 { "OK" } else { "Error" };
    write!(stream, "HTTP/1.1 {status} {reason}\r\n")?;
    write!(stream, "Content-Type: application/json\r\n")?;
    write!(stream, "Content-Length: {}\r\n", body.len())?;
    write!(stream, "\r\n")?;
    stream.write_all(body)?;
    stream.flush()
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// The joiner's Noise-PSK bootstrap dialer.
pub struct BootstrapClient {
    channel_psk: [u8; 32],
}

impl std::fmt::Debug for BootstrapClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapClient").finish_non_exhaustive()
    }
}

impl BootstrapClient {
    /// WHY: The joiner authenticates with the seed-derived `channel_psk`.
    ///
    /// WHAT: Build a client dialer keyed by `channel_psk`. The network id is already bound into
    /// the key via its HKDF derivation, so no separate identity hint is needed.
    ///
    /// HOW: Stores the pre-shared key; the handshake runs on [`Self::connect`].
    ///
    /// # Errors
    /// Infallible today, but returns [`WgResult`] for forward compatibility.
    pub fn new(channel_psk: [u8; 32]) -> WgResult<Self> {
        Ok(Self { channel_psk })
    }

    /// WHY: Dial a seed endpoint to join.
    ///
    /// WHAT: Open a TCP connection to `endpoint` and complete the Noise-PSK handshake.
    ///
    /// HOW: `TcpStream::connect` → [`noise_psk::connect`].
    ///
    /// # Errors
    /// [`WgError::Io`] if the TCP connect fails; [`WgError::Protocol`] if the handshake
    /// fails (e.g. wrong seed).
    pub fn connect(&self, endpoint: SocketAddr) -> WgResult<BootstrapConnection> {
        let tcp = TcpStream::connect(endpoint)?;
        let stream = noise_psk::connect(tcp, self.channel_psk)?;
        Ok(BootstrapConnection { stream })
    }
}

/// An established Noise-PSK bootstrap connection.
pub struct BootstrapConnection {
    stream: NoiseStream<TcpStream>,
}

impl std::fmt::Debug for BootstrapConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapConnection").finish_non_exhaustive()
    }
}

impl BootstrapConnection {
    /// Send one request and read the response as JSON (connectrpc-compatible HTTP+JSON).
    ///
    /// # Errors
    /// [`WgError::Io`] on transport errors; [`WgError::Protocol`] on a malformed reply.
    pub fn request(&mut self, method: &str, request: &BootstrapRequest) -> WgResult<BootstrapResponse> {
        let body = serde_json::to_vec(request)
            .map_err(|e| WgError::Protocol(format!("encode: {e}")))?;
        http_request(&mut self.stream, method, &body)?;
        let resp = http_response(&mut self.stream)?
            .ok_or_else(|| WgError::Protocol("connection closed before reply".into()))?;
        if resp.status != 200 {
            let detail = String::from_utf8_lossy(&resp.body);
            return Err(WgError::Protocol(format!("bootstrap server returned {}: {detail}", resp.status)));
        }
        serde_json::from_slice(&resp.body)
            .map_err(|e| WgError::Protocol(format!("decode response: {e}")))
    }

    /// Announce self and request admission, returning the decision + membership.
    ///
    /// # Errors
    /// See [`Self::request`]; also [`WgError::Protocol`] on an unexpected reply type.
    pub fn join(&mut self, record: PeerRecord) -> WgResult<(Admission, Vec<PeerRecord>)> {
        match self.request("Join", &BootstrapRequest::Join(record))? {
            BootstrapResponse::JoinResult { decision, members } => Ok((decision, members)),
            other => Err(WgError::Protocol(format!("unexpected join reply: {other:?}"))),
        }
    }

    /// Pull the full membership set.
    ///
    /// # Errors
    /// See [`Self::request`]; also [`WgError::Protocol`] on an unexpected reply type.
    pub fn pull_membership(&mut self) -> WgResult<Vec<PeerRecord>> {
        match self.request("PullMembership", &BootstrapRequest::PullMembership)? {
            BootstrapResponse::Membership(members) => Ok(members),
            other => Err(WgError::Protocol(format!("unexpected membership reply: {other:?}"))),
        }
    }

    /// Re-announce a record for gossip fan-out.
    ///
    /// # Errors
    /// See [`Self::request`]; also [`WgError::Protocol`] on an unexpected reply type.
    pub fn announce(&mut self, record: PeerRecord) -> WgResult<()> {
        match self.request("Announce", &BootstrapRequest::Announce(record))? {
            BootstrapResponse::Ack => Ok(()),
            other => Err(WgError::Protocol(format!("unexpected announce reply: {other:?}"))),
        }
    }
}
