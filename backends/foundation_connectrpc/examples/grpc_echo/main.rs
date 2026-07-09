//! gRPC over HTTP/2 cleartext (h2c) — all four RPC modes.
//!
//! Run with: `cargo run -p foundation_connectrpc --example grpc_echo`
//!
//! The server is **identical** to `examples/h2_echo` — the same router
//! dispatches Connect and gRPC calls automatically based on `Content-Type`.
//! The client adds `.with_grpc()`; everything else stays the same.
//!
//! Wire difference: gRPC always returns HTTP 200, errors ride `grpc-status`
//! trailing headers, and every message is enveloped (even unary). The library
//! handles all of that — from user code, it's the same `Client<Req, Res>`
//! with the same four method names.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::StreamExt;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::serve::h2::H2Serve;

use foundation_connectrpc::error::ConnectResult;
use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, HandlerOptions, JsonCodec,
    ProcedureCodecs, Request, RequestStream, Response, Router,
};

// ── Message type — plain serde, JSON-only ─────────────────────────────────────

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

impl Msg {
    fn of(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

const UNARY: &str = "/echo.Svc/Unary";
const SERVER_STREAM: &str = "/echo.Svc/ServerStream";
const CLIENT_STREAM: &str = "/echo.Svc/ClientStream";
const BIDI: &str = "/echo.Svc/Bidi";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

// ── Router — identical to h2_echo (plus a unary we don't call here) ──────────

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
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );

    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        |_ctx: Ctx, mut reqs: RequestStream<Msg>| async move {
            let mut parts = Vec::new();
            while let Some(msg) = reqs.next().await {
                parts.push(msg?.text);
            }
            Ok(Response::new(Msg::of(parts.join("+"))))
        },
        HandlerOptions::new(),
    );

    router.bidi_stream(
        BIDI,
        codecs(),
        |_ctx: Ctx, reqs: RequestStream<Msg>| async move {
            Ok(reqs.map(|msg| msg.map(|m| Msg::of(format!("re:{}", m.text)))))
        },
        HandlerOptions::new(),
    );

    router
}

// ── Server — identical to h2_echo ────────────────────────────────────────────

fn start_server() -> (String, Arc<OnSignal>) {
    let rpc: Arc<dyn H2Serve> =
        Arc::new(ConnectRpcServeH2::new(build_router().into_handler()));

    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(UNARY, rpc.clone());
    app.route_any_h2(SERVER_STREAM, rpc.clone());
    app.route_any_h2(CLIENT_STREAM, rpc.clone());
    app.route_any_h2(BIDI, rpc);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(OnSignal::new());

    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    (addr.to_string(), shutdown)
}

// ── Client helper — `.with_grpc()` is the only difference from h2_echo ───────

/// Build a gRPC client. The `.with_grpc()` call switches:
/// 1. Content-Type: `application/grpc+json` (instead of `application/connect+json`)
/// 2. Wire framing: gRPC envelopes on every message
/// 3. Errors: `grpc-status` in trailing headers (instead of HTTP status codes)
fn grpc_client_for(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_grpc().with_codec("json"),
    )
    .expect("build client")
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

// ── Entry point — all four modes, gRPC framing ───────────────────────────────

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let (addr, shutdown) = start_server();
    println!("h2c server on {addr} (serving both Connect and gRPC)");

    // ── gRPC unary ────────────────────────────────────────────────────────
    //
    // On the wire: single enveloped request → single enveloped response.
    // User code: the same `client.unary(ctx, request)` as Connect.

    let client = grpc_client_for(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("hello")))
        .await
        .expect("unary");
    println!("[grpc unary] {}", resp.msg.text);
    assert_eq!(resp.msg, Msg::of("echo:hello"));

    // ── gRPC server stream ────────────────────────────────────────────────
    //
    // One request → N enveloped response messages. The `GrpcClient` writer
    // task envelopes the request; the reader task de-envelopes each response.

    let client = grpc_client_for(&addr, SERVER_STREAM);
    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("s")))
        .await
        .expect("open server stream");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    println!("[grpc server_stream] received: {got:?}");
    assert_eq!(got, vec!["s-0", "s-1", "s-2"]);

    // ── gRPC client stream ────────────────────────────────────────────────
    //
    // N enveloped request messages → one enveloped response. The writer task
    // envelopes each outgoing message; the reader task de-envelopes the response.

    let client = grpc_client_for(&addr, CLIENT_STREAM);
    let outbound = futures::stream::iter(vec![Msg::of("a"), Msg::of("b"), Msg::of("c")]);
    let resp = client
        .client_stream(ctx(), outbound)
        .await
        .expect("client stream");
    println!("[grpc client_stream] {}", resp.msg.text);
    assert_eq!(resp.msg, Msg::of("a+b+c"));

    // ── gRPC bidi (interleaved) ───────────────────────────────────────────
    //
    // Send a message, receive its echo, repeat — full duplex over gRPC. The
    // reader and writer tasks run concurrently; the typed `BidiStream` handle
    // exposes `.send()` and `.receive()`.

    let client = grpc_client_for(&addr, BIDI);
    let mut stream = client
        .bidi_stream(ctx(), futures::stream::iter(Vec::<Msg>::new()))
        .await
        .expect("open bidi");

    for text in ["x", "y", "z"] {
        stream.send(&Msg::of(text)).await.expect("send");
        let echo = stream
            .receive()
            .await
            .expect("receive")
            .expect("echo per message");
        assert_eq!(echo, Msg::of(format!("re:{text}")));
    }
    stream.close_request().await.expect("close request");
    assert!(
        stream.receive().await.expect("receive").is_none(),
        "stream ends"
    );
    println!("[grpc bidi] interleaved send/receive OK");

    println!("\nall four gRPC modes verified ✓");
    shutdown.turn_on();
}
