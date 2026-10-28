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
//! `poll_body`, and `poll_send_*` via `std::future::poll_fn`. QUIC streams have
//! no waker registration (`quinn-proto` is sans-IO and the endpoint driver task
//! moves the datagrams), so a not-ready H3 poll wakes its own task before
//! yielding: valtron then requeues it behind the driver instead of parking it on
//! a wake that would never come.
//!
//! The request body of a streaming procedure is never collected: each DATA frame
//! is forwarded into the protocol's bounded request pipe as it arrives, while the
//! handler and the response pump run concurrently on the same stream. A unary
//! body is collected under the procedure's `read_max_bytes` cap; the first chunk
//! past it ends the read with a `413`.

use std::future::Future;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use foundation_core::valtron::{Pipe, Stream as VStream};
use foundation_http::native::serve::{BoxFuture, H3Serve};
use foundation_http::shared::context::ContextBag;
use foundation_netio::http3::connection::{error_code, H3Error, H3Request};
use foundation_netio::http3::types::{request_from_fields, response_to_fields};
use foundation_netio::netcap::context::ConnectionContext;
use foundation_netio::quic::QuinnBidiStream;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse, Status,
};
use futures::StreamExt;

use crate::shared::compression::CompressionRegistry;
use crate::shared::context::{CancelSignal, Ctx, Spec};
use crate::shared::protocol::{ExchangeConfig, HandlerExchange, ProtocolHandler};
use crate::shared::router::dispatch::{
    self, allowed_methods, blank_response, build_ctx, capabilities_for, deadline_exceeded,
    exchange_until_aborted, extract_path, grpc_status_trailers, h2_headers,
    http_version_not_supported, method_not_allowed,
    request_from_header, run_unary, status_only, stream_error_frame, stream_failure,
    unclosed_stream_error, unsupported_media_type, write_error, Abort, StreamEnd, ThreadTimer,
};
use crate::shared::router::erased::ErasedStreamHandler;
use crate::shared::router::{ConnectRpcHandler, HandlerKind};
use crate::shared::transport::capabilities::{requirements, ProtocolKind};
use crate::shared::transport::check_compatible;

type Req = H3Request<QuinnBidiStream>;

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
///
/// `connection` is the connection-scoped context populated from the QUIC
/// handshake — peer address, ALPN, TLS details — built once per connection
/// via [`QuinnConnection::connection_context()`](foundation_netio::quic::QuinnConnection::connection_context)
/// and shared across every request on this connection (F53).
///
/// # Errors
/// The stream failed: the peer reset it, the connection went away, or the
/// HTTP/3 framing was invalid. A request body that fails mid-read cancels the
/// call's [`Ctx`] and resets our response half with `H3_REQUEST_CANCELLED`.
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
    if !allowed.contains(&request.method) {
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
    let ctx = build_ctx(bag, &request, spec.clone(), protocol, CancelSignal::new());

    // ── 3. Dispatch ──────────────────────────────────────────────────────
    match &entry.handler {
        HandlerKind::Unary(handler_fn) => {
            // Unary carries exactly one message: collect it, but never past the
            // cap — the first chunk over `read_max_bytes` ends the read.
            let read_max_bytes = entry.options.limits.read_max_bytes;
            let body = match collect_h3_body(&mut req, read_max_bytes).await {
                Ok(CollectedBody::Complete(body)) => body,
                Ok(CollectedBody::TooLarge { read }) => {
                    tracing::warn!(
                        read,
                        read_max_bytes,
                        "h3 unary request body exceeds read_max_bytes; returning 413"
                    );
                    // The answer does not depend on the rest of the body: ask
                    // the peer to stop sending it (RFC 9114 §4.1.1).
                    req.stop_sending(error_code::H3_NO_ERROR);
                    let resp = status_only(&request, Status::PayloadTooLarge);
                    return send_h3_response(&mut req, resp).await;
                }
                Err(e) => return Err(abort_call(&mut req, ctx.cancel_signal(), &e)),
            };
            let mut request = request;
            request.body = Some(SendSafeBody::Bytes(body));

            // The deadline bounds the handler; the body read above is bounded
            // by the transport's own idle timeout.
            let cancel = ctx.cancel_signal().clone();
            let response = run_unary(
                handler_fn.clone(),
                protocol,
                request,
                codec_name,
                compression,
                ctx,
                read_max_bytes,
                Some(&ThreadTimer),
            )
            .await;
            let sent = send_h3_response(&mut req, response).await;
            if sent.is_err() {
                // The peer is gone; anything the handler left running stops.
                cancel.cancel();
            }
            sent
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
                entry.options.limits.read_max_bytes,
                req,
            )
            .await
        }
    }
}

// ── Streaming H3 dispatch ────────────────────────────────────────────────

/// Streaming over HTTP/3 — the h3 twin of `run_streaming_h2`.
///
/// Request DATA frames feed the protocol's bounded byte pipe as they arrive
/// while the protocol's output drains into response DATA frames, concurrently:
/// a client-stream handler sees each message as it lands, and a bidi handler
/// can read and write at once.
///
/// Receive and send are the two halves of the one QUIC stream behind the one
/// `H3Request`. Both sides of the exchange run inside this single future, so
/// they share it through a `Mutex` held only for one synchronous poll — never
/// across an `.await` — which keeps it uncontended.
#[allow(clippy::too_many_arguments)]
async fn run_streaming_h3(
    handler: Arc<dyn ErasedStreamHandler>,
    protocol: &dyn ProtocolHandler,
    request: SimpleIncomingRequest,
    spec: Spec,
    codec_name: String,
    compression: &CompressionRegistry,
    ctx: Ctx,
    pipe_depth: usize,
    read_max_bytes: usize,
    mut req: Req,
) -> io::Result<()> {
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(pipe_depth);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(pipe_depth);

    let cancel = ctx.cancel_signal().clone();
    let deadline = ctx.request.deadline;
    let config = ExchangeConfig {
        compression,
        cancel: cancel.clone(),
        read_max_bytes,
    };
    let exchange = match protocol.new_conn(&request, spec, req_rx, resp_tx, config) {
        Ok(exchange) => exchange,
        Err(e) => {
            // Reporting this needs none of the body; stop the peer sending it.
            req.stop_sending(error_code::H3_NO_ERROR);
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

    let shared = Mutex::new(req);
    let body_done = AtomicBool::new(false);

    // Forward framed response bytes out as DATA frames. HEADERS are withheld
    // until the first chunk so that a handler which fails before emitting
    // anything can still be reported with a real status.
    let content_type = protocol.streaming_response_content_type(&request, &codec_name);
    // The writer task compressed per this same negotiation — a flagged envelope
    // is only valid when the encoding header goes out with the response HEADERS.
    let response_encoding = protocol.streaming_response_encoding(&request, compression);
    let pump_content_type = content_type.clone();
    let handler_fut = handler.handle(ctx, &codec_name, conn);

    // As over h2: the request side lives as long as the CLIENT keeps its request
    // stream open, and a bidi peer may hold it open until it sees our response
    // end — joining on it first would deadlock. The response side decides the
    // outcome; a failed request read (a peer reset) or the deadline aborts the
    // call. Scoped so every borrow of `shared` ends before it is taken back.
    let pumped = {
        // Forward inbound DATA frames as they arrive — no buffering of the
        // body. `send` parks while the pipe is full, which stops us reading
        // and lets QUIC flow control push back on the peer. A failed read is
        // NOT end-of-body: it is returned so the call is aborted, instead of
        // the handler mistaking a truncated request for a complete one.
        let feeder = async {
            while let Some(chunk) = poll_shared(&shared, H3Request::poll_body).await? {
                if req_tx.send(chunk).await.is_err() {
                    // The protocol reader is gone and will never read the rest;
                    // the response side decides the outcome.
                    tracing::debug!("h3 request pipe closed by its reader; abandoning the body");
                    req_tx.close();
                    return Ok(());
                }
            }
            body_done.store(true, Ordering::Release);
            req_tx.close();
            Ok::<(), io::Error>(())
        };
        let request_side =
            futures::future::try_join(feeder, async { Ok::<_, io::Error>(reader_task.await) });
        let response_side = futures::future::join(writer_task, handler_fut);

        // The pump owns `resp_rx`, so when it stops the writer task's sends
        // fail instead of waiting on a full pipe forever.
        let pump = async {
            let resp_rx = resp_rx;
            let mut opened = false;
            while let Some(chunk) = resp_rx.receive().await {
                if !opened {
                    opened = true;
                    let mut headers = vec![
                        (Bytes::from_static(b":status"), Bytes::from_static(b"200")),
                        (
                            Bytes::from_static(b"content-type"),
                            Bytes::from(pump_content_type.clone()),
                        ),
                    ];
                    if let Some((name, enc)) = &response_encoding {
                        headers.push((Bytes::from(name.clone()), Bytes::from(enc.clone())));
                    }
                    let mut header_frame = Req::encode_headers(&headers);
                    poll_shared(&shared, |r| r.poll_send_headers(&mut header_frame)).await?;
                }
                let mut data_frame = Req::encode_data(chunk);
                poll_shared(&shared, |r| r.poll_send_data(&mut data_frame)).await?;
            }
            Ok::<bool, io::Error>(opened)
        };

        let rest = std::pin::pin!(exchange_until_aborted(
            request_side,
            response_side,
            dispatch::deadline_expiry(Some(&ThreadTimer), deadline),
            // A reset of our response half shows up as a failed send in the
            // pump; QUIC offers no separate signal to wait on.
            std::future::pending::<()>(),
            &cancel,
        ));
        let pump = std::pin::pin!(pump);
        match futures::future::select(pump, rest).await {
            futures::future::Either::Left((Err(e), rest)) => {
                // The response could not be written: the peer is gone.
                dispatch::cancel_and_settle(&cancel, rest);
                Err(e)
            }
            futures::future::Either::Left((Ok(opened), rest)) => Ok((opened, rest.await)),
            // The writer has finished or been dropped, which ends the pump.
            futures::future::Either::Right((end, pump)) => pump.await.map(|opened| (opened, end)),
        }
    };

    let mut req = shared
        .into_inner()
        .map_err(|_| io::Error::other("h3 request lock poisoned"))?;
    let (opened, end) = match pumped {
        Ok(pumped) => pumped,
        Err(e) => {
            // The response could not be written: the peer is gone.
            cancel.cancel();
            return Err(e);
        }
    };

    let is_grpc = protocol.kind() == ProtocolKind::Grpc;
    let failure = match end {
        StreamEnd::Aborted(Abort::BodyFailed(e)) => {
            return Err(abort_call(&mut req, &cancel, &e));
        }
        StreamEnd::Aborted(Abort::PeerGone) => {
            cancel.cancel();
            return Ok(());
        }
        StreamEnd::Aborted(Abort::Deadline) => {
            cancel.cancel();
            Some(deadline_exceeded())
        }
        // An opened Connect / gRPC-Web stream framed its own outcome unless the
        // call failed without closing it.
        StreamEnd::Done {
            writer, handler, ..
        } if opened && !is_grpc => unclosed_stream_error(writer, handler),
        StreamEnd::Done {
            reader,
            writer,
            handler,
        } => stream_failure(reader, writer, handler),
    };

    if !body_done.load(Ordering::Acquire) {
        // The response is settled without the rest of the request: stop the
        // peer sending it (RFC 9114 §4.1.1).
        req.stop_sending(error_code::H3_NO_ERROR);
    }

    if opened {
        if is_grpc {
            // gRPC's outcome rides trailing HEADERS (`grpc-status`); a stream
            // that ends without them reads as `Unknown` to every gRPC client.
            let mut trailer_frame = Req::encode_headers(&grpc_status_trailers(failure.as_ref()));
            poll_h3(|| req.poll_send_headers(&mut trailer_frame)).await?;
        } else if let Some(frame) = failure
            .as_ref()
            .and_then(|e| stream_error_frame(protocol, &request, e))
        {
            let mut data_frame = Req::encode_data(frame);
            poll_h3(|| req.poll_send_data(&mut data_frame)).await?;
        }
        poll_h3_finish(&mut req).await?;
        return Ok(());
    }

    // Nothing was emitted: report the failure, or an empty successful stream.
    let mut response = blank_response(&request);
    if let Some(e) = failure {
        write_error(&mut response, &request, &e);
    } else {
        response.status = Status::OK;
        response
            .headers
            .insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
        if is_grpc {
            // A gRPC stream with no messages still ends in `grpc-status`.
            response
                .trailers
                .insert(SimpleHeader::from("grpc-status".to_string()), vec!["0".to_string()]);
        }
    }
    send_h3_response(&mut req, response).await
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// The request body failed mid-read (peer reset, broken framing, connection
/// gone): cancel the call, reset our response half, and return the error.
fn abort_call(req: &mut Req, cancel: &CancelSignal, error: &io::Error) -> io::Error {
    tracing::debug!(error = %error, "h3 request body failed; cancelling the call");
    cancel.cancel();
    req.reset(error_code::H3_REQUEST_CANCELLED);
    io::Error::new(
        io::ErrorKind::ConnectionAborted,
        format!("h3 request aborted: {error}"),
    )
}

/// Map one H3 poll result onto `std::task::Poll`.
///
/// QUIC streams register no waker: bytes arrive when the endpoint driver task
/// moves datagrams. A not-ready poll therefore wakes its own task before
/// yielding, so valtron's `FutureTask` requeues it (`TaskStatus::Ignore`) behind
/// the driver rather than parking it (`Depends`) on a wake that never comes.
fn map_h3<T>(cx: &Context<'_>, polled: VStream<Result<T, H3Error>, ()>) -> Poll<io::Result<T>> {
    match polled {
        VStream::Next(Ok(v)) => Poll::Ready(Ok(v)),
        VStream::Next(Err(e)) => Poll::Ready(Err(io::Error::other(e.to_string()))),
        _ => {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Poll an `H3Request` method until it yields `Next(..)`.
fn poll_h3<T>(
    mut f: impl FnMut() -> VStream<Result<T, H3Error>, ()>,
) -> impl Future<Output = io::Result<T>> {
    std::future::poll_fn(move |cx: &mut Context<'_>| map_h3(cx, f()))
}

/// [`poll_h3`] over the request shared by the body feeder and the response
/// pump. The lock is taken for one synchronous poll and released before
/// yielding, so it is never held across an `.await`.
fn poll_shared<'a, T>(
    shared: &'a Mutex<Req>,
    mut f: impl FnMut(&mut Req) -> VStream<Result<T, H3Error>, ()> + 'a,
) -> impl Future<Output = io::Result<T>> + 'a {
    std::future::poll_fn(move |cx: &mut Context<'_>| {
        let polled = match shared.lock() {
            Ok(mut req) => f(&mut req),
            Err(_) => return Poll::Ready(Err(io::Error::other("h3 request lock poisoned"))),
        };
        map_h3(cx, polled)
    })
}

/// The outcome of collecting a unary request body.
enum CollectedBody {
    /// The whole body, within the cap.
    Complete(Vec<u8>),
    /// The body passed the cap at `read` bytes; reading stopped there.
    TooLarge { read: usize },
}

/// Collect a unary request body, stopping at the first chunk that takes it past
/// `max_bytes` (`0` = unlimited, as for `SizeLimits::read_max_bytes`).
async fn collect_h3_body(req: &mut Req, max_bytes: usize) -> io::Result<CollectedBody> {
    let mut body = Vec::new();
    while let Some(chunk) = poll_h3(|| req.poll_body()).await? {
        let read = body.len() + chunk.len();
        if max_bytes > 0 && read > max_bytes {
            return Ok(CollectedBody::TooLarge { read });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(CollectedBody::Complete(body))
}

/// Encode a response and send it on the H3 request stream.
///
/// Owns the response. Headers extracted synchronously; body streamed
/// chunk-by-chunk via `AsyncSendSafeBody` with no buffering.
async fn send_h3_response(req: &mut Req, mut response: SimpleOutgoingResponse) -> io::Result<()> {
    let response_fields = response_to_fields(&response);
    let body = response.body.take();
    // gRPC (unary, Trailers-Only, or an empty stream) carries its status here.
    let trailers = std::mem::take(&mut response.trailers);
    // response borrow done.

    let mut header_frame = Req::encode_headers(&response_fields);
    poll_h3(|| req.poll_send_headers(&mut header_frame)).await?;

    let stream = body
        .filter(|b| !matches!(b, SendSafeBody::None))
        .map(AsyncSendSafeBody::from);

    if let Some(mut stream) = stream {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| io::Error::other(e.to_string()))?;
            let mut data_frame = Req::encode_data(Bytes::from(chunk));
            poll_h3(|| req.poll_send_data(&mut data_frame)).await?;
        }
    }

    if !trailers.is_empty() {
        // The trailer section is a HEADERS frame after the body (RFC 9114 §4.1).
        let mut trailer_frame = Req::encode_headers(&h2_headers(&trailers));
        poll_h3(|| req.poll_send_headers(&mut trailer_frame)).await?;
    }

    poll_h3_finish(req).await
}

/// Poll `poll_finish` until the FIN is queued.
fn poll_h3_finish(req: &mut Req) -> impl Future<Output = io::Result<()>> + '_ {
    poll_h3(move || req.poll_finish())
}
