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
pub mod grpc_web;

use std::time::Duration;

use foundation_netio::simple_http::shared::{
    SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};

use crate::compression::CompressionRegistry;
use crate::context::{Spec, StreamType};
use crate::error::ConnectResult;
use crate::transport::{
    BoxFuture, ByteSink, ByteSource, ClientConn, HandlerConn, TransportStream,
};

/// A per-call reader/writer task — an async loop the framework spawns on valtron.
pub type BoxedTask = BoxFuture<'static, ConnectResult<()>>;

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
    /// HTTP methods this protocol accepts.
    fn allowed_methods(&self) -> &[SimpleMethod];
    /// Content-Types this handler can process.
    fn content_types(&self) -> Vec<String>;
    /// Parse the call timeout from request headers.
    fn parse_timeout(&self, headers: &SimpleHeaders) -> Option<Duration>;
    /// Can this handler process the given request?
    fn can_handle(&self, request: &SimpleIncomingRequest) -> bool;
    /// Build the per-call exchange over the transport's byte pipes (consumes the
    /// negotiated compression to build the reader/writer tasks).
    ///
    /// # Errors
    /// A [`ConnectError`](crate::ConnectError)-carrying trace on negotiation
    /// failure.
    fn new_conn(
        &self,
        request: &SimpleIncomingRequest,
        body: ByteSource,
        responder: ByteSink,
        compression: &CompressionRegistry,
    ) -> ConnectResult<HandlerExchange>;
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
    /// Build the per-call exchange over a live transport stream.
    ///
    /// # Errors
    /// A trace on setup failure.
    fn new_conn(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
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

/// The codec wire name + whether the content-type is a streaming one, parsed from
/// a canonicalized Content-Type per the Connect grammar
/// (`application/{codec}` unary, `application/connect+{codec}` streaming). Returns
/// `None` for a non-`application/` type.
#[must_use]
pub fn parse_connect_content_type(content_type: &str) -> Option<(String, bool)> {
    let canonical = canonicalize_content_type(content_type);
    let rest = canonical.strip_prefix("application/")?;
    match rest.strip_prefix("connect+") {
        Some(codec) => Some((codec.to_string(), true)),
        None => Some((rest.to_string(), false)),
    }
}
