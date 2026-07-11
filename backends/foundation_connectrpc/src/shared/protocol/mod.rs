//! Protocol handlers & clients (Decision 05).
//!
//! WHY: Each wire protocol (Connect, gRPC, gRPC-Web) implements the same
//! internal abstraction so the router/client treat them uniformly. A protocol
//! owns **wire framing** (enveloping + compression) via a per-call reader/writer
//! task pair that bridges the transport's byte pipes and the seam's `FramePipe`s
//! (Decision 11 §who-owns-enveloping) — the conn/facade layers never touch the
//! wire.
//!
//! WHAT: the [`ProtocolHandler`]/[`ProtocolClient`] traits, [`HandlerExchange`]/
//! [`ClientExchange`] (conn + reader/writer tasks), the [`BoxedTask`] alias, and
//! content-type canonicalization (Decision 05 P6). The Connect protocol lives in
//! [`connect`].
//!
//! HOW: reader/writer tasks are `async` futures the framework spawns on valtron
//! (via `from_future`); `BoxedTask` is their boxed form.

pub mod connect;
pub mod grpc;
pub mod grpc_web;

use std::time::Duration;

use bytes::Bytes;
use foundation_netio::shared::http::{
    SimpleHeaders, SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse,
};

use crate::shared::compression::CompressionRegistry;
use crate::shared::context::{CancelSignal, Spec, StreamType};
use crate::shared::error::ConnectResult;
use crate::shared::transport::{
    BoxFuture, ByteSink, ByteSource, ClientConn, HandlerConn, ProtocolKind, TransportStream,
};

/// A per-call reader/writer task — an async loop the framework spawns on valtron.
pub type BoxedTask = BoxFuture<'static, ConnectResult<()>>;

/// A unary handler result reduced to the encoded response-message bytes plus the
/// headers/trailers the handler set (Decision 08 §Type-Erased Handler Traits).
/// The protocol frames this into the HTTP response per its wire format.
pub struct UnaryOutcome {
    /// Encoded response-message bytes (pre-compression, pre-envelope).
    pub frame: Bytes,
    /// Response headers the handler set.
    pub headers: SimpleHeaders,
    /// Trailing metadata the handler set.
    pub trailers: SimpleHeaders,
}

/// What a protocol constructs per call on the **server** side
/// (Decision 05 / Decision 11).
pub struct HandlerExchange {
    /// The seam the handler machinery uses.
    pub conn: Box<dyn HandlerConn>,
    /// bytes → de-envelope/decompress → request `FramePipe`.
    pub reader_task: BoxedTask,
    /// response `FramePipe` → envelope/compress → bytes (flush per frame).
    pub writer_task: BoxedTask,
}

/// The client mirror of [`HandlerExchange`].
pub struct ClientExchange {
    /// The seam the client machinery uses.
    pub conn: Box<dyn ClientConn>,
    /// response bytes → de-envelope/decompress → response `FramePipe`.
    pub reader_task: BoxedTask,
    /// request `FramePipe` → envelope/compress → request bytes.
    pub writer_task: BoxedTask,
}

/// Internal protocol abstraction — one impl per protocol handles its HTTP
/// semantics (Decision 05).
pub trait ProtocolHandler: Send + Sync {
    /// The protocol family (for capability matching, Decision 11).
    fn kind(&self) -> ProtocolKind;
    /// HTTP methods this protocol accepts.
    fn allowed_methods(&self) -> &[SimpleMethod];
    /// Content-Types this handler can process.
    fn content_types(&self) -> Vec<String>;
    /// Parse the call timeout from request headers.
    fn parse_timeout(&self, headers: &SimpleHeaders) -> Option<Duration>;
    /// Can this handler process the given request?
    fn can_handle(&self, request: &SimpleIncomingRequest) -> bool;
    /// The negotiated codec **wire name** for this request (from `Content-Type`,
    /// or the GET `encoding` query for Connect) — the dispatcher resolves the
    /// typed pair from the procedure's [`ProcedureCodecs`](crate::ProcedureCodecs)
    /// (Decision 02). `None` if the request carries no recognizable codec token.
    fn codec_name(&self, request: &SimpleIncomingRequest) -> Option<String>;
    /// Build the per-call exchange over the transport's byte pipes (consumes the
    /// negotiated compression to build the reader/writer tasks).
    ///
    /// # Errors
    /// A [`ConnectError`](crate::ConnectError)-carrying trace on negotiation
    /// failure.
    fn new_conn(
        &self,
        request: &SimpleIncomingRequest,
        spec: Spec,
        body: ByteSource,
        responder: ByteSink,
        compression: &CompressionRegistry,
    ) -> ConnectResult<HandlerExchange>;

    /// Decode a **unary** request body into codec-level message bytes — the
    /// protocol-specific de-framing + decompression a unary call needs
    /// (Connect: a bare, optionally-compressed body, or the GET `message` query;
    /// gRPC-Web: a single envelope frame). Unary does **not** go through the
    /// streaming seam (Decision 08 §Type-Erased Handler Traits): the erased unary
    /// handler works on message bytes, and the wire framing differs from streaming
    /// (bare vs enveloped).
    ///
    /// # Errors
    /// A [`ConnectError`](crate::ConnectError)-carrying trace on a malformed frame,
    /// decompression failure, or a missing decompressor for a compressed body.
    fn decode_unary_request(
        &self,
        request: &SimpleIncomingRequest,
        body: Bytes,
        compression: &CompressionRegistry,
    ) -> ConnectResult<Bytes>;

    /// The `Content-Type` for a **streaming** response head in this protocol
    /// (Connect: `application/connect+{codec}`; gRPC-Web: `application/grpc-web[-text]+{codec}`).
    fn streaming_response_content_type(
        &self,
        request: &SimpleIncomingRequest,
        codec_name: &str,
    ) -> String;

    /// Frame a successful **unary** handler result into the HTTP response
    /// (Connect: bare body + `Trailer-` headers; gRPC-Web: message envelope +
    /// `0x80` trailer frame with `grpc-status: 0`). Errors are rendered separately
    /// by [`ErrorWriter`](crate::ErrorWriter) — protocol-aware already.
    ///
    /// # Errors
    /// A trace on a compression failure.
    fn encode_unary_response(
        &self,
        response: &mut SimpleOutgoingResponse,
        request: &SimpleIncomingRequest,
        codec_name: &str,
        outcome: UnaryOutcome,
        compression: &CompressionRegistry,
    ) -> ConnectResult<()>;
}

/// The client-side protocol abstraction (Decision 05).
pub trait ProtocolClient: Send + Sync {
    /// Build request headers for an outgoing RPC.
    fn write_request_headers(
        &self,
        stream_type: StreamType,
        headers: &mut SimpleHeaders,
        codec_name: &str,
        compression: Option<&str>,
    );
    /// Encode the (optionally compressed) marshaled request body for a **unary**
    /// call. Protocol-specific wrapping — Connect is the identity (bare bytes),
    /// gRPC and gRPC-Web wrap the body in a single 5-byte envelope frame.
    ///
    /// `is_compressed` is `true` when a compressor was applied to `body`; the
    /// protocol sets the corresponding flag (gRPC → envelope flags bit 0x01,
    /// Connect → already set as `Content-Encoding` by the caller).
    fn encode_unary_request(&self, body: &[u8], is_compressed: bool) -> Bytes;
    /// Decode the raw response body bytes into message-level bytes for a
    /// **unary** call. Protocol-specific unwrapping — Connect is the identity
    /// (bare bytes), gRPC and gRPC-Web strip the 5-byte envelope frame.
    ///
    /// # Errors
    /// A trace on a malformed envelope (truncated frame, missing compression
    /// handler).
    fn decode_unary_response(&self, body: Bytes) -> ConnectResult<Bytes>;
    /// Build the per-call exchange over a live transport stream.
    ///
    /// The `cancel` signal is linked into the conn's sender/receiver halves so
    /// caller cancellation wakes parked pipe ops. The framework passes
    /// `CancelSignal::linked(&caller_signal)` — tests and base contexts pass
    /// `CancelSignal::new()`.
    ///
    /// # Errors
    /// A trace on setup failure.
    fn new_conn(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
        cancel: CancelSignal,
    ) -> ConnectResult<ClientExchange>;
}

/// Canonicalize a `Content-Type` for matching (Decision 05 P6 / Decision 02 P1):
/// lowercase the media type and **strip parameters** (e.g. `charset`), so
/// `application/json; charset=utf-8` matches `application/json`.
#[must_use]
pub fn canonicalize_content_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}

/// The default codec when a content-type names a family but no `+suffix`.
const DEFAULT_CODEC: &str = "proto";

/// Match `canonical` against a content-type `prefix`, returning the codec suffix.
///
/// WHY: a bare `strip_prefix` is not a media-type match. `application/grpc` is a
/// prefix of `application/grpc-web+proto`, so matching that way makes the gRPC
/// handler claim every gRPC-Web request — and because handlers are tried in
/// order, the gRPC-Web handler is then never consulted. The request is judged
/// against gRPC's capabilities, which demand HTTP/2, and an HTTP/1.1 gRPC-Web
/// call is refused with 505. The subtype must therefore end where the prefix
/// ends.
///
/// WHAT: `Some(codec)` only when `canonical` is exactly `prefix` (codec defaults
/// to `proto`) or `prefix` followed by `+codec`. Anything that merely *starts
/// with* `prefix` and continues into a different subtype is `None`.
#[must_use]
pub fn codec_for_content_type<'a>(canonical: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = canonical.strip_prefix(prefix)?;
    match rest.as_bytes().first() {
        // Exactly the prefix: `application/grpc`.
        None => Some(DEFAULT_CODEC),
        // The prefix, then a codec: `application/grpc+json`. A trailing `+` with
        // nothing after it names no codec and is not a match.
        Some(b'+') => match &rest[1..] {
            "" => None,
            codec => Some(codec),
        },
        // The prefix ran into a longer subtype: `application/grpc-web+proto`.
        Some(_) => None,
    }
}

/// The codec wire name + whether the content-type is a streaming one, parsed from
/// a canonicalized Content-Type per the Connect grammar
/// (`application/{codec}` unary, `application/connect+{codec}` streaming). Returns
/// `None` for a non-`application/` type or for content types that belong to other
/// protocols (e.g. `application/grpc-web+{codec}`).
#[must_use]
pub fn parse_connect_content_type(content_type: &str) -> Option<(String, bool)> {
    let canonical = canonicalize_content_type(content_type);
    let rest = canonical.strip_prefix("application/")?;
    match rest.strip_prefix("connect+") {
        Some(codec) => Some((codec.to_string(), true)),
        None => {
            let codec = rest.to_string();
            // Reject compound content types (e.g. grpc-web+proto) that contain '+'
            // but don't start with 'connect+' — those belong to other protocols such
            // as gRPC-Web.
            if codec.contains('+') {
                None
            } else {
                Some((codec, false))
            }
        }
    }
}
