//! HTTP version gating on the HTTP/1.x connection handler.
//!
//! WHY: `Proto::from_str` is infallible — an unrecognised request-line token
//! parses to `Proto::Custom(..)` instead of failing. `detect_protocol` chooses
//! only which parser handles the connection, and any request-line shape suffices
//! for it; the version named inside that line is not its business. So the
//! request line is the last place the HTTP version is ever named, and nothing
//! downstream of the reader inspects it. Without a gate, `GET / SPDY/3.1` is
//! routed and served exactly like HTTP/1.1: a peer smuggles a protocol past a
//! server that never agreed to speak it.
//!
//! WHAT: These tests drive a real `HttpServer` over raw TCP sockets and assert
//! the handler answers `505` and closes for any version outside the `Proto`
//! enum's HTTP/1.0 and HTTP/1.1, while both of those are served normally.
//!
//! HOW: The rejected requests target a *registered* route, and the handler sets
//! `HANDLER_CALLED`. A 505 with that flag still clear proves the gate runs
//! before routing — not merely that a 505 was produced somewhere.
//!
//! Self-contained: raw `std::net::TcpStream` client + `std` timing only. Gated
//! on `multi` (the executor the server runs handler tasks on); empty otherwise.
#![cfg(feature = "multi")]

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

/// Set by `EchoHandler::serve`. If a rejected request ever reaches routing this
/// flips, and the corresponding assertion fails.
static HANDLER_CALLED: AtomicBool = AtomicBool::new(false);

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
        HANDLER_CALLED.store(true, Ordering::SeqCst);
        respond::text(&mut conn, 200, "ok")
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// Start a server routing `GET /echo`; returns its address and shutdown signal.
fn start_server() -> (std::net::SocketAddr, Arc<OnSignal>) {
    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let bind_addr = format!("127.0.0.1:{}", addr.port());

    let mut app = HttpApp::new_serve();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");
    let server = HttpServer::with_config(app, &bind_addr, ServerConfig::defaults());

    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
    });
    (addr, shutdown)
}

/// Send a request line naming `proto` verbatim, so we can put tokens on the
/// wire that no builder in this workspace would ever construct.
fn raw_request(stream: &mut std::net::TcpStream, proto: &str) -> std::io::Result<()> {
    write!(stream, "GET /echo {proto}\r\nHost: t\r\n\r\n")
}

fn read_available(stream: &mut std::net::TcpStream, timeout: Duration) -> String {
    stream.set_read_timeout(Some(timeout)).ok();
    let mut buf = vec![0u8; 8192];
    match stream.read(&mut buf) {
        Ok(n) => String::from_utf8_lossy(&buf[..n]).into_owned(),
        Err(_) => String::new(),
    }
}

/// `true` when the peer has closed: a read returns `Ok(0)` rather than data or
/// a timeout.
fn is_closed(stream: &mut std::net::TcpStream, timeout: Duration) -> bool {
    stream.set_read_timeout(Some(timeout)).ok();
    let mut buf = [0u8; 64];
    matches!(stream.read(&mut buf), Ok(0))
}

/// Drive one unsupported version end to end: 505, connection closed, handler
/// never reached.
fn assert_rejected(proto: &str) {
    HANDLER_CALLED.store(false, Ordering::SeqCst);
    let (addr, shutdown) = start_server();

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    raw_request(&mut stream, proto).expect("write");
    let resp = read_available(&mut stream, Duration::from_secs(3));

    assert!(
        resp.starts_with("HTTP/1.1 505"),
        "{proto} should be refused with 505, got: {resp:?}"
    );
    assert!(
        !HANDLER_CALLED.load(Ordering::SeqCst),
        "{proto} reached the route handler; the version gate ran too late"
    );
    assert!(
        is_closed(&mut stream, Duration::from_secs(3)),
        "{proto} should close the connection after 505; framing is untrusted"
    );

    shutdown.turn_on();
}

/// Drive one supported version end to end: served normally by the route.
fn assert_served(proto: &str) {
    HANDLER_CALLED.store(false, Ordering::SeqCst);
    let (addr, shutdown) = start_server();

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    raw_request(&mut stream, proto).expect("write");
    let resp = read_available(&mut stream, Duration::from_secs(3));

    assert!(
        resp.contains("HTTP/1.1 200"),
        "{proto} should be served, got: {resp:?}"
    );
    assert!(
        HANDLER_CALLED.load(Ordering::SeqCst),
        "{proto} should have reached the route handler"
    );

    shutdown.turn_on();
}

// -- Valid input: the two versions this handler speaks.

#[test]
#[serial(version_guard)]
fn serves_http_11() {
    let _pool = initialize_pool(41, Some(4));
    assert_served("HTTP/1.1");
}

#[test]
#[serial(version_guard)]
fn serves_http_10() {
    let _pool = initialize_pool(41, Some(4));
    assert_served("HTTP/1.0");
}

// -- Invalid input: everything else, refused with 505.

/// A version the workspace *does* implement, but not on this connection: an
/// HTTP/2 request line on a socket that never sent the h2c preface must not be
/// quietly downgraded into the HTTP/1.1 router.
#[test]
#[serial(version_guard)]
fn rejects_http_20_request_line() {
    let _pool = initialize_pool(41, Some(4));
    assert_rejected("HTTP/2.0");
}

#[test]
#[serial(version_guard)]
fn rejects_http_30_request_line() {
    let _pool = initialize_pool(41, Some(4));
    assert_rejected("HTTP/3.0");
}

/// The `Proto::Custom(..)` path — a wholly foreign protocol token.
#[test]
#[serial(version_guard)]
fn rejects_foreign_protocol_token() {
    let _pool = initialize_pool(41, Some(4));
    assert_rejected("SPDY/3.1");
}

// -- Edge cases.

/// A plausible-looking but unsupported 1.x minor. `Proto` has no HTTP/1.2, so
/// it lands in `Custom` and must be refused rather than treated as 1.1.
#[test]
#[serial(version_guard)]
fn rejects_unknown_minor_version() {
    let _pool = initialize_pool(41, Some(4));
    assert_rejected("HTTP/1.2");
}

/// Garbage in the version slot is refused, not routed.
#[test]
#[serial(version_guard)]
fn rejects_garbage_version_token() {
    let _pool = initialize_pool(41, Some(4));
    assert_rejected("NOT-A-PROTOCOL");
}

/// A two-token request line (`GET /echo`) omits the version entirely. The
/// reader defaults it to HTTP/1.1 (HTTP/0.9-style simple request), so the gate
/// must let it through — the default is chosen by us, not named by the peer.
#[test]
#[serial(version_guard)]
fn serves_request_line_without_version() {
    let _pool = initialize_pool(41, Some(4));
    HANDLER_CALLED.store(false, Ordering::SeqCst);
    let (addr, shutdown) = start_server();

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    write!(stream, "GET /echo\r\nHost: t\r\n\r\n").expect("write");
    let resp = read_available(&mut stream, Duration::from_secs(3));

    assert!(
        resp.contains("HTTP/1.1 200"),
        "versionless request line should default to HTTP/1.1 and be served, got: {resp:?}"
    );

    shutdown.turn_on();
}

/// Case-insensitivity is a property of `Proto::from_str`; a lowercase token is
/// still HTTP/1.1 and must be served, not refused.
#[test]
#[serial(version_guard)]
fn serves_lowercase_http_11_token() {
    let _pool = initialize_pool(41, Some(4));
    assert_served("http/1.1");
}


// -- Persistence defaults are version-dependent (RFC 9112 §9.3).
//
// HTTP/1.1: persistent unless `Connection: close`.
// HTTP/1.0: closes unless `Connection: keep-alive`.
// Deciding on the header alone — as this handler once did — inverts 1.0.

/// Send one request and report whether the server closed the connection after
/// answering it. A closed peer makes the next read return `Ok(0)`.
fn closes_after_request(request: &str) -> bool {
    let (addr, shutdown) = start_server();

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    stream.write_all(request.as_bytes()).expect("write");
    let resp = read_available(&mut stream, Duration::from_secs(3));
    assert!(
        resp.contains("HTTP/1.1 200"),
        "request should be served, got: {resp:?}"
    );

    let closed = is_closed(&mut stream, Duration::from_secs(2));
    shutdown.turn_on();
    closed
}

#[test]
#[serial(version_guard)]
fn http_11_keeps_the_connection_alive_by_default() {
    let _pool = initialize_pool(41, Some(4));
    assert!(
        !closes_after_request("GET /echo HTTP/1.1\r\nHost: t\r\n\r\n"),
        "HTTP/1.1 defaults to persistent"
    );
}

#[test]
#[serial(version_guard)]
fn http_11_closes_when_asked() {
    let _pool = initialize_pool(41, Some(4));
    assert!(
        closes_after_request("GET /echo HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n"),
        "`Connection: close` closes an HTTP/1.1 connection"
    );
}

/// The inverted case. Before the version was consulted, this held the socket
/// open until the idle timeout.
#[test]
#[serial(version_guard)]
fn http_10_closes_by_default() {
    let _pool = initialize_pool(41, Some(4));
    assert!(
        closes_after_request("GET /echo HTTP/1.0\r\nHost: t\r\n\r\n"),
        "HTTP/1.0 defaults to closing; it is not persistent"
    );
}

#[test]
#[serial(version_guard)]
fn http_10_stays_open_when_it_opts_in() {
    let _pool = initialize_pool(41, Some(4));
    assert!(
        !closes_after_request("GET /echo HTTP/1.0\r\nHost: t\r\nConnection: keep-alive\r\n\r\n"),
        "HTTP/1.0 opts into persistence with `Connection: keep-alive`"
    );
}

/// `Connection: close` beats the 1.0 opt-in, and a multi-token header value is
/// parsed per-token rather than compared whole.
#[test]
#[serial(version_guard)]
fn close_wins_over_keep_alive_and_tokens_are_split() {
    let _pool = initialize_pool(41, Some(4));
    assert!(
        closes_after_request(
            "GET /echo HTTP/1.0\r\nHost: t\r\nConnection: keep-alive, close\r\n\r\n"
        ),
        "`close` wins when both tokens are present"
    );
}
