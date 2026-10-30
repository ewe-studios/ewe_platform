//! Feature 08 — `ConnectionResult::Async` for long-lived HTTP/1.1 handlers.
//!
//! A handler that needs to wait returns `Async(future)` at once. The connection
//! task parks on the future via the valtron wake bridge and applies its result
//! (`Keep` / `Take` / `Close`) when it completes. This tests the three
//! observable behaviours: keep-alive after `Keep`, drop-on-shutdown for a
//! still-pending future, and close on a nested `Async`.

#![cfg(feature = "multi")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::ShutdownSignal;
use foundation_core::valtron::{initialize_pool, timer};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleMethod};
use serial_test::serial;

use foundation_http::{
    native::server::{HttpServer, KeepAliveConfig, ServerConfig},
    shared::{
        app::HttpApp,
        context::ContextBag,
        serve::{respond, ConnectionResult, Serve, ServeFactory},
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

impl Serve for AsyncKeepHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        respond::text(&mut conn, 200, "first").map_or(ConnectionResult::Close(None), |()| {
            let fut = async move {
                timer::sleep_for(Duration::from_millis(50)).await;
                ConnectionResult::Keep
            };
            ConnectionResult::Async(Box::pin(fut))
        })
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
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        respond::text(&mut conn, 200, "pending").map_or(ConnectionResult::Close(None), |()| {
            let fut = async move {
                let _guard = DropFlag;
                std::future::pending::<()>().await;
                ConnectionResult::Keep
            };
            ConnectionResult::Async(Box::pin(fut))
        })
    }
}

/// Writes a 200, then resolves to a nested `Async`. The connection must close.
struct NestedAsyncHandler;

impl ServeFactory for NestedAsyncHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for NestedAsyncHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        respond::text(&mut conn, 200, "nested").map_or(ConnectionResult::Close(None), |()| {
            let fut = async move {
                ConnectionResult::Async(Box::pin(async { ConnectionResult::Keep }))
            };
            ConnectionResult::Async(Box::pin(fut))
        })
    }
}

// ---------------------------------------------------------------------------
// Harness

/// Start a server with `handler` on `GET /async`; returns its address, the
/// shutdown signal, and a receiver that fires once `serve_with_listener` returns.
fn start_server<H: ServeFactory + 'static>(
) -> (std::net::SocketAddr, Arc<ShutdownSignal>, std::sync::mpsc::Receiver<()>) {
    let mut app = HttpApp::new_serve();
    app.route::<H>(SimpleMethod::GET, "/async");

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

/// A future that resolves to a nested `Async` is a programming error: the
/// connection closes.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn nested_async_closes_with_error() {
    let _pool = initialize_pool(202, Some(4));
    let (addr, shutdown, done_rx) = start_server::<NestedAsyncHandler>();

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .write_all(b"GET /async HTTP/1.1\r\nHost: t\r\n\r\n")
        .expect("write request");

    let mut acc = String::new();
    let mut buf = [0u8; 4096];
    let mut closed = false;
    loop {
        match stream.read(&mut buf) {
            Ok(0) => {
                closed = true;
                break;
            }
            Ok(n) => acc.push_str(&String::from_utf8_lossy(&buf[..n])),
            Err(_) => break,
        }
    }

    assert!(acc.contains("HTTP/1.1 200"), "response: {acc:?}");
    assert!(
        closed,
        "the connection should close after a nested Async result"
    );

    stop(&shutdown, &done_rx);
}
