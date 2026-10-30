//! Feature 06 regression: an RPC over the WireGuard overlay parks on the
//! overlay's own readiness signal (no pollable fd) and completes.
//!
//! WHY: `ConnIo` used to return `Unsupported` for the overlay transport, so a
//! body or response that waited on the peer stalled. This test drives a real
//! unary and a real server-streaming RPC through the overlay and asserts both
//! complete, which only works once the overlay reports readiness.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_http::ShutdownSignal;
use foundation_core::valtron::valtron_test;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_netio::native::connection::Connector;
use foundation_netio::HttpClientBuilder;
use foundation_wireguard::native::overlay_acceptor::OverlayAcceptor;
use foundation_wireguard::native::overlay_connector::OverlayConnector;
use foundation_wireguard::native::{WgHandle, WgNode};
use foundation_wireguard::{SeedBits, WgConfig, WgSeed};

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectResult, ConnectRpcServe, Ctx, H1Transport, HandlerOptions,
    JsonCodec, ProcedureCodecs, Request, Response, Router,
};

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

const UNARY: &str = "/t.Svc/Unary";
const SERVER_STREAM: &str = "/t.Svc/ServerStream";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

/// One `ConnectRpcServe` handling both procedures; the `HttpApp` router points
/// both paths at it, and the handler dispatches on the full path.
fn serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        UNARY,
        codecs(),
        |_ctx, req: Request<Msg>| async move {
            Ok(Response::new(Msg {
                text: format!("echo:{}", req.msg.text),
            }))
        },
        HandlerOptions::new(),
    );
    router.server_stream(
        SERVER_STREAM,
        codecs(),
        |_ctx, req: Request<Msg>| async move {
            let seed = req.msg.text;
            let items: Vec<ConnectResult<Msg>> = (0..3)
                .map(|i| {
                    Ok(Msg {
                        text: format!("{seed}-{i}"),
                    })
                })
                .collect();
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// A connectrpc client for one procedure, over an H1 transport that dials the
/// mesh through `OverlayConnector` instead of DNS + TCP.
fn client_for(handle: Arc<WgHandle>, base: &str, procedure: &str) -> Client<Msg, Msg> {
    let connector: Arc<dyn Connector> = Arc::new(OverlayConnector::new(handle));
    let http_client = HttpClientBuilder::new()
        .with_connector(connector)
        // The overlay has no pollable fd; the `Expect: 100-continue` body wait in
        // `foundation_http` retries on a fixed delay instead of parking on the
        // overlay's read waker, so it can 408 before the body lands (see
        // `docs/bug-backlog.md`). This test targets the `ConnIo` overlay
        // readiness path, so it sends the body at once.
        .expect_continue(false)
        .build();
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(http_client));
    let url = format!("{base}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client")
}

#[valtron_test(threads = 42, tracing = "debug")]
async fn overlay_rpc_parks_and_completes() {
    let seed = WgSeed::generate(SeedBits::Bits256).expect("seed");
    let net = seed.derive_network_id();

    let handle_a = WgNode::from_config(WgConfig::seed(seed.clone(), net))
        .join()
        .expect("A join");
    let a_boot = handle_a.bootstrap_addr();
    let handle_b = WgNode::from_config(WgConfig::joiner(seed.clone(), net, vec![a_boot]))
        .join()
        .expect("B join");

    let dl = Instant::now() + Duration::from_secs(10);
    while Instant::now() < dl {
        if handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_millis(100))
            && handle_b.wait_for_peer(handle_a.overlay_ip(), Duration::from_millis(100))
        {
            break;
        }
    }
    assert!(handle_a.wait_for_peer(
        handle_b.overlay_ip(),
        Duration::from_millis(10)
    ));

    // ── B: `HttpServer` on the overlay via `OverlayAcceptor` ──────────
    let b_ip = handle_b.overlay_ip();
    let listener = handle_b.tcp_listen(8083).expect("overlay listen");
    let port = listener.local_addr().port();
    let acceptor = OverlayAcceptor::new(listener, b_ip, port);

    let mut app = HttpApp::new_serve();
    let serve = serve();
    app.router.add_route_any(UNARY, &serve);
    app.router.add_route_any(SERVER_STREAM, &serve);

    let server = HttpServer::with_config(
        app,
        &format!("{b_ip}:{port}"),
        ServerConfig::defaults(),
    );
    let shutdown = Arc::new(ShutdownSignal::new());
    let s = shutdown.clone();
    let acc = acceptor.clone();
    std::thread::spawn(move || {
        server.serve_with_acceptor(&acc, &s);
    });
    std::thread::sleep(Duration::from_millis(500));

    // ── A: connectrpc clients through the mesh ────────────────────────
    let base = format!("http://{b_ip}:{port}");
    let handle_a: Arc<WgHandle> = Arc::new(handle_a);

    let unary_client = client_for(Arc::clone(&handle_a), &base, UNARY);
    let resp = unary_client
        .unary(
            Ctx::background().with_deadline(Duration::from_secs(15)),
            Request::new(Msg {
                text: "hello".to_string(),
            }),
        )
        .await
        .expect("unary over overlay");
    assert_eq!(
        resp.msg,
        Msg {
            text: "echo:hello".to_string()
        }
    );

    let stream_client = client_for(Arc::clone(&handle_a), &base, SERVER_STREAM);
    let mut stream = stream_client
        .server_stream(
            Ctx::background().with_deadline(Duration::from_secs(15)),
            Request::new(Msg {
                text: "item".to_string(),
            }),
        )
        .await
        .expect("open server stream over overlay");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive over overlay") {
        got.push(msg.text);
    }
    assert_eq!(got, vec!["item-0", "item-1", "item-2"]);

    shutdown.turn_on();
    handle_b.shutdown();
}
