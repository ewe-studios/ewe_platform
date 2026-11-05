//! Feature 08 — `ConnectionResult::Async` for long-lived HTTP/1.1 handlers.
//!
//! A handler that needs to wait returns `Async(future)` at once. The connection
//! task parks on the future via the valtron wake bridge and applies its
//! [`ServeOutcome`] when it completes. This tests the observable behaviours:
//! keep-alive after `Keep`, drop-on-shutdown for a still-pending future, and the
//! connection ending on `Take` and on `Close(Some(_))`.

#![cfg(feature = "multi")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{initialize_pool, timer};
use foundation_errstacks::ErrorTrace;
use foundation_http::native::writer::send_text;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleMethod};
use serial_test::serial;

use foundation_http::{
    native::server::{HttpServer, KeepAliveConfig, ServerConfig},
    shared::{
        app::HttpApp,
        context::ContextBag,
        serve::{ConnectionResult, Serve, ServeError, ServeFactory, ServeOutcome},
    },
};

// ---------------------------------------------------------------------------
// Handlers

/// Writes a 200 at once, then continues asynchronously and keeps the connection.
struct AsyncKeepHandler;

impl ServeFactory for AsyncKeepHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

/// Write a complete response and report how the connection ends.
async fn write_or_close(response: ConnectionResult) -> ServeOutcome {
    match response.split() {
        Ok(future) => future.await,
        Err(outcome) => outcome,
    }
}

impl Serve for AsyncKeepHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        ConnectionResult::Async(Box::pin(async move {
            if let ServeOutcome::Close(err) = write_or_close(send_text(conn, 200, "first")).await {
                return ServeOutcome::Close(err);
            }
            timer::sleep_for(Duration::from_millis(50)).await;
            ServeOutcome::Keep
        }))
    }
}

/// Set to `true` when the pending handler future is dropped (Feature 08 §8.1:
/// shutdown drops a pending future).
static PENDING_DROPPED: AtomicBool = AtomicBool::new(false);

/// A guard whose drop records that the future was dropped.
struct DropFlag;

impl Drop for DropFlag {
    fn drop(&mut self) {
        PENDING_DROPPED.store(true, Ordering::SeqCst);
    }
}

/// Writes a 200, then parks on a future that never completes. Shutdown must
/// drop it.
struct PendingDropHandler;

impl ServeFactory for PendingDropHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for PendingDropHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        ConnectionResult::Async(Box::pin(async move {
            if let ServeOutcome::Close(err) = write_or_close(send_text(conn, 200, "pending")).await
            {
                return ServeOutcome::Close(err);
            }
            let _guard = DropFlag;
            std::future::pending::<()>().await;
            ServeOutcome::Keep
        }))
    }
}

/// Writes a response, then ends the future with `Take` on `GET /take` and with
/// an error `Close` on `GET /close`.
///
/// Feature 84 deleted the nested-`Async` test: `ServeFuture` now resolves to
/// [`ServeOutcome`], which has no `Async` variant, so a future cannot return
/// another future and that run-time error is impossible.
struct AsyncEndHandler;

impl ServeFactory for AsyncEndHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for AsyncEndHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let take = req.request_url.url.starts_with("/take");
        ConnectionResult::Async(Box::pin(async move {
            let body = if take { "take-ok" } else { "close-ok" };
            if let ServeOutcome::Close(err) = write_or_close(send_text(conn, 200, body)).await {
                return ServeOutcome::Close(err);
            }
            if take {
                ServeOutcome::Take
            } else {
                ServeOutcome::Close(Some(ErrorTrace::new(ServeError::InternalError {
                    status: 500,
                    reason: "feature 84: a future may end with an error close".to_string(),
                })))
            }
        }))
    }
}

// ---------------------------------------------------------------------------
// Harness

/// Start a server with `handler` on `GET /async`; returns its address, the
/// shutdown signal, and a receiver that fires once `serve_with_listener` returns.
fn start_server<H: ServeFactory + 'static>() -> (
    std::net::SocketAddr,
    Arc<ShutdownSignal>,
    std::sync::mpsc::Receiver<()>,
) {
    let mut app = HttpApp::new_serve();
    app.route::<H>(SimpleMethod::GET, "/async");
    app.route::<H>(SimpleMethod::GET, "/take");
    app.route::<H>(SimpleMethod::GET, "/close");

    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), config);

    let shutdown_thread = shutdown.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
        let _ = done_tx.send(());
    });
    (addr, shutdown, done_rx)
}

/// Read from `stream` until `needle` appears in the accumulated bytes.
fn read_until(stream: &mut TcpStream, needle: &str) -> String {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut acc = String::new();
    let mut buf = [0u8; 4096];
    while !acc.contains(needle) {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => acc.push_str(&String::from_utf8_lossy(&buf[..n])),
        }
    }
    acc
}

/// Stop the server started by [`start_server`] and wait for it to return.
fn stop(shutdown: &Arc<ShutdownSignal>, done_rx: &std::sync::mpsc::Receiver<()>) {
    shutdown.turn_on();
    let _ = done_rx.recv_timeout(Duration::from_secs(3));
}

// ---------------------------------------------------------------------------
// Tests

/// An `Async` future that resolves to `Keep` returns the connection to `Idle`,
/// so a second request on the same connection is served.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn async_result_keep_serves_next_request() {
    let _pool = initialize_pool(200, Some(4));
    let (addr, shutdown, done_rx) = start_server::<AsyncKeepHandler>();

    let mut stream = TcpStream::connect(addr).expect("connect");
    let request = b"GET /async HTTP/1.1\r\nHost: t\r\n\r\n";

    stream.write_all(request).expect("write request 1");
    let first = read_until(&mut stream, "first");
    assert!(first.contains("HTTP/1.1 200"), "first response: {first:?}");
    assert!(first.contains("first"), "first response body: {first:?}");

    // Let the future resolve to `Keep` and the connection return to `Idle`.
    std::thread::sleep(Duration::from_millis(150));

    stream.write_all(request).expect("write request 2");
    let second = read_until(&mut stream, "first");
    assert!(
        second.contains("HTTP/1.1 200"),
        "second request on the same connection should be served: {second:?}"
    );
    assert!(second.contains("first"), "second response body: {second:?}");

    stop(&shutdown, &done_rx);
}

/// Shutdown during a pending `Async` drops the handler future and ends the task.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn async_result_shutdown_drops_future() {
    let _pool = initialize_pool(201, Some(4));
    PENDING_DROPPED.store(false, Ordering::SeqCst);
    let (addr, shutdown, done_rx) = start_server::<PendingDropHandler>();

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .write_all(b"GET /async HTTP/1.1\r\nHost: t\r\n\r\n")
        .expect("write request");
    let resp = read_until(&mut stream, "pending");
    assert!(resp.contains("HTTP/1.1 200"), "response: {resp:?}");

    // The future is now parked and pending; it must not be dropped yet.
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        !PENDING_DROPPED.load(Ordering::SeqCst),
        "the future should still be pending before shutdown"
    );

    let t0 = Instant::now();
    shutdown.turn_on();
    let mut dropped = false;
    while t0.elapsed() < Duration::from_millis(100) {
        if PENDING_DROPPED.load(Ordering::SeqCst) {
            dropped = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        dropped,
        "the pending future should be dropped within 100 ms of shutdown"
    );

    let returned = done_rx.recv_timeout(Duration::from_secs(3)).is_ok();
    assert!(returned, "serve_with_listener should return after shutdown");
}

/// A future that resolves to `Take` ends the connection task, and a future
/// that resolves to `Close(Some(_))` logs the error and closes.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn async_result_take_and_close() {
    let _pool = initialize_pool(202, Some(4));
    let (addr, shutdown, done_rx) = start_server::<AsyncEndHandler>();

    for (path, needle) in [("/take", "take-ok"), ("/close", "close-ok")] {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: t\r\n\r\n").as_bytes())
            .expect("write request");

        let response = read_until(&mut stream, needle);
        assert!(
            response.contains("HTTP/1.1 200"),
            "{path} response: {response:?}"
        );
        assert!(response.contains(needle), "{path} body: {response:?}");

        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut buf = [0u8; 64];
        let closed = matches!(stream.read(&mut buf), Ok(0));
        assert!(
            closed,
            "{path} must end the connection task and close the socket"
        );
    }

    stop(&shutdown, &done_rx);
}
