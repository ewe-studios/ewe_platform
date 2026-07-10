//! Connection-scoped context carried once per accepted connection.
//!
//! WHY: RPC and HTTP handlers need typed access to connection-level facts —
//! who the peer is (network address + cryptographic identity), whether the
//! transport is TLS/mTLS and with which certificate, what protocol was
//! negotiated (ALPN), and the QUIC/HTTP-3 specifics (0-RTT early data, the
//! connection id). These are transport-scoped: built once where the connection
//! is accepted/negotiated and shared across every request multiplexed on it
//! (the per-request HTTP/2 or HTTP/3 stream id stays request-scoped elsewhere).
//! Defining the type here in `netcap` (the connection layer) keeps the
//! dependency direction correct — `simple_http` builds on `netcap`, never the
//! reverse — and lets the front ends populate it at accept/handshake time
//! (Decision 12 §13 / Decision 04 Q13 of spec 41-connectrpc).
//!
//! WHAT: [`ConnectionContext`], plus the [`PeerIdentity`] and [`TlsInfo`]
//! sub-types it carries. Every field has an empty/`None` default so a
//! [`ConnectionContext::default`] is a valid "no connection metadata known"
//! value — the additive default for callers that construct requests directly
//! (tests, wasm client rendering) with no behavior change.
//!
//! HOW: A plain owned struct wrapped in `Arc` by the request that carries it
//! (see `SimpleIncomingRequest.connection`). Fields use only cross-target types
//! so the context compiles on wasm; the network peer address (a native-only
//! `netcap` socket type) is the sole target-gated field.

#[cfg(not(target_family = "wasm"))]
use crate::native::connection::SocketAddr;

/// Cryptographic identity of the connection peer.
///
/// WHY: Some transports authenticate the peer with a public key rather than (or
/// in addition to) a network address — notably iroh, which addresses nodes by
/// their Ed25519 public key (spec 41 T10). Carrying identity as a typed enum
/// lets handlers match on the authentication that actually happened instead of
/// guessing from side channels.
///
/// WHAT: `Anonymous` (no cryptographic identity — plain TCP, or TLS without a
/// verified peer key) or `Ed25519` holding the 32-byte compressed public key.
///
/// HOW: Populated by the transport that established the identity; defaults to
/// `Anonymous`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PeerIdentity {
    /// No cryptographic peer identity was established.
    #[default]
    Anonymous,
    /// iroh / raw Ed25519 public key (32-byte compressed form).
    Ed25519(Vec<u8>),
}

impl core::fmt::Display for PeerIdentity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Anonymous => write!(f, "anonymous"),
            Self::Ed25519(key) => write!(f, "ed25519:{} bytes", key.len()),
        }
    }
}

/// TLS/mTLS negotiation details for a connection.
///
/// WHY: mTLS handlers need the client certificate chain to authorize the caller,
/// and both sides may want the negotiated server name (SNI) for routing/logging.
///
/// WHAT: The peer certificate chain in DER (leaf first; empty when the peer
/// presented no certificate) and the negotiated server name.
///
/// HOW: Built by the TLS front end after a successful handshake; the connection
/// carries `Some(TlsInfo)` only for TLS connections (`None` on cleartext).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlsInfo {
    /// Peer (client) certificate chain in DER, leaf first. Empty = none presented.
    pub peer_certificates: Vec<Vec<u8>>,
    /// Negotiated server name (SNI), if the peer supplied one.
    pub server_name: Option<String>,
}

impl core::fmt::Display for TlsInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "tls(certs={}, sni={})",
            self.peer_certificates.len(),
            self.server_name.as_deref().unwrap_or("-")
        )
    }
}

/// Connection-scoped metadata carried once per accepted connection.
///
/// WHY: See the module docs — handlers reach `connection.peer_addr`,
/// `connection.tls`, `connection.alpn`, … for per-connection facts that are
/// shared across every request multiplexed on the connection.
///
/// WHAT: The peer's network address and cryptographic identity, optional TLS
/// details, the negotiated ALPN protocol id, and the QUIC/HTTP-3 0-RTT and
/// connection-id fields.
///
/// HOW: Constructed empty by [`ConnectionContext::default`] and filled in by the
/// connection front end at accept/handshake time (HTTP/1.1 at accept, HTTP/2 and
/// HTTP/3 at connection setup, WebSocket at upgrade). Wrapped in `Arc` by the
/// request that carries it so cloning the request is a refcount bump.
#[derive(Clone, Debug, Default)]
pub struct ConnectionContext {
    /// Remote peer network address. Native transports only (wasm has no accept
    /// path); `None` when the address is unknown (e.g. directly-built requests).
    #[cfg(not(target_family = "wasm"))]
    pub peer_addr: Option<SocketAddr>,
    /// Peer cryptographic identity (iroh Ed25519 key / verified key), when known.
    pub peer_identity: PeerIdentity,
    /// TLS/mTLS negotiation details; `None` on cleartext connections.
    pub tls: Option<TlsInfo>,
    /// Negotiated ALPN protocol id (e.g. `b"h2"`, `b"http/1.1"`, `b"h3"`), if any.
    pub alpn: Option<Vec<u8>>,
    /// QUIC/HTTP-3 0-RTT early-data flag: the request arrived in early data.
    pub early_data: bool,
    /// QUIC connection id (HTTP/3 transports), when applicable.
    pub quic_connection_id: Option<Vec<u8>>,
}

impl ConnectionContext {
    /// Creates an empty connection context — no metadata known.
    ///
    /// The additive default used by callers that construct requests directly
    /// (tests, wasm client rendering); equivalent to [`Default::default`].
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns the negotiated ALPN protocol id as a UTF-8 string, if present and
    /// valid UTF-8 (ALPN ids are ASCII tokens in practice).
    #[must_use]
    pub fn alpn_str(&self) -> Option<&str> {
        self.alpn.as_deref().and_then(|b| core::str::from_utf8(b).ok())
    }
}

impl core::fmt::Display for ConnectionContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ConnectionContext(peer_identity={}", self.peer_identity)?;
        #[cfg(not(target_family = "wasm"))]
        if let Some(addr) = &self.peer_addr {
            write!(f, ", peer_addr={addr:?}")?;
        }
        if let Some(tls) = &self.tls {
            write!(f, ", {tls}")?;
        }
        if let Some(alpn) = self.alpn_str() {
            write!(f, ", alpn={alpn}")?;
        }
        if self.early_data {
            write!(f, ", early_data")?;
        }
        write!(f, ")")
    }
}
