//! Native TLS-PSK bootstrap channel + Join protocol (spec-55, feature 02; decision 06).
//!
//! WHY: A joiner must reach a member over an **encrypted, seed-authenticated** channel
//! before any tunnel exists, and be handed the current membership. The seed's derived
//! `tls_psk` authenticates a TLS-PSK handshake — a wrong seed simply fails the handshake.
//!
//! WHAT: [`BootstrapServer`] (a member's listener) and [`BootstrapClient`] /
//! [`BootstrapConnection`] (the joiner), exchanging [`BootstrapRequest`]/
//! [`BootstrapResponse`] over a length-prefixed frame protocol on the TLS stream.
//!
//! HOW: `boring` (BoringSSL) TLS 1.2 with a PSK cipher suite and PSK client/server
//! callbacks keyed by `tls_psk`; the PSK identity hint is the network id. A
//! [`BootstrapHandler`] supplies admission decisions + membership.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;

use boring::ssl::{Ssl, SslContext, SslContextBuilder, SslMethod, SslStream, SslVersion};

use crate::shared::bootstrap::rpc::{Admission, BootstrapRequest, BootstrapResponse};
use crate::shared::error::{WgError, WgResult};
use crate::shared::keys::NetworkId;
use crate::shared::membership::PeerRecord;

/// PSK cipher suites (TLS 1.2). BoringSSL only ships the CBC-SHA PSK suites (no GCM PSK),
/// so we negotiate one of these when both sides present the PSK callbacks.
const PSK_CIPHERS: &str = "PSK-AES256-CBC-SHA:PSK-AES128-CBC-SHA";

/// Maximum accepted frame length (guards against a hostile/desynced peer).
const MAX_FRAME: usize = 4 * 1024 * 1024;

// ---------------------------------------------------------------------------
// TLS-PSK contexts
// ---------------------------------------------------------------------------

fn base_builder() -> WgResult<SslContextBuilder> {
    let mut builder =
        SslContextBuilder::new(SslMethod::tls()).map_err(|e| tls_err("context", &e))?;
    builder
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .map_err(|e| tls_err("min version", &e))?;
    builder
        .set_max_proto_version(Some(SslVersion::TLS1_2))
        .map_err(|e| tls_err("max version", &e))?;
    builder
        .set_cipher_list(PSK_CIPHERS)
        .map_err(|e| tls_err("cipher list", &e))?;
    Ok(builder)
}

/// Build the server-side TLS-PSK context keyed by `tls_psk`.
fn server_context(tls_psk: [u8; 32]) -> WgResult<SslContext> {
    let mut builder = base_builder()?;
    builder.set_psk_server_callback(move |_ssl, _identity, psk_out| {
        let n = tls_psk.len().min(psk_out.len());
        psk_out[..n].copy_from_slice(&tls_psk[..n]);
        Ok(n)
    });
    Ok(builder.build())
}

/// Build the client-side TLS-PSK context keyed by `tls_psk`, advertising `network_id` as
/// the PSK identity hint.
fn client_context(tls_psk: [u8; 32], network_id: NetworkId) -> WgResult<SslContext> {
    let identity = network_id.to_hex().into_bytes();
    let mut builder = base_builder()?;
    builder.set_psk_client_callback(move |_ssl, _hint, identity_out, psk_out| {
        // Null-terminated identity string.
        if identity.len() + 1 > identity_out.len() {
            return Ok(0);
        }
        identity_out[..identity.len()].copy_from_slice(&identity);
        identity_out[identity.len()] = 0;
        let n = tls_psk.len().min(psk_out.len());
        psk_out[..n].copy_from_slice(&tls_psk[..n]);
        Ok(n)
    });
    Ok(builder.build())
}

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

/// A member's TLS-PSK bootstrap listener.
pub struct BootstrapServer {
    listener: TcpListener,
    context: SslContext,
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
    /// WHAT: Bind a TCP listener and build a TLS-PSK server context keyed by `tls_psk`.
    ///
    /// HOW: Binds `addr`, builds the BoringSSL PSK context, stores the handler.
    ///
    /// # Errors
    /// [`WgError::Io`] if binding fails; [`WgError::Protocol`] on TLS context errors.
    pub fn bind(
        addr: SocketAddr,
        tls_psk: [u8; 32],
        handler: Arc<dyn BootstrapHandler>,
    ) -> WgResult<Self> {
        let listener = TcpListener::bind(addr)?;
        let context = server_context(tls_psk)?;
        Ok(Self {
            listener,
            context,
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

    /// WHY: Tests and simple deployments accept one joiner at a time.
    ///
    /// WHAT: Accept a single TCP connection, complete the TLS-PSK handshake, and serve its
    /// requests until the peer closes.
    ///
    /// HOW: `accept` → BoringSSL `accept` → frame loop dispatched through the handler.
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
    /// HOW: Non-blocking `accept` with a short sleep when idle; each accepted connection
    /// is handshaken and served (sequentially) to completion.
    ///
    /// # Errors
    /// [`WgError::Io`] only on a fatal listener error; per-connection failures (e.g. a
    /// wrong-seed handshake) are logged and skipped.
    pub fn serve_until(&self, stop: &std::sync::atomic::AtomicBool) -> WgResult<()> {
        use std::sync::atomic::Ordering;
        self.listener.set_nonblocking(true)?;
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
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn handshake_and_serve(&self, tcp: TcpStream) -> WgResult<()> {
        let ssl = Ssl::new(&self.context).map_err(|e| tls_err("ssl", &e))?;
        let mut stream = SslStream::new(ssl, tcp).map_err(|e| tls_err("ssl stream", &e))?;
        stream
            .accept()
            .map_err(|e| WgError::Protocol(format!("tls-psk handshake failed: {e}")))?;
        self.serve_stream(&mut stream)
    }

    fn serve_stream(&self, stream: &mut SslStream<TcpStream>) -> WgResult<()> {
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

/// The joiner's TLS-PSK bootstrap dialer.
pub struct BootstrapClient {
    context: SslContext,
}

impl std::fmt::Debug for BootstrapClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapClient").finish_non_exhaustive()
    }
}

impl BootstrapClient {
    /// WHY: The joiner authenticates with the seed-derived `tls_psk`.
    ///
    /// WHAT: Build a client dialer keyed by `tls_psk` for `network_id`.
    ///
    /// HOW: Builds the BoringSSL PSK client context.
    ///
    /// # Errors
    /// [`WgError::Protocol`] on TLS context errors.
    pub fn new(tls_psk: [u8; 32], network_id: NetworkId) -> WgResult<Self> {
        Ok(Self {
            context: client_context(tls_psk, network_id)?,
        })
    }

    /// WHY: Dial a seed endpoint to join.
    ///
    /// WHAT: Open a TCP connection to `endpoint` and complete the TLS-PSK handshake.
    ///
    /// HOW: `TcpStream::connect` → BoringSSL `connect`.
    ///
    /// # Errors
    /// [`WgError::Io`] if the TCP connect fails; [`WgError::Protocol`] if the handshake
    /// fails (e.g. wrong seed).
    pub fn connect(&self, endpoint: SocketAddr) -> WgResult<BootstrapConnection> {
        let tcp = TcpStream::connect(endpoint)?;
        let ssl = Ssl::new(&self.context).map_err(|e| tls_err("ssl", &e))?;
        let mut stream = SslStream::new(ssl, tcp).map_err(|e| tls_err("ssl stream", &e))?;
        stream
            .connect()
            .map_err(|e| WgError::Protocol(format!("tls-psk handshake failed: {e}")))?;
        Ok(BootstrapConnection { stream })
    }
}

/// An established TLS-PSK bootstrap connection.
pub struct BootstrapConnection {
    stream: SslStream<TcpStream>,
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

fn tls_err(what: &str, err: &boring::error::ErrorStack) -> WgError {
    WgError::Protocol(format!("tls {what}: {err}"))
}
