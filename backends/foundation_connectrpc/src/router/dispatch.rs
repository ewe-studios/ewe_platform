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

use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use foundation_core::valtron::Pipe;
use foundation_http::shared::context::ContextBag;
use foundation_netio::simple_http::client::shared::body_reader::try_collect_bytes;
use foundation_netio::simple_http::shared::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

use crate::context::{CancelSignal, Ctx, Peer, RequestContext, Spec};
use crate::error::{ConnectError, ConnectResult};
use crate::error_writer::ErrorWriter;
use crate::protocol::{ProtocolHandler, UnaryOutcome};
use crate::transport::{
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
    if !allowed.iter().any(|m| *m == request.method) {
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
    if entry.options.require_connect_protocol_header
        && protocol.kind() == ProtocolKind::Connect
    {
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
            run_unary(handler.clone(), protocol, request, codec_name, compression, ctx).await
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
async fn run_unary(
    handler: Arc<dyn super::erased::ErasedUnaryHandler>,
    protocol: &dyn ProtocolHandler,
    mut request: SimpleIncomingRequest,
    codec_name: String,
    compression: &crate::compression::CompressionRegistry,
    ctx: Ctx,
) -> SimpleOutgoingResponse {
    let mut response = blank_response(&request);

    // Decode first (borrows `request`), so the borrow ends *before* the handler
    // await — otherwise `&request` would be held across it and poison `Send`
    // (`SimpleIncomingRequest` is not `Sync`).
    let decoded = {
        let body = read_body(&mut request);
        protocol.decode_unary_request(&request, Bytes::from(body), compression)
    };
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
    compression: &crate::compression::CompressionRegistry,
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
    let crate::protocol::HandlerExchange {
        conn,
        reader_task,
        writer_task,
    } = exchange;

    // Feed the (buffered) request body into the transport byte pipe, then close.
    let body = read_body(&mut request);
    let feeder = async move {
        if !body.is_empty() {
            let _ = req_tx.send(Bytes::from(body)).await;
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
    if !collected.is_empty() {
        // The stream framed itself (including any in-band terminal error).
        response.status = Status::OK;
        response
            .headers
            .insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
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
fn extract_path(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

/// The union of HTTP methods any protocol on this entry accepts (for `Allow`).
fn allowed_methods(entry: &HandlerEntry) -> Vec<SimpleMethod> {
    let mut methods: Vec<SimpleMethod> = Vec::new();
    for protocol in &entry.protocol_handlers {
        for method in protocol.allowed_methods() {
            if !methods.contains(method) {
                methods.push(method.clone());
            }
        }
    }
    methods
}

/// Server-side transport capabilities inferred from the request's HTTP version.
fn capabilities_for(proto: &Proto) -> TransportCapabilities {
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
fn build_ctx(
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
        foundation_netio::simple_http::shared::Extensions::new(),
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
fn require_connect_version(request: &SimpleIncomingRequest) -> ConnectResult<()> {
    use crate::protocol::connect::constants;
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
            .get(&SimpleHeader::from(constants::HEADER_PROTOCOL_VERSION.to_string()))
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

/// Take the request body and collect it into a byte vector. In-memory bodies
/// (`Bytes`/`Text`) return directly; iterator-backed bodies (what the live server
/// hands us for a sized/chunked request) are drained via `try_collect_bytes`.
fn read_body(request: &mut SimpleIncomingRequest) -> Vec<u8> {
    match request.body.take() {
        None | Some(SendSafeBody::None) => Vec::new(),
        Some(SendSafeBody::Bytes(bytes)) => bytes,
        Some(SendSafeBody::Text(text)) => text.into_bytes(),
        Some(other) => try_collect_bytes(other).unwrap_or_default(),
    }
}

/// A blank response carrying the request's HTTP version and no headers/body.
fn blank_response(request: &SimpleIncomingRequest) -> SimpleOutgoingResponse {
    SimpleOutgoingResponse {
        proto: request.proto.clone(),
        status: Status::OK,
        headers: SimpleHeaders::new(),
        body: None,
        trailers: SimpleHeaders::new(),
    }
}

fn status_only(request: &SimpleIncomingRequest, status: Status) -> SimpleOutgoingResponse {
    let mut response = blank_response(request);
    response.status = status;
    response
}

fn method_not_allowed(
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

fn unsupported_media_type(
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

fn http_version_not_supported(request: &SimpleIncomingRequest) -> SimpleOutgoingResponse {
    status_only(request, Status::HttpVersionNotSupported)
}

/// Render an RPC error into `response` via the protocol-aware [`ErrorWriter`].
fn write_error(
    response: &mut SimpleOutgoingResponse,
    request: &SimpleIncomingRequest,
    error: &foundation_errstacks::ErrorTrace<ConnectError>,
) {
    let _ = ErrorWriter::new().write(response, request, error);
}
