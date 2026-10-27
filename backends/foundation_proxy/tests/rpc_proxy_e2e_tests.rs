//! End-to-end RPC proxy tests — real connectrpc `Client` → `foundation_proxy` →
//! real connectrpc `Router` backend.
//!
//! WHY: The proxy's RPC support must be proven with the *actual* RPC stack, not
//! hand-rolled wire frames. gRPC in particular is only correct end-to-end: the
//! client must observe `grpc-status` in trailing HEADERS, which only survives if
//! the proxy forwards the H2 leg (`application/grpc`) without downgrade and
//! relays the trailers. Connect and gRPC-Web are HTTP/1.1-transparent and must
//! round-trip through the plain forward leg.
//!
//! WHAT: A dual-protocol backend serves one connectrpc `Router` over HTTP/1.1
//! (`ConnectRpcServe`, answering Connect + gRPC-Web) and HTTP/2
//! (`ConnectRpcServeH2`, answering gRPC + Connect-over-H2). `foundation_proxy`
//! fronts it with the transparent `http` backend leg. A typed `Client` for each
//! of the four protocol/transport shapes drives unary and server-streaming calls
//! through the proxy and asserts the round-trip (and, for gRPC, `grpc-status`).
//!
//! HOW: Each test starts its own backend + proxy on ephemeral ports (`:0`), then
//! dials the proxy with the appropriate `H1Transport` / `H2Transport` and
//! protocol option (`with_connect` / `with_grpc` / `with_grpc_web`).

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use bytes::Bytes;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron_test;
use foundation_http::native::server::HttpServer;
use foundation_http::native::serve::H2Serve;
use foundation_http::shared::serve::Serve;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::http::{SimpleHeader, Status};
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectResult, ConnectRpcServe, ConnectRpcServeH2, Ctx, Envelope,
    H1Transport, H2Transport, HandlerOptions, JsonCodec, ProcedureCodecs, Request, RequestStream,
    Response, Router,
};

use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

// ── Shared message + procedures ─────────────────────────────────────────────

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

impl Msg {
    fn of(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

const UNARY: &str = "/e2e.Rpc/Unary";
const SERVER_STREAM: &str = "/e2e.Rpc/ServerStream";
const CLIENT_STREAM: &str = "/e2e.Rpc/ClientStream";
const BIDI_STREAM: &str = "/e2e.Rpc/BidiStream";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

/// One router with a unary echo and a 3-item server-stream, dispatched by
/// `Content-Type` for Connect, gRPC, and gRPC-Web alike (Decision 08).
fn build_router() -> Router {
    let mut router = Router::new();

    router.unary(
        UNARY,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move {
            Ok(Response::new(Msg::of(format!("echo:{}", req.msg.text))))
        },
        HandlerOptions::new(),
    );

    router.server_stream(
        SERVER_STREAM,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move {
            let items: Vec<ConnectResult<Msg>> = (0..3)
                .map(|i| Ok(Msg::of(format!("{}-{i}", req.msg.text))))
                .collect();
            Ok(futures_util::stream::iter(items))
        },
        HandlerOptions::new(),
    );

    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        |_ctx: Ctx, reqs: RequestStream<Msg>| async move {
            use futures_util::StreamExt;
            let mut parts = Vec::new();
            let mut reqs = reqs;
            while let Some(msg) = reqs.next().await {
                parts.push(msg.map(|m| m.text).unwrap_or_else(|_| "<err>".to_string()));
            }
            Ok(Response::new(Msg::of(parts.join("+"))))
        },
        HandlerOptions::new(),
    );

    router.bidi_stream(
        BIDI_STREAM,
        codecs(),
        |_ctx: Ctx, reqs: RequestStream<Msg>| async move {
            use futures_util::StreamExt;
            Ok(reqs.map(|msg| msg.map(|m| Msg::of(format!("re:{}", m.text)))))
        },
        HandlerOptions::new(),
    );

    router
}

// ── Dual-protocol backend ───────────────────────────────────────────────────

/// Start a real `foundation_http` backend serving one connectrpc router over
/// both HTTP/1.1 (Connect + gRPC-Web) and HTTP/2 (gRPC + Connect-over-H2).
fn start_backend() -> (u16, Arc<OnSignal>) {
    // H1: Connect and gRPC-Web are HTTP/1.1-native.
    let h1: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(build_router().into_handler()));
    let mut h1_app = HttpApp::new_serve();
    h1_app.router.add_route_any(UNARY, &h1);
    h1_app.router.add_route_any(SERVER_STREAM, &h1);
    h1_app.router.add_route_any(CLIENT_STREAM, &h1);
    h1_app.router.add_route_any(BIDI_STREAM, &h1);

    // H2: gRPC requires HTTP/2 end-to-end.
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(build_router().into_handler()));
    let mut h2_app = HttpApp::new_h2_serve();
    h2_app.route_any_h2(UNARY, h2.clone());
    h2_app.route_any_h2(SERVER_STREAM, h2.clone());
    h2_app.route_any_h2(CLIENT_STREAM, h2.clone());
    h2_app.route_any_h2(BIDI_STREAM, h2);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind backend");
    let port = listener.local_addr().expect("backend addr").port();
    let addr = format!("127.0.0.1:{port}");

    let shutdown = Arc::new(OnSignal::new());
    let server_shutdown = Arc::clone(&shutdown);
    let server = HttpServer::from_app(ServerApp::both(h1_app, h2_app), &addr);
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    (port, shutdown)
}

/// A router whose unary handler stamps every response with `tag`, so a test can
/// observe which of several backend instances answered a call.
fn build_tagged_router(tag: &str) -> Router {
    let tag = tag.to_string();
    let mut router = Router::new();
    router.unary(
        UNARY,
        codecs(),
        move |_ctx: Ctx, req: Request<Msg>| {
            let tag = tag.clone();
            async move { Ok(Response::new(Msg::of(format!("{tag}:{}", req.msg.text)))) }
        },
        HandlerOptions::new(),
    );
    router
}

/// Start a backend serving **only native gRPC** (HTTP/2 `ConnectRpcServeH2`),
/// with no HTTP/1.1 / gRPC-Web dispatch at all. Used by the D-leg tests to prove
/// the proxy translates an inbound gRPC-Web request rather than forwarding it.
fn start_backend_grpc_only() -> (u16, Arc<OnSignal>) {
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(build_router().into_handler()));
    let mut h2_app = HttpApp::new_h2_serve();
    h2_app.route_any_h2(UNARY, h2.clone());
    h2_app.route_any_h2(SERVER_STREAM, h2.clone());
    h2_app.route_any_h2(CLIENT_STREAM, h2.clone());
    h2_app.route_any_h2(BIDI_STREAM, h2);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind backend");
    let port = listener.local_addr().expect("backend addr").port();
    let addr = format!("127.0.0.1:{port}");

    let shutdown = Arc::new(OnSignal::new());
    let server_shutdown = Arc::clone(&shutdown);
    let server = HttpServer::from_app(ServerApp::http2(h2_app), &addr);
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    (port, shutdown)
}

/// A native-gRPC-only backend (H2) whose unary responses carry `tag`, so a test
/// can observe which of several backend instances answered.
fn start_backend_grpc_only_tagged(tag: &str) -> (u16, Arc<OnSignal>) {
    let h2: Arc<dyn H2Serve> =
        Arc::new(ConnectRpcServeH2::new(build_tagged_router(tag).into_handler()));
    let mut h2_app = HttpApp::new_h2_serve();
    h2_app.route_any_h2(UNARY, h2);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind backend");
    let port = listener.local_addr().expect("backend addr").port();
    let addr = format!("127.0.0.1:{port}");

    let shutdown = Arc::new(OnSignal::new());
    let server_shutdown = Arc::clone(&shutdown);
    let server = HttpServer::from_app(ServerApp::http2(h2_app), &addr);
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    (port, shutdown)
}

// ── Proxy + client helpers ──────────────────────────────────────────────────

/// Build a proxy config whose service routes host `127.0.0.1` to the backend.
/// (The connectrpc clients dial the proxy and set the authority to the same
/// loopback host, so the service host must be `127.0.0.1`, not a virtual name.)
fn proxy_config(backend_port: u16) -> ProxyConfig {
    let svc = ServiceConfig::new("rpc", "127.0.0.1")
        .backend(&format!("http://127.0.0.1:{backend_port}"));
    ProxyConfig::new("127.0.0.1", "127.0.0.1")
        .bind("127.0.0.1:0")
        .service(svc)
}

/// A proxy whose backend is declared `grpc://` (native gRPC), selecting the
/// D-leg relay instead of the transparent HTTP forward.
fn proxy_config_grpc(backend_port: u16) -> ProxyConfig {
    let svc = ServiceConfig::new("rpc", "127.0.0.1")
        .backend(&format!("grpc://127.0.0.1:{backend_port}"));
    ProxyConfig::new("127.0.0.1", "127.0.0.1")
        .bind("127.0.0.1:0")
        .service(svc)
}

/// A proxy whose service routes to several `grpc://` backends (equal weight),
/// for stream-level load-balancing tests.
fn proxy_config_grpc_multi(ports: &[u16]) -> ProxyConfig {
    let mut svc = ServiceConfig::new("rpc", "127.0.0.1");
    for port in ports {
        svc = svc.backend(&format!("grpc://127.0.0.1:{port}"));
    }
    ProxyConfig::new("127.0.0.1", "127.0.0.1")
        .bind("127.0.0.1:0")
        .service(svc)
}

/// Per-test hang guard for this suite, in milliseconds.
///
/// WHY: a hang here once ran until CI's 60-minute job timeout. With this, a
/// hung test fails on its own and the pool is torn down for the next test (see
/// `#[valtron_test(timeout = ..)]`). It is a safety net, not a fix: a passing
/// test takes well under a second.
///
/// Default 30 s. Override with `RPC_E2E_TEST_TIMEOUT_MS` (e.g. for slow
/// instrumented builds).
fn test_timeout_ms() -> u64 {
    const DEFAULT_TIMEOUT_MS: u64 = 30_000;
    std::env::var("RPC_E2E_TEST_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

/// A typed client over HTTP/1.1 speaking the Connect protocol.
fn connect_h1_client(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> =
        Arc::new(H1Transport::new(HttpClientBuilder::new().build()));
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_connect().with_codec("json"),
    )
    .expect("connect h1 client")
}

/// A typed client over HTTP/2 speaking the Connect protocol.
fn connect_h2_client(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_connect().with_codec("json"),
    )
    .expect("connect h2 client")
}

/// A typed client over HTTP/2 speaking gRPC.
fn grpc_client(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_grpc().with_codec("json"),
    )
    .expect("grpc client")
}

/// A typed client over HTTP/1.1 speaking gRPC-Web.
fn grpc_web_client(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> =
        Arc::new(H1Transport::new(HttpClientBuilder::new().build()));
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_grpc_web().with_codec("json"),
    )
    .expect("grpc-web client")
}

/// Read a server-stream to completion and return the message texts in order.
async fn collect_stream(client: &Client<Msg, Msg>, seed: &str) -> Vec<String> {
    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of(seed)))
        .await
        .expect("open server stream");
    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    got
}

// ── Connect: unary + server-streaming over the H1 and H2 legs ───────────────

/// WHY: Connect is HTTP/1.1-native and must pass through the proxy's plain H1
/// forward leg untouched, over the H1 front end.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_unary_via_h1_proxy() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h1_client(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("connect-h1")))
        .await
        .expect("connect unary");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(resp.msg, Msg::of("echo:connect-h1"), "Connect unary over H1 must round-trip");
}

/// WHY: Connect server-streaming (framed `application/connect+json`) must flow
/// through the H1 leg without buffering or dropping the envelope frames.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_server_stream_via_h1_proxy() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h1_client(&addr, SERVER_STREAM);
    let got = collect_stream(&client, "connect-h1-stream").await;

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        got,
        vec!["connect-h1-stream-0", "connect-h1-stream-1", "connect-h1-stream-2"],
        "Connect server-stream over H1 must deliver all frames in order"
    );
}

/// WHY: Connect over an H2 front end is legal (Connect runs over HTTP/2 too);
/// the proxy must not misroute it — it is not `application/grpc`, so it takes
/// the HTTP/1.1 upstream leg without dropping the call.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_unary_via_h2_proxy() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h2_client(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("connect-h2")))
        .await
        .expect("connect unary");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(resp.msg, Msg::of("echo:connect-h2"), "Connect unary over H2 must round-trip");
}

/// WHY: Connect server-streaming over the H2 front end exercises the streaming
/// forward path through the H2 front end's downgrade-to-H1 upstream leg.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_server_stream_via_h2_proxy() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h2_client(&addr, SERVER_STREAM);
    let got = collect_stream(&client, "connect-h2-stream").await;

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        got,
        vec!["connect-h2-stream-0", "connect-h2-stream-1", "connect-h2-stream-2"],
        "Connect server-stream over H2 must deliver all frames in order"
    );
}

// ── gRPC: unary + streaming with `grpc-status` intact ───────────────────────

/// WHY: gRPC requires HTTP/2 end-to-end and delivers status in trailing HEADERS.
/// The proxy's H2 leg (`application/grpc`) must relay the backend's `grpc-status`
/// trailer to the client — a lost trailer would surface as a silent `Ok(default)`.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn grpc_unary_via_proxy_carries_status() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = grpc_client(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("grpc")))
        .await
        .expect("grpc unary");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(resp.msg, Msg::of("echo:grpc"), "gRPC unary must round-trip");

    let trailers = resp.trailers();
    let grpc_status = trailers
        .get(&SimpleHeader::GRPC_STATUS)
        .and_then(|v| v.first())
        .map(|s| s.as_str());
    assert_eq!(grpc_status, Some("0"), "grpc-status must be relayed as 0");
}

/// WHY: gRPC server-streaming over the proxy's H2 leg must deliver every message
/// in order *and* still carry `grpc-status` after the stream ends.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn grpc_server_stream_via_proxy_carries_status() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = grpc_client(&addr, SERVER_STREAM);
    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("grpc-stream")))
        .await
        .expect("grpc server stream");
    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    let trailers = stream
        .response_trailers()
        .await
        .expect("grpc stream trailers");
    let grpc_status = trailers
        .get(&SimpleHeader::GRPC_STATUS)
        .and_then(|v| v.first())
        .map(|s| s.as_str());

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        got,
        vec!["grpc-stream-0", "grpc-stream-1", "grpc-stream-2"],
        "gRPC server-stream must deliver all messages in order"
    );
    assert_eq!(grpc_status, Some("0"), "grpc-status must follow the stream");
}

// ── gRPC-Web: unary + server-streaming ──────────────────────────────────────

/// WHY: gRPC-Web is browser-compatible and HTTP/1.1-transparent; it must pass
/// through the proxy's H1 leg to the backend's gRPC-Web dispatch untouched.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn grpc_web_unary_via_proxy() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = grpc_web_client(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("grpc-web")))
        .await
        .expect("grpc-web unary");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(resp.msg, Msg::of("echo:grpc-web"), "gRPC-Web unary must round-trip");
}

/// WHY: gRPC-Web server-streaming carries its status in a final body frame (not
/// trailers); the proxy's transparent H1 leg must relay every frame so the
/// client can reassemble the stream.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn grpc_web_server_stream_via_proxy() {
    let (backend_port, backend_shutdown) = start_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = grpc_web_client(&addr, SERVER_STREAM);
    let got = collect_stream(&client, "grpc-web-stream").await;

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        got,
        vec!["grpc-web-stream-0", "grpc-web-stream-1", "grpc-web-stream-2"],
        "gRPC-Web server-stream must deliver all frames in order"
    );
}

// ── D leg: gRPC-Web → native gRPC translation ───────────────────────────────

/// WHY: a `grpc://` backend serves only native gRPC over HTTP/2 — it has no
/// gRPC-Web dispatch at all. An inbound gRPC-Web unary call (HTTP/1.1) must be
/// translated at the frame level: de-framed by `GrpcWebHandler` and re-enveloped
/// as native gRPC by `GrpcClient` over `H2Transport`, then translated back.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn grpc_web_unary_via_grpc_backend() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = grpc_web_client(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("grpc-web-relay")))
        .await
        .expect("grpc-web unary through grpc backend");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        resp.msg,
        Msg::of("echo:grpc-web-relay"),
        "gRPC-Web unary must be translated to native gRPC and back"
    );
}

/// WHY: a server-stream through the D leg must be relayed frame-by-frame (not
/// buffered whole into a single envelope), and the terminal `grpc-status` must
/// survive the gRPC→gRPC-Web trailer-frame translation — a dropped status would
/// surface as a silent `Ok(default)` on the client.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn grpc_web_server_stream_via_grpc_backend_carries_status() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = grpc_web_client(&addr, SERVER_STREAM);
    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("grpc-web-relay-stream")))
        .await
        .expect("grpc-web server stream through grpc backend");
    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    let trailers = stream
        .response_trailers()
        .await
        .expect("grpc-web stream trailers");
    let grpc_status = trailers
        .get(&SimpleHeader::GRPC_STATUS)
        .and_then(|v| v.first())
        .map(|s| s.as_str());

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        got,
        vec![
            "grpc-web-relay-stream-0",
            "grpc-web-relay-stream-1",
            "grpc-web-relay-stream-2"
        ],
        "gRPC-Web server-stream through a grpc:// backend must deliver all frames in order"
    );
    assert_eq!(
        grpc_status, Some("0"),
        "grpc-status must be translated into the gRPC-Web trailer frame"
    );
}

/// WHY: gRPC-Web **text** mode base64-encodes the whole body. The relay must
/// preserve text mode end-to-end: de-frame a base64 request, re-envelope as
/// native gRPC, and base64-encode the framed response back.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn grpc_web_text_unary_via_grpc_backend() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    // Build the request by hand: one `0x00` message envelope (JSON), then
    // base64-encode the framed bytes as the gRPC-Web-text body.
    let msg_json = serde_json::to_vec(&Msg::of("grpc-web-text-relay")).expect("encode msg");
    let mut frame = Vec::new();
    Envelope {
        flags: 0,
        data: Bytes::from(msg_json),
    }
    .encode_into(&mut frame);
    let body = base64::engine::general_purpose::STANDARD.encode(&frame);

    // Add the content-type as a *new* header, not via `.headers(..)` — that
    // replaces the whole map and drops the `Host` header the builder inserted,
    // which the proxy router needs to match the service.
    let client = HttpClientBuilder::new().build();
    let request = PreparedRequestBuilder::post(&format!("http://{addr}{UNARY}"))
        .expect("build text request")
        .header(SimpleHeader::CONTENT_TYPE, "application/grpc-web-text+json")
        .body_bytes(body.into_bytes())
        .build();
    let resp = client.send_async(request).await.expect("send");
    let (status, resp_headers, resp_body) = resp.into_parts();

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(status, Status::OK, "gRPC-Web-text unary must answer 200");
    assert_eq!(
        resp_headers
            .get(&SimpleHeader::CONTENT_TYPE)
            .and_then(|v| v.first())
            .map(|s| s.as_str()),
        Some("application/grpc-web-text+json"),
        "text mode must be echoed back in the response content-type"
    );

    // Decode the base64 body back into a message envelope + trailer envelope.
    let raw = base64::engine::general_purpose::STANDARD
        .decode(collect_bytes_from_send_safe(resp_body))
        .expect("response body is valid base64");
    let bytes = Bytes::from(raw);

    let (msg_env, used) = Envelope::decode(&bytes).expect("message envelope");
    assert_eq!(msg_env.flags, 0, "first frame is a plain message");
    let msg: Msg = serde_json::from_slice(&msg_env.data).expect("message json");
    assert_eq!(
        msg,
        Msg::of("echo:grpc-web-text-relay"),
        "gRPC-Web-text unary must round-trip through native gRPC"
    );

    let (trailer_env, _) = Envelope::decode(&bytes.slice(used..)).expect("trailer envelope");
    assert_eq!(
        trailer_env.flags,
        Envelope::FLAG_TRAILER,
        "second frame is the 0x80 trailer"
    );
    let trailer_text = String::from_utf8_lossy(&trailer_env.data);
    assert!(
        trailer_text.contains("grpc-status: 0"),
        "trailer must carry grpc-status 0, got: {trailer_text:?}"
    );
}

// ── D leg: Connect → native gRPC translation ────────────────────────────────

/// WHY: a `grpc://` backend serves only native gRPC. An inbound Connect **unary**
/// call (bare `application/json` body) must be translated: de-framed as bare
/// bytes, enveloped as a single gRPC message, and the single gRPC response
/// message decoded back to a bare Connect body.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_unary_via_grpc_backend() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h1_client(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("connect-relay")))
        .await
        .expect("connect unary through grpc backend");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        resp.msg,
        Msg::of("echo:connect-relay"),
        "Connect unary must be translated to native gRPC and back"
    );
}

/// WHY: a Connect server-stream through the D leg must be relayed frame-by-frame
/// (not buffered whole), with the streaming shape (`application/connect+json`)
/// preserved on the way back.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_server_stream_via_grpc_backend() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h1_client(&addr, SERVER_STREAM);
    let got = collect_stream(&client, "connect-relay-stream").await;

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        got,
        vec![
            "connect-relay-stream-0",
            "connect-relay-stream-1",
            "connect-relay-stream-2"
        ],
        "Connect server-stream through a grpc:// backend must deliver all frames in order"
    );
}

/// WHY: a Connect client-stream (many enveloped request frames, one response)
/// must flow through the relay in the request direction, which the unary path
/// never exercises.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_client_stream_via_grpc_backend() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h1_client(&addr, CLIENT_STREAM);
    let reqs = futures_util::stream::iter(vec![Msg::of("a"), Msg::of("b"), Msg::of("c")]);
    let resp = client
        .client_stream(ctx(), reqs)
        .await
        .expect("connect client stream through grpc backend");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        resp.msg,
        Msg::of("a+b+c"),
        "Connect client-stream must be translated to native gRPC and folded back"
    );
}

/// WHY: Connect **bidi** (full-duplex) is the one stream shape Workstream 1 left
/// unexercised, and it needs the H2 front end — the H1 leg is half-duplex. Each
/// request message must be echoed back as it is sent, in both directions, through
/// the D-leg translation to a native-gRPC backend.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_bidi_via_grpc_backend() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h2_client(&addr, BIDI_STREAM);
    // `bidi_stream` hands back a drive-it-yourself `BidiStream`; the request is
    // sent by explicit `send` calls, not the (ignored) input stream.
    let mut stream = client
        .bidi_stream(ctx(), futures_util::stream::empty())
        .await
        .expect("connect bidi stream through grpc backend");

    stream.send(&Msg::of("a")).await.expect("send a");
    stream.send(&Msg::of("b")).await.expect("send b");
    stream.send(&Msg::of("c")).await.expect("send c");
    stream.close_request().await.expect("close request");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        got,
        vec!["re:a", "re:b", "re:c"],
        "Connect bidi must echo every request message back through native gRPC"
    );
}

/// WHY: a Connect **unary** call arriving over the H2 front end (bare
/// `application/json` body) to a `grpc://` backend must take the same
/// bare-bytes↔envelope translation as the H1 leg — the H2 leg must not downgrade
/// it to H1 (the backend is H2-only) nor misroute it as streaming.
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_unary_via_grpc_backend_h2() {
    let (backend_port, backend_shutdown) = start_backend_grpc_only();
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h2_client(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("connect-h2-relay")))
        .await
        .expect("connect unary through grpc backend over h2");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(
        resp.msg,
        Msg::of("echo:connect-h2-relay"),
        "Connect unary over H2 must be translated to native gRPC and back"
    );
}

// ── D leg: stream-level load balancing ──────────────────────────────────────

/// WHY: with more than one `grpc://` backend, each relayed RPC must pick a
/// backend at open time — not pin to the first healthy one — so concurrent
/// streams spread across the pool.
/// WHAT: two tagged backends behind one proxy; many concurrent Connect unary
/// calls over the H2 front end; both tags must be observed (neither backend sits
/// idle while the other serves everything).
#[valtron_test(seed = 63, threads = 8, timeout = test_timeout_ms())]
async fn connect_streams_spread_across_grpc_backends() {
    let (port_a, shutdown_a) = start_backend_grpc_only_tagged("A");
    let (port_b, shutdown_b) = start_backend_grpc_only_tagged("B");
    let proxy =
        ProxyServer::start(proxy_config_grpc_multi(&[port_a, port_b])).await.expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    // Each call dials the proxy with a fresh H2 connection (H2Transport is
    // one-connection-per-call), so the proxy performs a fresh backend pick per
    // RPC — the smooth weighted round-robin must alternate across the two.
    let futs = (0..12).map(|i| {
        let addr = addr.clone();
        async move {
            let client = connect_h2_client(&addr, UNARY);
            client
                .unary(ctx(), Request::new(Msg::of(format!("lb-{i}"))))
                .await
                .expect("connect unary")
                .msg
                .text
        }
    });
    let tags: Vec<String> = futures_util::future::join_all(futs).await;

    proxy.shutdown_async().await;
    shutdown_a.turn_on();
    shutdown_b.turn_on();

    let seen_a = tags.iter().any(|t| t.starts_with("A:"));
    let seen_b = tags.iter().any(|t| t.starts_with("B:"));
    assert!(seen_a, "backend A must observe traffic, got: {tags:?}");
    assert!(seen_b, "backend B must observe traffic, got: {tags:?}");
}
