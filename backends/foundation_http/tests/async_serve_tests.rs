//! Feature 08 — `ConnectionResult::Async` for long-lived HTTP/1.1 handlers.
//!
//! A handler that needs to wait returns `Async(future)` at once. The connection
//! task parks on the future via the valtron wake bridge and applies its
//! [`ServeOutcome`] when it completes. This tests the observable behaviours:
//! keep-alive after `Keep`, drop-on-shutdown for a still-pending future, and the
//! connection ending on `Take` and on `Close(Some(_))`.

#![cfg(feature = "multi")]

use std::future::Future;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
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
    native::serve::{Serve, ServeFactory},
    native::server::{HttpServer, KeepAliveConfig, ServerConfig},
    shared::{
        app::HttpApp,
        context::Ctx,
        serve::{ConnectionResult, ServeError, ServeOutcome},
    },
};

// ---------------------------------------------------------------------------
// Handlers

/// Writes a 200 at once, then continues asynchronously and keeps the connection.
struct AsyncKeepHandler;

impl ServeFactory for AsyncKeepHandler {
    fn create(_ctx: &Ctx) -> Self {
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
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        ConnectionResult::Async(Box::pin(async move {
            if let ServeOutcome::Close(err) = write_or_close(send_text(conn, 200, "first")).await {
                return ServeOutcome::Close(err);
            }
            timer::sleep_for(
                Duration::from_millis(50),
                &foundation_core::valtron::Ctx::host(),
            )
            .await;
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
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

impl Serve for PendingDropHandler {
    fn serve(
        &self,
        _ctx: Ctx,
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
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

impl Serve for AsyncEndHandler {
    fn serve(
        &self,
        _ctx: Ctx,
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
// Worker-parking handler

/// A one-shot gate an async handler awaits until the test opens it.
///
/// `Gate::open` sets the flag and wakes every waiting task. The waiting
/// futures park, so they hold no worker while the gate is closed. The read of
/// the flag and the registration of a waker both happen under the lock, so a
/// wake cannot be lost between them.
#[derive(Default)]
struct Gate {
    open: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

impl Gate {
    /// Open the gate and wake every task that waits on it.
    fn open(&self) {
        self.open.store(true, Ordering::SeqCst);
        let wakers = std::mem::take(
            &mut *self
                .wakers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for waker in wakers {
            waker.wake();
        }
    }
}

/// A future that resolves when its [`Gate`] opens.
struct GateWait(Arc<Gate>);

impl Future for GateWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0.open.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        let mut wakers = self
            .0
            .wakers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Re-check under the lock: `Gate::open` may have run between the first
        // check and the lock, in which case its wake saw no waker to wake.
        if self.0.open.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        if !wakers.iter().any(|kept| kept.will_wake(cx.waker())) {
            wakers.push(cx.waker().clone());
        }
        Poll::Pending
    }
}

/// Counts the long RPC futures that have started and are waiting on the gate.
type StartedCount = Arc<AtomicUsize>;

/// `GET /long` returns `Async` at once and writes only after the gate opens.
/// This is the shape of a long RPC: the connection task parks on the future.
struct LongRpcHandler {
    gate: Arc<Gate>,
    started: StartedCount,
}

impl ServeFactory for LongRpcHandler {
    fn create(ctx: &Ctx) -> Self {
        Self {
            gate: ctx
                .get_cloned::<Arc<Gate>>()
                .expect("the test stores the gate in the context ctx"),
            started: ctx
                .get_cloned::<StartedCount>()
                .expect("the test stores the started counter in the context ctx"),
        }
    }
}

impl Serve for LongRpcHandler {
    fn serve(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let gate = Arc::clone(&self.gate);
        let started = Arc::clone(&self.started);
        ConnectionResult::Async(Box::pin(async move {
            started.fetch_add(1, Ordering::SeqCst);
            GateWait(gate).await;
            match send_text(conn, 200, "long").split() {
                Ok(future) => future.await,
                Err(outcome) => outcome,
            }
        }))
    }
}

/// `GET /short` answers at once with a small response.
struct ShortHandler;

impl ServeFactory for ShortHandler {
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

impl Serve for ShortHandler {
    fn serve(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        send_text(conn, 200, "short")
    }
}

// ---------------------------------------------------------------------------
// Harness

/// Start a server that serves `GET /long` and `GET /short`, with the gate and
/// the started counter in the context ctx. Returns the same tuple as
/// [`start_server`].
fn start_gated_server(
    gate: Arc<Gate>,
    started: StartedCount,
) -> (
    std::net::SocketAddr,
    Arc<ShutdownSignal>,
    std::sync::mpsc::Receiver<()>,
) {
    let mut app = HttpApp::new_serve();
    app.context().store(gate);
    app.context().store(started);
    app.route::<LongRpcHandler>(SimpleMethod::GET, "/long");
    app.route::<ShortHandler>(SimpleMethod::GET, "/short");

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
    read_until_within(stream, needle, Duration::from_secs(5))
}

/// Read from `stream` until `needle` appears or `timeout` expires. The read
/// timeout is the hard bound: a stalled peer returns the bytes read so far.
fn read_until_within(stream: &mut TcpStream, needle: &str, timeout: Duration) -> String {
    stream.set_read_timeout(Some(timeout)).ok();
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

/// A long RPC that returns `Async` does not hold a valtron worker: short
/// requests are still served while the RPC waits (#254).
///
/// The multi-threaded valtron pool splits its threads and keeps at least two
/// task workers, so this case fills the pool with four parked RPCs. A handler
/// that held its worker would start only two of them and then queue the rest,
/// including the short request. Parking starts all four and leaves the workers
/// free, so the short request answers before the gate opens.
///
/// The started-counter wait and the short response read both have a hard
/// upper bound, so a regression fails with a message instead of hanging.
#[foundation_macros::valtron_test(seed = 633, threads = 1, timeout = 60_000)]
fn a_long_rpc_does_not_block_a_short_request() {
    // More parked RPCs than any task worker count this pool can have.
    const PARKED_RPCS: usize = 4;

    let gate = Arc::new(Gate::default());
    let started: StartedCount = Arc::new(AtomicUsize::new(0));
    let (addr, shutdown, done_rx) = start_gated_server(Arc::clone(&gate), Arc::clone(&started));

    // Open every long RPC on its own connection and write its request.
    let mut long_streams: Vec<TcpStream> = (0..PARKED_RPCS)
        .map(|_| {
            let mut stream = TcpStream::connect(addr).expect("connect long");
            stream
                .write_all(b"GET /long HTTP/1.1\r\nHost: t\r\n\r\n")
                .expect("write long request");
            stream
        })
        .collect();

    // Wait until every long RPC future has started. On the defect only the
    // task-worker count can start; the rest wait for a worker, so this bound
    // fails with a clear message.
    let start_deadline = Instant::now() + Duration::from_secs(5);
    while started.load(Ordering::SeqCst) < PARKED_RPCS {
        let running = started.load(Ordering::SeqCst);
        assert!(
            Instant::now() < start_deadline,
            "only {running} of {PARKED_RPCS} long RPCs started; a parked RPC must not hold a worker"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    // The gate is still closed. A short request on a new connection must answer
    // within the hard read bound.
    let t0 = Instant::now();
    let mut short = TcpStream::connect(addr).expect("connect short");
    short
        .write_all(b"GET /short HTTP/1.1\r\nHost: t\r\n\r\n")
        .expect("write short request");
    let response = read_until_within(&mut short, "short", Duration::from_secs(2));
    let elapsed = t0.elapsed();

    assert!(
        response.contains("HTTP/1.1 200"),
        "the short request should be served while the long RPCs wait: {response:?}"
    );
    assert!(
        response.contains("short"),
        "short response body: {response:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "the short request took {elapsed:?} while a long RPC waited; its future held a worker"
    );
    assert!(
        started.load(Ordering::SeqCst) >= PARKED_RPCS,
        "every long RPC must still be waiting on the gate"
    );

    // Open the gate: all long RPCs now write their response and close.
    gate.open();
    for stream in &mut long_streams {
        let response = read_until(stream, "long");
        assert!(
            response.contains("HTTP/1.1 200"),
            "long response: {response:?}"
        );
    }

    stop(&shutdown, &done_rx);
}
