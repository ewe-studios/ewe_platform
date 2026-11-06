//! Client-side deadline enforcement (feature 123).
//!
//! WHY: `Ctx::with_deadline` only records a timestamp. A call parked on a read
//! must fail on its own deadline, not on the peer's idle timeout.
//!
//! WHAT: drives a typed `Client` over the real native transports:
//! - `H1Transport` against a real `HttpServer` (the normal path completes) and
//!   against a listener that accepts but never answers (the deadline ends it).
//! - `H2Transport` against the same silent listener.
//!
//! HOW: every test runs under `#[valtron_test]`, because the transports and the
//! server share the process-wide valtron pool. A `SilentServer` accepts each
//! connection and holds it open, so nothing on the peer ever answers.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use foundation_core::valtron::valtron_test;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_http::ShutdownSignal;

use foundation_connectrpc::{
    Client, ClientOptions, Code, ConnectRpcServe, Ctx, H1Transport, H2Transport, HandlerOptions,
    JsonCodec, ProcedureCodecs, Request, Response, Router, Transport,
};

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

impl Msg {
    fn of(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

const UNARY: &str = "/t.Svc/Unary";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

fn router() -> Router {
    let mut router = Router::new();
    router.unary(
        UNARY,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new(),
    );
    router
}

/// Start the real Connect server over HTTP/1.1 on `127.0.0.1:0`.
fn start_h1_server() -> (SocketAddr, Arc<ShutdownSignal>) {
    let serve: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(router().into_handler()));
    let mut app = HttpApp::new_serve();
    app.router.add_route_any(UNARY, &serve);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(ShutdownSignal::new());
    let server = HttpServer::with_config(
        app,
        &format!("127.0.0.1:{}", addr.port()),
        ServerConfig::defaults(),
    );
    let server_shutdown = shutdown.clone();
    thread::spawn(move || server.serve_with_listener(&listener, &server_shutdown));
    (addr, shutdown)
}

/// A listener that accepts every connection and never answers. Each accepted
/// socket is held open until the server is dropped.
struct SilentServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl SilentServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        listener.set_nonblocking(true).expect("nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut held: Vec<TcpStream> = Vec::new();
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _peer)) => held.push(stream),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
            drop(held);
        });
        Self {
            addr,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for SilentServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn client_for(transport: Arc<dyn Transport>, addr: SocketAddr) -> Client<Msg, Msg> {
    client_with_options(transport, addr, ClientOptions::new().with_codec("json"))
}

fn client_with_options(
    transport: Arc<dyn Transport>,
    addr: SocketAddr,
    options: ClientOptions,
) -> Client<Msg, Msg> {
    let url = format!("http://127.0.0.1:{}{UNARY}", addr.port());
    Client::new(transport, &url, codecs(), options).expect("build client")
}

/// A call with a deadline fails on that deadline when the peer never answers.
///
/// This fails on the code before feature 123: the call waits for the H1 client's
/// own read timeout (or the test's cap), not for the 200 ms deadline.
#[valtron_test(timeout = 5_000)]
async fn a_call_fails_on_its_deadline_when_the_server_never_answers() {
    let server = SilentServer::start();
    let client = client_for(Arc::new(H1Transport::default()), server.addr);

    let started = Instant::now();
    let result = client
        .unary(
            Ctx::background().with_deadline(Duration::from_millis(200)),
            Request::new(Msg::of("never answered")),
        )
        .await;
    let elapsed = started.elapsed();

    let err = result.expect_err("the call must fail on its deadline");
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the 200 ms deadline must end the call in under 1 s, took {elapsed:?}"
    );
}

/// An HTTP/2 call enforces the same deadline through the same helper.
#[valtron_test(timeout = 5_000)]
async fn an_h2_call_fails_on_its_deadline_when_the_server_never_answers() {
    let server = SilentServer::start();
    let client = client_for(Arc::new(H2Transport::new()), server.addr);

    let started = Instant::now();
    let result = client
        .unary(
            Ctx::background().with_deadline(Duration::from_millis(200)),
            Request::new(Msg::of("never answered")),
        )
        .await;
    let elapsed = started.elapsed();

    let err = result.expect_err("the call must fail on its deadline");
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the 200 ms deadline must end the call in under 1 s, took {elapsed:?}"
    );
}

/// A client-wide default deadline ends a call that has no `Ctx` deadline.
#[valtron_test(timeout = 5_000)]
async fn a_client_default_deadline_ends_a_call_without_a_ctx_deadline() {
    let server = SilentServer::start();
    let client = client_with_options(
        Arc::new(H1Transport::default()),
        server.addr,
        ClientOptions::new()
            .with_codec("json")
            .with_default_deadline(Duration::from_millis(200)),
    );

    let started = Instant::now();
    let result = client
        .unary(Ctx::background(), Request::new(Msg::of("never answered")))
        .await;
    let elapsed = started.elapsed();

    let err = result.expect_err("the client default deadline must end the call");
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the 200 ms default deadline must end the call in under 1 s, took {elapsed:?}"
    );
}

/// The normal path still works when the call has no deadline.
#[valtron_test(timeout = 10_000)]
async fn a_call_with_no_deadline_still_completes() {
    let (addr, shutdown) = start_h1_server();
    let client = client_for(Arc::new(H1Transport::default()), addr);

    let response = client
        .unary(Ctx::background(), Request::new(Msg::of("hello")))
        .await
        .expect("a call with no deadline must complete");

    assert_eq!(response.msg, Msg::of("hello"));
    shutdown.turn_on();
}

/// A deadline that already passed fails before any request opens a connection.
#[valtron_test(timeout = 5_000)]
async fn a_deadline_in_the_past_fails_without_a_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let addr = listener.local_addr().expect("addr");
    let client = client_for(Arc::new(H1Transport::default()), addr);

    let result = client
        .unary(
            Ctx::background().with_deadline(Duration::ZERO),
            Request::new(Msg::of("too late")),
        )
        .await;

    let Err(err) = result else {
        panic!("a past deadline must fail the call");
    };
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    match listener.accept() {
        Ok(_) => panic!("a call with a past deadline must not connect"),
        Err(e) => assert_eq!(
            e.kind(),
            std::io::ErrorKind::WouldBlock,
            "unexpected accept error: {e}"
        ),
    }
}
/// A bidi call with a past deadline also fails before it dials. The setup is
/// lazy, so the past-deadline check runs before `transport.open`.
#[valtron_test(timeout = 5_000)]
async fn a_bidi_deadline_in_the_past_fails_without_a_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let addr = listener.local_addr().expect("addr");
    let client = client_for(Arc::new(H2Transport::new()), addr);

    let result = client
        .bidi_stream(
            Ctx::background().with_deadline(Duration::ZERO),
            futures::stream::empty::<Msg>(),
        )
        .await;

    let Err(err) = result else {
        panic!("a past deadline must fail the call");
    };
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    match listener.accept() {
        Ok(_) => panic!("a call with a past deadline must not connect"),
        Err(e) => assert_eq!(
            e.kind(),
            std::io::ErrorKind::WouldBlock,
            "unexpected accept error: {e}"
        ),
    }
}
