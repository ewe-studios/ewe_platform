//! Code-first **server-streaming** service with `#[service]`
//! (Feature 27, Decision 10 Mode 3) — round-tripped over a real loopback socket.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_connectrpc --example code_first_streaming
//! ```
//!
//! The trait method is authored as `-> ConnectResult<impl Stream<Item = …>>`.
//! The macro can't emit that shape verbatim (`impl Trait` nested in
//! `ConnectResult` / `impl Future` is illegal), so it rewrites the generated trait
//! to a **boxed** stream: `ConnectResult<Pin<Box<dyn Stream<…> + Send>>>`. The
//! implementor therefore returns `Ok(Box::pin(stream))`.

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_netio::http::SimpleHttpClient;

use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    service, ClientOptions, ConnectResult, ConnectRpcServe, Ctx, H1Transport, Request, Router,
};

// Plain serde types — `codecs(json)`, no protobuf.
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct TickRequest {
    count: i32,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tick {
    seq: i32,
}

#[service(package = "demo.tick.v1", codecs(json))]
pub trait TickService {
    async fn subscribe(
        &self,
        _ctx: Ctx,
        _req: Request<TickRequest>,
    ) -> ConnectResult<impl futures::Stream<Item = ConnectResult<Tick>>>;
}

struct Ticker;

impl TickService for Ticker {
    // The generated trait method returns a boxed stream, so the impl does too.
    async fn subscribe(
        &self,
        _ctx: Ctx,
        req: Request<TickRequest>,
    ) -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<Tick>> + Send>>> {
        let n = req.msg.count.max(0);
        let items: Vec<ConnectResult<Tick>> = (0..n).map(|i| Ok(Tick { seq: i })).collect();
        Ok(Box::pin(futures::stream::iter(items)))
    }
}

fn start_server() -> (std::net::SocketAddr, Arc<OnSignal>) {
    let mut router = Router::new();
    register_tick_service(&mut router, Arc::new(Ticker));
    let serve: Arc<dyn foundation_http::shared::serve::Serve> =
        Arc::new(ConnectRpcServe::new(router.into_handler()));

    let mut app = HttpApp::new_serve();
    app.router.add_route_any(procedure::SUBSCRIBE, &serve);

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

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let (addr, shutdown) = start_server();

    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(Arc::new(SimpleHttpClient::from_system())));
    let base_url = format!("http://127.0.0.1:{}", addr.port());
    let client =
        TickServiceClient::new(transport, &base_url, ClientOptions::new().with_codec("json"))
            .expect("build client");

    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let mut stream = client
        .subscribe(ctx, Request::new(TickRequest { count: 3 }))
        .await
        .expect("open stream");

    let mut ticks = Vec::new();
    while let Some(tick) = stream.receive().await.expect("receive tick") {
        println!("tick: seq={}", tick.seq);
        ticks.push(tick);
    }

    assert_eq!(ticks.len(), 3);
    assert_eq!(ticks[2].seq, 2);
    println!("code-first server-streaming round-trip verified ✓ ({} ticks)", ticks.len());

    shutdown.turn_on();
}
