//! `ConnectRpcServeH3` — bridges ConnectRPC dispatch to HTTP/3 request streams (F35).
//!
//! WHY: `ConnectRpcServeH2` bridges ConnectRPC to h2 frame pipes. This does the
//! same for HTTP/3, which uses a poll-based `H3Request` API (headers, body chunks,
//! response send) instead of pipe-based h2 frames. The layering mirrors `h2_serve`
//! exactly: the trait lives in `foundation_http`, the impl here.
//!
//! WHAT: [`ConnectRpcServeH3`] wrapping a [`ConnectRpcHandler`].
//!
//! HOW: `serve_h3()` returns a future that drives `H3Request::poll_headers`,
//! `poll_body`, and `poll_send_*` via `std::future::poll_fn`. When the H3 stream
//! is not ready, the future yields (`Poll::Pending`) and the valtron executor
//! re-polls the QUIC driver — which makes network progress — before coming back.
//! Unary requests are fully supported; streaming (server/bidi/client) returns
//! `501 Not Implemented` for now.

use std::future::Future;
use std::io;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use foundation_core::valtron::{Pipe, Stream as VStream};
use foundation_http::native::serve::{BoxFuture, H3Serve};
use foundation_http::shared::context::ContextBag;
use foundation_netio::http3::connection::{H3Error, H3Request};
use foundation_netio::http3::types::{request_from_fields, response_to_fields};
use foundation_netio::netcap::context::ConnectionContext;
use foundation_netio::quic::QuinnBidiStream;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::http::{SendSafeBody, SimpleOutgoingResponse, Status};
use futures::StreamExt;

use crate::shared::context::{Ctx, Spec};
use crate::shared::protocol::{HandlerExchange, ProtocolHandler};
use crate::shared::router::dispatch::{
    self, allowed_methods, blank_response, build_ctx, capabilities_for, extract_path,
    http_version_not_supported, method_not_allowed, request_from_header, run_unary,
    status_only, unsupported_media_type, write_error,
};
use crate::shared::router::{ConnectRpcHandler, HandlerKind};
use crate::shared::transport::capabilities::{requirements, ProtocolKind};
use crate::shared::transport::check_compatible;

/// `H3Serve` impl that dispatches through a ConnectRPC router.
pub struct ConnectRpcServeH3 {
    handler: Arc<ConnectRpcHandler>,
}

impl ConnectRpcServeH3 {
    #[must_use]
    pub fn new(handler: ConnectRpcHandler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }
}

impl H3Serve for ConnectRpcServeH3 {
    fn serve_h3(
        &self,
        bag: Arc<ContextBag>,
        connection: Arc<ConnectionContext>,
        request: H3Request<QuinnBidiStream>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let handler = self.handler.clone();
        Box::pin(dispatch_h3(handler, bag, connection, request))
    }
}

/// Drive one HTTP/3 request through the ConnectRPC router (F35 server half).
/// Drive one HTTP/3 request through the ConnectRPC router (F35 server half).
///
/// `connection` is the connection-scoped context populated from the QUIC
/// handshake — peer address, ALPN, TLS details — built once per connection
/// via [`QuinnConnection::connection_context()`](foundation_netio::quic::QuinnConnection::connection_context)
/// and shared across every request on this connection (F53).
pub async fn dispatch_h3(
    handler: Arc<ConnectRpcHandler>,
    bag: Arc<ContextBag>,
    connection: Arc<ConnectionContext>,
    mut req: H3Request<QuinnBidiStream>,
) -> io::Result<()> {
    // ── 1. Read the request headers ──────────────────────────────────────
    let fields = poll_h3(|| req.poll_headers()).await?;

    let header = request_from_fields(&fields, connection)
        .map_err(|e| io::Error::other(e.to_string()))?;
    let request = request_from_header(&header);
    let path = extract_path(&request.request_url.url);

    // ── 2. Route lookup (same decision sequence as dispatch_h2) ──────────
    let router = handler.router();
    let Some(entry) = router.lookup(&path) else {
        let resp = status_only(&request, Status::NotFound);
        send_h3_response(&mut req, resp).await?;
        return Ok(());
    };

    let allowed = allowed_methods(entry);
    if !allowed.iter().any(|m| *m == request.method) {
        let resp = method_not_allowed(&request, &allowed);
        send_h3_response(&mut req, resp).await?;
        return Ok(());
    }

    let Some(protocol) = entry
        .protocol_handlers
        .iter()
        .find(|p| p.can_handle(&request))
    else {
        let resp = unsupported_media_type(&request, entry);
        send_h3_response(&mut req, resp).await?;
        return Ok(());
    };
    let protocol = protocol.as_ref();

    let Some(codec_name) = protocol.codec_name(&request) else {
        let resp = unsupported_media_type(&request, entry);
        send_h3_response(&mut req, resp).await?;
        return Ok(());
    };
    if !entry.codec_meta.has_codec(&codec_name) {
        let resp = unsupported_media_type(&request, entry);
        send_h3_response(&mut req, resp).await?;
        return Ok(());
    }

    let caps = capabilities_for(&request.proto);
    let reqs = requirements(protocol.kind(), entry.spec.stream_type);
    if check_compatible(&reqs, &caps).is_err() {
        let resp = http_version_not_supported(&request);
        send_h3_response(&mut req, resp).await?;
        return Ok(());
    }

    if entry.options.require_connect_protocol_header && protocol.kind() == ProtocolKind::Connect {
        if let Err(e) = dispatch::require_connect_version(&request) {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            send_h3_response(&mut req, response).await?;
            return Ok(());
        }
    }

    let compression = entry
        .options
        .compression
        .as_ref()
        .unwrap_or_else(|| router.global_compression());
    let spec = entry.spec.clone();
    let ctx = build_ctx(bag, &request, spec.clone(), protocol);

    // ── 3. Collect body and dispatch ─────────────────────────────────────
    match &entry.handler {
        HandlerKind::Unary(handler_fn) => {
            let body_bytes = collect_h3_body(&mut req).await?;
            let mut request = request;
            request.body = Some(send_safe_bytes(body_bytes));

            let response = run_unary(
                handler_fn.clone(),
                protocol,
                request,
                codec_name,
                compression,
                ctx,
                entry.options.limits.read_max_bytes,
            )
            .await;
            send_h3_response(&mut req, response).await
        }
        HandlerKind::ServerStream(handler)
        | HandlerKind::ClientStream(handler)
        | HandlerKind::BidiStream(handler) => {
            run_streaming_h3(
                handler.clone(),
                protocol,
                request,
                spec,
                codec_name,
                compression,
                ctx,
                entry.options.pipe_depth,
                req,
            )
            .await
        }
    }
}

// ── Streaming H3 dispatch ────────────────────────────────────────────────

/// Streaming over H3. Body chunks from `poll_body()` are drained into the
/// protocol's byte pipe first (H3Request uses `&mut self` so body and response
/// cannot be polled concurrently in a join!); then handler + protocol pump run
/// concurrently with `futures::join!`. Server-stream and client-stream work;
/// true bidi interleaving needs a split of the underlying QUIC stream.
#[allow(clippy::too_many_arguments)]
async fn run_streaming_h3(
    handler: Arc<dyn crate::shared::router::erased::ErasedStreamHandler>,
    protocol: &dyn ProtocolHandler,
    request: foundation_netio::shared::http::SimpleIncomingRequest,
    spec: Spec,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    pipe_depth: usize,
    mut req: H3Request<QuinnBidiStream>,
) -> io::Result<()> {
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(pipe_depth);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(pipe_depth);

    // Drain body chunks into req_tx before the join — H3Request borrows &mut
    // so we can't interleave poll_body and poll_send_*.
    loop {
        match poll_h3(|| req.poll_body()).await {
            Ok(Some(chunk)) => {
                let _ = req_tx.send(chunk).await;
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    req_tx.close();

    let exchange = match protocol.new_conn(&request, spec, req_rx, resp_tx, compression) {
        Ok(exchange) => exchange,
        Err(e) => {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return send_h3_response(&mut req, response).await;
        }
    };
    let HandlerExchange {
        conn,
        reader_task,
        writer_task,
    } = exchange;

    // Pump protocol output into H3 DATA frames.
    let content_type = protocol.streaming_response_content_type(&request, &codec_name);
    let pump = async {
        let mut opened = false;
        while let Some(chunk) = resp_rx.receive().await {
            if !opened {
                opened = true;
                let headers = vec![(
                    Bytes::from_static(b"content-type"),
                    Bytes::from(content_type.clone()),
                )];
                let mut header_frame = H3Request::<QuinnBidiStream>::encode_headers(&headers);
                poll_h3(|| req.poll_send_headers(&mut header_frame)).await?;
            }
            let mut data_frame = H3Request::<QuinnBidiStream>::encode_data(chunk);
            poll_h3(|| req.poll_send_data(&mut data_frame)).await?;
        }
        Ok::<bool, io::Error>(opened)
    };

    let handler_fut = handler.handle(ctx, &codec_name, conn);

    let (reader_res, writer_res, handler_res, pumped) =
        futures::join!(reader_task, writer_task, handler_fut, pump);

    if pumped? {
        poll_h3_finish(&mut req).await?;
        return Ok(());
    }

    let mut response = blank_response(&request);
    if let Some(e) = handler_res.err().or(writer_res.err()).or(reader_res.err()) {
        write_error(&mut response, &request, &e);
    }
    send_h3_response(&mut req, response).await
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Convert `Bytes` into a `SendSafeBody`.
fn send_safe_bytes(b: Bytes) -> SendSafeBody {
    SendSafeBody::Bytes(b.to_vec())
}

/// Poll an `H3Request` method until it yields `Next(Ok(v))`, converting
/// `Stream` to `std::task::Poll`.
fn poll_h3<T>(
    mut f: impl FnMut() -> VStream<Result<T, H3Error>, ()>,
) -> impl Future<Output = io::Result<T>> {
    std::future::poll_fn(move |_cx: &mut Context<'_>| match f() {
        VStream::Next(Ok(v)) => Poll::Ready(Ok(v)),
        VStream::Next(Err(e)) => Poll::Ready(Err(io::Error::other(e.to_string()))),
        _ => Poll::Pending,
    })
}

/// Collect the full request body.
async fn collect_h3_body(req: &mut H3Request<QuinnBidiStream>) -> io::Result<Bytes> {
    let mut body = Bytes::new();
    loop {
        match poll_h3(|| req.poll_body()).await? {
            Some(chunk) => {
                if body.is_empty() {
                    body = chunk;
                } else {
                    let mut merged = bytes::BytesMut::with_capacity(body.len() + chunk.len());
                    merged.extend_from_slice(&body);
                    merged.extend_from_slice(&chunk);
                    body = merged.freeze();
                }
            }
            None => return Ok(body),
        }
    }
}

/// Encode a response and send it on the H3 request stream.
///
/// Owns the response. Headers extracted synchronously; body streamed
/// chunk-by-chunk via `AsyncSendSafeBody` with no buffering.
async fn send_h3_response(
    req: &mut H3Request<QuinnBidiStream>,
    mut response: SimpleOutgoingResponse,
) -> io::Result<()> {
    let response_fields = response_to_fields(&response);
    let body = response.body.take();
    // response borrow done.

    let mut header_frame = H3Request::<QuinnBidiStream>::encode_headers(&response_fields);
    poll_h3(|| req.poll_send_headers(&mut header_frame)).await?;

    let stream = body
        .filter(|b| !matches!(b, SendSafeBody::None))
        .map(AsyncSendSafeBody::from);

    if let Some(mut stream) = stream {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| io::Error::other(e.to_string()))?;
            let mut data_frame = H3Request::<QuinnBidiStream>::encode_data(Bytes::from(chunk));
            poll_h3(|| req.poll_send_data(&mut data_frame)).await?;
        }
    }

    poll_h3_finish(req).await
}

/// Poll `poll_finish` until it completes. Specialised because the finish
/// returns `Stream<Result<(), H3Error>, ()>`.
fn poll_h3_finish(
    req: &mut H3Request<QuinnBidiStream>,
) -> impl Future<Output = io::Result<()>> + '_ {
    std::future::poll_fn(move |_cx: &mut Context<'_>| match req.poll_finish() {
        VStream::Next(Ok(())) => Poll::Ready(Ok(())),
        VStream::Next(Err(e)) => Poll::Ready(Err(io::Error::other(e.to_string()))),
        _ => Poll::Pending,
    })
}
