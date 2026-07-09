//! ConnectRPC over HTTP/2 cleartext (h2c) — same Router/Client API as unary_echo.
//!
//! Run with: `cargo run -p foundation_connectrpc --example h2_echo`
//!
//! Server: the real `HttpServer`. It already spawns one valtron task per accepted
//! connection, detects the h2c preface, and runs `H2ConnectionHandler` — which in
//! turn spawns one task per h2 stream. Nothing here hand-rolls a connection loop
//! and nothing blocks on a handler.
//! Client: `H2Transport` + typed `Client::unary()` under `#[valtron]`.
//! Both over a real loopback TCP socket.

use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::serve::h2::H2Serve;

use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, HandlerOptions, JsonCodec,
    ProcedureCodecs, Request, Response, Router,
};

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
struct EchoMsg {
    text: String,
}

const ECHO_PATH: &str = "/echo.EchoService/Echo";

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let mut router = Router::new();
    router.unary(
        ECHO_PATH,
        ProcedureCodecs::<EchoMsg, EchoMsg>::of((JsonCodec,)),
        |_ctx: Ctx, req: Request<EchoMsg>| async move {
            Ok(Response::new(EchoMsg {
                text: format!("ECHO: {}", req.msg.text),
            }))
        },
        HandlerOptions::new(),
    );

    // One ConnectRPC handler serves every h2 stream on every connection.
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(ECHO_PATH, rpc);

    // Bind before spawning so the client can never race the listener.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(OnSignal::new());

    // `serve_with_listener` blocks its caller until shutdown, so it gets a thread
    // of its own. Connections themselves never get a thread: the accept loop
    // hands each one to valtron.
    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });
    println!("h2c server on {addr}");

    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{ECHO_PATH}");
    // The router registers JSON only, so the client must ask for it — the
    // default codec is `proto`.
    let client: Client<EchoMsg, EchoMsg> = Client::new(
        transport,
        &url,
        ProcedureCodecs::<EchoMsg, EchoMsg>::of((JsonCodec,)),
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client");

    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let resp = client
        .unary(
            ctx,
            Request::new(EchoMsg {
                text: "hello-h2".into(),
            }),
        )
        .await
        .expect("unary");

    println!("[client] response: {:?}", resp.msg.text);
    assert_eq!(resp.msg.text, "ECHO: hello-h2");
    println!("h2 round-trip verified ✓");
    shutdown.turn_on();
}
