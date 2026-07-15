//! ConnectRPC over HTTP/2 cleartext (h2c) — all four RPC modes.
//!
//! Run with: `cargo run -p foundation_connectrpc --example h2_echo`
//!
//! Server: the real `HttpServer` detects the h2c preface, spawns one valtron
//! task per connection, and `H2ConnectionHandler` spawns one task per stream.
//! Nothing hand-rolls a connection loop and nothing blocks on a handler.
//!
//! Client: `H2Transport` + typed `Client` with Connect protocol (the default).
//! Call `.unary()`, `.server_stream()`, `.client_stream()`, `.bidi_stream()` —
//! the library handles framing, envelopes, and end-of-stream.
//!
//! For the same surface over gRPC, see `examples/grpc_echo`.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::StreamExt;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::native::serve::H2Serve;

use foundation_connectrpc::shared::error::ConnectResult;
use foundation_connectrpc::shared::transport::Transport;
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

// ── Router — four procedures, one router ──────────────────────────────────────

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

// ── Server ────────────────────────────────────────────────────────────────────

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

// ── Client helper ─────────────────────────────────────────────────────────────

fn client_for(addr: &str, procedure: &str) -> Client<Msg, Msg> {
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

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

// ── Entry point — all four modes, one run ─────────────────────────────────────

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let (addr, shutdown) = start_server();
    println!("h2c server on {addr}");

    // ── Unary ──────────────────────────────────────────────────────────────

    let client = client_for(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("hello")))
        .await
        .expect("unary");
    println!("[unary] {}", resp.msg.text);
    assert_eq!(resp.msg, Msg::of("echo:hello"));

    // ── Server stream ──────────────────────────────────────────────────────

    let client = client_for(&addr, SERVER_STREAM);
    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("s")))
        .await
        .expect("open server stream");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    println!("[server_stream] received: {got:?}");
    assert_eq!(got, vec!["s-0", "s-1", "s-2"]);

    // ── Client stream ──────────────────────────────────────────────────────

    let client = client_for(&addr, CLIENT_STREAM);
    let outbound = futures::stream::iter(vec![Msg::of("a"), Msg::of("b"), Msg::of("c")]);
    let resp = client
        .client_stream(ctx(), outbound)
        .await
        .expect("client stream");
    println!("[client_stream] {}", resp.msg.text);
    assert_eq!(resp.msg, Msg::of("a+b+c"));

    // ── Bidi (interleaved) ─────────────────────────────────────────────────

    let client = client_for(&addr, BIDI);
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
    println!("[bidi] interleaved send/receive OK");

    println!("\nall four h2c modes verified ✓");
    shutdown.turn_on();
}
