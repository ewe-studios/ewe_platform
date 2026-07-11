//! Server-streaming RPC over a real loopback socket — **JSON-only, no protobuf**.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_connectrpc --example server_streaming
//! ```
//!
//! One request in, a *stream* of responses out. The handler returns any
//! `futures::Stream<Item = ConnectResult<Res>>`; the client drains it with
//! `stream.receive().await` until it yields `None`. On the wire this is the
//! Connect streaming framing (5-byte enveloped frames + a terminating EndStream
//! frame); the library handles all of that for you.
//!
//! Because the route registers `ProcedureCodecs::of((JsonCodec,))` — a JSON-only
//! table — the message types need **only serde** (`JsonCodec` is bounded on
//! `Serialize + DeserializeOwned`, not `buffa::Message`). No buffa, no hand-written
//! protobuf encoding, no `DefaultInstance` statics. The client must ask for `json`
//! (the client's default wire codec is `proto`), which a JSON-only table lacks.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_netio::simple_http::client::SimpleHttpClient;

use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectResult, ConnectRpcServe, Ctx, H1Transport, HandlerOptions,
    JsonCodec, ProcedureCodecs, Request, Router,
};

const PROCEDURE: &str = "/demo.CountService/Count";

// ── Messages — plain serde types, nothing else ──────────────────────────────

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
struct CountRequest {
    /// How many items to stream back.
    count: i32,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
struct CountItem {
    index: i32,
    label: String,
}

// ── Server ───────────────────────────────────────────────────────────────────

fn count_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.server_stream(
        PROCEDURE,
        // JSON-only codec table → serde-only message types, no protobuf.
        ProcedureCodecs::<CountRequest, CountItem>::of((JsonCodec,)),
        |_ctx: Ctx, req: Request<CountRequest>| async move {
            // `.max(0)` floors a negative `count` to 0 (stream nothing), rather
            // than capping — it returns the larger of `count` and `0`.
            let n = req.msg.count.max(0);
            let items: Vec<ConnectResult<CountItem>> = (0..n)
                .map(|i| Ok(CountItem { index: i, label: format!("item-{i}") }))
                .collect();
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

fn start_server() -> (std::net::SocketAddr, Arc<OnSignal>) {
    let mut app = HttpApp::new_serve();
    let serve = count_serve();
    app.router.add_route_any(PROCEDURE, &serve);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let server =
        HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), ServerConfig::defaults());

    let shutdown = Arc::new(OnSignal::new());
    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &shutdown_thread));
    while std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err() {}

    (addr, shutdown)
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let (addr, shutdown) = start_server();

    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(Arc::new(SimpleHttpClient::from_system())));
    let url = format!("http://127.0.0.1:{}{PROCEDURE}", addr.port());
    let client: Client<CountRequest, CountItem> = Client::new(
        transport,
        &url,
        ProcedureCodecs::<CountRequest, CountItem>::of((JsonCodec,)),
        // The client's default wire codec is `proto`; this route is JSON-only,
        // so select `json` explicitly.
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client");

    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let mut stream = client
        .server_stream(ctx, Request::new(CountRequest { count: 3 }))
        .await
        .expect("open server stream");

    let mut received = Vec::new();
    while let Some(item) = stream.receive().await.expect("receive item") {
        println!("streamed: index={} label={:?}", item.index, item.label);
        received.push(item);
    }

    assert_eq!(received.len(), 3, "expected 3 streamed items");
    assert_eq!(received[2].label, "item-2");
    println!("server-stream round-trip verified ✓ ({} items)", received.len());

    shutdown.turn_on();
}
