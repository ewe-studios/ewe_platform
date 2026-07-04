//! Capability matching (Decision 11 §Capability matching).
//!
//! WHY: "any protocol on any transport" becomes an enforced predicate rather than
//! scattered special cases. A protocol declares what a call needs **per (protocol
//! × stream type)**; a transport declares what it offers; the router/client check
//! `requirements ⊆ capabilities` and reject mismatches uniformly.
//!
//! WHAT: [`ProtocolKind`], [`TransportCapabilities`], [`CallRequirements`],
//! [`requirements`], and [`check_compatible`].

use foundation_netio::simple_http::shared::Proto;

use crate::context::StreamType;
use crate::error::{ConnectError, ConnectResult};

/// The RPC wire protocol family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtocolKind {
    /// The Connect protocol.
    Connect,
    /// gRPC (requires HTTP/2).
    Grpc,
    /// gRPC-Web.
    GrpcWeb,
}

/// What a transport can do (Decision 11 — the *actual* transport abilities).
#[derive(Debug, Clone)]
pub struct TransportCapabilities {
    /// Can it keep pushing request-body frames after sending has begun?
    /// (HTTP/1.1 chunked / HTTP/2/3 / WebSocket = yes; WASM Fetch = no.)
    pub request_streaming: bool,
    /// Can reader and writer run concurrently on one exchange (full duplex)?
    pub full_duplex: bool,
    /// Real HTTP/2 trailing HEADERS — the only trailer form that is a *transport*
    /// capability (Connect `Trailer-` headers / gRPC-Web `0x80` frame need nothing
    /// beyond a body).
    pub h2_trailers: bool,
    /// The HTTP versions the transport speaks.
    pub http_versions: &'static [Proto],
    /// Whether the transport multiplexes concurrent streams on one connection.
    pub multiplexed: bool,
}

/// What a specific call needs — computed per (protocol × stream type).
#[derive(Debug, Clone)]
pub struct CallRequirements {
    /// Needs to stream the request body (`ClientStream` | `BidiStream`).
    pub request_streaming: bool,
    /// Needs full duplex (`BidiStream` only).
    pub full_duplex: bool,
    /// Needs HTTP/2 trailing HEADERS (gRPC only — its status lives there).
    pub h2_trailers: bool,
    /// Minimum HTTP version (gRPC ⇒ HTTP/2; Connect / gRPC-Web ⇒ HTTP/1.1).
    pub min_http_version: Proto,
}

/// Compute the requirements for a `(protocol, stream_type)` pair.
#[must_use]
pub fn requirements(protocol: ProtocolKind, stream_type: StreamType) -> CallRequirements {
    let request_streaming = matches!(stream_type, StreamType::ClientStream | StreamType::BidiStream);
    let full_duplex = matches!(stream_type, StreamType::BidiStream);
    let h2_trailers = matches!(protocol, ProtocolKind::Grpc);
    let min_http_version = if matches!(protocol, ProtocolKind::Grpc) {
        Proto::HTTP20
    } else {
        Proto::HTTP11
    };
    CallRequirements {
        request_streaming,
        full_duplex,
        h2_trailers,
        min_http_version,
    }
}

/// `Ok` iff every required ability is offered by the transport.
///
/// # Errors
/// A [`ConnectError`]-carrying trace ([`Code::Unimplemented`](crate::Code::Unimplemented))
/// naming the missing ability. `full_duplex` mismatch corresponds to the HTTP/1.1
/// bidi `505` rejection.
pub fn check_compatible(
    req: &CallRequirements,
    cap: &TransportCapabilities,
) -> ConnectResult<()> {
    if req.request_streaming && !cap.request_streaming {
        return Err(ConnectError::unimplemented(
            "this transport cannot stream the request body (client/bidi streaming unsupported)",
        )
        .into());
    }
    if req.full_duplex && !cap.full_duplex {
        return Err(ConnectError::unimplemented(
            "this transport is not full-duplex; bidi streaming requires HTTP/2+ (505)",
        )
        .into());
    }
    if req.h2_trailers && !cap.h2_trailers {
        return Err(ConnectError::unimplemented(
            "this transport lacks HTTP/2 trailing HEADERS required by gRPC",
        )
        .into());
    }
    if !cap
        .http_versions
        .iter()
        .any(|v| *v >= req.min_http_version)
    {
        return Err(ConnectError::unimplemented(format!(
            "this transport does not offer HTTP version >= {:?} required by the protocol",
            req.min_http_version
        ))
        .into());
    }
    Ok(())
}
