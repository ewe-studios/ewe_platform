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
use crate::shared::error::{ConnectError, ConnectResult};
use foundation_errstacks::ErrorTrace;
use crate::shared::transport::{
    fail_stream, race, BodyStream, BoxFuture, ByteSink, ByteSource, ClientConn, Frame,
    HandlerConn, HeadStream, ProtocolKind, TransportStream,
};
use foundation_core::valtron::PipeSender;
use foundation_netio::shared::http::Status;
use std::sync::Mutex;

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

/// What a protocol needs, beyond the request and the byte pipes, to build a
/// server exchange ([`ProtocolHandler::new_conn`]).
pub struct ExchangeConfig<'a> {
    /// The compressors to negotiate the call's compression from.
    pub compression: &'a CompressionRegistry,
    /// The call's cancel signal (the handler's `Ctx` carries the same one).
    /// The seam's parked receives and sends wake on it, so a cancelled call
    /// stops a handler that is waiting on its request or response stream.
    pub cancel: CancelSignal,
    /// The largest request message accepted, after decompression (`0` =
    /// unlimited; see [`SizeLimits::read_max_bytes`](crate::SizeLimits)). An
    /// envelope whose length prefix claims more is rejected from its 5-byte
    /// header, before its payload is buffered, and decompression stops at it.
    pub read_max_bytes: usize,
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
    /// A reader task that fails (a malformed or oversized envelope, a failed
    /// decompression) hands the error to the handler as the end of the request
    /// stream: a failed read must never look like a request that ended cleanly.
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
        config: ExchangeConfig<'_>,
    ) -> ConnectResult<HandlerExchange>;

    /// The bytes that end an already-started **streaming** response with
    /// `error`, in band — for when the call fails after the response head and
    /// some frames went out, so the status can no longer change (a deadline
    /// passing, a handler that fails without closing its stream).
    ///
    /// `None` when the protocol carries the outcome outside the body (gRPC's
    /// trailing HEADERS, which the transport writes).
    ///
    /// # Errors
    /// A trace when the terminal frame cannot be encoded.
    fn encode_stream_error(
        &self,
        request: &SimpleIncomingRequest,
        error: &ErrorTrace<ConnectError>,
    ) -> ConnectResult<Option<Bytes>> {
        let _ = (request, error);
        Ok(None)
    }

    /// Decode a **unary** request body into codec-level message bytes — the
    /// protocol-specific de-framing + decompression a unary call needs
    /// (Connect: a bare, optionally-compressed body, or the GET `message` query;
    /// gRPC-Web: a single envelope frame). Unary does **not** go through the
    /// streaming seam (Decision 08 §Type-Erased Handler Traits): the erased unary
    /// handler works on message bytes, and the wire framing differs from streaming
    /// (bare vs enveloped).
    ///
    /// The decoded message is capped at `read_max_bytes` (`0` = unlimited):
    /// decompression stops there with `resource_exhausted`, so a small
    /// compressed body cannot inflate without bound.
    ///
    /// # Errors
    /// A [`ConnectError`](crate::ConnectError)-carrying trace on a malformed frame,
    /// decompression failure, a missing decompressor for a compressed body, or
    /// a message over `read_max_bytes` (`resource_exhausted`).
    fn decode_unary_request(
        &self,
        request: &SimpleIncomingRequest,
        body: Bytes,
        compression: &CompressionRegistry,
        read_max_bytes: usize,
    ) -> ConnectResult<Bytes>;

    /// The `Content-Type` for a **streaming** response head in this protocol
    /// (Connect: `application/connect+{codec}`; gRPC-Web: `application/grpc-web[-text]+{codec}`).
    fn streaming_response_content_type(
        &self,
        request: &SimpleIncomingRequest,
        codec_name: &str,
    ) -> String;

    /// The header advertising the **streaming** response compression this
    /// handler's [`new_conn`](Self::new_conn) writer will apply for `request`
    /// — `(header_name, encoding)`, or `None` for identity.
    ///
    /// Dispatch merges this into the streaming response HEADERS. A compressed
    /// envelope flag without this header is a protocol error to strict peers
    /// (grpc-go: "compressed flag set with identity or empty encoding"), so
    /// the negotiation here MUST match `new_conn`'s — both call
    /// [`negotiate_compression`](crate::shared::compression::negotiate_compression)
    /// on the same request headers. Mirrors connect-go, which always
    /// advertises the compression it applies (`Grpc-Encoding` /
    /// `Connect-Content-Encoding`).
    fn streaming_response_encoding(
        &self,
        request: &SimpleIncomingRequest,
        compression: &CompressionRegistry,
    ) -> Option<(String, String)>;

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
    /// The reader task consumes the whole stream: the response head (its
    /// headers are published for `ClientReceiver::response_headers`, and a
    /// non-200 status ends the stream with the matching error, as for a unary
    /// call), the body, and the trailers.
    ///
    /// # Errors
    /// A trace on setup failure.
    fn new_conn(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
        config: ClientExchangeConfig,
    ) -> ConnectResult<ClientExchange>;
}

/// What a protocol needs, beyond the stream, to build a client exchange
/// ([`ProtocolClient::new_conn`]).
pub struct ClientExchangeConfig {
    /// Linked into the conn's sender/receiver halves so caller cancellation
    /// wakes parked pipe ops. The framework passes
    /// `CancelSignal::linked(&caller_signal)`; tests and base contexts pass
    /// `CancelSignal::new()`.
    pub cancel: CancelSignal,
    /// The largest response message accepted (`0` = unlimited); an envelope
    /// whose length prefix claims more fails the stream before its payload is
    /// read. Also bounds how much of an error body is read.
    pub read_max_bytes: usize,
}

/// Start reading a streaming response: take its head, publish the headers for
/// the receiver, and return them. A failed or missing head, or a non-200
/// status, ends the stream with the error instead (the status maps to a code
/// as for a unary call, with up to `read_max_bytes` of the body as the
/// message) and returns `None`.
pub(crate) async fn open_response_stream(
    head: &mut HeadStream,
    body: &mut BodyStream,
    tx: &PipeSender<Frame>,
    response_headers: &Mutex<Option<SimpleHeaders>>,
    cancel: &CancelSignal,
    read_max_bytes: usize,
) -> Option<SimpleHeaders> {
    use futures::StreamExt;

    let (status, headers) = match head.next().await {
        Some(Ok(head)) => head,
        Some(Err(e)) => {
            fail_stream(tx, &ConnectError::from(e).into(), cancel).await;
            return None;
        }
        None => {
            let e = ConnectError::unavailable("transport closed without response head");
            fail_stream(tx, &e.into(), cancel).await;
            return None;
        }
    };
    *response_headers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(headers.clone());
    if status == Status::OK {
        return Some(headers);
    }

    // Only a bounded part of an error body is read, for the message.
    let mut message = Vec::new();
    while let Some(Ok(chunk)) = body.next().await {
        message.extend_from_slice(&chunk);
        if read_max_bytes != 0 && message.len() >= read_max_bytes {
            message.truncate(read_max_bytes);
            break;
        }
    }
    let message = if message.is_empty() {
        format!("HTTP {status}")
    } else {
        String::from_utf8_lossy(&message).to_string()
    };
    let error: ErrorTrace<ConnectError> =
        ConnectError::new(crate::shared::client::code_from_http_status(&status), message).into();
    let end = Frame::EndStream {
        error: Some(error),
        trailers: headers,
    };
    // Delivered, the consumer gone, or the call cancelled: the stream is over.
    let _ = race(tx.send(end), cancel.cancelled()).await;
    tx.close();
    None
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
            // as gRPC-Web. Likewise the bare `grpc` / `grpc-web` subtypes: plain
            // `application/grpc` is a gRPC request with the default (proto) codec,
            // not a Connect request with a codec named "grpc". Claiming it here
            // starves the gRPC handler in dispatch and 415s every grpc-go client
            // (e.g. buildkitd's session health check / FileSync calls).
            if codec.contains('+') || codec == "grpc" || codec == "grpc-web" {
                None
            } else {
                Some((codec, false))
            }
        }
    }
}
