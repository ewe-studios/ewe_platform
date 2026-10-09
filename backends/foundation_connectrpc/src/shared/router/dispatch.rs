//! The frozen prefix route + request dispatch flow (Decision 08 §Request Dispatch
//! Flow / §Integration with foundation_http).
//!
//! WHY: One [`ConnectRpcHandler`] is registered as a **single** foundation_http
//! prefix route; it sub-routes internally by path. Dispatch is a fixed sequence
//! of decisions — 404 → 405(+Allow) → protocol match → codec membership(+415) →
//! capability check(+505) — then it runs the erased handler and frames the
//! response per protocol. Errors at any point render through the protocol-aware
//! [`ErrorWriter`].
//!
//! WHAT: [`ConnectRpcHandler`] and the async `dispatch` entry (returns a
//! `SimpleOutgoingResponse`; never `Err` — failures become error responses).
//!
//! HOW: unary runs sequentially (decode → handle → encode — it does **not** use
//! the streaming seam, Decision 08); streaming builds the protocol exchange and
//! drives its reader/writer tasks, the handler, a body feeder, and a response
//! collector concurrently with [`futures::join!`] — cooperatively inside this one
//! dispatch future (no `spawn`, so no executor-pool dependency). Response bytes
//! are collected in memory here; a live streaming transport swaps that for true
//! streaming in a later feature.

#[cfg(not(target_family = "wasm"))]
use std::io;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use foundation_core::extensions::result_ext::SendableBoxedError;
use foundation_core::valtron::Pipe;
use foundation_core::valtron::Stream;
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_http::shared::context::ContextBag;
// HTTP/2 dispatch is native-only (drives foundation_netio::http2). The h2 section
// below is cfg-gated to match; on wasm the router serves H1 protocols only.
#[cfg(not(target_family = "wasm"))]
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

use crate::shared::context::{CancelSignal, Ctx, IdempotencyLevel, Peer, RequestContext, Spec};
use crate::shared::error::{ConnectError, ConnectResult};
use crate::shared::error_writer::ErrorWriter;
use crate::shared::protocol::{ProtocolHandler, UnaryOutcome};
use crate::shared::transport::{
    check_compatible, requirements, BoxFuture, ProtocolKind, TransportCapabilities,
};

use super::{HandlerEntry, HandlerKind, Router};

/// The frozen foundation_http prefix handler produced by
/// [`Router::into_handler`](super::Router::into_handler) (Q10 — no post-build
/// mutation). Register it as one prefix route; it dispatches every ConnectRPC
/// request by path.
pub struct ConnectRpcHandler {
    router: Arc<Router>,
}

impl ConnectRpcHandler {
    pub(crate) fn new(router: Router) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    /// The underlying router (read-only).
    #[must_use]
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// Dispatch one request, resolving to the response to write. Never `Err` —
    /// every failure is rendered as a protocol-appropriate error response.
    #[must_use]
    pub fn dispatch(
        &self,
        bag: Arc<ContextBag>,
        request: SimpleIncomingRequest,
    ) -> BoxFuture<'static, SimpleOutgoingResponse> {
        let router = self.router.clone();
        Box::pin(dispatch_request(router, bag, request))
    }
}

async fn dispatch_request(
    router: Arc<Router>,
    bag: Arc<ContextBag>,
    request: SimpleIncomingRequest,
) -> SimpleOutgoingResponse {
    let path = extract_path(&request.request_url.url);

    // 1. Path lookup → 404.
    let Some(entry) = router.lookup(&path) else {
        return status_only(&request, Status::NotFound);
    };

    // 2. Method check → 405 (+Allow).
    let allowed = allowed_methods(entry);
    if !allowed.contains(&request.method) {
        return method_not_allowed(&request, &allowed);
    }

    // 3. Protocol match by Content-Type / GET query → 415 (+Accept-Post).
    let Some(protocol) = entry
        .protocol_handlers
        .iter()
        .find(|p| p.can_handle(&request))
    else {
        return unsupported_media_type(&request, entry);
    };
    let protocol = protocol.as_ref();

    // 4. Codec name + membership → 415 (+Accept-Post).
    let Some(codec_name) = protocol.codec_name(&request) else {
        return unsupported_media_type(&request, entry);
    };
    if !entry.codec_meta.has_codec(&codec_name) {
        return unsupported_media_type(&request, entry);
    }

    // 5. Capability check → 505 (bidi over HTTP/1.1, gRPC over HTTP/1.1, …).
    let caps = capabilities_for(&request.proto);
    let reqs = requirements(protocol.kind(), entry.spec.stream_type);
    if check_compatible(&reqs, &caps).is_err() {
        return http_version_not_supported(&request);
    }

    // 5b. P7: enforce the Connect protocol-version marker when the procedure
    // requires it (opt-in per procedure).
    if entry.options.require_connect_protocol_header && protocol.kind() == ProtocolKind::Connect {
        if let Err(e) = require_connect_version(&request) {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return response;
        }
    }

    // 6. Execute. Compression: per-procedure override else the router's global.
    let compression = entry
        .options
        .compression
        .as_ref()
        .unwrap_or_else(|| router.global_compression());
    let ctx = build_ctx(bag, &request, entry.spec.clone(), protocol);
    let spec = entry.spec.clone();
    let pipe_depth = entry.options.pipe_depth;

    // Move `request` (a `Send` value) into the run functions — the decision phase
    // above is fully synchronous, so no `&request` is held across an await (which
    // would poison `Send`, as `SimpleIncomingRequest` is not `Sync`).
    match &entry.handler {
        HandlerKind::Unary(handler) => {
            run_unary(
                handler.clone(),
                protocol,
                request,
                codec_name,
                compression,
                ctx,
                entry.options.limits.read_max_bytes,
            )
            .await
        }
        HandlerKind::ServerStream(handler)
        | HandlerKind::ClientStream(handler)
        | HandlerKind::BidiStream(handler) => {
            run_streaming(
                handler.clone(),
                protocol,
                request,
                spec,
                codec_name,
                compression,
                ctx,
                pipe_depth,
            )
            .await
        }
    }
}

/// Unary: decode the request body → invoke the erased handler → frame the
/// response per protocol. Unary bypasses the streaming seam (Decision 08).
pub(crate) async fn run_unary(
    handler: Arc<dyn super::erased::ErasedUnaryHandler>,
    protocol: &dyn ProtocolHandler,
    mut request: SimpleIncomingRequest,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    read_max_bytes: usize,
) -> SimpleOutgoingResponse {
    let mut response = blank_response(&request);

    let (tx, mut rx) = Pipe::<Bytes>::new();
    if let Err(e) = pipe_body(&tx, &mut request) {
        tracing::warn!(error = %e, "body pipe error; proceeding with partial body");
    }
    drop(tx);
    let body = match drain_to_bytes(&mut rx, read_max_bytes) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "request body exceeded read_max_bytes; returning 413");
            response.status = Status::PayloadTooLarge;
            return response;
        }
    };
    let decoded = protocol.decode_unary_request(&request, body, compression);
    let outcome: ConnectResult<UnaryOutcome> = match decoded {
        Ok(msg) => match handler.handle(ctx, &codec_name, msg).await {
            Ok((frame, headers, trailers)) => Ok(UnaryOutcome {
                frame,
                headers,
                trailers,
            }),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    };

    match outcome {
        Ok(outcome) => {
            if let Err(e) = protocol.encode_unary_response(
                &mut response,
                &request,
                &codec_name,
                outcome,
                compression,
            ) {
                write_error(&mut response, &request, &e);
            }
        }
        Err(e) => write_error(&mut response, &request, &e),
    }
    response
}

/// Streaming: build the protocol exchange over byte pipes, then drive its
/// reader/writer tasks, the handler, a body feeder, and a response collector
/// concurrently. Response bytes are collected in memory (see module docs).
#[allow(clippy::too_many_arguments)]
async fn run_streaming(
    handler: Arc<dyn super::erased::ErasedStreamHandler>,
    protocol: &dyn ProtocolHandler,
    mut request: SimpleIncomingRequest,
    spec: Spec,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    pipe_depth: usize,
) -> SimpleOutgoingResponse {
    let mut response = blank_response(&request);

    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(pipe_depth);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(pipe_depth);

    let exchange = match protocol.new_conn(&request, spec, req_rx, resp_tx, compression) {
        Ok(exchange) => exchange,
        Err(e) => {
            write_error(&mut response, &request, &e);
            return response;
        }
    };
    let crate::shared::protocol::HandlerExchange {
        conn,
        reader_task,
        writer_task,
    } = exchange;

    // Feed request body chunks into the transport byte pipe, then close.
    let (body_tx, body_rx) = Pipe::<Bytes>::new();
    if let Err(e) = pipe_body(&body_tx, &mut request) {
        tracing::warn!(error = %e, "body pipe error; request body will be empty");
    }
    drop(body_tx);
    let feeder = async move {
        let rx = body_rx;
        while let Ok(chunk) = rx.try_recv() {
            if req_tx.send(chunk).await.is_err() {
                break;
            }
        }
        req_tx.close();
    };
    // Collect the framed response bytes the writer task emits.
    let collector = async move {
        let mut out = Vec::new();
        while let Some(chunk) = resp_rx.receive().await {
            out.extend_from_slice(&chunk);
        }
        out
    };
    let handler_fut = handler.handle(ctx, &codec_name, conn);

    let (_, reader_res, writer_res, handler_res, collected) =
        futures::join!(feeder, reader_task, writer_task, handler_fut, collector);

    let content_type = protocol.streaming_response_content_type(&request, &codec_name);
    // The writer task compressed per this same negotiation — the peer only
    // accepts flagged envelopes when the encoding is advertised.
    let response_encoding = protocol.streaming_response_encoding(&request, compression);
    if !collected.is_empty() {
        // The stream framed itself (including any in-band terminal error).
        response.status = Status::OK;
        response
            .headers
            .insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
        if let Some((name, enc)) = response_encoding {
            response.headers.insert(SimpleHeader::from(name), vec![enc]);
        }
        response.body = Some(SendSafeBody::Bytes(collected));
    } else if let Some(e) = handler_res.err().or(writer_res.err()).or(reader_res.err()) {
        write_error(&mut response, &request, &e);
    } else {
        // No frames and no error — an empty successful stream.
        response.status = Status::OK;
        response
            .headers
            .insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
        response.body = Some(SendSafeBody::Bytes(Vec::new()));
    }
    response
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// The routing key: the path with any query string stripped (R1 leading slash).
pub(crate) fn extract_path(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

/// The union of HTTP methods any protocol on this entry accepts (for `Allow`).
///
/// Connect GET is only valid for idempotent/NoSideEffects procedures. When the
/// procedure is not NoSideEffects, GET is excluded from the allowed list so that
/// a GET to a POST-only endpoint correctly returns 405 (Decision 05 §Unary GET).
pub(crate) fn allowed_methods(entry: &HandlerEntry) -> Vec<SimpleMethod> {
    let mut methods: Vec<SimpleMethod> = Vec::new();
    let is_nosideeffects = entry.spec.idempotency == IdempotencyLevel::NoSideEffects;
    for protocol in &entry.protocol_handlers {
        for method in protocol.allowed_methods() {
            // Filter out GET from Connect protocol when the procedure is not
            // NoSideEffects — the protocol handler always advertises GET, but
            // procedure-level idempotency restricts it (Decision 05 §Unary GET).
            if !is_nosideeffects && method == &SimpleMethod::GET {
                continue;
            }
            if !methods.contains(method) {
                methods.push(method.clone());
            }
        }
    }
    methods
}

/// Server-side transport capabilities inferred from the request's HTTP version.
pub(crate) fn capabilities_for(proto: &Proto) -> TransportCapabilities {
    let h2 = *proto >= Proto::HTTP20;
    TransportCapabilities {
        // HTTP/1.1 chunked and HTTP/2+ can both stream the request body.
        request_streaming: true,
        full_duplex: h2,
        h2_trailers: h2,
        http_versions: match proto {
            Proto::HTTP10 => &[Proto::HTTP10],
            Proto::HTTP11 => &[Proto::HTTP11],
            Proto::HTTP20 => &[Proto::HTTP20],
            Proto::HTTP30 => &[Proto::HTTP30],
            Proto::Custom(_) => &[Proto::HTTP11],
        },
        multiplexed: h2,
    }
}

/// Build the per-call [`Ctx`] the dispatcher hands the handler.
pub(crate) fn build_ctx(
    bag: Arc<ContextBag>,
    request: &SimpleIncomingRequest,
    spec: Spec,
    protocol: &dyn ProtocolHandler,
) -> Ctx {
    let deadline = protocol
        .parse_timeout(&request.headers)
        .map(|d| Instant::now() + d);
    let peer = Peer {
        addr: request
            .connection
            .peer_addr
            .as_ref()
            .map(|a| format!("{a:?}"))
            .unwrap_or_default(),
        protocol: protocol_name(protocol.kind()).to_string(),
    };
    let request_context = RequestContext::for_dispatch(
        spec,
        peer,
        request.headers.clone(),
        deadline,
        foundation_netio::shared::http::Extensions::new(),
        request.connection.clone(),
        CancelSignal::new(),
    );
    Ctx {
        bag,
        request: request_context,
    }
}

/// P7: a required Connect procedure must carry the version marker —
/// `Connect-Protocol-Version: 1` on POST, or `connect=v1` in the GET query.
pub(crate) fn require_connect_version(request: &SimpleIncomingRequest) -> ConnectResult<()> {
    use crate::shared::protocol::connect::constants;
    let present = if request.method == SimpleMethod::GET {
        request
            .request_url
            .queries
            .as_ref()
            .and_then(|q| q.get(constants::QUERY_CONNECT_VERSION))
            .is_some_and(|v| v == constants::QUERY_CONNECT_VERSION_VALUE)
    } else {
        request
            .headers
            .get(&SimpleHeader::from(
                constants::HEADER_PROTOCOL_VERSION.to_string(),
            ))
            .and_then(|v| v.first())
            .is_some_and(|v| v == constants::PROTOCOL_VERSION)
    };
    if present {
        Ok(())
    } else {
        Err(ConnectError::invalid_argument(format!(
            "missing required {} header",
            constants::HEADER_PROTOCOL_VERSION
        ))
        .into())
    }
}

fn protocol_name(kind: ProtocolKind) -> &'static str {
    match kind {
        ProtocolKind::Connect => "connect",
        ProtocolKind::Grpc => "grpc",
        ProtocolKind::GrpcWeb => "grpc-web",
    }
}

/// Pump request body chunks into `tx`. Caller creates the pipe and holds `rx`.
/// No buffering — the caller drains chunks from `rx`.
///
/// # Errors
/// Stream error from the body iterator, or pipe-full from `try_send`.
fn pipe_body(tx: &PipeSender<Bytes>, request: &mut SimpleIncomingRequest) -> Result<(), SendableBoxedError> {
    let Some(body) = request.body.take() else { return Ok(()) };
    let mut iter = SendSafeBodyBytesIterator::new(body);
    loop {
        match iter.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                tx.try_send(bytes).map_err(|e| Box::new(std::io::Error::other(e.to_string())) as SendableBoxedError)?;
            }
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                return Err(Box::new(std::io::Error::other(e.to_string())));
            }
            None => return Ok(()),
            _ => {}
        }
    }
}

/// Drain a closed pipe receiver into `Bytes`, enforcing `max_bytes`.
/// `0` means unlimited. Returns `Err` if the body exceeds the cap.
fn drain_to_bytes(rx: &mut PipeReceiver<Bytes>, max_bytes: usize) -> Result<Bytes, SendableBoxedError> {
    let mut buf = bytes::BytesMut::new();
    while let Ok(chunk) = rx.try_recv() {
        buf.extend_from_slice(&chunk);
        if max_bytes > 0 && buf.len() > max_bytes {
            return Err(Box::new(std::io::Error::other(
                format!("request body of {} bytes exceeds read_max_bytes of {max_bytes}", buf.len()),
            )));
        }
    }
    Ok(buf.freeze())
}

/// A blank response carrying the request's HTTP version and no headers/body.
pub(crate) fn blank_response(request: &SimpleIncomingRequest) -> SimpleOutgoingResponse {
    SimpleOutgoingResponse {
        proto: request.proto.clone(),
        status: Status::OK,
        headers: SimpleHeaders::new(),
        body: None,
        trailers: SimpleHeaders::new(),
    }
}

pub(crate) fn status_only(
    request: &SimpleIncomingRequest,
    status: Status,
) -> SimpleOutgoingResponse {
    let mut response = blank_response(request);
    response.status = status;
    response
}

pub(crate) fn method_not_allowed(
    request: &SimpleIncomingRequest,
    allowed: &[SimpleMethod],
) -> SimpleOutgoingResponse {
    let mut response = status_only(request, Status::MethodNotAllowed);
    let allow = allowed
        .iter()
        .map(SimpleMethod::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    response
        .headers
        .insert(SimpleHeader::from("allow".to_string()), vec![allow]);
    response
}

pub(crate) fn unsupported_media_type(
    request: &SimpleIncomingRequest,
    entry: &HandlerEntry,
) -> SimpleOutgoingResponse {
    let mut response = status_only(request, Status::UnsupportedMediaType);
    // Accept-Post advertises the codecs (as content-types) this procedure accepts.
    let names = entry.codec_meta.codec_names();
    let accept_post = names
        .iter()
        .map(|codec| format!("application/{codec}"))
        .collect::<Vec<_>>()
        .join(", ");
    if !accept_post.is_empty() {
        response.headers.insert(
            SimpleHeader::from("accept-post".to_string()),
            vec![accept_post],
        );
    }
    response
}

pub(crate) fn http_version_not_supported(
    request: &SimpleIncomingRequest,
) -> SimpleOutgoingResponse {
    status_only(request, Status::HttpVersionNotSupported)
}

/// Render an RPC error into `response` via the protocol-aware [`ErrorWriter`].
pub(crate) fn write_error(
    response: &mut SimpleOutgoingResponse,
    request: &SimpleIncomingRequest,
    error: &foundation_errstacks::ErrorTrace<ConnectError>,
) {
    let _ = ErrorWriter::new().write(response, request, error);
}

// ── HTTP/2 dispatch ───────────────────────────────────────────────────────────

/// Rebuild the request shell from an h2 HEADERS frame. The body is supplied
/// separately (a pipe), so it stays `None` here.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn request_from_header(header: &SimpleIncomingRequestHeader) -> SimpleIncomingRequest {
    SimpleIncomingRequest {
        proto: Proto::HTTP20,
        request_uri: header.uri.clone(),
        request_url: header.url.clone(),
        body: None,
        headers: header.headers.clone(),
        method: header.method.clone(),
        extensions: None,
        connection: header.connection.clone(),
    }
}

/// Headers that are connection-specific and forbidden on h2 (RFC 7540 §8.1.2.2).
#[cfg(not(target_family = "wasm"))]
const H2_FORBIDDEN_HEADERS: [&str; 5] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
];

/// Flatten `SimpleHeaders` into h2 wire pairs.
///
/// h2 field names must be lowercase (RFC 7540 §8.1.2) — a capitalised name is a
/// protocol error at a strict peer, so names are lowered rather than trusted.
#[cfg(not(target_family = "wasm"))]
fn h2_headers(headers: &SimpleHeaders) -> Vec<(Bytes, Bytes)> {
    let mut out = Vec::new();
    for (key, values) in headers.iter() {
        let name = key.to_string().to_lowercase();
        if H2_FORBIDDEN_HEADERS.contains(&name.as_str()) {
            continue;
        }
        for value in values {
            out.push((Bytes::from(name.clone()), Bytes::from(value.clone())));
        }
    }
    out
}

/// The pipe is gone because the connection handler dropped the stream.
#[cfg(not(target_family = "wasm"))]
fn stream_gone<T>(_: T) -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "h2 response stream closed")
}

/// Serialize a complete (non-streaming) response: HEADERS, then one DATA frame
/// per body chunk, then optional trailing HEADERS.
#[cfg(not(target_family = "wasm"))]
async fn send_whole_response(
    tx: &PipeSender<H2Frame>,
    mut response: SimpleOutgoingResponse,
) -> io::Result<()> {
    let status = response.status.into_usize() as u16;
    let headers = h2_headers(&response.headers);
    let has_trailers = !response.trailers.is_empty();
    let body = response.body.take();

    fn next_chunk(it: &mut Option<SendSafeBodyBytesIterator>) -> io::Result<Option<Bytes>> {
        let Some(ref mut iter) = it else {
            return Ok(None);
        };
        loop {
            match iter.next() {
                Some(Stream::Next(SendSafeBodyBytesItem::Chunk(b))) => return Ok(Some(b)),
                Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                    return Err(io::Error::other(e.to_string()));
                }
                None => return Ok(None),
                _ => {} // Ignore, Init, etc — skip.
            }
        }
    }

    let mut iter = body.map(SendSafeBodyBytesIterator::new);
    let mut chunk = next_chunk(&mut iter)?;
    let end_stream = chunk.is_none() && !has_trailers;

    tx.send(H2Frame::Headers {
        status,
        headers,
        end_stream,
    })
    .await
    .map_err(stream_gone)?;

    while let Some(cur) = chunk.take() {
        let next = next_chunk(&mut iter)?;
        let data_end = next.is_none() && !has_trailers;
        tx.send(H2Frame::Data {
            payload: cur,
            end_stream: data_end,
        })
        .await
        .map_err(stream_gone)?;
        chunk = next;
    }

    if has_trailers {
        // Real trailing HEADERS (no pseudo-headers) — the old shape here was a
        // `Headers { status: 0, .. }` frame, which put an invalid `:status: 0`
        // pseudo-header in the trailers (RFC 9113 §8.1 forbids them).
        tx.send(H2Frame::Trailers {
            headers: h2_headers(&response.trailers),
        })
        .await
        .map_err(stream_gone)?;
    }

    Ok(())
}

/// Drain the inbound h2 body pipe into a buffer (unary: exactly one message).
///
/// A peer RST_STREAM ends collection early; the caller abandons the stream.
#[cfg(not(target_family = "wasm"))]
async fn collect_h2_body(body: &PipeReceiver<H2IncomingFrame>) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    while let Some(frame) = body.receive().await {
        match frame {
            H2IncomingFrame::Data(chunk) => out.extend_from_slice(&chunk),
            H2IncomingFrame::Reset(_) => return None,
        }
    }
    Some(out)
}

#[cfg(not(target_family = "wasm"))]
impl ConnectRpcHandler {
    /// HTTP/2 dispatch: bridge one h2 stream through the router.
    ///
    /// Unlike [`dispatch`](Self::dispatch) this never materialises a
    /// `SimpleOutgoingResponse` for streaming procedures — response frames are
    /// pushed into `tx` as the handler produces them, and request DATA frames
    /// are forwarded into the protocol's byte pipe as they arrive.
    pub fn dispatch_h2(
        &self,
        bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let router = self.router.clone();
        Box::pin(dispatch_h2_stream(router, bag, header, body, tx))
    }
}

#[cfg(not(target_family = "wasm"))]
async fn dispatch_h2_stream(
    router: Arc<Router>,
    bag: Arc<ContextBag>,
    header: SimpleIncomingRequestHeader,
    body: PipeReceiver<H2IncomingFrame>,
    tx: PipeSender<H2Frame>,
) -> io::Result<()> {
    let request = request_from_header(&header);
    let path = extract_path(&request.request_url.url);

    // Decision phase — identical to `dispatch_request`, but every rejection is
    // serialized as h2 frames rather than returned.
    let Some(entry) = router.lookup(&path) else {
        return send_whole_response(&tx, status_only(&request, Status::NotFound)).await;
    };

    let allowed = allowed_methods(entry);
    if !allowed.contains(&request.method) {
        return send_whole_response(&tx, method_not_allowed(&request, &allowed)).await;
    }

    let Some(protocol) = entry
        .protocol_handlers
        .iter()
        .find(|p| p.can_handle(&request))
    else {
        return send_whole_response(&tx, unsupported_media_type(&request, entry)).await;
    };
    let protocol = protocol.as_ref();

    let Some(codec_name) = protocol.codec_name(&request) else {
        return send_whole_response(&tx, unsupported_media_type(&request, entry)).await;
    };
    if !entry.codec_meta.has_codec(&codec_name) {
        return send_whole_response(&tx, unsupported_media_type(&request, entry)).await;
    }

    let caps = capabilities_for(&request.proto);
    let reqs = requirements(protocol.kind(), entry.spec.stream_type);
    if check_compatible(&reqs, &caps).is_err() {
        return send_whole_response(&tx, http_version_not_supported(&request)).await;
    }

    if entry.options.require_connect_protocol_header && protocol.kind() == ProtocolKind::Connect {
        if let Err(e) = require_connect_version(&request) {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return send_whole_response(&tx, response).await;
        }
    }

    let compression = entry
        .options
        .compression
        .as_ref()
        .unwrap_or_else(|| router.global_compression());
    let ctx = build_ctx(bag, &request, entry.spec.clone(), protocol);
    let spec = entry.spec.clone();
    let pipe_depth = entry.options.pipe_depth;

    match &entry.handler {
        HandlerKind::Unary(handler) => {
            // Unary carries exactly one message: collect it, then reuse the
            // sequential unary path.
            let Some(collected) = collect_h2_body(&body).await else {
                // Peer reset the stream — nothing to answer.
                return Ok(());
            };
            let mut request = request;
            request.body = Some(SendSafeBody::Bytes(collected));

            let response = run_unary(
                handler.clone(),
                protocol,
                request,
                codec_name,
                compression,
                ctx,
                entry.options.limits.read_max_bytes,
            )
            .await;
            send_whole_response(&tx, response).await
        }
        HandlerKind::ServerStream(handler)
        | HandlerKind::ClientStream(handler)
        | HandlerKind::BidiStream(handler) => {
            run_streaming_h2(
                handler.clone(),
                protocol,
                request,
                spec,
                codec_name,
                compression,
                ctx,
                pipe_depth,
                body,
                &tx,
            )
            .await
        }
    }
}

/// Streaming over h2: request DATA frames feed the protocol's byte pipe while
/// the protocol's output pipe drains into response DATA frames — concurrently,
/// so a bidi handler can read and write at the same time.
#[allow(clippy::too_many_arguments)]
#[cfg(not(target_family = "wasm"))]
async fn run_streaming_h2(
    handler: Arc<dyn super::erased::ErasedStreamHandler>,
    protocol: &dyn ProtocolHandler,
    request: SimpleIncomingRequest,
    spec: Spec,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    pipe_depth: usize,
    body: PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
) -> io::Result<()> {
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(pipe_depth);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(pipe_depth);

    let exchange = match protocol.new_conn(&request, spec, req_rx, resp_tx, compression) {
        Ok(exchange) => exchange,
        Err(e) => {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return send_whole_response(tx, response).await;
        }
    };
    let crate::shared::protocol::HandlerExchange {
        conn,
        reader_task,
        writer_task,
    } = exchange;

    // Forward inbound DATA frames as they arrive — no buffering of the body.
    let feeder = async {
        while let Some(frame) = body.receive().await {
            match frame {
                H2IncomingFrame::Data(chunk) => {
                    if req_tx.send(chunk).await.is_err() {
                        break;
                    }
                }
                H2IncomingFrame::Reset(_) => break,
            }
        }
        req_tx.close();
    };

    // Forward framed response bytes out as DATA frames. HEADERS are withheld
    // until the first chunk so that a handler which fails before emitting
    // anything can still be reported with a real status.
    let content_type = protocol.streaming_response_content_type(&request, &codec_name);
    // The writer task compressed per this same negotiation — a flagged envelope
    // is only valid when the encoding header goes out with the response HEADERS.
    let response_encoding = protocol.streaming_response_encoding(&request, compression);
    let pump_content_type = content_type.clone();
    let pump = async {
        let mut opened = false;
        while let Some(chunk) = resp_rx.receive().await {
            if !opened {
                opened = true;
                let mut headers = vec![(
                    Bytes::from_static(b"content-type"),
                    Bytes::from(pump_content_type.clone()),
                )];
                if let Some((name, enc)) = &response_encoding {
                    headers.push((Bytes::from(name.clone()), Bytes::from(enc.clone())));
                }
                tx.send(H2Frame::Headers {
                    status: 200,
                    headers,
                    end_stream: false,
                })
                .await
                .map_err(stream_gone)?;
            }
            tx.send(H2Frame::Data {
                payload: chunk,
                end_stream: false,
            })
            .await
            .map_err(stream_gone)?;
        }
        Ok::<bool, io::Error>(opened)
    };

    let handler_fut = handler.handle(ctx, &codec_name, conn);

    // The request side (feeder + reader) lives as long as the CLIENT keeps its
    // request stream open — a gRPC bidi peer like buildkitd's fsutil holds it
    // open until it sees OUR trailers. Joining on it before closing the
    // response therefore deadlocks (both sides waiting on the other). The
    // response side alone decides the RPC outcome: race the two, and if the
    // response finishes first, abandon the request side (the stream is over;
    // RFC 9113 lets the server end a stream with a half-open request).
    let request_side = std::pin::pin!(async { futures::join!(feeder, reader_task) });
    let response_side = std::pin::pin!(async { futures::join!(writer_task, handler_fut, pump) });
    let (reader_res, writer_res, handler_res, pumped) =
        match futures::future::select(request_side, response_side).await {
            futures::future::Either::Left(((_, reader_res), response_side)) => {
                let (writer_res, handler_res, pumped) = response_side.await;
                (reader_res, writer_res, handler_res, pumped)
            }
            futures::future::Either::Right(((writer_res, handler_res, pumped), _)) => {
                (Ok(()), writer_res, handler_res, pumped)
            }
        };

    let outcome_err = handler_res.err().or(writer_res.err()).or(reader_res.err());

    if pumped? {
        // The response HEADERS are out. Close the stream per the protocol:
        // gRPC's outcome rides trailing HEADERS (`grpc-status`), and grpc-go
        // hangs/errors on a stream that ends without them. Connect and
        // gRPC-Web carry their terminal frame in-band (EndStreamResponse /
        // 0x80 trailer frame — already written by the protocol writer task),
        // so a bare END_STREAM DATA closes those.
        let closing = if protocol.kind() == ProtocolKind::Grpc {
            H2Frame::Trailers { headers: grpc_status_trailers(outcome_err.as_ref()) }
        } else {
            H2Frame::Data { payload: Bytes::new(), end_stream: true }
        };
        tx.send(closing).await.map_err(stream_gone)?;
        return Ok(());
    }

    // Nothing was emitted: report the failure, or an empty successful stream.
    let mut response = blank_response(&request);
    if let Some(e) = outcome_err {
        write_error(&mut response, &request, &e);
    } else {
        response.status = Status::OK;
        response
            .headers
            .insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
        if protocol.kind() == ProtocolKind::Grpc {
            // A gRPC stream with zero messages is still a gRPC response — it
            // MUST end in `grpc-status` trailers or the client reports
            // `Unknown` with an empty message (bit BuildKit's FileSend sink,
            // whose response stream is legitimately empty).
            response
                .trailers
                .insert(SimpleHeader::from("grpc-status".to_string()), vec!["0".to_string()]);
        }
    }
    send_whole_response(tx, response).await
}

/// gRPC trailing headers for a streaming outcome: `grpc-status: 0` on success,
/// the error's code and percent-encoded message otherwise (gRPC spec §Responses).
fn grpc_status_trailers(
    error: Option<&foundation_errstacks::ErrorTrace<crate::ConnectError>>,
) -> Vec<(Bytes, Bytes)> {
    match error {
        None => vec![(Bytes::from_static(b"grpc-status"), Bytes::from_static(b"0"))],
        Some(e) => {
            let ctx = e.current_context();
            let code = ctx.code() as u32;
            let message = ctx.message().to_string();
            let mut encoded = String::new();
            for b in message.bytes() {
                // Percent-encode per gRPC spec: space and non-printable/percent.
                if b == b'%' || !(0x20..=0x7e).contains(&b) {
                    encoded.push_str(&format!("%{b:02X}"));
                } else {
                    encoded.push(b as char);
                }
            }
            vec![
                (Bytes::from_static(b"grpc-status"), Bytes::from(code.to_string())),
                (Bytes::from_static(b"grpc-message"), Bytes::from(encoded)),
            ]
        }
    }
}
