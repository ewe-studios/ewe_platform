//! Client-side deadline enforcement (features 123, 124 and 131).
//!
//! WHY: `Ctx::with_deadline` only records a timestamp. A call parked on a read
//! must fail on its own deadline, not on the peer's idle timeout — during both
//! setup and every later receive. A facade that drops, or a wait that times
//! out, must also stop the call's reader and writer tasks, not only the caller.
//!
//! WHAT: drives a typed `Client` over the real native transports:
//! - `H1Transport` against a real `HttpServer` (the normal path completes) and
//!   against a listener that accepts but never answers (the deadline ends it).
//! - `H2Transport` against the same silent listener.
//! - `H2Transport` against a raw HTTP/2 peer that sends one Connect message and
//!   then stalls. The peer ignores the call timeout header, so only the client
//!   can end the later receives. The same peer reports whether it saw the
//!   client close its socket after a drop or a timeout.
//! - `H2Transport` against a real `HttpServer` whose streaming handler resumes
//!   after a stall (no deadline is set, so the receive waits).
//!
//! HOW: every test runs under `#[valtron_test]`, because the transports and the
//! server share the process-wide valtron pool. A `SilentServer` accepts each
//! connection and holds it open, so nothing on the peer ever answers. The raw
//! peer holds its socket open after the first message.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures::StreamExt;

use foundation_core::valtron::valtron_test;
use foundation_http::native::serve::H2Serve;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::serve::Serve;
use foundation_http::ShutdownSignal;
use foundation_netio::http2::channel::H2Channel;
use foundation_netio::http2::frame::Kind;

use foundation_connectrpc::shared::envelope::Envelope;
use foundation_connectrpc::shared::error::ConnectResult;
use foundation_connectrpc::{
    Client, ClientOptions, Code, ConnectRpcServe, ConnectRpcServeH2, Ctx, H1Transport, H2Transport,
    HandlerOptions, JsonCodec, ProcedureCodecs, Request, Response, Router, Transport,
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
const SERVER_STREAM: &str = "/t.Svc/ServerStream";
const RESUME_STREAM: &str = "/t.Svc/ResumeStream";
const BIDI: &str = "/t.Svc/Bidi";

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

/// One server-stream item, then a second item after 400 ms. Shows that a
/// stream without a deadline still waits and later resumes.
fn first_then_later() -> impl futures::Stream<Item = ConnectResult<Msg>> + Send {
    let later = futures::stream::once(async {
        foundation_core::valtron::sleep_async(Duration::from_millis(400)).await;
        Ok::<Msg, _>(Msg::of("second"))
    });
    futures::stream::once(async { Ok::<Msg, _>(Msg::of("first")) }).chain(later)
}

/// A router with one procedure that answers once and resumes after 400 ms.
fn resume_router() -> Router {
    let mut router = Router::new();
    router.server_stream(
        RESUME_STREAM,
        codecs(),
        |_ctx: Ctx, _req: Request<Msg>| async move { Ok(first_then_later()) },
        HandlerOptions::new(),
    );
    router
}

/// Start the real Connect server over HTTP/2 on `127.0.0.1:0`.
fn start_resume_server() -> (String, Arc<ShutdownSignal>) {
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(resume_router().into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(RESUME_STREAM, rpc);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(ShutdownSignal::new());
    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || server.serve_with_listener(&listener, &server_shutdown));
    (addr.to_string(), shutdown)
}

fn streaming_client(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client")
}

fn deadline_ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_millis(200))
}

// ── Raw HTTP/2 peer that does not enforce the deadline ──────────────────────

/// Write every queued frame to the socket.
fn flush_frames(channel: &mut H2Channel, socket: &mut TcpStream) {
    let out = channel.drain_output();
    if !out.is_empty() {
        socket.write_all(&out).expect("server write");
        socket.flush().expect("server flush");
    }
}

/// Read the bytes the socket has ready into the channel.
fn feed_available(channel: &mut H2Channel, socket: &mut TcpStream) {
    let mut buf = [0u8; 8192];
    match socket.read(&mut buf) {
        Ok(0) => panic!("client closed the connection"),
        Ok(n) => channel.feed_input(&buf[..n]),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) => {}
        Err(e) => panic!("server read failed: {e}"),
    }
}

/// Run the server side of the h2 handshake to completion.
fn server_handshake(channel: &mut H2Channel, socket: &mut TcpStream) {
    loop {
        match channel.server_handshake_step() {
            Ok(()) => {
                flush_frames(channel, socket);
                return;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                flush_frames(channel, socket);
                feed_available(channel, socket);
            }
            Err(e) => panic!("server handshake failed: {e}"),
        }
    }
}

/// Read frames until the request HEADERS arrives, and return its stream id.
fn read_request_stream_id(channel: &mut H2Channel, socket: &mut TcpStream) -> u32 {
    loop {
        match channel.read_frame() {
            Ok((head, _payload)) => {
                flush_frames(channel, socket);
                if head.kind == Kind::Headers {
                    return head.stream_id;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                flush_frames(channel, socket);
                feed_available(channel, socket);
            }
            Err(e) => panic!("server frame read failed: {e}"),
        }
    }
}

/// Read the socket until the client closes it or `bound` passes.
///
/// Returns `true` on `Ok(0)` (a clean TCP close) or a reset, `false` when the
/// socket is still open at the bound. A read timeout is not a close: it only
/// means the peer sent nothing in that slice.
fn observes_close(socket: &mut TcpStream, bound: Duration) -> bool {
    let until = Instant::now() + bound;
    let mut buf = [0u8; 8192];
    while Instant::now() < until {
        match socket.read(&mut buf) {
            Ok(0) => return true,
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                return true;
            }
            Err(_) => return false,
        }
    }
    false
}

/// A raw HTTP/2 peer that sends the response head and one Connect message,
/// then stalls.
///
/// WHY: the real `HttpServer` enforces the call's `Connect-Timeout-Ms` header
/// itself, so it cannot show whether the *client* enforces its own deadline.
/// This peer ignores the header. Only a client-side timer can end the wait.
///
/// It returns whether the client closed the socket within two seconds of the
/// stall. A deadline that stops only the caller leaves the socket open.
fn start_stalled_peer() -> (SocketAddr, thread::JoinHandle<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");

        let mut channel = H2Channel::new(true);
        server_handshake(&mut channel, &mut socket);
        let stream_id = read_request_stream_id(&mut channel, &mut socket);

        // The response head names the Connect streaming protocol.
        let headers = vec![(
            Bytes::from_static(b"content-type"),
            Bytes::from_static(b"application/connect+json"),
        )];
        channel
            .send_headers_response(stream_id, 200, &headers, false)
            .expect("response head");
        flush_frames(&mut channel, &mut socket);

        // One enveloped `Msg { text: "first" }`, then no more frames.
        let mut frame = Vec::new();
        Envelope {
            flags: 0,
            data: Bytes::from_static(br#"{"text":"first"}"#),
        }
        .encode_into(&mut frame);
        channel
            .send_data_frame(stream_id, &frame, false)
            .expect("first message");
        flush_frames(&mut channel, &mut socket);

        // Stall until the client drops the connection or the bound passes.
        observes_close(&mut socket, Duration::from_secs(2))
    });
    (addr, handle)
}

/// A raw HTTP/2 peer that sends only the response head, then stalls.
///
/// WHY: a peer that sends a head and no body frame must still let the call
/// setup finish. The reader publishes the head before it waits for a frame, so
/// `server_stream` returns without a frame.
///
/// It returns whether the client closed the socket within two seconds of the
/// stall. A call that never returns, or a drop that stops nothing, leaves the
/// socket open.
fn start_head_only_peer() -> (SocketAddr, thread::JoinHandle<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");

        let mut channel = H2Channel::new(true);
        server_handshake(&mut channel, &mut socket);
        let stream_id = read_request_stream_id(&mut channel, &mut socket);

        // The response head names the Connect streaming protocol. No frame
        // follows it.
        let headers = vec![(
            Bytes::from_static(b"content-type"),
            Bytes::from_static(b"application/connect+json"),
        )];
        channel
            .send_headers_response(stream_id, 200, &headers, false)
            .expect("response head");
        flush_frames(&mut channel, &mut socket);

        // Stall until the client drops the connection or the bound passes.
        observes_close(&mut socket, Duration::from_secs(2))
    });
    (addr, handle)
}

/// A raw HTTP/2 peer for a bidi call. It sends the response head at once, then
/// waits for the test to say the sender half has dropped before it sends one
/// message.
///
/// This order proves that the surviving receive half is still live after its
/// sibling drops: the message is sent after the drop and must still arrive.
fn start_bidi_peer() -> (SocketAddr, mpsc::Sender<()>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (go_tx, go_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");

        let mut channel = H2Channel::new(true);
        server_handshake(&mut channel, &mut socket);
        let stream_id = read_request_stream_id(&mut channel, &mut socket);

        let headers = vec![(
            Bytes::from_static(b"content-type"),
            Bytes::from_static(b"application/connect+json"),
        )];
        channel
            .send_headers_response(stream_id, 200, &headers, false)
            .expect("response head");
        flush_frames(&mut channel, &mut socket);

        // Send only after the test drops the sender half.
        go_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("the test must release the peer after the drop");

        let mut frame = Vec::new();
        Envelope {
            flags: 0,
            data: Bytes::from_static(br#"{"text":"after-drop"}"#),
        }
        .encode_into(&mut frame);
        channel
            .send_data_frame(stream_id, &frame, false)
            .expect("message after the drop");
        flush_frames(&mut channel, &mut socket);

        // Drain until the test drops the receive half, so the call ends
        // cleanly rather than on the peer's own bound.
        let _ = observes_close(&mut socket, Duration::from_secs(1));
    });
    (addr, go_tx, handle)
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

/// After the first message, a stalled server-stream receive fails on the call
/// deadline.
///
/// The peer ignores the call's timeout header, so only the client's own timer
/// can end the wait. This fails on the code before feature 124: the second
/// receive has no timer and runs to the test's own bound.
#[valtron_test(timeout = 10_000)]
async fn a_server_stream_receive_fails_on_the_call_deadline() {
    let (addr, peer) = start_stalled_peer();
    let client = streaming_client(&addr.to_string(), SERVER_STREAM);

    let mut stream = client
        .server_stream(deadline_ctx(), Request::new(Msg::of("item")))
        .await
        .expect("server stream");
    let first = stream
        .receive()
        .await
        .expect("the first receive must succeed")
        .expect("the server sends one message");
    assert_eq!(first, Msg::of("first"));

    let started = Instant::now();
    let err = stream
        .receive()
        .await
        .expect_err("the second receive must fail on its deadline");
    let elapsed = started.elapsed();

    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the 200 ms deadline must end the receive in under 1 s, took {elapsed:?}"
    );
    drop(stream);
    peer.join().expect("peer thread");
}

/// After the first message, a stalled bidi receive fails on the call deadline,
/// for both `BidiStream::receive` and the split `BidiReceiver::receive`.
#[valtron_test(timeout = 10_000)]
async fn a_bidi_receive_fails_on_the_call_deadline() {
    // Whole stream.
    let (addr, peer) = start_stalled_peer();
    let client = streaming_client(&addr.to_string(), BIDI);
    let mut stream = client
        .bidi_stream(deadline_ctx(), futures::stream::empty::<Msg>())
        .await
        .expect("bidi stream");
    let first = stream
        .receive()
        .await
        .expect("the first receive must succeed")
        .expect("the server sends one message");
    assert_eq!(first, Msg::of("first"));

    let started = Instant::now();
    let err = stream
        .receive()
        .await
        .expect_err("the second receive must fail on its deadline");
    let elapsed = started.elapsed();
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the 200 ms deadline must end the receive in under 1 s, took {elapsed:?}"
    );
    drop(stream);
    peer.join().expect("peer thread");

    // Split receive half.
    let (addr, peer) = start_stalled_peer();
    let client = streaming_client(&addr.to_string(), BIDI);
    let stream = client
        .bidi_stream(deadline_ctx(), futures::stream::empty::<Msg>())
        .await
        .expect("bidi stream");
    let (_sender, mut receiver) = stream.split();
    let first = receiver
        .receive()
        .await
        .expect("the first receive must succeed")
        .expect("the server sends one message");
    assert_eq!(first, Msg::of("first"));

    let started = Instant::now();
    let err = receiver
        .receive()
        .await
        .expect_err("the split second receive must fail on its deadline");
    let elapsed = started.elapsed();
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the 200 ms deadline must end the receive in under 1 s, took {elapsed:?}"
    );
    drop(receiver);
    peer.join().expect("peer thread");
}

/// Dropping a server stream closes the socket, even though the peer never
/// sends a body frame.
///
/// The peer sends only the head, then nothing. The reader publishes the head
/// before it waits for a frame, so the call setup returns without a frame.
/// Before feature 135 `response_headers` waited for the first body frame, so
/// `server_stream(..).await` never returned. After the setup, the drop fires
/// the call's cancel signal, which stops the reader and closes the socket.
#[valtron_test(timeout = 10_000)]
async fn dropping_a_server_stream_closes_the_socket() {
    let (addr, peer) = start_head_only_peer();
    let client = streaming_client(&addr.to_string(), SERVER_STREAM);

    let stream = client
        .server_stream(Ctx::background(), Request::new(Msg::of("item")))
        .await
        .expect("a head-only peer must not stall the call setup");
    drop(stream);
    let started = Instant::now();
    while !peer.is_finished() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the peer never saw the close"
        );
        foundation_core::valtron::sleep_async(Duration::from_millis(10)).await;
    }

    let closed = peer.join().expect("peer thread");
    assert!(
        closed,
        "dropping the stream must close the peer's socket within 2 s"
    );
}

/// Dropping one half of a split bidi stream keeps the other half live.
///
/// The peer sends its message only after the sender half drops, so a receive
/// that still works proves the drop did not cancel the call.
#[valtron_test(timeout = 10_000)]
async fn dropping_one_bidi_half_keeps_the_other_alive() {
    let (addr, go, peer) = start_bidi_peer();
    let client = streaming_client(&addr.to_string(), BIDI);

    let stream = client
        .bidi_stream(Ctx::background(), futures::stream::empty::<Msg>())
        .await
        .expect("bidi stream");
    let (sender, mut receiver) = stream.split();
    drop(sender);
    go.send(()).expect("release the peer after the drop");

    let msg = receiver
        .receive()
        .await
        .expect("the receive half must still be live")
        .expect("the peer sends a message after the drop");
    assert_eq!(msg, Msg::of("after-drop"));

    drop(receiver);
    peer.join().expect("peer thread");
}

/// A timed-out receive stops the call, so the peer sees the socket close.
///
/// Before feature 131 the wait passed `None` as the cancel argument: the caller
/// got `DeadlineExceeded` but the reader task stayed parked and the socket
/// stayed open.
#[valtron_test(timeout = 10_000)]
async fn a_timed_out_receive_cancels_the_call() {
    let (addr, peer) = start_stalled_peer();
    let client = streaming_client(&addr.to_string(), SERVER_STREAM);

    let mut stream = client
        .server_stream(deadline_ctx(), Request::new(Msg::of("item")))
        .await
        .expect("server stream");
    let first = stream
        .receive()
        .await
        .expect("the first receive must succeed")
        .expect("the server sends one message");
    assert_eq!(first, Msg::of("first"));

    let err = stream
        .receive()
        .await
        .expect_err("the second receive must fail on its deadline");
    assert_eq!(
        err.current_context().code(),
        Code::DeadlineExceeded,
        "expected DeadlineExceeded, got: {err}"
    );

    let closed = peer.join().expect("peer thread");
    assert!(
        closed,
        "a timed-out receive must close the peer's socket within 2 s"
    );
    drop(stream);
}

/// Without a deadline the stall is observable, and the stream still delivers
/// its data when the server resumes.
#[valtron_test(timeout = 10_000)]
async fn a_receive_with_no_deadline_still_waits() {
    let (addr, shutdown) = start_resume_server();
    let client = streaming_client(&addr, RESUME_STREAM);

    let mut stream = client
        .server_stream(Ctx::background(), Request::new(Msg::of("item")))
        .await
        .expect("server stream");
    let first = stream
        .receive()
        .await
        .expect("the first receive must succeed")
        .expect("the server sends one message");
    assert_eq!(first, Msg::of("first"));

    // The server sends the second item after 400 ms. A 150 ms bound must lose
    // the race, which proves the client is still waiting, not failing.
    let receive = stream.receive();
    futures::pin_mut!(receive);
    let sleeper = Box::pin(foundation_core::valtron::sleep_async(
        Duration::from_millis(150),
    ));
    match futures::future::select(receive, sleeper).await {
        futures::future::Either::Left((item, _)) => {
            panic!("the second receive must still wait, got {item:?}");
        }
        futures::future::Either::Right(((), receive)) => {
            let second = receive
                .await
                .expect("the stream must resume after the stall")
                .expect("the server sends the second message");
            assert_eq!(second, Msg::of("second"));
        }
    }
    shutdown.turn_on();
}
