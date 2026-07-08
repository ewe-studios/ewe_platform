//! Code-first service definition with `#[service]` (Decision 10
//! Mode 3, Feature 27) — round-tripped over a real loopback socket.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_connectrpc --example code_first_service
//! ```
//!
//! Instead of a `.proto` file, the service is a plain Rust trait. The
//! `#[service]` attribute generates, from that trait:
//!   - `procedure::GREET` — the procedure path constant,
//!   - the `GreetService` trait (with default `unimplemented` bodies),
//!   - `register_greet_service(&mut Router, Arc<S>)` — server registration,
//!   - `GreetServiceClient` — a typed client with one async method per RPC.
//!
//! The generated code refers to the runtime crate by its real name
//! (`foundation_connectrpc::…`), so no crate alias is needed — just import the
//! `service` macro and the types you use.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_netio::simple_http::client::SimpleHttpClient;

use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    service, ClientOptions, ConnectResult, ConnectRpcServe, Ctx, H1Transport, Request, Response,
    Router,
};

// ── Messages ─────────────────────────────────────────────────────────────────
//
// A code-first `codecs(json)` service needs **only serde** — no buffa, no
// protobuf. `JsonCodec: CodecFor<M>` is bounded on `Serialize + DeserializeOwned`
// alone, and the generated `ProcedureCodecs::of((JsonCodec,))` table carries no
// `Message` bound. (A `codecs(proto, …)` service would additionally need a
// buffa-generated `Message` impl, since proto requires a schema.)
//
// `pub`: the generated `GreetService` trait / client are `pub` and mention these
// in their signatures, so the message types must be at least as visible.
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct GreetRequest {
    name: String,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct GreetResponse {
    message: String,
}

// ── The service, defined code-first ──────────────────────────────────────────

#[service(package = "demo.greet.v1", codecs(json))]
pub trait GreetService {
    // Param names are underscored here because the macro's generated default
    // (`unimplemented`) body ignores them; an implementor names them freely.
    async fn greet(
        &self,
        _ctx: Ctx,
        _req: Request<GreetRequest>,
    ) -> ConnectResult<Response<GreetResponse>>;
}

/// An implementation of the generated trait.
struct Greeter;

impl GreetService for Greeter {
    async fn greet(
        &self,
        _ctx: Ctx,
        req: Request<GreetRequest>,
    ) -> ConnectResult<Response<GreetResponse>> {
        Ok(Response::new(GreetResponse { message: format!("Hello, {}!", req.msg.name) }))
    }
}

// ── Wiring ───────────────────────────────────────────────────────────────────

fn start_server() -> (std::net::SocketAddr, Arc<OnSignal>) {
    // Server registration: the generated `register_greet_service` installs every
    // procedure onto the router.
    let mut router = Router::new();
    register_greet_service(&mut router, Arc::new(Greeter));
    let serve: Arc<dyn foundation_http::shared::serve::Serve> =
        Arc::new(ConnectRpcServe::new(router.into_handler()));

    let mut app = HttpApp::new_serve();
    app.router.add_route_any(procedure::GREET, &serve);

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

    // Client: the generated typed `GreetServiceClient`. `new` takes a base URL;
    // it appends each procedure path internally. Code-first JSON → select "json".
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(SimpleHttpClient::from_system()));
    let base_url = format!("http://127.0.0.1:{}", addr.port());
    let client = GreetServiceClient::new(transport, &base_url, ClientOptions::new().with_codec("json"))
        .expect("build client");

    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let resp = client
        .greet(ctx, Request::new(GreetRequest { name: "code-first".into() }))
        .await
        .expect("greet call");

    println!("greet response: {:?}", resp.msg.message);
    assert_eq!(resp.msg.message, "Hello, code-first!");
    println!("code-first #[service] round-trip verified ✓  (service = {GREETSERVICE_NAME})");

    shutdown.turn_on();
}
