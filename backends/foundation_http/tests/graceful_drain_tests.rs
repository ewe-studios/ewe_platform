//! Graceful shutdown / connection draining (spec-41 F11 / Decision 12 §10).
//!
//! WHY: After shutdown is signalled the server must stop accepting, let handlers
//! stop taking *new* keep-alive requests at their idle checkpoint, let in-flight
//! requests finish within a bounded grace window, and force-close (stop waiting
//! and return) once the grace deadline passes. These tests drive a real
//! `HttpServer` over raw TCP sockets to verify all three behaviours.
//!
//! Self-contained: raw `std::net::TcpStream` client + `std` timing only, so it
//! needs no HTTP client / macro deps. Gated on `multi` (the executor the server
//! runs handler tasks on); empty otherwise.
#![cfg(feature = "multi")]

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::initialize_pool;
use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::shared::{SimpleIncomingRequest, SimpleMethod};
use serial_test::serial;

use foundation_http::{
    native::server::{HttpServer, ServerConfig},
    shared::{
        app::HttpApp,
        context::ContextBag,
        serve::{respond, ConnectionResult, Serve, ServeFactory},
    },
};

/// Immediately answers `200 OK` and keeps the connection alive.
struct EchoHandler;

impl ServeFactory for EchoHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for EchoHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        respond::text(&mut conn, 200, "ok")
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// Sleeps `SLOW_SLEEP_MS` before responding, modelling an in-flight RPC.
static SLOW_SLEEP_MS: AtomicU64 = AtomicU64::new(0);

struct SlowHandler;

impl ServeFactory for SlowHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for SlowHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        std::thread::sleep(Duration::from_millis(SLOW_SLEEP_MS.load(Ordering::SeqCst)));
        respond::text(&mut conn, 200, "slow-ok")
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// Start a server with `config`; returns its address, the shutdown signal, and a
/// receiver that fires once `serve_with_listener` returns (drain finished).
fn start_server(app: HttpApp<Arc<dyn Serve>>, config: ServerConfig) -> (std::net::SocketAddr, Arc<OnSignal>, Receiver<()>) {
    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let server = HttpServer::with_config(app, &bind_addr, config);

    let shutdown_thread = shutdown.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
        let _ = done_tx.send(());
    });
    (addr, shutdown, done_rx)
}

fn raw_get(stream: &mut std::net::TcpStream, path: &str) -> std::io::Result<()> {
    write!(stream, "GET {path} HTTP/1.1\r\nHost: t\r\n\r\n")
}

/// Read whatever the server sends next (one read; the small responses fit).
fn read_available(stream: &mut std::net::TcpStream, timeout: Duration) -> String {
    stream.set_read_timeout(Some(timeout)).ok();
    let mut buf = vec![0u8; 8192];
    match stream.read(&mut buf) {
        Ok(n) => String::from_utf8_lossy(&buf[..n]).into_owned(),
        Err(_) => String::new(),
    }
}

/// Post-shutdown a keep-alive connection stops taking NEW requests: the first is
/// served, then shutdown closes the idle connection so a second request on the
/// same socket is never answered — and the server returns after draining.
#[test]
#[serial(drain)]
fn stops_new_keepalive_requests_after_shutdown() {
    let _pool = initialize_pool(41, Some(4));
    let mut app = HttpApp::new_serve();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");
    let (addr, shutdown, done_rx) =
        start_server(app, ServerConfig::defaults().with_shutdown_grace(Duration::from_secs(2)));

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    raw_get(&mut stream, "/echo").expect("write first");
    let first = read_available(&mut stream, Duration::from_secs(3));
    assert!(
        first.contains("HTTP/1.1 200"),
        "first keep-alive request should be served, got: {first:?}"
    );

    // Signal shutdown; give the idle handler a moment to close at its checkpoint.
    shutdown.turn_on();
    std::thread::sleep(Duration::from_millis(400));

    // Second request on the same connection must NOT be served.
    let _ = raw_get(&mut stream, "/echo");
    let second = read_available(&mut stream, Duration::from_secs(2));
    assert!(
        !second.contains("HTTP/1.1 200"),
        "no new request should be served after shutdown, got: {second:?}"
    );

    assert!(
        done_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
        "serve_loop should return after draining"
    );
}

/// An in-flight request that began before shutdown completes within the grace
/// window, then the server drains and returns.
#[test]
#[serial(drain)]
fn completes_inflight_request_within_grace() {
    let _pool = initialize_pool(41, Some(4));
    SLOW_SLEEP_MS.store(500, Ordering::SeqCst);
    let mut app = HttpApp::new_serve();
    app.route::<SlowHandler>(SimpleMethod::GET, "/slow");
    let (addr, shutdown, done_rx) =
        start_server(app, ServerConfig::defaults().with_shutdown_grace(Duration::from_secs(5)));

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    raw_get(&mut stream, "/slow").expect("write");

    // Signal shutdown while the handler is still sleeping (in-flight).
    std::thread::sleep(Duration::from_millis(100));
    shutdown.turn_on();

    let resp = read_available(&mut stream, Duration::from_secs(5));
    assert!(
        resp.contains("HTTP/1.1 200"),
        "in-flight request should complete within grace, got: {resp:?}"
    );
    assert!(
        done_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
        "serve_loop should return after the in-flight request drains"
    );
}

/// When an in-flight request outlasts the grace window, the server force-closes:
/// `serve_loop` returns around the grace deadline rather than waiting for the
/// long handler to finish.
#[test]
#[serial(drain)]
fn force_closes_past_grace_deadline() {
    let _pool = initialize_pool(41, Some(4));
    SLOW_SLEEP_MS.store(4000, Ordering::SeqCst); // far exceeds the grace below
    let mut app = HttpApp::new_serve();
    app.route::<SlowHandler>(SimpleMethod::GET, "/slow");
    let (addr, shutdown, done_rx) =
        start_server(app, ServerConfig::defaults().with_shutdown_grace(Duration::from_millis(500)));

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    raw_get(&mut stream, "/slow").expect("write");
    std::thread::sleep(Duration::from_millis(100)); // ensure the handler is in-flight

    let t0 = Instant::now();
    shutdown.turn_on();

    let returned = done_rx.recv_timeout(Duration::from_secs(3)).is_ok();
    let elapsed = t0.elapsed();
    assert!(returned, "serve_loop should return after the grace deadline");
    assert!(
        elapsed < Duration::from_secs(3),
        "serve_loop should force-close past the grace deadline (took {elapsed:?}), \
         not wait for the 4s in-flight handler"
    );
}
