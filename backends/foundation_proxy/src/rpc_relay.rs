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
//! WHAT: [`relay_grpc_web_to_grpc`] — the T8 keystone seam. An inbound
//! gRPC-Web call (HTTP/1.1 front end) is de-framed by
//! [`GrpcWebHandler`](foundation_connectrpc::GrpcWebHandler) and re-enveloped
//! as native gRPC by [`GrpcClient`](foundation_connectrpc::GrpcClient) over
//! [`H2Transport`](foundation_connectrpc::H2Transport). The backend's
//! `grpc-status` trailing headers come back through the gRPC-Web in-body `0x80`
//! trailer frame.
//!
//! HOW: Four pump tasks (inbound/outbound reader/writer) plus two relay loops
//! (request frames inbound→outbound, response frames outbound→inbound) run
//! concurrently on the valtron pool, exactly as the server-side streaming
//! dispatch does. The sync H1 relay thread drives them with
//! [`block_on_future`](foundation_core::valtron::block_on_future) — the pool is
//! guaranteed initialised (a `ProxyServer::start` requirement) — and then writes
//! the framed gRPC-Web response back to the client connection. For the unary
//! case the request body and response body are small and buffered whole; the
//! streaming case (T9) feeds the body incrementally.

use std::sync::Arc;

use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::url::Uri;
use foundation_core::valtron::{block_on_future, Pipe};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest,
    SimpleMethod, SimpleUrl, Status,
};

use foundation_connectrpc::shared::protocol::grpc::content_type as grpc_content_type;
use foundation_connectrpc::shared::protocol::grpc_web::{
    content_type as grpc_web_content_type, parse_content_type as parse_grpc_web_content_type,
};
use foundation_connectrpc::shared::transport::DEFAULT_PIPE_DEPTH;
use foundation_connectrpc::{
    CancelSignal, ClientExchange, ClientExchangeConfig, CompressionRegistry, ExchangeConfig,
    GrpcClient, GrpcWebHandler, H2Transport, HandlerExchange, ProtocolClient, ProtocolHandler,
    Spec, Transport,
};

use futures_util::FutureExt;

use crate::forward::{forward_request_headers, upstream_url, write_response};
use crate::runtime::BackendRuntime;

/// Relay a gRPC-Web request from the HTTP/1.1 front end to a native-gRPC
/// backend, translating at the frame level. Runs on the caller's thread (the
/// H1 relay thread); drives the async relay on the valtron pool via
/// `block_on_future` and writes the framed gRPC-Web response back to `conn`.
///
/// The unary request body is buffered whole (it is a single small envelope);
/// the response body is collected whole too.
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
    // `req` intact so it can move into the relay future below.
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

    let body = block_on_future(grpc_web_to_grpc_future(req, req_body, url, out_headers))
        .map_err(|e| format!("grpc-web→grpc relay failed: {e}"))?;

    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::CONTENT_TYPE, vec![resp_content_type]);
    write_response(
        conn,
        Status::OK,
        &headers,
        SendSafeBody::Bytes(body.to_vec()),
        sticky_header,
    )
}

/// The async relay body: build the inbound/outbound exchanges, then drive the
/// four protocol pump tasks and two frame-relay loops concurrently, collecting
/// the framed gRPC-Web response bytes.
///
/// Returns the complete gRPC-Web response body (message envelope(s) + `0x80`
/// trailer frame), ready to be written verbatim as the H1 response body.
async fn grpc_web_to_grpc_future(
    request: SimpleIncomingRequest,
    req_body: Bytes,
    url: String,
    out_headers: SimpleHeaders,
) -> Result<Bytes, String> {
    let (body_tx, body_rx) = Pipe::<Bytes>::with_depth(DEFAULT_PIPE_DEPTH);
    let (responder_tx, responder_rx) = Pipe::<Bytes>::with_depth(DEFAULT_PIPE_DEPTH);

    // Feed the buffered request body, then half-close the request direction so
    // the inbound reader sees end-of-stream. Depth ≥ 1 means this never parks.
    if !req_body.is_empty() {
        let _ = body_tx.send(req_body).await;
    }
    body_tx.close();

    let cancel = CancelSignal::new();

    // Inbound: gRPC-Web handler de-frames the request, frames the response.
    // Scope the registry so it is dropped before the first await.
    let inbound = {
        let compression = CompressionRegistry::empty();
        GrpcWebHandler
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
            .map_err(|e| format!("build grpc-web inbound exchange: {e}"))?
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

    // Response direction: gRPC response frames → inbound gRPC-Web frames; the
    // terminal outcome (clean end or error) is handed back as `EndStream` so the
    // inbound writer emits the `0x80` trailer with the right `grpc-status`.
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

    // Collect the framed gRPC-Web response bytes the inbound writer emits.
    let response_body = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let response_body_ref = Arc::clone(&response_body);
    let collect = async move {
        let rx = responder_rx;
        while let Some(chunk) = rx.receive().await {
            response_body_ref.lock().unwrap().extend_from_slice(&chunk);
        }
    };

    // The reader/writer tasks' own `ConnectResult` is only informational: their
    // failures already surface through the `Frame::EndStream` they push, which
    // the pumps observe.
    let tasks: Vec<futures_util::future::BoxFuture<'static, ()>> = vec![
        in_reader.map(|_| ()).boxed(),
        in_writer.map(|_| ()).boxed(),
        out_reader.map(|_| ()).boxed(),
        out_writer.map(|_| ()).boxed(),
        request_pump.boxed(),
        response_pump.boxed(),
        collect.boxed(),
    ];
    futures_util::future::join_all(tasks).await;

    let bytes = Bytes::from(std::mem::take(
        &mut *response_body.lock().unwrap(),
    ));
    Ok(bytes)
}
