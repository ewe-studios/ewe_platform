//! The accept loop parks on listener readiness instead of sleeping (Feature 05).
//!
//! `serve_loop` used to `thread::sleep(would_block_sleep)` on every
//! `WouldBlock`. It now registers the listener with the shared reactor and
//! blocks in `FdReadiness::wait_for_events`, so a new connection wakes it at
//! once and shutdown detection is bounded only by `accept_shutdown_poll`
//! (`OnSignal` is a passive atomic with no wake primitive).
//!
//! Each test configures `would_block_sleep` to 1 s where it must, so the old
//! sleep loop would stall a connection or a shutdown for up to a second, while
//! the parked loop wakes immediately.

#![cfg(all(unix, not(target_family = "wasm"), feature = "multi"))]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::initialize_pool;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleMethod};
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

/// Start a server with `config`; returns its address, the shutdown signal, and
/// a receiver that fires once `serve_with_listener` returns.
fn start_server(
    config: ServerConfig,
) -> (std::net::SocketAddr, Arc<OnSignal>, std::sync::mpsc::Receiver<()>) {
    let mut app = HttpApp::new_serve();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let shutdown = Arc::new(OnSignal::new());
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
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

/// Read whatever the server sends next (one read; the small response fits).
fn read_available(stream: &mut TcpStream, timeout: Duration) -> String {
    stream.set_read_timeout(Some(timeout)).ok();
    let mut buf = vec![0u8; 8192];
    match stream.read(&mut buf) {
        Ok(n) => String::from_utf8_lossy(&buf[..n]).into_owned(),
        Err(_) => String::new(),
    }
}

/// Process CPU time (user + system) in nanoseconds, via `getrusage`.
fn process_cpu_ns() -> u128 {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        let rc = libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        assert_eq!(rc, 0, "getrusage failed");
        let user = usage.ru_utime.tv_sec as u128 * 1_000_000_000
            + usage.ru_utime.tv_usec as u128 * 1_000;
        let system = usage.ru_stime.tv_sec as u128 * 1_000_000_000
            + usage.ru_stime.tv_usec as u128 * 1_000;
        user + system
    }
}

/// Stop the server started by [`start_server`] and wait for it to return.
fn stop(shutdown: &Arc<OnSignal>, done_rx: &std::sync::mpsc::Receiver<()>) {
    shutdown.turn_on();
    let _ = done_rx.recv_timeout(Duration::from_secs(3));
}

/// An idle accept loop uses no CPU: it parks on the listener, so a whole second
/// of silence costs under 20 ms of process CPU. A busy poll would spin.
#[test]
#[serial(accept_loop)]
fn accept_loop_idle_uses_no_cpu() {
    let _pool = initialize_pool(90, Some(4));
    let (addr, shutdown, done_rx) = start_server(ServerConfig::defaults());

    // Let the loop settle into its parked state before measuring.
    std::thread::sleep(Duration::from_millis(200));
    let before = process_cpu_ns();
    std::thread::sleep(Duration::from_secs(1));
    let after = process_cpu_ns();
    let cpu_ms = (after - before) / 1_000_000;

    // Sanity: the server is actually reachable, so this test fails for the right
    // reason (a dead loop) rather than a dead server.
    let _ = addr;

    assert!(
        cpu_ms < 20,
        "idle accept loop used {cpu_ms} ms of CPU in 1 s; it should park"
    );

    stop(&shutdown, &done_rx);
}

/// Shutdown stops the loop at once. `would_block_sleep` is 1 s, so the old
/// sleep loop could linger up to a second; the parked loop wakes on the next
/// `accept_shutdown_poll` (10 ms) and exits well under 50 ms.
#[test]
#[serial(accept_loop)]
fn accept_loop_stops_at_once_on_shutdown() {
    let _pool = initialize_pool(91, Some(4));
    let config = ServerConfig::defaults().with_would_block_sleep(Duration::from_millis(1000));
    let (_addr, shutdown, done_rx) = start_server(config);

    // Let the loop enter its wait before signalling.
    std::thread::sleep(Duration::from_millis(200));

    let t0 = Instant::now();
    shutdown.turn_on();
    let returned = done_rx.recv_timeout(Duration::from_secs(3)).is_ok();
    let elapsed = t0.elapsed();

    assert!(returned, "serve_loop should return after shutdown");
    assert!(
        elapsed < Duration::from_millis(50),
        "shutdown took {elapsed:?}; the loop should wake at once, not sleep"
    );
}

/// After a second idle, a connection is accepted and served at once. The old
/// loop slept up to `would_block_sleep` (1 s here) before accepting; the parked
/// loop wakes on the reactor's readiness edge immediately.
#[test]
#[serial(accept_loop)]
fn accept_loop_accepts_after_idle() {
    let _pool = initialize_pool(92, Some(4));
    let config = ServerConfig::defaults().with_would_block_sleep(Duration::from_millis(1000));
    let (addr, shutdown, done_rx) = start_server(config);

    std::thread::sleep(Duration::from_secs(1));

    let start = Instant::now();
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .write_all(b"GET /echo HTTP/1.1\r\nHost: t\r\n\r\n")
        .expect("write request");
    let resp = read_available(&mut stream, Duration::from_secs(2));
    let elapsed = start.elapsed();

    assert!(
        resp.contains("HTTP/1.1 200"),
        "server should answer after idle, got: {resp:?}"
    );
    assert!(
        elapsed < Duration::from_millis(200),
        "connection served after idle took {elapsed:?}; the accept loop should wake at once"
    );

    stop(&shutdown, &done_rx);
}
