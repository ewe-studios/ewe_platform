//! Keep-alive idleness and error-propagation regressions.
//!
//! WHY: a keep-alive wait must use the configured idle timeout, not a
//! size-based read timeout (GitHub issue #288). A response written without
//! reading the request body, or a failed middleware write, must not reuse a
//! desynchronised connection (GitHub issue #284).
//!
//! WHAT: drives a `ConnectionHandler` directly on an injected `SimulatedClock`
//! for the idle waits, and on an in-memory overlay for the write paths.
//!
//! HOW: the idle checks compare `clock.now()` with `idle_since`, so a
//! simulated advance proves the wait without a real sleep. The overlay lets a
//! client close its side on demand, so a write fails with `BrokenPipe`.

mod fake_overlay;

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::WaitGroup;
use foundation_core::valtron::{Ctx, SimulatedClock, TaskIterator, TaskStatus};
use foundation_http::native::app::H1App;
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::server::{ConnectionHandler, KeepAliveConfig};
use foundation_http::native::writer::send_text;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_http::shared::serve::ConnectionResult;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::{ConnectionContext, RawStream};
use foundation_netio::shared::http::{
    HTTPStreams, SendSafeBody, SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse, Status,
};

/// Answers a request with a marker that must never appear on a smuggled path.
struct SecretHandler;

impl ServeFactory for SecretHandler {
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

impl Serve for SecretHandler {
    fn serve(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        send_text(conn, 200, "smuggled-secret")
    }
}

/// Answers every request itself, without reading the request body.
struct AlwaysRespond;

impl RequestMiddleware for AlwaysRespond {
    fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Response(
            SimpleOutgoingResponse::builder()
                .with_status(Status::Forbidden)
                .with_body(SendSafeBody::Text("blocked".into()))
                .build()
                .expect("valid response"),
        )
    }
}

fn build_handler(
    app: H1App,
    stream: SharedByteBufferStream<RawStream>,
    config: &KeepAliveConfig,
    ctx: Ctx,
) -> ConnectionHandler {
    let waits = WaitGroup::new();
    waits.add(1);
    ConnectionHandler::new(
        app,
        HTTPStreams::new(stream.clone()),
        stream,
        "127.0.0.1:0".to_string(),
        Arc::new(ConnectionContext::default()),
        Arc::new(ShutdownSignal::new()),
        waits.guard(),
        config,
        None,
        ctx,
    )
}

/// A connected, non-blocking TCP pair. The client stays silent.
fn silent_tcp_pair() -> (TcpStream, SharedByteBufferStream<RawStream>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    server.set_nonblocking(true).expect("non-blocking");
    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).expect("raw stream"));
    (client, stream)
}

/// Drive the handler until it closes, or until `limit` polls prove it stays open.
fn drive_until_closed(handler: &mut ConnectionHandler, ctx: &Ctx, limit: usize) -> bool {
    for _ in 0..limit {
        if handler.next_status(ctx.clone()).is_none() {
            return true;
        }
    }
    false
}

#[test]
fn keepalive_waits_the_default_idle_timeout() {
    let (_client, stream) = silent_tcp_pair();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let app: H1App = Arc::new(HttpApp::<Arc<dyn Serve>>::with_ctx(Ctx::fixed(
        clock.clone(),
    )));
    let config = KeepAliveConfig::defaults();

    assert_eq!(
        config.idle_timeout(),
        Duration::from_secs(75),
        "the default idle timeout is the nginx keepalive_timeout default"
    );

    let ctx = Ctx::fixed(clock.clone());
    let mut handler = build_handler(app, stream, &config, ctx.clone());

    // First poll arms the idle deadline.
    assert!(
        handler.next_status(ctx.clone()).is_some(),
        "a fresh idle connection stays open"
    );

    clock.advance(Duration::from_secs(1));
    assert!(
        handler.next_status(ctx.clone()).is_some(),
        "a connection quiet for 1 s stays open under the 75 s default"
    );

    clock.advance(Duration::from_secs(73)); // 74 s total
    assert!(
        handler.next_status(ctx.clone()).is_some(),
        "the connection stays open before the idle timeout passes"
    );

    clock.advance(Duration::from_secs(2)); // 76 s total
    assert!(
        handler.next_status(ctx).is_none(),
        "the connection closes once the default 75 s idle timeout passes"
    );

    // A configured two-second idle timeout closes at 2 s, not at the
    // calculator's 120 s minimum read timeout.
    let (_client, stream) = silent_tcp_pair();
    let config = KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(2));
    let app: H1App = Arc::new(HttpApp::<Arc<dyn Serve>>::with_ctx(Ctx::fixed(
        clock.clone(),
    )));
    let ctx = Ctx::fixed(clock.clone());
    let mut handler = build_handler(app, stream, &config, ctx.clone());

    assert!(handler.next_status(ctx.clone()).is_some());
    clock.advance(Duration::from_secs(1));
    assert!(
        handler.next_status(ctx.clone()).is_some(),
        "a connection quiet for 1 s stays open under a 2 s idle timeout"
    );
    clock.advance(Duration::from_secs(2));
    assert!(
        handler.next_status(ctx).is_none(),
        "a configured 2 s idle timeout closes the connection at 2 s"
    );
}

#[test]
fn keepalive_disabled_closes_after_one_second() {
    let (_client, stream) = silent_tcp_pair();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let app: H1App = Arc::new(HttpApp::<Arc<dyn Serve>>::with_ctx(Ctx::fixed(
        clock.clone(),
    )));
    let config = KeepAliveConfig::disabled();

    assert_eq!(config.idle_timeout(), Duration::from_secs(1));

    let ctx = Ctx::fixed(clock.clone());
    let mut handler = build_handler(app, stream, &config, ctx.clone());

    assert!(
        drive_until_closed(&mut handler, &ctx, 1),
        "a disabled keep-alive closes at the first idle poll"
    );
}

#[test]
fn a_404_with_a_body_does_not_desync_the_next_request() {
    let (stream, client) = fake_overlay::overlay_pair(16 * 1024);

    // The body is itself a valid request line for a real route.
    let inner = "GET /secret HTTP/1.1\r\nHost: test\r\n\r\n";
    let request = format!(
        "POST /missing HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n\r\n{inner}",
        inner.len()
    );
    client.queue_request(request.as_bytes());

    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let mut app: HttpApp<Arc<dyn Serve>> = HttpApp::with_ctx(Ctx::fixed(clock.clone()));
    app.route::<SecretHandler>(SimpleMethod::GET, "/secret");

    let mut handler = build_handler(
        Arc::new(app),
        stream,
        &KeepAliveConfig::defaults(),
        Ctx::fixed(clock.clone()),
    );
    drive_until_closed(&mut handler, &Ctx::fixed(clock), 200);

    let response = String::from_utf8_lossy(&client.read_available()).into_owned();
    assert!(response.contains("404"), "expected a 404, got {response:?}");
    assert!(
        !response.contains("smuggled-secret"),
        "the unread body was served as a request: {response:?}"
    );
}

#[test]
fn a_middleware_write_error_closes_the_connection() {
    let (stream, client) = fake_overlay::overlay_pair(16 * 1024);
    client.queue_request(b"GET /blocked HTTP/1.1\r\nHost: test\r\n\r\n");

    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let mut app: HttpApp<Arc<dyn Serve>> = HttpApp::with_ctx(Ctx::fixed(clock.clone()));
    app.middleware(AlwaysRespond);

    // The client is gone, so the middleware response cannot be written.
    client.close();

    let mut handler = build_handler(
        Arc::new(app),
        stream,
        &KeepAliveConfig::defaults(),
        Ctx::fixed(clock.clone()),
    );

    assert!(
        drive_until_closed(&mut handler, &Ctx::fixed(clock), 200),
        "a middleware write error must close the connection"
    );
}

/// An idle keep-alive connection waits on its socket, not on a poll timer.
///
/// WHY: the idle path used to spin (`Pending`) and then fall back to `Delayed`
/// for a plain TCP socket, because only the overlay transport had a read waker.
/// The handler now parks on the socket through the shared reactor, so a parked
/// keep-alive connection wakes as soon as the next request arrives.
#[test]
fn an_idle_keepalive_connection_parks_on_its_socket() {
    let (mut client, stream) = silent_tcp_pair();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let app: H1App = Arc::new(HttpApp::<Arc<dyn Serve>>::with_ctx(Ctx::fixed(
        clock.clone(),
    )));
    let config = KeepAliveConfig::defaults();
    let mut handler = build_handler(app, stream, &config, Ctx::fixed(clock));

    // No bytes yet: the handler parks on the socket readiness, never on a poll
    // timer and never on a busy spin.
    let ready = match handler.next_status(Ctx::host()) {
        Some(TaskStatus::Depends(ready)) => ready,
        Some(TaskStatus::Delayed(_)) => {
            panic!("an idle keep-alive connection polled on a timer")
        }
        _ => panic!("an idle keep-alive connection did not park on readiness"),
    };
    assert!(!ready.is_ready(None), "readiness fired before any request");

    // The next request wakes the parked task through the reactor.
    client
        .write_all(b"GET / HTTP/1.1\r\nHost: test\r\n\r\n")
        .expect("client write");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !ready.is_ready(None) {
        assert!(
            std::time::Instant::now() < deadline,
            "the socket readiness never fired"
        );
        std::thread::sleep(Duration::from_millis(1));
    }

    // The task runs again and reads the request.
    match handler.next_status(Ctx::host()) {
        Some(TaskStatus::Pending(())) => {}
        _ => panic!("the parked handler did not read the request"),
    }
}
