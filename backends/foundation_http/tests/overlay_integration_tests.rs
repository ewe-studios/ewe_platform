//! WireGuard overlay HTTP integration test (spec-55, F11).
//!
//! Proves the full pipeline: WgNode mesh → OverlayAcceptor → HttpServer →
//! OverlayConnector → NativeHttpClient. Two nodes form a mesh, B serves HTTP
//! on the overlay, A sends a request through NativeHttpClient with
//! OverlayConnector — zero raw HTTP, zero manual framing.

#![cfg(feature = "multi")]

use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::OnSignal;
use foundation_netio::PreparedRequestBuilder;
use foundation_netio::native::connection::Connector;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::{HttpClient, SystemDnsResolver};
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleIncomingRequest, SimpleMethod, SimpleResponse,
};
use foundation_netio::http::{HttpConnectionPool, NativeHttpClient};
use foundation_wireguard::{SeedBits, WgConfig, WgSeed};
use foundation_wireguard::native::{
    overlay_acceptor::OverlayAcceptor,
    overlay_connector::OverlayConnector,
    WgNode,
};
use serial_test::serial;
use tracing_test::traced_test;

use foundation_http::{
    native::server::{HttpServer, KeepAliveConfig, ServerConfig},
    shared::{
        app::HttpApp,
        context::ContextBag,
        serve::{respond, ConnectionResult, Serve, ServeError, ServeFactory},
    },
};

// ── Echo handler ──────────────────────────────────────────────────────────

struct EchoHandler;

impl ServeFactory for EchoHandler {
    fn create(_bag: &ContextBag) -> Self { Self }
}

impl Serve for EchoHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let body_bytes = match req.body {
            Some(body) => match try_collect_bytes(body) {
                Ok(b) => b,
                Err(e) => {
                    return ConnectionResult::Close(Some(foundation_errstacks::ErrorTrace::new(
                        ServeError::InternalError { status: 500, reason: format!("body: {e}") },
                    )));
                }
            },
            None => Vec::new(),
        };
        let body_text = String::from_utf8_lossy(&body_bytes);
        respond::text(&mut conn, 200, &body_text)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

// ── Integration test ──────────────────────────────────────────────────────

#[traced_test]
#[serial]
#[test]
fn overlay_connector_http_round_trip() {
    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);

    let seed = WgSeed::generate(SeedBits::Bits256).expect("seed");
    let net = seed.derive_network_id();

    let handle_a = WgNode::from_config(WgConfig::seed(seed.clone(), net))
        .join().expect("A join");
    let a_boot = handle_a.bootstrap_addr();

    let handle_b = WgNode::from_config(
        WgConfig::joiner(seed.clone(), net, vec![a_boot])
    ).join().expect("B join");

    let dl = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < dl {
        if handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_millis(100))
            && handle_b.wait_for_peer(handle_a.overlay_ip(), Duration::from_millis(100))
        { break; }
    }
    assert!(handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_millis(10)));

    // ── B: HttpServer on overlay via OverlayAcceptor ────────────────
    let b_ip = handle_b.overlay_ip();
    let listener = handle_b.tcp_listen(8082).expect("overlay listen");
    let port = listener.local_addr().port();
    let acceptor = OverlayAcceptor::new(listener, b_ip, port);

    let mut app = HttpApp::new_serve();
    app.router.add_route_any("/", &(Arc::new(EchoHandler) as Arc<dyn Serve>));

    let server = HttpServer::with_config(app, &format!("{}:{}", b_ip, port),
        ServerConfig::defaults().with_keep_alive(KeepAliveConfig::disabled()));

    let shutdown = Arc::new(OnSignal::new());
    let s = shutdown.clone();
    let acc = acceptor.clone();
    std::thread::spawn(move || { server.serve_with_acceptor(&acc, &s); });
    std::thread::sleep(Duration::from_millis(500));

    // ── A: NativeHttpClient with OverlayConnector ───────────────────
    // The connector opens overlay connections instead of DNS+TcpStream.
    let connector: Arc<dyn Connector> =
        Arc::new(OverlayConnector::new(Arc::new(handle_a)));
    let pool = Arc::new(
        HttpConnectionPool::<SystemDnsResolver>::default()
            .with_connector(connector),
    );
    // Keep Expect: 100-continue enabled (client default) so this test also
    // exercises interim (100 Continue) handling over the non-blocking overlay.
    let client = NativeHttpClient::with_config_and_pool(
        Default::default(),
        pool,
        SystemDnsResolver::new(),
    );

    // Send request. The overlay URL uses the mesh IP (10.x.y.z).
    // The connector intercepts the connection, DNS is skipped.
    let req = PreparedRequestBuilder::new(
        SimpleMethod::POST,
        &format!("http://{}:{}/", b_ip, port),
    )
    .unwrap()
    .body_bytes(b"hello-overlay".to_vec())
    .build();

    let resp: SimpleResponse<SendSafeBody> = client.send(req).expect("send");
    let status = resp.get_status().into_usize();
    let body = try_collect_bytes(resp.take_body()).unwrap_or_default();
    let body_text = String::from_utf8_lossy(&body);

    assert_eq!(status, 200, "expected 200, got {}", body_text);
    assert!(body_text.contains("hello-overlay"), "expected echo, got: {body_text}");

    // ── GET (no body) over the overlay ──────────────────────────────
    // Exercises the parking-aware no-body response-head read (`ReadingHead`)
    // in RedirectingRequestTask: no Expect handshake, response read directly.
    let get_req = PreparedRequestBuilder::new(
        SimpleMethod::GET,
        &format!("http://{}:{}/", b_ip, port),
    )
    .unwrap()
    .build();

    let get_resp: SimpleResponse<SendSafeBody> = client.send(get_req).expect("get send");
    let get_status = get_resp.get_status().into_usize();
    let _ = try_collect_bytes(get_resp.take_body());
    assert_eq!(get_status, 200, "GET over overlay expected 200, got {get_status}");

    shutdown.turn_on();
    handle_b.shutdown();
}
