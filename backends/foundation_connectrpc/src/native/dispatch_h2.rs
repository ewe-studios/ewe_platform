//! The HTTP/2 server dispatch path (native only).
//!
//! WHY: this code drives `foundation_netio::http2` (`H2Frame`,
//! `H2IncomingFrame`, `SimpleIncomingRequestHeader`) and valtron pipes, so it
//! cannot compile on wasm. It used to live in `shared/router/dispatch.rs`
//! behind nineteen `cfg(not(target_family = "wasm"))` gates; moving it here
//! lets that module stay target-agnostic and serve HTTP/1.1 on every target.
//!
//! WHAT: [`ConnectRpcHandler::dispatch_h2`] and the frame-level helpers it uses
//! to bridge one h2 stream through the router.
//!
//! HOW: the decision phase mirrors `dispatch_request`, but every rejection is
//! serialized as h2 frames. Request DATA frames feed the protocol's byte pipe
//! as they arrive while response frames are pushed out as they are produced,
//! so a streaming body is never held whole in memory.

use std::io;
use std::sync::Arc;

use bytes::Bytes;
use foundation_core::valtron::{Pipe, PipeReceiver, PipeSender, Stream};
use foundation_http::shared::context::Ctx;
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest,
    SimpleOutgoingResponse, Status,
};
use futures::future::{select, Either};

use crate::shared::compression::CompressionRegistry;
use crate::shared::context::{CancelSignal, RpcCtx, Spec};
use crate::shared::protocol::{ExchangeConfig, HandlerExchange, ProtocolHandler};
use crate::shared::router::dispatch::{
    allowed_methods, blank_response, build_ctx, cancel_and_settle, capabilities_for,
    deadline_exceeded, deadline_expiry, deadline_response, exchange_until_aborted, extract_path,
    http_version_not_supported, in_memory_body, is_framing_header, merge_handler_headers,
    method_not_allowed, protocol_encoding_header, require_connect_version,
    run_unary_until_deadline, status_only, stream_error_frame, stream_failure,
    take_handler_headers, unclosed_stream_error, unsupported_media_type, write_error, Abort,
    StreamEnd,
};
use crate::shared::router::erased::{ErasedStreamHandler, ErasedUnaryHandler};
use crate::shared::router::{ConnectRpcHandler, DeadlineTimer, HandlerKind, Router};
use crate::shared::transport::{check_compatible, requirements, BoxFuture, ProtocolKind};

/// The [`DeadlineTimer`](crate::shared::router::DeadlineTimer) for connection owners that
/// run dispatch on valtron (HTTP/2, HTTP/3).
pub(crate) use crate::native::deadline_timer::ThreadTimer;

/// Unary over a request whose body is already in `request.body` (h2, h3),
/// bounded by the call's deadline when `timer` is given. See
/// [`run_unary_until_deadline`].
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_unary(
    handler: Arc<dyn ErasedUnaryHandler>,
    protocol: &dyn ProtocolHandler,
    mut request: SimpleIncomingRequest,
    codec_name: String,
    compression: &CompressionRegistry,
    ctx: RpcCtx,
    read_max_bytes: usize,
    timer: Option<&dyn DeadlineTimer>,
) -> SimpleOutgoingResponse {
    let body = in_memory_body(request.body.take());
    run_unary_until_deadline(
        handler,
        protocol,
        request,
        body,
        codec_name,
        compression,
        ctx,
        read_max_bytes,
        timer,
    )
    .await
}

/// Append the handler's headers to a wire header list (h2 and h3).
///
/// One entry is added for each value. The same framing rule as
/// [`merge_handler_headers`] applies.
pub(crate) fn extend_handler_headers(
    headers: &mut Vec<(Bytes, Bytes)>,
    handler: SimpleHeaders,
    encoding_header: &str,
) {
    for (name, values) in handler {
        if is_framing_header(&name, encoding_header) {
            tracing::warn!(
                header = %name,
                "handler response header discarded: the protocol sets this framing header"
            );
            continue;
        }
        let wire = name.to_string();
        for value in values {
            headers.push((Bytes::from(wire.clone()), Bytes::from(value)));
        }
    }
}

/// Rebuild the request shell from an h2 HEADERS frame. The body is supplied
/// separately (a pipe), so it stays `None` here.
pub(crate) fn request_from_header(header: &SimpleIncomingRequestHeader) -> SimpleIncomingRequest {
    SimpleIncomingRequest {
        proto: Proto::HTTP20,
        request_uri: header.uri.clone(),
        request_url: header.url.clone(),
        body: None,
        body_readiness: None,
        headers: header.headers.clone(),
        method: header.method.clone(),
        extensions: None,
        connection: header.connection.clone(),
    }
}

/// Headers that are connection-specific and forbidden on h2 (RFC 7540 §8.1.2.2).
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
pub(crate) fn h2_headers(headers: &SimpleHeaders) -> Vec<(Bytes, Bytes)> {
    let mut out = Vec::new();
    for (key, values) in headers {
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
fn stream_gone<T>(_: T) -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "h2 response stream closed")
}

/// Serialize a complete (non-streaming) response: HEADERS, then one DATA frame
/// per body chunk, then optional trailing HEADERS.
// The HTTP status value fits in u16; the enum defines only such values.
#[allow(
    clippy::cast_possible_truncation,
    clippy::items_after_statements,
    reason = "the local helper keeps the write order in one place"
)]
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
                // The body's transport has nothing yet: this writer cannot wait
                // for it, so report it rather than spin.
                Some(Stream::Wait) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "response body not ready",
                    ));
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

/// What reading an h2 unary request body produced.
enum H2Body {
    Collected(Vec<u8>),
    /// The peer reset the stream, or the connection dropped it.
    Reset,
    /// The body passed `read_max_bytes`; carries the size read so far.
    TooLarge(usize),
}

/// The next request DATA chunk of an h2 stream: `Ok(None)` once the peer set
/// `END_STREAM`, and an error when the peer reset the stream or the
/// connection dropped it — never a clean end.
///
/// The connection closes the stream's response pipe (`tx`) before its body
/// pipe when it drops a stream, so a body pipe that closes under a closed
/// response pipe is a cancelled stream, not `END_STREAM`.
async fn next_h2_chunk(
    body: &PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
) -> io::Result<Option<Bytes>> {
    match body.receive().await {
        Some(H2IncomingFrame::Data(chunk)) => Ok(Some(chunk)),
        Some(H2IncomingFrame::Reset(code)) => Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            format!("the peer reset the h2 stream ({code:?})"),
        )),
        None if tx.is_closed() => Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "the h2 stream was dropped before its request ended",
        )),
        None => Ok(None),
    }
}

/// Drain the inbound h2 body pipe into a buffer (unary: exactly one message),
/// stopping as soon as it passes `max_bytes` (`0` means unlimited) so an
/// oversized body is never held whole.
///
/// A peer `RST_STREAM` (or a dropped stream) ends collection early; the caller
/// abandons the stream.
async fn collect_h2_body(
    body: &PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
    max_bytes: usize,
) -> H2Body {
    let mut out = Vec::new();
    loop {
        match next_h2_chunk(body, tx).await {
            Ok(Some(chunk)) => {
                out.extend_from_slice(&chunk);
                if max_bytes > 0 && out.len() > max_bytes {
                    return H2Body::TooLarge(out.len());
                }
            }
            Ok(None) => return H2Body::Collected(out),
            Err(e) => {
                tracing::debug!(error = %e, "h2 unary request abandoned");
                return H2Body::Reset;
            }
        }
    }
}

impl ConnectRpcHandler {
    /// HTTP/2 dispatch: bridge one h2 stream through the router.
    ///
    /// Unlike [`dispatch`](Self::dispatch) this never materialises a
    /// `SimpleOutgoingResponse` for streaming procedures — response frames are
    /// pushed into `tx` as the handler produces them, and request DATA frames
    /// are forwarded into the protocol's byte pipe as they arrive.
    ///
    /// The call is cancelled — its `RpcCtx` fires and the handler is dropped — when
    /// the peer resets the stream, the connection drops it (`tx` closes), or
    /// the call's deadline passes (answered with `deadline_exceeded`).
    pub fn dispatch_h2(
        &self,
        app_ctx: Ctx,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let router = self.router.clone();
        Box::pin(dispatch_h2_stream(router, app_ctx, header, body, tx))
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the h2 stream lifecycle reads best as one ordered function"
)]
async fn dispatch_h2_stream(
    router: Arc<Router>,
    app_ctx: Ctx,
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
    let ctx = build_ctx(
        app_ctx,
        &request,
        entry.spec.clone(),
        protocol,
        CancelSignal::new(),
    );
    let spec = entry.spec.clone();
    let pipe_depth = entry.options.pipe_depth;

    match &entry.handler {
        HandlerKind::Unary(handler) => {
            // Unary carries exactly one message: collect it, then reuse the
            // sequential unary path.
            let read_max_bytes = entry.options.limits.read_max_bytes;
            let cancel = ctx.cancel_signal().clone();
            let deadline = ctx.request.deadline;
            let timer_ctx = ctx.ctx.clone();
            let timed_out = deadline.map(|_| deadline_response(&request));
            let too_large = status_only(&request, Status::PayloadTooLarge);
            let handler = handler.clone();
            let (body, tx_ref) = (&body, &tx);
            let call = std::pin::pin!(async move {
                let collected = match collect_h2_body(body, tx_ref, read_max_bytes).await {
                    H2Body::Collected(collected) => collected,
                    // The peer reset the stream — nothing to answer.
                    H2Body::Reset => return None,
                    H2Body::TooLarge(len) => {
                        tracing::warn!(
                            len,
                            read_max_bytes,
                            "request body exceeded read_max_bytes; returning 413"
                        );
                        return Some(too_large);
                    }
                };
                let mut request = request;
                request.body = Some(SendSafeBody::Bytes(collected));
                Some(
                    run_unary(
                        handler,
                        protocol,
                        request,
                        codec_name,
                        compression,
                        ctx,
                        read_max_bytes,
                        // The deadline is enforced below, over the body read too.
                        None,
                    )
                    .await,
                )
            });
            let expiry = std::pin::pin!(deadline_expiry(Some(&ThreadTimer), deadline, &timer_ctx));
            let gone = std::pin::pin!(tx.closed());
            let cut_short = select(expiry, gone);
            match select(call, cut_short).await {
                Either::Left((Some(response), _)) => send_whole_response(&tx, response).await,
                Either::Left((None, _)) | Either::Right((Either::Right(_), _)) => {
                    // Reset by the peer, or dropped by the connection.
                    cancel.cancel();
                    Ok(())
                }
                Either::Right((Either::Left(_), _call)) => {
                    cancel.cancel();
                    match timed_out {
                        Some(response) => send_whole_response(&tx, response).await,
                        None => Ok(()),
                    }
                }
            }
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
                entry.options.limits.read_max_bytes,
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
///
/// A peer reset, the connection dropping the stream, or a failed write cancels
/// the call and drops the handler; nothing more is sent. The call's deadline
/// passing does the same, but the call is still answered with
/// `deadline_exceeded`.
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "the h2 stream lifecycle and its trailer encoding share one state machine"
)]
#[allow(
    clippy::format_push_string,
    reason = "the percent-encoded trailer string is short; `format!` reads clearer"
)]
async fn run_streaming_h2(
    handler: Arc<dyn ErasedStreamHandler>,
    protocol: &dyn ProtocolHandler,
    request: SimpleIncomingRequest,
    spec: Spec,
    codec_name: String,
    compression: &CompressionRegistry,
    ctx: RpcCtx,
    pipe_depth: usize,
    read_max_bytes: usize,
    body: PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
) -> io::Result<()> {
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(pipe_depth);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(pipe_depth);

    let cancel = ctx.cancel_signal().clone();
    let deadline = ctx.request.deadline;
    let timer_ctx = ctx.ctx.clone();
    let config = ExchangeConfig {
        compression,
        cancel: cancel.clone(),
        read_max_bytes,
    };
    let exchange = match protocol.new_conn(&request, spec, req_rx, resp_tx, config) {
        Ok(exchange) => exchange,
        Err(e) => {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return send_whole_response(tx, response).await;
        }
    };
    let HandlerExchange {
        conn,
        reader_task,
        writer_task,
        response_headers,
    } = exchange;

    // Forward inbound DATA frames as they arrive — no buffering of the body. A
    // reset is not an end of the body: it aborts the call.
    let feeder = async {
        while let Some(chunk) = next_h2_chunk(&body, tx).await? {
            if req_tx.send(chunk).await.is_err() {
                break;
            }
        }
        let _ = req_tx.close();
        Ok::<(), io::Error>(())
    };

    // Forward framed response bytes out as DATA frames. HEADERS are withheld
    // until the first chunk so that a handler which fails before emitting
    // anything can still be reported with a real status. The pump owns
    // `resp_rx`, so when it stops the writer task's sends fail instead of
    // waiting on a full pipe forever.
    let content_type = protocol.streaming_response_content_type(&request, &codec_name);
    // The writer task compressed per this same negotiation — a flagged envelope
    // is only valid when the encoding header goes out with the response HEADERS.
    let response_encoding = protocol.streaming_response_encoding(&request, compression);
    let encoding_header = protocol_encoding_header(protocol);
    let pump_content_type = content_type.clone();
    let pump_headers = Arc::clone(&response_headers);
    let pump = async {
        let resp_rx = resp_rx;
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
                let handler = take_handler_headers(&pump_headers);
                extend_handler_headers(&mut headers, handler, encoding_header);
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

    let request_side =
        futures::future::try_join(feeder, async { Ok::<_, io::Error>(reader_task.await) });
    let response_side = futures::future::join(writer_task, handler_fut);
    let rest = std::pin::pin!(exchange_until_aborted(
        request_side,
        response_side,
        deadline_expiry(Some(&ThreadTimer), deadline, &timer_ctx),
        tx.closed(),
        &cancel,
    ));
    let (opened, end) = {
        let pump = std::pin::pin!(pump);
        match select(pump, rest).await {
            // The stream is gone: cancel the call and drop everything now.
            Either::Left((Err(e), rest)) => {
                cancel_and_settle(&cancel, rest);
                return Err(e);
            }
            Either::Left((Ok(opened), rest)) => (opened, rest.await),
            // The writer has finished or been dropped, which ends the pump.
            Either::Right((end, pump)) => match pump.await {
                Ok(opened) => (opened, end),
                Err(e) => {
                    cancel.cancel();
                    return Err(e);
                }
            },
        }
    };

    let is_grpc = protocol.kind() == ProtocolKind::Grpc;
    let failure = match end {
        StreamEnd::Aborted(Abort::PeerGone | Abort::BodyFailed(_)) => {
            // Reset by the peer or dropped by the connection: nothing more can
            // be sent on this stream.
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

    if opened {
        // The response HEADERS are out. Close the stream per the protocol:
        // gRPC's outcome rides trailing HEADERS (`grpc-status`), and grpc-go
        // hangs/errors on a stream that ends without them. Connect and
        // gRPC-Web carry their terminal frame in-band (EndStreamResponse /
        // 0x80 trailer frame — written by the protocol writer task, or here
        // when the call failed without closing its stream), so a bare
        // END_STREAM DATA closes those.
        let closing = if is_grpc {
            H2Frame::Trailers {
                headers: grpc_status_trailers(failure.as_ref()),
            }
        } else {
            if let Some(frame) = failure
                .as_ref()
                .and_then(|e| stream_error_frame(protocol, &request, e))
            {
                tx.send(H2Frame::Data {
                    payload: frame,
                    end_stream: false,
                })
                .await
                .map_err(stream_gone)?;
            }
            H2Frame::Data {
                payload: Bytes::new(),
                end_stream: true,
            }
        };
        tx.send(closing).await.map_err(stream_gone)?;
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
            // A gRPC stream with zero messages is still a gRPC response — it
            // MUST end in `grpc-status` trailers or the client reports
            // `Unknown` with an empty message (bit BuildKit's FileSend sink,
            // whose response stream is legitimately empty).
            response.trailers.insert(
                SimpleHeader::from("grpc-status".to_string()),
                vec!["0".to_string()],
            );
        }
    }
    let handler = take_handler_headers(&response_headers);
    merge_handler_headers(&mut response.headers, handler, encoding_header);
    send_whole_response(tx, response).await
}

/// gRPC trailing headers for a streaming outcome: `grpc-status: 0` on success,
/// the error's code and percent-encoded message otherwise (gRPC spec §Responses).
#[allow(
    clippy::format_push_string,
    reason = "the percent-encoded trailer string is short; `format!` reads clearer"
)]
pub(crate) fn grpc_status_trailers(
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
                (
                    Bytes::from_static(b"grpc-status"),
                    Bytes::from(code.to_string()),
                ),
                (Bytes::from_static(b"grpc-message"), Bytes::from(encoded)),
            ]
        }
    }
}
