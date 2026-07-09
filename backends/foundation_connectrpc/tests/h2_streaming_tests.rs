//! All four ConnectRPC modes over HTTP/2 cleartext, end-to-end (F47).
//!
//! WHY: `h2_echo` proves unary over h2c. F47's acceptance criteria also require
//! server-stream, client-stream and bidi, plus evidence that the h2 connection
//! poll loop never blocks on a handler — otherwise "multiplexed" is a claim, not
//! a property.
//!
//! WHAT: A real `HttpServer` serving `ServerApp::http2`, driven by the real
//! `H2Transport` client over a loopback TCP socket. No hand-rolled framing, no
//! mock transport — everything goes through the production path.
//!
//! HOW: One router registers all four procedures. Each test spins the server on
//! its own thread (the accept loop blocks; connections themselves are valtron
//! tasks) and drives a typed `Client` under `#[valtron_test]`.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::StreamExt;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron_test;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::serve::h2::H2Serve;

use foundation_connectrpc::error::ConnectResult;
use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, HandlerOptions, JsonCodec,
    ProcedureCodecs, Request, RequestStream, Response, Router,
};

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

impl Msg {
    fn of(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

const UNARY: &str = "/t.Svc/Unary";
const SERVER_STREAM: &str = "/t.Svc/ServerStream";
const CLIENT_STREAM: &str = "/t.Svc/ClientStream";
const BIDI: &str = "/t.Svc/Bidi";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

/// One router carrying all four procedures.
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

    // Fan one request out into three responses.
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

    // Fold many requests into one response.
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

    // Echo each request as it arrives.
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

/// Start a real `HttpServer` speaking h2 only. Returns its address and a
/// shutdown signal. Binding happens before the thread spawns, so the client can
/// never race the listener.
fn start_h2_server() -> (String, Arc<OnSignal>) {
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(build_router().into_handler()));

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

fn client_for(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        // The router registers JSON only; the default codec is `proto`.
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client")
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

#[valtron_test]
async fn unary_over_h2c() {
    let (addr, shutdown) = start_h2_server();

    let client = client_for(&addr, UNARY);
    let resp = client
        .unary(ctx(), Request::new(Msg::of("hello")))
        .await
        .expect("unary");

    assert_eq!(resp.msg, Msg::of("echo:hello"));
    shutdown.turn_on();
}

/// The handler yields three messages; each must arrive as its own h2 DATA frame
/// payload, in order, followed by a clean end-of-stream.
#[valtron_test]
async fn server_stream_over_h2c() {
    let (addr, shutdown) = start_h2_server();

    let client = client_for(&addr, SERVER_STREAM);
    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("item")))
        .await
        .expect("open server stream");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }

    assert_eq!(got, vec!["item-0", "item-1", "item-2"]);
    shutdown.turn_on();
}

/// The client sends three messages as separate h2 DATA frames before closing the
/// request direction; the handler folds them into one response. This is the mode
/// that the old pump could not express — it committed END_STREAM on HEADERS.
#[valtron_test]
async fn client_stream_over_h2c() {
    let (addr, shutdown) = start_h2_server();

    let client = client_for(&addr, CLIENT_STREAM);
    let outbound = futures::stream::iter(vec![Msg::of("a"), Msg::of("b"), Msg::of("c")]);
    let resp = client
        .client_stream(ctx(), outbound)
        .await
        .expect("client stream");

    assert_eq!(resp.msg, Msg::of("a+b+c"));
    shutdown.turn_on();
}

/// Send-all-then-read bidi. Weaker than the interleaved test below — a client
/// that buffers the whole request before sending HEADERS would still pass this —
/// but it pins the simple case.
#[valtron_test]
async fn bidi_stream_over_h2c() {
    let (addr, shutdown) = start_h2_server();

    let client = client_for(&addr, BIDI);
    let mut stream = client
        .bidi_stream(ctx(), futures::stream::iter(Vec::<Msg>::new()))
        .await
        .expect("open bidi");

    for text in ["x", "y", "z"] {
        stream.send(&Msg::of(text)).await.expect("send");
    }
    stream.close_request().await.expect("close request");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }

    assert_eq!(got, vec!["re:x", "re:y", "re:z"]);
    shutdown.turn_on();
}

/// True full duplex: read each echo *before* sending the next message, with the
/// request direction still open throughout.
///
/// This is the test that a buffering client cannot pass. A pump that waits for
/// `send_rx` to close before emitting HEADERS deadlocks here: the first
/// `receive()` blocks on a response that the server cannot produce, because the
/// request it is waiting on has not left the client.
#[valtron_test]
async fn bidi_stream_interleaves_send_and_receive() {
    let (addr, shutdown) = start_h2_server();

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
            .expect("an echo per message, before the request closes");
        assert_eq!(echo, Msg::of(format!("re:{text}")));
    }

    stream.close_request().await.expect("close request");
    assert!(
        stream.receive().await.expect("receive").is_none(),
        "stream ends once the request direction closes"
    );

    shutdown.turn_on();
}

/// The connection poll loop must never block on a handler. Two calls issued
/// against the same server make progress independently; if `serve_h2` were run
/// inline on the poll loop, a slow stream would stall the other.
#[valtron_test]
async fn two_concurrent_calls_both_complete() {
    let (addr, shutdown) = start_h2_server();

    let unary_client = client_for(&addr, UNARY);
    let stream_client = client_for(&addr, SERVER_STREAM);

    let unary_fut = unary_client.unary(ctx(), Request::new(Msg::of("one")));
    let stream_fut = stream_client.server_stream(ctx(), Request::new(Msg::of("many")));

    let (unary_res, stream_res) = futures::join!(unary_fut, stream_fut);

    assert_eq!(unary_res.expect("unary").msg, Msg::of("echo:one"));

    let mut stream = stream_res.expect("server stream");
    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    assert_eq!(got, vec!["many-0", "many-1", "many-2"]);

    shutdown.turn_on();
}
