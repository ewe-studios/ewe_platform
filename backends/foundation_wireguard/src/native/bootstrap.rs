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

use crate::shared::bootstrap::rpc::{
    decode_request, decode_response, encode_request, encode_response, Admission, BootstrapRequest,
    BootstrapResponse,
};
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
// Frame protocol
// ---------------------------------------------------------------------------

fn write_frame(stream: &mut impl Write, data: &[u8]) -> io::Result<()> {
    let len = u32::try_from(data.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too large"))?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(data)?;
    stream.flush()
}

fn read_frame(stream: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame exceeds limit"));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    Ok(Some(buf))
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
        let ssl = Ssl::new(&self.context).map_err(|e| tls_err("ssl", &e))?;
        let mut stream =
            SslStream::new(ssl, tcp).map_err(|e| tls_err("ssl stream", &e))?;
        stream
            .accept()
            .map_err(|e| WgError::Protocol(format!("tls-psk handshake failed: {e}")))?;
        self.serve_stream(&mut stream)
    }

    fn serve_stream(&self, stream: &mut SslStream<TcpStream>) -> WgResult<()> {
        while let Some(frame) = read_frame(stream)? {
            let request = decode_request(&frame)?;
            let response = match request {
                BootstrapRequest::Join(record) => {
                    let (decision, members) = self.handler.on_join(record);
                    BootstrapResponse::JoinResult { decision, members }
                }
                BootstrapRequest::PullMembership => {
                    BootstrapResponse::Membership(self.handler.membership())
                }
                BootstrapRequest::Announce(record) => {
                    self.handler.on_announce(record);
                    BootstrapResponse::Ack
                }
            };
            write_frame(stream, &encode_response(&response))?;
        }
        Ok(())
    }
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
    /// Send one request and read the response.
    ///
    /// # Errors
    /// [`WgError::Io`] on transport errors; [`WgError::Protocol`] on a malformed reply.
    pub fn request(&mut self, request: &BootstrapRequest) -> WgResult<BootstrapResponse> {
        write_frame(&mut self.stream, &encode_request(request))?;
        let frame = read_frame(&mut self.stream)?
            .ok_or_else(|| WgError::Protocol("connection closed before reply".into()))?;
        decode_response(&frame)
    }

    /// Announce self and request admission, returning the decision + membership.
    ///
    /// # Errors
    /// See [`Self::request`]; also [`WgError::Protocol`] on an unexpected reply type.
    pub fn join(&mut self, record: PeerRecord) -> WgResult<(Admission, Vec<PeerRecord>)> {
        match self.request(&BootstrapRequest::Join(record))? {
            BootstrapResponse::JoinResult { decision, members } => Ok((decision, members)),
            other => Err(WgError::Protocol(format!("unexpected join reply: {other:?}"))),
        }
    }

    /// Pull the full membership set.
    ///
    /// # Errors
    /// See [`Self::request`]; also [`WgError::Protocol`] on an unexpected reply type.
    pub fn pull_membership(&mut self) -> WgResult<Vec<PeerRecord>> {
        match self.request(&BootstrapRequest::PullMembership)? {
            BootstrapResponse::Membership(members) => Ok(members),
            other => Err(WgError::Protocol(format!("unexpected membership reply: {other:?}"))),
        }
    }

    /// Re-announce a record for gossip fan-out.
    ///
    /// # Errors
    /// See [`Self::request`]; also [`WgError::Protocol`] on an unexpected reply type.
    pub fn announce(&mut self, record: PeerRecord) -> WgResult<()> {
        match self.request(&BootstrapRequest::Announce(record))? {
            BootstrapResponse::Ack => Ok(()),
            other => Err(WgError::Protocol(format!("unexpected announce reply: {other:?}"))),
        }
    }
}

fn tls_err(what: &str, err: &boring::error::ErrorStack) -> WgError {
    WgError::Protocol(format!("tls {what}: {err}"))
}
