//! D-leg RPC relay — protocol translation at the frame level (spec 63,
//! Workstream 2).
//!
//! WHY: A backend declared `connect://` / `grpc://` / `grpc-web://` speaks a
//! specific RPC wire protocol, and the proxy must translate the inbound
//! protocol into it rather than blindly forward bytes. The proxy knows no
//! protobuf types (it is a *generic* reverse proxy), so it cannot build a typed
//! `Client<Req, Res>` per method. It relays at the **frame level** instead: the
//! inbound `foundation_connectrpc` protocol handler de-frames the request into
//! opaque message frames, and the outbound protocol client re-envelopes them in
//! the backend's protocol. Method identity is the URL path, which the proxy's
//! router already matches by host + `path_prefix`.
//!
//! WHAT: [`relay_grpc_web_to_grpc`] and [`relay_connect_to_grpc`] — the
//! translation seams. An inbound gRPC-Web or Connect call (HTTP/1.1 front end)
//! is de-framed by
//! [`GrpcWebHandler`](foundation_connectrpc::GrpcWebHandler) /
//! [`ConnectHandler`](foundation_connectrpc::ConnectHandler) and re-enveloped as
//! native gRPC by [`GrpcClient`](foundation_connectrpc::GrpcClient) over
//! [`H2Transport`](foundation_connectrpc::H2Transport). The backend's
//! `grpc-status` trailing headers come back through the inbound protocol's
//! terminal framing (gRPC-Web's in-body `0x80` trailer frame, Connect's
//! `Trailer-` headers / streaming end-stream frame).
//!
//! HOW: Four pump tasks (inbound/outbound reader/writer) plus two relay loops
//! (request frames inbound→outbound, response frames outbound→inbound) run
//! concurrently on the valtron pool, exactly as the server-side streaming
//! dispatch does. The exchanges are built synchronously on the H1 relay thread
//! (only `open`/`new_conn` — no await), the pump tasks are then spawned on the
//! pool via [`send`](foundation_core::valtron::send), and the framed response
//! body is streamed back to the client chunk-by-chunk through a response pipe:
//! the sync renderer blocks on [`block_on_future`] per chunk rather than
//! buffering the whole (possibly long) server-stream. The unary request body is
//! small and buffered whole before the relay starts; Connect unary (bare bytes)
//! is a single-message round trip buffered whole, distinct from the enveloped
//! streaming seam.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::url::Uri;
use foundation_core::valtron::{block_on_future, from_future, send, Pipe, PipeReceiver, PipeSender};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::http::{
    ChunkedData, Proto, RequestDescriptor, SendSafeBody, SimpleHeader, SimpleHeaders,
    SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse, SimpleUrl, Status,
};

use foundation_connectrpc::shared::protocol::connect::constants as connect_consts;
use foundation_connectrpc::shared::protocol::connect::{
    streaming_content_type as connect_streaming_content_type,
};
use foundation_connectrpc::shared::protocol::grpc::constants as grpc_consts;
use foundation_connectrpc::shared::protocol::grpc::content_type as grpc_content_type;
use foundation_connectrpc::shared::protocol::grpc_web::{
    content_type as grpc_web_content_type, parse_content_type as parse_grpc_web_content_type,
};
use foundation_connectrpc::shared::protocol::UnaryOutcome;
use foundation_connectrpc::shared::transport::DEFAULT_PIPE_DEPTH;
use foundation_connectrpc::{
    encode_grpc_timeout, parse_connect_content_type, CancelSignal, ClientExchange,
    ClientExchangeConfig, CompressionRegistry, ConnectHandler, ExchangeConfig, GrpcClient,
    GrpcWebHandler, H2Transport, HandlerExchange, ProtocolClient, ProtocolHandler, Spec, Transport,
};

use futures_util::FutureExt;

use crate::forward::{forward_request_headers, upstream_url, write_response};
use crate::runtime::BackendRuntime;

/// Translate a forwarded Connect deadline into the `grpc-timeout` header a
/// native-gRPC backend expects (spec 63 T13).
///
/// Connect carries the call deadline as `connect-timeout-ms` (an integer number
/// of milliseconds); gRPC carries `grpc-timeout` as `<value><unit>` (see
/// [`encode_grpc_timeout`]). The relay re-types the request to gRPC, so the
/// deadline must be re-encoded too — the backend's `GrpcHandler` reads only
/// `grpc-timeout`. The Connect header is removed so the backend never sees a
/// foreign deadline field, and a malformed value is dropped rather than
/// mis-relayed. `out_headers` is the hop-by-hop-stripped request-header map
/// produced by [`forward_request_headers`].
pub(crate) fn translate_connect_deadline_to_grpc(out_headers: &mut SimpleHeaders) {
    let connect_timeout = SimpleHeader::from(connect_consts::HEADER_TIMEOUT.to_string());
    let Some(ms) = out_headers
        .get(&connect_timeout)
        .and_then(|v| v.first())
        .cloned()
    else {
        return;
    };
    out_headers.remove(&connect_timeout);
    if let Ok(ms) = ms.trim().parse::<u64>() {
        out_headers.insert(
            SimpleHeader::from(grpc_consts::HEADER_TIMEOUT.to_string()),
            vec![encode_grpc_timeout(Duration::from_millis(ms))],
        );
    }
}


/// Relay a gRPC-Web request from the HTTP/1.1 front end to a native-gRPC
/// backend, translating at the frame level. Runs on the caller's thread (the
/// H1 relay thread); drives the async relay on the valtron pool and streams the
/// framed gRPC-Web response back to `conn` without buffering the whole body.
///
/// The unary request body is buffered whole (it is a single small envelope);
/// the response body is streamed incrementally through a pipe.
///
/// # Errors
/// Returns `Err(String)` on a setup/relay failure; the caller answers `502` so
/// the client never sees a silent drop.
#[allow(clippy::too_many_arguments)]
pub(crate) fn relay_grpc_web_to_grpc(
    conn: &mut SharedByteBufferStream<RawStream>,
    backend: &Arc<BackendRuntime>,
    mut req: SimpleIncomingRequest,
    client_ip: &str,
    scheme: &str,
    sticky_header: Option<&SimpleHeaders>,
) -> Result<(), String> {
    // The inbound content-type selects the codec + text mode, which decide both
    // the outbound gRPC content-type and the response content-type echoed back.
    let inbound_ct = req
        .headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| "application/grpc-web+proto".to_string());
    let (codec, text) = parse_grpc_web_content_type(&inbound_ct)
        .unwrap_or_else(|| ("proto".to_string(), false));
    let out_content_type = grpc_content_type(&codec);
    let resp_content_type = grpc_web_content_type(&codec, text);

    // Buffer the request body: a unary gRPC-Web request is one small envelope,
    // and the relay needs it whole to feed the request pipe. `take()` leaves
    // `req` intact so it can move into the setup below.
    let req_body = Bytes::from(collect_bytes_from_send_safe(
        req.body.take().unwrap_or(SendSafeBody::None),
    ));

    // Outbound gRPC request headers: the same hop-by-hop + forwarding policy as
    // the transparent leg, re-typed to gRPC and given `te: trailers`.
    let mut out_headers = forward_request_headers(&req.headers, client_ip, scheme);
    out_headers.insert(SimpleHeader::CONTENT_TYPE, vec![out_content_type]);
    out_headers.insert(
        SimpleHeader::from("te".to_string()),
        vec!["trailers".to_string()],
    );

    let url = upstream_url(backend.url(), &req.request_url.url);

    // Build the inbound/outbound exchanges synchronously (no await), so a setup
    // failure is reported to the caller *before* any response bytes are written.
    let (driver, responder_rx) =
        setup_streaming_relay_to_grpc(GrpcWebHandler, req, req_body, url, out_headers)
            .map_err(|e| format!("grpc-web→grpc relay setup failed: {e}"))?;

    // Drive the relay on the pool, non-blocking: it re-envelopes frames and
    // pushes the framed gRPC-Web response body into the responder pipe.
    send(from_future(driver))
        .map_err(|e| format!("grpc-web→grpc relay spawn failed: {e}"))?;

    // Stream the framed response body back, chunk by chunk, blocking only on
    // the response pipe (never buffering the whole server-stream).
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::CONTENT_TYPE, vec![resp_content_type]);
    let body = SendSafeBody::ChunkedStream(Some(Box::new(BlockingChunkReader {
        receiver: responder_rx,
        ended: false,
    })));
    write_response(conn, Status::OK, &headers, body, sticky_header)
}

/// Relay a Connect request from the HTTP/1.1 front end to a native-gRPC
/// backend, translating at the frame level. Runs on the caller's thread (the H1
/// relay thread), exactly like [`relay_grpc_web_to_grpc`].
///
/// Connect has two wire shapes, selected by the inbound `Content-Type`:
/// - **streaming** (`application/connect+{codec}`) is envelope-framed like gRPC,
///   so it relays through the same streaming seam, streamed back chunk-by-chunk;
/// - **unary** (`application/{codec}`) is a bare body, so it is decoded to the
///   message, enveloped as a single gRPC message, and the single gRPC response
///   message is decoded back to a bare Connect body (all small, buffered whole).
///
/// # Errors
/// Returns `Err(String)` on a setup/relay failure; the caller answers `502`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn relay_connect_to_grpc(
    conn: &mut SharedByteBufferStream<RawStream>,
    backend: &Arc<BackendRuntime>,
    mut req: SimpleIncomingRequest,
    client_ip: &str,
    scheme: &str,
    sticky_header: Option<&SimpleHeaders>,
) -> Result<(), String> {
    // The inbound content-type selects the codec + streaming shape, which decide
    // the outbound gRPC content-type and the response content-type echoed back.
    let inbound_ct = req
        .headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| "application/json".to_string());
    let (codec, is_streaming) = parse_connect_content_type(&inbound_ct)
        .ok_or_else(|| format!("unrecognized connect content-type {inbound_ct:?}"))?;
    let out_content_type = grpc_content_type(&codec);

    // Buffer the request body. A unary body is bare bytes; a streaming body is a
    // single small envelope-fed chunk — both are small and needed whole to start
    // the relay. `take()` leaves `req` intact for the branches below.
    let req_body = Bytes::from(collect_bytes_from_send_safe(
        req.body.take().unwrap_or(SendSafeBody::None),
    ));

    // Outbound gRPC request headers: the same hop-by-hop + forwarding policy as
    // the transparent leg, re-typed to gRPC and given `te: trailers`.
    let mut out_headers = forward_request_headers(&req.headers, client_ip, scheme);
    out_headers.insert(SimpleHeader::CONTENT_TYPE, vec![out_content_type]);
    out_headers.insert(
        SimpleHeader::from("te".to_string()),
        vec!["trailers".to_string()],
    );
    translate_connect_deadline_to_grpc(&mut out_headers);

    let url = upstream_url(backend.url(), &req.request_url.url);

    if is_streaming {
        // Streaming: envelope relay through the seam, streamed back frame-by-frame.
        let resp_content_type = connect_streaming_content_type(&codec);
        let (driver, responder_rx) =
            setup_streaming_relay_to_grpc(ConnectHandler, req, req_body, url, out_headers)
                .map_err(|e| format!("connect→grpc relay setup failed: {e}"))?;
        send(from_future(driver))
            .map_err(|e| format!("connect→grpc relay spawn failed: {e}"))?;

        let mut headers = SimpleHeaders::new();
        headers.insert(SimpleHeader::CONTENT_TYPE, vec![resp_content_type]);
        let body = SendSafeBody::ChunkedStream(Some(Box::new(BlockingChunkReader {
            receiver: responder_rx,
            ended: false,
        })));
        write_response(conn, Status::OK, &headers, body, sticky_header)
    } else {
        // Unary: bare bytes ↔ envelope, a single message each way — buffered whole.
        // The round trip is shared with the H2 front end; only the response
        // rendering differs (H1 `write_response` vs H2 frames).
        let response = block_on_future(connect_unary_roundtrip(
            req, req_body, codec, url, out_headers,
        ))
        .map_err(|e| format!("connect unary→grpc relay failed: {e}"))?;
        write_response(
            conn,
            response.status,
            &response.headers,
            response.body.unwrap_or(SendSafeBody::None),
            sticky_header,
        )
    }
}

/// Drive a unary Connect → gRPC round trip: envelope the already-decoded message
/// as a single gRPC message, send it over H2, and return the decoded gRPC
/// response message plus its trailing headers (carrying `grpc-status`). All
/// bodies are single small messages.
async fn connect_unary_to_grpc(
    message: Bytes,
    url: String,
    out_headers: SimpleHeaders,
) -> Result<(Bytes, SimpleHeaders), String> {
    let uri = Uri::parse(&url).map_err(|e| format!("parse upstream uri {url}: {e}"))?;
    let out_request = RequestDescriptor {
        proto: Proto::HTTP20,
        request_url: SimpleUrl::url_only(&url),
        request_uri: uri,
        headers: out_headers,
        method: SimpleMethod::POST,
    };
    let stream = H2Transport::new()
        .open(out_request)
        .map_err(|e| format!("open h2 upstream {url}: {e}"))?;
    let cancel = CancelSignal::new();
    let outbound = GrpcClient
        .new_conn(
            &Spec::empty(true),
            SimpleHeaders::new(),
            stream,
            ClientExchangeConfig {
                cancel: cancel.clone(),
                read_max_bytes: 0,
            },
        )
        .map_err(|e| format!("build grpc outbound exchange: {e}"))?;
    let ClientExchange {
        conn,
        reader_task,
        writer_task,
    } = outbound;
    let (mut out_send, mut out_recv) = conn.split();

    // The four pump/relay futures run concurrently; the receiver stashes its
    // outcome in a shared slot so the caller can surface it after `join_all`.
    let result: Arc<std::sync::Mutex<Option<Result<(Bytes, SimpleHeaders), String>>>> =
        Arc::new(std::sync::Mutex::new(None));

    let sender = async move {
        let _ = out_send.send(message).await;
        let _ = out_send.close_send().await;
    };

    let result_ref = Arc::clone(&result);
    let receiver = async move {
        let outcome = async {
            // Unary is one message, but drain to end-of-stream so the reader task
            // has finished and published the trailing `grpc-status`.
            let mut msg: Option<Bytes> = None;
            loop {
                match out_recv.receive().await {
                    Ok(Some(bytes)) => msg = Some(bytes),
                    Ok(None) => break,
                    Err(e) => return Err(format!("gRPC unary receive failed: {e}")),
                }
            }
            let msg = msg.ok_or_else(|| "gRPC unary returned no message".to_string())?;
            let trailers = out_recv
                .response_trailers()
                .await
                .map(|t| t.clone())
                .unwrap_or_default();
            Ok((msg, trailers))
        }
        .await;
        *result_ref.lock().unwrap() = Some(outcome);
    };

    futures_util::future::join_all(vec![
        sender.boxed(),
        receiver.boxed(),
        reader_task.map(|_| ()).boxed(),
        writer_task.map(|_| ()).boxed(),
    ])
    .await;

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .unwrap_or_else(|| Err("gRPC unary relay produced no result".to_string()));
    outcome
}

/// Drive the full unary Connect → gRPC round trip: decode the bare Connect body,
/// envelope the message as a single gRPC message, send it over H2, and encode the
/// single gRPC response message back to a bare Connect body. Shared by the H1 and
/// H2 front ends — only the response *rendering* differs (H1 `write_response` vs
/// H2 frames). All bodies are single small messages, buffered whole.
///
/// `request` and `codec` are taken by value so the future owns only `'static`
/// data — the H1 leg drives it via `block_on_future`, which requires that.
pub(crate) async fn connect_unary_roundtrip(
    request: SimpleIncomingRequest,
    req_body: Bytes,
    codec: String,
    url: String,
    out_headers: SimpleHeaders,
) -> Result<SimpleOutgoingResponse, String> {
    let compression = CompressionRegistry::empty();
    let message = ConnectHandler
        .decode_unary_request(&request, req_body, &compression, 0)
        .map_err(|e| format!("decode connect unary request: {e}"))?;
    let (resp_bytes, trailers) = connect_unary_to_grpc(message, url, out_headers)
        .await
        .map_err(|e| format!("connect unary→grpc relay failed: {e}"))?;

    let outcome = UnaryOutcome {
        frame: resp_bytes,
        headers: SimpleHeaders::new(),
        trailers,
    };
    let mut response = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .with_headers(SimpleHeaders::new())
        .with_body(SendSafeBody::None)
        .build()
        .map_err(|e| format!("build connect unary response: {e}"))?;
    ConnectHandler
        .encode_unary_response(&mut response, &request, &codec, outcome, &compression)
        .map_err(|e| format!("encode connect unary response: {e}"))?;
    Ok(response)
}

/// Build the inbound/outbound exchanges and the frame-relay driver, consuming
/// the caller's request-body pipe (`body_rx`) and response pipe (`responder_tx`).
///
/// `handler` is the inbound protocol (`GrpcWebHandler` / `ConnectHandler`): it
/// de-frames the request and frames the response; the outbound side is always
/// native gRPC over `H2Transport`. The seam is message-level (`Bytes`), so the
/// relay is codec-agnostic — the inbound handler's framing and the outbound
/// `GrpcClient`'s framing are what differ, not the pump loops.
///
/// The caller owns the two pipes: it feeds `body_rx` (buffered whole on the H1
/// leg, streamed from H2 DATA frames on the H2 leg) and drains the response pipe
/// paired with `responder_tx` (a sync chunk reader on H1, H2 DATA frames on H2).
/// This function only builds the exchanges and the pump/reader/writer tasks; its
/// I/O is the spawned tasks' job. A failure returns `Err` so the caller can
/// answer `502`.
///
/// Returns the driver **and** the shared [`CancelSignal`] wired into both
/// exchanges: firing it tears the inbound and outbound sides down together
/// (spec 63 T13). The caller fires it when the client resets the stream.
pub(crate) fn build_streaming_relay_to_grpc<H: ProtocolHandler>(
    handler: H,
    request: SimpleIncomingRequest,
    body_rx: PipeReceiver<Bytes>,
    responder_tx: PipeSender<Bytes>,
    url: String,
    out_headers: SimpleHeaders,
) -> Result<(futures_util::future::BoxFuture<'static, ()>, CancelSignal), String> {
    let cancel = CancelSignal::new();

    // Inbound: the handler de-frames the request, frames the response.
    // Scope the registry so it is dropped before the exchanges are split.
    let inbound = {
        let compression = CompressionRegistry::empty();
        handler
            .new_conn(
                &request,
                Spec::empty(false),
                body_rx,
                responder_tx,
                ExchangeConfig {
                    compression: &compression,
                    cancel: cancel.clone(),
                    read_max_bytes: 0,
                },
            )
            .map_err(|e| format!("build inbound exchange: {e}"))?
    };

    // Outbound: native gRPC over H2. The request headers (content-type,
    // te: trailers) ride the `RequestDescriptor`; the client seam headers can
    // stay empty (they are not written to the wire).
    let uri = Uri::parse(&url).map_err(|e| format!("parse upstream uri {url}: {e}"))?;
    let out_request = RequestDescriptor {
        proto: Proto::HTTP20,
        request_url: SimpleUrl::url_only(&url),
        request_uri: uri,
        headers: out_headers,
        method: SimpleMethod::POST,
    };
    let stream = H2Transport::new()
        .open(out_request)
        .map_err(|e| format!("open h2 upstream {url}: {e}"))?;
    let outbound = GrpcClient
        .new_conn(
            &Spec::empty(true),
            SimpleHeaders::new(),
            stream,
            ClientExchangeConfig {
                cancel: cancel.clone(),
                read_max_bytes: 0,
            },
        )
        .map_err(|e| format!("build grpc outbound exchange: {e}"))?;

    let HandlerExchange {
        conn: inbound_conn,
        reader_task: in_reader,
        writer_task: in_writer,
    } = inbound;
    let ClientExchange {
        conn: outbound_conn,
        reader_task: out_reader,
        writer_task: out_writer,
    } = outbound;

    let (mut in_recv, mut in_send) = inbound_conn.split();
    let (mut out_send, mut out_recv) = outbound_conn.split();

    // Request direction: de-framed inbound messages → gRPC request frames.
    let request_pump = async move {
        loop {
            match in_recv.receive().await {
                Ok(Some(bytes)) => {
                    if out_send.send(bytes).await.is_err() {
                        break;
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
        let _ = out_send.close_send().await;
    };

    // Response direction: gRPC response frames → inbound frames; the terminal
    // outcome (clean end or error) is handed back as `EndStream` so the inbound
    // writer emits the protocol's terminal framing with the right `grpc-status`.
    let response_pump = async move {
        loop {
            match out_recv.receive().await {
                Ok(Some(bytes)) => {
                    if in_send.send(bytes).await.is_err() {
                        return;
                    }
                }
                Ok(None) => {
                    let trailers = out_recv
                        .response_trailers()
                        .await
                        .map(|t| t.clone())
                        .unwrap_or_default();
                    let _ = in_send.close(None, trailers).await;
                    return;
                }
                Err(e) => {
                    let trailers = out_recv
                        .response_trailers()
                        .await
                        .map(|t| t.clone())
                        .unwrap_or_default();
                    let _ = in_send.close(Some(e), trailers).await;
                    return;
                }
            }
        }
    };

    // The reader/writer tasks' own `ConnectResult` is only informational: their
    // failures already surface through the `Frame::EndStream` they push, which
    // the pumps observe. The response bytes are drained by the caller through
    // the response pipe, not collected here.
    let work = futures_util::future::join_all(vec![
        in_reader.map(|_| ()).boxed(),
        in_writer.map(|_| ()).boxed(),
        out_reader.map(|_| ()).boxed(),
        out_writer.map(|_| ()).boxed(),
        request_pump.boxed(),
        response_pump.boxed(),
    ]);

    // Race the whole relay against the shared cancel. Firing it drops every task
    // — in particular the outbound reader/writer, whose transport halves keep the
    // upstream H2 socket open — so a cancelled call tears the backend side down
    // instead of leaving it streaming into a dead client (spec 63 T13).
    let cancel_for_driver = cancel.clone();
    let driver: futures_util::future::BoxFuture<'static, ()> = async move {
        let cancelled = Box::pin(cancel_for_driver.cancelled());
        match futures_util::future::select(work, cancelled).await {
            futures_util::future::Either::Left(_) => {}
            futures_util::future::Either::Right(_) => {}
        }
    }
    .boxed();

    Ok((driver, cancel))
}

/// H1-leg wrapper around [`build_streaming_relay_to_grpc`]: buffer the request
/// body whole (a single small unary envelope, or a small client-stream), feed it
/// into the request pipe, and half-close it. Returns the driver (spawned on the
/// pool by the caller) and the response-pipe receiver (drained by the caller to
/// stream the framed body back).
///
/// All work here is synchronous — pipe creation, the buffered request feed, and
/// the `new_conn`/`open` calls return immediately; their I/O is the spawned pump
/// tasks' job. A failure returns `Err` so the caller can answer `502`.
fn setup_streaming_relay_to_grpc<H: ProtocolHandler>(
    handler: H,
    request: SimpleIncomingRequest,
    req_body: Bytes,
    url: String,
    out_headers: SimpleHeaders,
) -> Result<(futures_util::future::BoxFuture<'static, ()>, PipeReceiver<Bytes>), String> {
    let (body_tx, body_rx) = Pipe::<Bytes>::with_depth(DEFAULT_PIPE_DEPTH);
    let (responder_tx, responder_rx) = Pipe::<Bytes>::with_depth(DEFAULT_PIPE_DEPTH);

    // Feed the buffered request body, then half-close the request direction so
    // the inbound reader sees end-of-stream. Depth ≥ 1 means `try_send` never
    // blocks; the body is small and buffered whole.
    if !req_body.is_empty() {
        let _ = body_tx.try_send(req_body);
    }
    body_tx.close();

    let (driver, _cancel) =
        build_streaming_relay_to_grpc(handler, request, body_rx, responder_tx, url, out_headers)?;
    Ok((driver, responder_rx))
}

/// Chunked response-body reader that drains a response pipe one chunk at a time,
/// blocking (via [`block_on_future`]) until a chunk arrives or the pipe closes.
///
/// WHY: a `SendSafeBody::Stream` adapter surfaces an empty-but-open pipe as
/// `Data::Retry`, which the *sync* H1 response renderer would busy-spin on. This
/// reader blocks per chunk instead, so a long server-stream never buffered whole
/// and never spins a relay thread.
struct BlockingChunkReader {
    receiver: PipeReceiver<Bytes>,
    ended: bool,
}

impl Iterator for BlockingChunkReader {
    type Item = Result<ChunkedData, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        // `receive` borrows the receiver; clone the cheap `Arc` handle so the
        // future owns it and can run `'static` on the pool.
        let rx = self.receiver.clone();
        match block_on_future(async move { rx.receive().await }) {
            Some(bytes) => Some(Ok(ChunkedData::Data(bytes.to_vec(), None))),
            None => {
                self.ended = true;
                Some(Ok(ChunkedData::DataEnded))
            }
        }
    }
}
