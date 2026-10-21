//! Deadline pass-through + cancellation propagation (spec 63 T13).
//!
//! WHY: A `grpc://` backend enforces the deadline it reads from the `grpc-timeout`
//! header and cancels its handler when the peer goes away. The proxy's
//! Connect→gRPC relay must therefore (a) translate the Connect wire deadline
//! (`connect-timeout-ms`, an integer of milliseconds) into the gRPC
//! `grpc-timeout` form, and (b) propagate a client reset into the outbound
//! exchange so a cancelled call tears down both sides instead of leaving the
//! backend streaming into a dead connection.
//!
//! WHAT: Two e2e tests. The first observes the backend's `ctx.remaining_timeout()`
//! equals the client's deadline; the second cancels a server-stream mid-flight
//! and observes the backend handler's `ctx.is_canceled()` flip.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron_test;
use foundation_http::native::server::HttpServer;
use foundation_http::native::serve::H2Serve;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_netio::http2::channel::H2Channel;
use foundation_netio::http2::connection::H2Request;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, HandlerOptions, JsonCodec,
    ProcedureCodecs, Request, Response, Router,
};

use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

const UNARY: &str = "/e2e.Rpc/Unary";
const SERVER_STREAM: &str = "/e2e.Rpc/ServerStream";

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

/// A unary handler that reports the call's remaining deadline in milliseconds,
/// so a test can observe the deadline the backend parsed from `grpc-timeout`.
fn build_deadline_router() -> Router {
    let mut router = Router::new();
    router.unary(
        UNARY,
        codecs(),
        |ctx: Ctx, _req: Request<Msg>| async move {
            let ms = ctx
                .remaining_timeout()
                .map(|d| d.as_millis())
                .unwrap_or(0);
            Ok(Response::new(Msg::of(ms.to_string())))
        },
        HandlerOptions::new(),
    );
    router
}

/// Where a handler leaves its `Ctx` for the test to watch (mirrors
/// `foundation_connectrpc`'s `cancellation_tests`).
#[derive(Clone, Default)]
struct Slot(Arc<Mutex<Option<Ctx>>>);

impl Slot {
    fn put(&self, ctx: &Ctx) {
        *self.0.lock().unwrap() = Some(ctx.clone());
    }

    /// Wait until a handler parked its `Ctx`.
    fn wait_entered(&self) -> Ctx {
        let start = Instant::now();
        loop {
            if let Some(ctx) = self.0.lock().unwrap().clone() {
                return ctx;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the handler never ran"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Wait until `ctx` is cancelled; panics after `limit`.
fn assert_cancelled_within(ctx: &Ctx, limit: Duration) {
    let start = Instant::now();
    while !ctx.is_canceled() {
        assert!(
            start.elapsed() < limit,
            "the handler's Ctx was not cancelled within {limit:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A server-stream handler that yields one item (which flushes the response head,
/// withheld until the first frame) and then parks until the call is cancelled.
/// It leaves its `Ctx` in `slot` so the test can watch `is_canceled()` flip.
fn build_cancel_router(slot: Slot) -> Router {
    let mut router = Router::new();
    router.server_stream(
        SERVER_STREAM,
        codecs(),
        move |ctx: Ctx, _req: Request<Msg>| {
            let slot = slot.clone();
            async move {
                slot.put(&ctx);
                let stream = futures_util::stream::unfold(0u8, move |state| async move {
                    match state {
                        // The first item flushes the response head so the
                        // client gets the stream before it cancels.
                        0 => Some((Ok(Msg::of("first")), 1u8)),
                        // Then park forever; the test watches the parked `Ctx`
                        // (not the stream) for cancellation.
                        _ => {
                            futures_util::future::pending::<()>().await;
                            None
                        }
                    }
                });
                Ok(stream)
            }
        },
        HandlerOptions::new(),
    );
    router
}

/// Start an HTTP/2-only backend (native gRPC) serving `router`.
fn start_grpc_backend(router: Router) -> (u16, Arc<OnSignal>) {
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
    let mut h2_app = HttpApp::new_h2_serve();
    h2_app.route_any_h2(UNARY, h2.clone());
    h2_app.route_any_h2(SERVER_STREAM, h2);

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

/// A proxy whose service routes to a `grpc://` backend (the D-leg relay).
fn proxy_config_grpc(backend_port: u16) -> ProxyConfig {
    let svc = ServiceConfig::new("rpc", "127.0.0.1")
        .backend(&format!("grpc://127.0.0.1:{backend_port}"));
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

// ── Deadline pass-through ───────────────────────────────────────────────────

/// WHY: a Connect client sets its deadline in `connect-timeout-ms`; a native-gRPC
/// backend reads `grpc-timeout`. The relay must translate the former into the
/// latter, or the backend sees no deadline at all.
#[valtron_test(seed = 63, threads = 8, timeout = 60_000)]
async fn connect_deadline_is_observed_at_grpc_backend() {
    let (backend_port, backend_shutdown) = start_grpc_backend(build_deadline_router());
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    let client = connect_h2_client(&addr, UNARY);
    let deadline = Duration::from_secs(3);
    let ctx = Ctx::background().with_deadline(deadline);
    let resp = client
        .unary(ctx, Request::new(Msg::of("deadline")))
        .await
        .expect("connect unary");

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    let ms: u128 = resp.msg.text.parse().expect("backend deadline millis");
    assert!(ms > 0, "backend must observe a deadline, got {ms}ms");
    assert!(
        ms <= deadline.as_millis(),
        "backend deadline {ms}ms must not exceed the client's {}ms",
        deadline.as_millis()
    );
}

// ── Cancellation propagation ────────────────────────────────────────────────

/// A single Connect envelope around a JSON `Msg` (flag byte + 4-byte big-endian
/// length + payload), the request shape `ConnectHandler` de-frames.
fn connect_envelope(text: &str) -> Vec<u8> {
    let payload = serde_json::to_vec(&Msg::of(text)).expect("marshal Msg");
    let mut buf = vec![0u8];
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(&payload);
    buf
}

/// Drive a raw HTTP/2 client: open a Connect server-stream through the proxy,
/// read the first item, report the stream is flowing, then (on a go signal from
/// the test) send a genuine `RST_STREAM`. Runs on a dedicated thread — its
/// socket reads block.
///
/// WHY a raw client: `foundation_connectrpc`'s typed client `cancel()` does not
/// yet emit a wire-level reset (its H2 pump keeps the socket open), so it cannot
/// represent "the client cancelled" here. An explicit `RST_STREAM` is exactly
/// what an HTTP/2 client does when it aborts a call, so it is the faithful
/// signal the proxy's `feed_request_body` turns into the shared cancel.
fn drive_raw_h2_cancel(
    addr: &str,
    flowing: std::sync::mpsc::Sender<bool>,
    reset: std::sync::mpsc::Receiver<()>,
) {
    let mut stream = TcpStream::connect(addr).expect("connect proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("write timeout");

    let mut channel = H2Channel::new(false); // client

    // Handshake: send preface + SETTINGS, exchange SETTINGS with the proxy.
    loop {
        match channel.client_handshake_step() {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                let out = channel.drain_output();
                if !out.is_empty() {
                    stream.write_all(&out).expect("write handshake");
                }
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).expect("read handshake");
                assert!(n > 0, "proxy closed during handshake");
                channel.feed_input(&buf[..n]);
            }
            Err(e) => panic!("handshake failed: {e}"),
        }
    }
    let out = channel.drain_output();
    if !out.is_empty() {
        stream.write_all(&out).expect("flush handshake");
    }

    let sid = channel
        .send_request(&H2Request {
            method: Bytes::from_static(b"POST"),
            scheme: Bytes::from_static(b"http"),
            authority: Bytes::from(addr.to_string()),
            path: Bytes::from(SERVER_STREAM.to_string()),
            headers: vec![(
                Bytes::from_static(b"content-type"),
                Bytes::from_static(b"application/connect+json"),
            )],
            body: None,
            end_stream: false,
        })
        .expect("send request headers");
    channel
        .send_data_frame(sid, &connect_envelope("cancel-me"), true)
        .expect("send request envelope");
    stream
        .write_all(&channel.drain_output())
        .expect("flush request");

    // Read until the response head and the first item arrive.
    let mut body = Vec::new();
    let mut got_head = false;
    loop {
        let mut buf = [0u8; 8192];
        match stream.read(&mut buf) {
            Ok(0) => {
                let _ = flowing.send(false);
                return;
            }
            Ok(n) => channel.feed_input(&buf[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                let _ = flowing.send(false);
                return;
            }
            Err(e) => panic!("read response: {e}"),
        }
        if !got_head {
            if let Ok(Some((_sid, _head))) = channel.recv_response() {
                got_head = true;
            }
        }
        while let Ok(Some((_sid, data, _end))) = channel.recv_data_frame() {
            body.extend_from_slice(&data);
        }
        if got_head && String::from_utf8_lossy(&body).contains("\"first\"") {
            break;
        }
    }

    // The stream is flowing; the test waits for the handler's `Ctx` before
    // granting the reset so it can assert the call is still live.
    let _ = flowing.send(true);
    if reset.recv_timeout(Duration::from_secs(10)).is_err() {
        return;
    }

    // Abort the call: RST_STREAM, which the proxy relays into a teardown of the
    // outbound gRPC exchange.
    channel.send_rst_stream(sid);
    let rst = channel.drain_output();
    let _ = stream.write_all(&rst);
}

/// WHY: cancelling a client call must tear down both sides — the proxy must fire
/// the outbound exchange's cancel signal so the backend stops streaming into a
/// dead connection, not just half-close the request direction.
#[valtron_test(seed = 63, threads = 8, timeout = 60_000)]
async fn cancelled_connect_stream_tears_down_grpc_backend() {
    let slot = Slot::default();
    let (backend_port, backend_shutdown) =
        start_grpc_backend(build_cancel_router(slot.clone()));
    let proxy = ProxyServer::start(proxy_config_grpc(backend_port)).expect("start proxy");
    let addr = format!("127.0.0.1:{}", proxy.local_addr().port());

    // The raw client reports once it has read the first item, then waits for the
    // test's go signal before sending its reset.
    let (flowing_tx, flowing_rx) = std::sync::mpsc::channel::<bool>();
    let (reset_tx, reset_rx) = std::sync::mpsc::channel::<()>();
    let client_addr = addr.clone();
    std::thread::spawn(move || drive_raw_h2_cancel(&client_addr, flowing_tx, reset_rx));

    let flowing = flowing_rx.recv_timeout(Duration::from_secs(10)).unwrap_or(false);
    assert!(flowing, "the stream must flow before the reset");

    // The handler parked its `Ctx` before yielding the first item; the call is
    // still live (the reset has not been granted yet).
    let ctx = slot.wait_entered();
    assert!(!ctx.is_canceled(), "the call must still be live before the reset");

    // Abort the client call; the relay's cancel signal must reach the backend.
    reset_tx.send(()).expect("grant the reset");
    assert_cancelled_within(&ctx, Duration::from_secs(5));

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();
}
