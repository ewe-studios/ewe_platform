//! Handler response headers over the three streaming transports (spec 73 F03).
//!
//! WHY: a streaming handler installs its response head with
//! `MessageSink::set_headers`. The map reached the shared conn slot, but no
//! transport read the slot. All three pumps built the head from the content type
//! and the encoding alone, so every handler header was dropped.
//!
//! WHAT:
//! - `handler_headers_reach_the_response_head_h1`, `_h2`, `_h3`: a header a
//!   handler sets before its first message is on the response head.
//! - `a_handler_cannot_override_content_type`: the protocol keeps its framing
//!   header.
//! - `set_headers_after_the_first_message_fails`: the late call returns
//!   `FailedPrecondition` with the documented message.
//!
//! HOW: a real server on loopback for each transport, driven by the typed
//! `Client`. The client-stream handler returns its headers through
//! `Response::headers_mut`, which the erased handler installs with
//! `set_headers` before it sends the response message. The last test drives the
//! seam directly, because a typed handler cannot send and then set headers.

use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::StreamExt;

use foundation_core::valtron::valtron_test;
use foundation_http::native::app::ServerApp;
use foundation_http::native::serve::H2Serve;
use foundation_http::native::serve::Serve;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
use foundation_netio::HttpClientBuilder;

use foundation_connectrpc::shared::transport::{PipeHandlerConn, DEFAULT_PIPE_DEPTH};
use foundation_connectrpc::{
    CancelSignal, Client, ClientOptions, Code, CodecFor, ConnectRpcServe, ConnectRpcServeH2, Ctx,
    H1Transport, H2Transport, HandlerConn, HandlerOptions, JsonCodec, MessageSink, Peer,
    ProcedureCodecs, RequestStream, Response, Router, Spec, Transport,
};

#[cfg(feature = "h3")]
use foundation_connectrpc::native::h3_serve::ConnectRpcServeH3;
#[cfg(feature = "h3")]
use foundation_connectrpc::H3Transport;
#[cfg(feature = "h3")]
use foundation_http::native::serve::H3Serve;
#[cfg(feature = "h3")]
use foundation_http::native::server::serve_h3;
#[cfg(feature = "h3")]
use foundation_netio::quic::client_config_trusting_pem;

#[cfg(feature = "h3")]
const CERT_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_cert.pem");
#[cfg(feature = "h3")]
const KEY_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_key.pem");
#[cfg(feature = "h3")]
const CA_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_ca.pem");

const HEADERS_PROC: &str = "/t.Svc/Headers";
const OVERRIDE_PROC: &str = "/t.Svc/Override";
const FRAMING_PROC: &str = "/t.Svc/Framing";

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

fn msg(text: &str) -> Msg {
    Msg { text: text.into() }
}

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

fn build_router() -> Router {
    let mut router = Router::new();

    router.client_stream(
        HEADERS_PROC,
        codecs(),
        |_ctx: Ctx, mut reqs: RequestStream<Msg>| async move {
            let mut parts = Vec::new();
            while let Some(m) = reqs.next().await {
                parts.push(m?.text);
            }
            let mut reply = Response::new(msg(&parts.join("+")));
            reply.headers_mut().insert(
                SimpleHeader::from("x-handler".to_string()),
                vec!["one".to_string()],
            );
            Ok(reply)
        },
        HandlerOptions::new(),
    );

    router.client_stream(
        OVERRIDE_PROC,
        codecs(),
        |_ctx: Ctx, mut reqs: RequestStream<Msg>| async move {
            let mut parts = Vec::new();
            while let Some(m) = reqs.next().await {
                parts.push(m?.text);
            }
            let mut reply = Response::new(msg(&parts.join("+")));
            reply.headers_mut().insert(
                SimpleHeader::CONTENT_TYPE,
                vec!["application/x-handler".to_string()],
            );
            Ok(reply)
        },
        HandlerOptions::new(),
    );

    // A handler that tries to set every header the transport owns, plus one
    // header of its own. The framing values must not reach the response
    // (issue #389).
    router.client_stream(
        FRAMING_PROC,
        codecs(),
        |_ctx: Ctx, mut reqs: RequestStream<Msg>| async move {
            let mut parts = Vec::new();
            while let Some(m) = reqs.next().await {
                parts.push(m?.text);
            }
            let mut reply = Response::new(msg(&parts.join("+")));
            let headers = reply.headers_mut();
            headers.insert(
                SimpleHeader::from(":status".to_string()),
                vec!["599".to_string()],
            );
            headers.insert(
                SimpleHeader::from("content-length".to_string()),
                vec!["999999".to_string()],
            );
            headers.insert(
                SimpleHeader::from("transfer-encoding".to_string()),
                vec!["handler-wins".to_string()],
            );
            headers.insert(
                SimpleHeader::from("connection".to_string()),
                vec!["handler-wins".to_string()],
            );
            headers.insert(
                SimpleHeader::from("content-type".to_string()),
                vec!["application/x-handler".to_string()],
            );
            headers.insert(
                SimpleHeader::from("x-keep".to_string()),
                vec!["yes".to_string()],
            );
            Ok(reply)
        },
        HandlerOptions::new(),
    );

    router
}

/// The first value of `name`, compared without case sensitivity.
fn header_value(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(header, _)| header.to_string().eq_ignore_ascii_case(name))
        .and_then(|(_, values)| values.first().cloned())
}

fn call_ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

// ── h1 ───────────────────────────────────────────────────────────────────

fn start_h1_server() -> (String, Arc<ShutdownSignal>) {
    let rpc: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(build_router().into_handler()));
    let mut app = HttpApp::new_serve();
    app.router.add_route_any(HEADERS_PROC, &rpc);
    app.router.add_route_any(OVERRIDE_PROC, &rpc);
    app.router.add_route_any(FRAMING_PROC, &rpc);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(ShutdownSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    let server = HttpServer::from_app(ServerApp::http1(app), &addr.to_string());
    thread::spawn(move || server.serve_with_listener(&listener, &thread_shutdown));

    (addr.to_string(), shutdown)
}

fn h1_client(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> =
        Arc::new(H1Transport::new(HttpClientBuilder::new().build()));
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client")
}

#[valtron_test]
async fn handler_headers_reach_the_response_head_h1() {
    let (addr, shutdown) = start_h1_server();
    let response = h1_client(&addr, HEADERS_PROC)
        .client_stream(call_ctx(), futures::stream::iter(vec![msg("a")]))
        .await
        .expect("client stream");

    assert_eq!(
        header_value(response.headers(), "x-handler").as_deref(),
        Some("one"),
        "the handler header is on the response head"
    );
    assert_eq!(response.msg, msg("a"));
    shutdown.turn_on();
}

#[valtron_test]
async fn a_handler_cannot_override_content_type() {
    let (addr, shutdown) = start_h1_server();
    let response = h1_client(&addr, OVERRIDE_PROC)
        .client_stream(call_ctx(), futures::stream::iter(vec![msg("a")]))
        .await
        .expect("client stream");

    assert_eq!(
        header_value(response.headers(), "content-type").as_deref(),
        Some("application/connect+json"),
        "the protocol keeps the framing content type"
    );
    assert_eq!(response.msg, msg("a"));
    shutdown.turn_on();
}

/// The transport keeps every framing header, and the handler's own header
/// still reaches the head.
fn assert_framing_headers_kept(headers: &SimpleHeaders) {
    assert_eq!(
        header_value(headers, "x-keep").as_deref(),
        Some("yes"),
        "a handler header that is not a framing header reaches the head"
    );
    assert_ne!(
        header_value(headers, "content-length").as_deref(),
        Some("999999"),
        "the handler cannot set content-length"
    );
    assert_ne!(
        header_value(headers, "transfer-encoding").as_deref(),
        Some("handler-wins"),
        "the handler cannot set transfer-encoding"
    );
    assert_ne!(
        header_value(headers, "connection").as_deref(),
        Some("handler-wins"),
        "the handler cannot set connection"
    );
    assert_ne!(
        header_value(headers, "content-type").as_deref(),
        Some("application/x-handler"),
        "the handler cannot set the framing content type"
    );
    assert_ne!(
        header_value(headers, ":status").as_deref(),
        Some("599"),
        "the handler cannot set the HTTP status"
    );
}

#[valtron_test]
async fn a_handler_cannot_set_framing_headers_h1() {
    let (addr, shutdown) = start_h1_server();
    let response = h1_client(&addr, FRAMING_PROC)
        .client_stream(call_ctx(), futures::stream::iter(vec![msg("a")]))
        .await
        .expect("client stream");

    assert_framing_headers_kept(response.headers());
    assert_eq!(response.msg, msg("a"));
    shutdown.turn_on();
}

// ── h2 ───────────────────────────────────────────────────────────────────

fn start_h2_server() -> (String, Arc<ShutdownSignal>) {
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(build_router().into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(HEADERS_PROC, rpc.clone());
    app.route_any_h2(OVERRIDE_PROC, rpc.clone());
    app.route_any_h2(FRAMING_PROC, rpc);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(ShutdownSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || server.serve_with_listener(&listener, &thread_shutdown));

    (addr.to_string(), shutdown)
}

fn h2_client(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client")
}

#[valtron_test]
async fn handler_headers_reach_the_response_head_h2() {
    let (addr, shutdown) = start_h2_server();
    let response = h2_client(&addr, HEADERS_PROC)
        .client_stream(call_ctx(), futures::stream::iter(vec![msg("a")]))
        .await
        .expect("client stream");

    assert_eq!(
        header_value(response.headers(), "x-handler").as_deref(),
        Some("one"),
        "the handler header is on the response head"
    );
    assert_eq!(response.msg, msg("a"));
    shutdown.turn_on();
}

#[valtron_test]
async fn a_handler_cannot_set_framing_headers_h2() {
    let (addr, shutdown) = start_h2_server();
    let response = h2_client(&addr, FRAMING_PROC)
        .client_stream(call_ctx(), futures::stream::iter(vec![msg("a")]))
        .await
        .expect("client stream");

    assert_framing_headers_kept(response.headers());
    assert_eq!(response.msg, msg("a"));
    shutdown.turn_on();
}

// ── h3 ───────────────────────────────────────────────────────────────────

#[cfg(feature = "h3")]
fn start_h3_server() -> (std::net::SocketAddr, Arc<ShutdownSignal>) {
    let rpc: Arc<dyn H3Serve> = Arc::new(ConnectRpcServeH3::new(build_router().into_handler()));
    let mut app = HttpApp::new_h3_serve();
    app.route_any_h3("/*", rpc);
    let shutdown = Arc::new(ShutdownSignal::new());
    let addr = serve_h3(
        "127.0.0.1:0".parse().unwrap(),
        CERT_PEM,
        KEY_PEM,
        Arc::new(app),
        Arc::clone(&shutdown),
    )
    .expect("start h3 server");
    (addr, shutdown)
}

#[cfg(feature = "h3")]
fn h3_client(addr: std::net::SocketAddr, procedure: &str) -> Client<Msg, Msg> {
    let cfg = client_config_trusting_pem(CA_PEM).expect("client config");
    let transport: Arc<dyn Transport> =
        Arc::new(H3Transport::new(cfg).with_server_name("localhost"));
    Client::new(
        transport,
        &format!("https://{addr}{procedure}"),
        codecs(),
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client")
}

#[cfg(feature = "h3")]
#[valtron_test]
async fn handler_headers_reach_the_response_head_h3() {
    let (addr, shutdown) = start_h3_server();
    let response = h3_client(addr, HEADERS_PROC)
        .client_stream(call_ctx(), futures::stream::iter(vec![msg("a")]))
        .await
        .expect("client stream");

    assert_eq!(
        header_value(response.headers(), "x-handler").as_deref(),
        Some("one"),
        "the handler header is on the response head"
    );
    assert_eq!(response.msg, msg("a"));
    shutdown.turn_on();
}

#[cfg(feature = "h3")]
#[valtron_test]
async fn a_handler_cannot_set_framing_headers_h3() {
    let (addr, shutdown) = start_h3_server();
    let response = h3_client(addr, FRAMING_PROC)
        .client_stream(call_ctx(), futures::stream::iter(vec![msg("a")]))
        .await
        .expect("client stream");

    assert_framing_headers_kept(response.headers());
    assert_eq!(response.msg, msg("a"));
    shutdown.turn_on();
}

// ── The late-call rule ───────────────────────────────────────────────────

/// The seam rejects a header map once the first message went out.
#[valtron_test]
async fn set_headers_after_the_first_message_fails() {
    let (conn, _ends) = PipeHandlerConn::new(
        Spec::empty(false),
        Peer::empty(),
        SimpleHeaders::new(),
        CancelSignal::new(),
        DEFAULT_PIPE_DEPTH,
    );
    let (_receiver, sender) = Box::new(conn).split();
    let codec: Arc<dyn CodecFor<Msg>> = Arc::new(JsonCodec);
    let mut sink = MessageSink::new(sender, codec);

    sink.send(msg("first")).await.expect("first message");
    let error = sink
        .set_headers(SimpleHeaders::new())
        .await
        .expect_err("a late header map is rejected");

    assert_eq!(error.current_context().code(), Code::FailedPrecondition);
    assert_eq!(
        error.current_context().message(),
        "response headers must be set before the first message"
    );
}
