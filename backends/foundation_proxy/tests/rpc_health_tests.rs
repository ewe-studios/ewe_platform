//! gRPC health gate integration test — a `grpc://` backend's health is probed
//! with the standard `grpc.health.v1.Health/Check` protocol, and its
//! `ServingStatus` drives rotation.
//!
//! WHY: `foundation_connectrpc` has no health module, so the proxy hand-encodes
//! the two well-known health messages (`HealthCheckRequest` / `HealthCheckResponse`)
//! and probes the backend over H2. A `NOT_SERVING` verdict must eject the backend
//! from rotation and a return to `SERVING` must readmit it — not merely flip a
//! flag that nothing consults.
//!
//! WHAT: A backend serves both `/grpc.health.v1.Health/Check` (whose verdict is
//! an `Arc<AtomicBool>`) and a unary Connect echo over the same H2 connection.
//! The proxy fronts it with a `grpc://` backend plus a short-interval health
//! check. Flipping the verdict ejects/readmits the backend, observed by whether
//! the unary echo succeeds through the proxy.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::{valtron_test, sleep_async, PipeReceiver, PipeSender};
use foundation_http::native::serve::{BoxFuture, H2Serve};
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::context::ContextBag;
use foundation_netio::http2::types::{
    H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader,
};
use futures_util::FutureExt;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, HandlerOptions, JsonCodec,
    ProcedureCodecs, Request, Response, Router,
};

use foundation_proxy::config::{HealthCheckConfig, ServiceConfig};
use foundation_proxy::{ProxyConfig, ProxyServer};

const UNARY: &str = "/e2e.Rpc/Unary";
const HEALTH: &str = "/grpc.health.v1.Health/Check";

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

impl Msg {
    fn of(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

/// A unary echo router — the traffic-observation half of the health backend.
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
    router
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

/// A backend that answers `grpc.health.v1.Health/Check` with a flippable verdict
/// and delegates every other path to a Connect unary router, all over one H2
/// connection.
struct HealthGatedBackend {
    inner: Arc<dyn H2Serve>,
    serving: Arc<AtomicBool>,
}

impl H2Serve for HealthGatedBackend {
    fn serve_h2(
        &self,
        bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        if header.url.url == HEALTH {
            let serving = self.serving.load(Ordering::SeqCst);
            async move {
                // Drain the (empty) request: a 5-byte gRPC frame header.
                while body.receive().await.is_some() {}

                let _ = tx
                    .send(H2Frame::Headers {
                        status: 200,
                        headers: vec![(
                            Bytes::from_static(b"content-type"),
                            Bytes::from_static(b"application/grpc"),
                        )],
                        end_stream: false,
                    })
                    .await;

                // `HealthCheckResponse { status }` (field 1, varint). SERVING (0)
                // is the proto3 default — an empty message; NOT_SERVING (1) is
                // `0x08 0x01`.
                let message: &[u8] = if serving { &[] } else { &[0x08, 0x01] };
                let mut frame = Vec::with_capacity(5 + message.len());
                frame.push(0x00); // flags
                frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
                frame.extend_from_slice(message);

                let _ = tx
                    .send(H2Frame::Data {
                        payload: Bytes::from(frame),
                        end_stream: false,
                    })
                    .await;
                let _ = tx
                    .send(H2Frame::Trailers {
                        headers: vec![(
                            Bytes::from_static(b"grpc-status"),
                            Bytes::from_static(b"0"),
                        )],
                    })
                    .await;
                Ok(())
            }
            .boxed()
        } else {
            self.inner.serve_h2(bag, header, body, tx)
        }
    }
}

/// Start the health-gated backend and return its port and shutdown signal.
fn start_health_gated_backend(serving: Arc<AtomicBool>) -> (u16, Arc<OnSignal>) {
    let backend = Arc::new(HealthGatedBackend {
        inner: Arc::new(ConnectRpcServeH2::new(build_router().into_handler())),
        serving,
    });

    let mut h2_app = HttpApp::new_h2_serve();
    h2_app.route_any_h2(UNARY, backend.clone());
    h2_app.route_any_h2(HEALTH, backend);

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

/// A proxy whose service routes to a `grpc://` backend with a short-interval
/// gRPC health check (threshold 1 so a single verdict flips rotation).
fn proxy_config_grpc_health(backend_port: u16) -> ProxyConfig {
    let svc = ServiceConfig::new("rpc", "127.0.0.1")
        .backend(&format!("grpc://127.0.0.1:{backend_port}"))
        .health_check_config(HealthCheckConfig {
            path: HEALTH.to_string(),
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(500),
            healthy_threshold: 1,
            unhealthy_threshold: 1,
        });
    ProxyConfig::new("127.0.0.1", "127.0.0.1")
        .bind("127.0.0.1:0")
        .service(svc)
}

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

/// Poll `cond` until it returns true or `timeout` elapses.
async fn wait_until<F, Fut>(timeout: Duration, mut cond: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond().await {
            return true;
        }
        sleep_async(Duration::from_millis(50)).await;
    }
    false
}

/// WHY: a `grpc://` backend whose health check reports `NOT_SERVING` must be
/// ejected from rotation (traffic fails with no eligible backend), and a return
/// to `SERVING` must readmit it.
#[valtron_test(seed = 63, threads = 8, timeout = 60_000)]
async fn grpc_health_gate_excludes_and_restores_backend() {
    let serving = Arc::new(AtomicBool::new(true));
    let (backend_port, backend_shutdown) = start_health_gated_backend(Arc::clone(&serving));
    let proxy = ProxyServer::start(proxy_config_grpc_health(backend_port)).expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h2_client(&addr, UNARY);

    // SERVING: the backend is in rotation (backends also start healthy). The very
    // first probe can transiently fail while the backend's H2 loop is still
    // spinning up, so poll briefly instead of assuming the first call lands
    // before that probe.
    let initial_ok = wait_until(Duration::from_secs(5), || {
        let client = &client;
        async move {
            client
                .unary(ctx(), Request::new(Msg::of("healthy")))
                .await
                .map(|r| r.msg == Msg::of("echo:healthy"))
                .unwrap_or(false)
        }
    })
    .await;
    assert!(initial_ok, "initial SERVING call must succeed");

    // NOT_SERVING: the next probe ejects the backend, so the call fails.
    serving.store(false, Ordering::SeqCst);
    let ejected = wait_until(Duration::from_secs(5), || {
        let client = &client;
        async move { client.unary(ctx(), Request::new(Msg::of("probe"))).await.is_err() }
    })
    .await;
    assert!(ejected, "backend must be ejected when NOT_SERVING");

    // SERVING again: the next probe readmits it, and the echo succeeds.
    serving.store(true, Ordering::SeqCst);
    let readmitted = wait_until(Duration::from_secs(5), || {
        let client = &client;
        async move {
            client
                .unary(ctx(), Request::new(Msg::of("recovered")))
                .await
                .map(|r| r.msg == Msg::of("echo:recovered"))
                .unwrap_or(false)
        }
    })
    .await;
    assert!(readmitted, "backend must be readmitted when SERVING again");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();
}
