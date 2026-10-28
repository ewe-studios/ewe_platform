//! Request streaming for Connect RPC over HTTP/3 (F35).
//!
//! WHY: `dispatch_h3` used to read the whole request body into memory before a
//! streaming RPC ran. A client-stream handler then saw nothing until the client
//! finished sending, bidi could not interleave, and a unary body was collected
//! in full before `read_max_bytes` was checked. These tests pin the streaming
//! behaviour: request DATA frames reach the handler as they arrive, a unary body
//! over the cap is refused with `413` without waiting for the rest of it, and a
//! peer reset mid-body aborts the call instead of passing for a clean end.
//!
//! HOW: most tests use a live QUIC pair on loopback with `dispatch_h3` polled by
//! hand alongside the two QUIC drivers, as `h3_conformance_tests` does, so each
//! can interleave client sends with server progress deterministically; those run
//! no executor and are plain `#[test]`s. The last two run the production path —
//! `foundation_http`'s H3 server on the valtron pool and the typed `Client` over
//! `H3Transport` — under `#[valtron_test]`.

#![cfg(feature = "h3")]

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::{valtron_test, Ctx as VCtx, Stream as VStream, TaskIterator};
use foundation_http::native::serve::H3Serve;
use foundation_http::native::server::serve_h3;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_netio::http3::connection::error_code;
use foundation_netio::http3::{H3Connection, H3Error, H3Request};
use foundation_netio::netcap::context::ConnectionContext;
use foundation_netio::quic::{
    client_config_trusting_pem, server_config_from_pem, QuicDriver, QuicStreamError,
    QuinnBidiStream, QuinnConnection,
};

use foundation_connectrpc::native::h3_serve::{dispatch_h3, ConnectRpcServeH3};
use foundation_connectrpc::shared::context::CancelSignal;
use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H3Transport, HandlerOptions, JsonCodec, ProcedureCodecs, Request,
    RequestStream, Response, Router,
};
use futures::StreamExt;

const CERT_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_key.pem");
const CA_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_ca.pem");
const BUDGET: Duration = Duration::from_secs(10);

const UNARY: &str = "/t.Svc/Unary";
const CLIENT_STREAM: &str = "/t.Svc/ClientStream";
const BIDI: &str = "/t.Svc/Bidi";

/// Connect envelope flag marking the end-of-stream message.
const END_STREAM_FLAG: u8 = 0x02;

type Req = H3Request<QuinnBidiStream>;
type DispatchFuture = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

fn msg(text: &str) -> Msg {
    Msg { text: text.into() }
}

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

/// What the handlers saw, shared with the test.
#[derive(Default)]
struct Seen {
    messages: Mutex<Vec<String>>,
    cancel: Mutex<Option<CancelSignal>>,
}

impl Seen {
    fn messages(&self) -> Vec<String> {
        self.messages.lock().unwrap().clone()
    }

    fn cancel_signal(&self) -> Option<CancelSignal> {
        self.cancel.lock().unwrap().clone()
    }
}

fn router(seen: &Arc<Seen>, unary_read_max_bytes: usize) -> Router {
    let mut router = Router::new();

    router.unary(
        UNARY,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move {
            Ok(Response::new(msg(&format!("echo:{}", req.msg.text))))
        },
        HandlerOptions::new().with_read_max_bytes(unary_read_max_bytes),
    );

    // Records every message the moment it is read, then folds them.
    let cs_seen = Arc::clone(seen);
    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        move |ctx: Ctx, mut reqs: RequestStream<Msg>| {
            let seen = Arc::clone(&cs_seen);
            async move {
                *seen.cancel.lock().unwrap() = Some(ctx.cancel_signal().clone());
                let mut parts = Vec::new();
                while let Some(m) = reqs.next().await {
                    let text = m?.text;
                    seen.messages.lock().unwrap().push(text.clone());
                    parts.push(text);
                }
                Ok(Response::new(msg(&parts.join("+"))))
            }
        },
        HandlerOptions::new(),
    );

    router.bidi_stream(
        BIDI,
        codecs(),
        |_ctx: Ctx, reqs: RequestStream<Msg>| async move {
            Ok(reqs.map(|m| m.map(|m| msg(&format!("re:{}", m.text)))))
        },
        HandlerOptions::new(),
    );

    router
}

// ── Wire harness ─────────────────────────────────────────────────────────

/// A live QUIC pair, one client request stream, and the server's
/// `dispatch_h3` future for it — stepped together by hand.
struct Harness {
    sq: QuicDriver,
    cq: QuicDriver,
    // Kept alive: dropping a connection handle closes it.
    _server_h3: H3Connection<QuinnConnection>,
    _client_h3: H3Connection<QuinnConnection>,
    client: Req,
    call: Option<DispatchFuture>,
    result: Option<io::Result<()>>,
}

impl Harness {
    /// Open `path` with `content_type` and start `dispatch_h3` on the server end.
    fn open(router: Router, path: &str, content_type: &str) -> Self {
        let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
        let client_cfg = client_config_trusting_pem(CA_PEM).expect("client config");
        let mut sq = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");
        let addr = sq.local_addr().expect("addr");
        let (mut cq, client_conn) =
            QuicDriver::connect(addr, client_cfg, "localhost").expect("connect");
        let dl = Instant::now() + BUDGET;
        let server_conn = loop {
            if let Some(c) = QuicDriver::take_accepted(&sq) {
                break c;
            }
            pump(&mut sq, &mut cq);
            assert!(Instant::now() < dl, "timed out waiting for QUIC accept");
        };

        let mut server_h3 = H3Connection::new(server_conn);
        let mut client_h3 = H3Connection::new(client_conn);
        wire(&mut sq, &mut cq, || server_h3.poll_setup());
        wire(&mut sq, &mut cq, || client_h3.poll_setup());
        wire(&mut sq, &mut cq, || client_h3.poll_peer_settings());
        wire(&mut sq, &mut cq, || server_h3.poll_peer_settings());

        let mut client: Req = wire(&mut sq, &mut cq, || client_h3.poll_open_request());
        let fields: Vec<(&[u8], &[u8])> = vec![
            (b":method", b"POST"),
            (b":scheme", b"https"),
            (b":authority", b"localhost"),
            (b":path", path.as_bytes()),
            (b"content-type", content_type.as_bytes()),
        ];
        let mut headers = Req::encode_headers(&fields);
        wire(&mut sq, &mut cq, || client.poll_send_headers(&mut headers));

        let inbound = wire(&mut sq, &mut cq, || server_h3.poll_accept());
        let call: DispatchFuture = Box::pin(dispatch_h3(
            Arc::new(router.into_handler()),
            Arc::new(ContextBag::default()),
            Arc::new(ConnectionContext::default()),
            inbound,
        ));

        Self {
            sq,
            cq,
            _server_h3: server_h3,
            _client_h3: client_h3,
            client,
            call: Some(call),
            result: None,
        }
    }

    /// Poll the server call once (if still running), then move datagrams.
    fn step(&mut self) {
        if let Some(call) = self.call.as_mut() {
            let mut cx = Context::from_waker(Waker::noop());
            if let Poll::Ready(result) = call.as_mut().poll(&mut cx) {
                self.result = Some(result);
                self.call = None;
            }
        }
        pump(&mut self.sq, &mut self.cq);
    }

    /// Step until `f` (given the client request) yields a value.
    fn until<T>(&mut self, what: &str, mut f: impl FnMut(&mut Req) -> Option<T>) -> T {
        let dl = Instant::now() + BUDGET;
        loop {
            if let Some(v) = f(&mut self.client) {
                return v;
            }
            self.step();
            assert!(Instant::now() < dl, "timed out: {what}");
        }
    }

    /// Step until the server call completes, returning its result.
    fn finish_call(&mut self) -> io::Result<()> {
        let dl = Instant::now() + BUDGET;
        while self.result.is_none() {
            self.step();
            assert!(
                Instant::now() < dl,
                "timed out waiting for dispatch_h3 to complete"
            );
        }
        self.result.take().expect("checked above")
    }

    fn send_data(&mut self, payload: Bytes) {
        let mut frame = Req::encode_data(payload);
        self.until("client DATA send", |r| ok(r.poll_send_data(&mut frame)));
    }

    fn finish_request(&mut self) {
        self.until("client FIN", |r| ok(r.poll_finish()));
    }

    fn response_status(&mut self) -> String {
        let fields = self.until("response HEADERS", |r| ok(r.poll_headers()));
        let status = fields
            .iter()
            .find(|(n, _)| &n[..] == b":status")
            .map(|(_, v)| String::from_utf8_lossy(v).to_string())
            .expect(":status must be present");
        status
    }

    /// Read response DATA until `buf` holds one whole envelope; return it.
    fn next_envelope(&mut self, buf: &mut Vec<u8>) -> (u8, Vec<u8>) {
        loop {
            if let Some(env) = take_envelope(buf) {
                return env;
            }
            match self.until("response DATA", |r| ok(r.poll_body())) {
                Some(chunk) => buf.extend_from_slice(&chunk),
                None => panic!("response ended mid-envelope; buffered {} bytes", buf.len()),
            }
        }
    }

    fn response_end(&mut self) -> Option<Bytes> {
        self.until("response end", |r| ok(r.poll_body()))
    }
}

fn pump(sq: &mut QuicDriver, cq: &mut QuicDriver) {
    let _ = sq.next_status(VCtx::new());
    let _ = cq.next_status(VCtx::new());
}

/// Drive a connection-level step until it completes, pumping only the wire.
fn wire<T>(
    sq: &mut QuicDriver,
    cq: &mut QuicDriver,
    mut f: impl FnMut() -> VStream<Result<T, H3Error>, ()>,
) -> T {
    let dl = Instant::now() + BUDGET;
    loop {
        if let Some(v) = ok(f()) {
            return v;
        }
        assert!(Instant::now() < dl, "timed out driving HTTP/3 setup");
        pump(sq, cq);
    }
}

/// `Some(v)` once an H3 poll is done; panics on an error.
fn ok<T>(polled: VStream<Result<T, H3Error>, ()>) -> Option<T> {
    match polled {
        VStream::Next(Ok(v)) => Some(v),
        VStream::Next(Err(e)) => panic!("HTTP/3 error: {e}"),
        _ => None,
    }
}

/// A Connect streaming envelope: flags, big-endian length, JSON payload.
fn envelope(m: &Msg) -> Bytes {
    let payload = serde_json::to_vec(m).expect("encode");
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(0);
    out.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    out.extend_from_slice(&payload);
    Bytes::from(out)
}

/// Split one whole envelope off the front of `buf`, if one is there.
fn take_envelope(buf: &mut Vec<u8>) -> Option<(u8, Vec<u8>)> {
    if buf.len() < 5 {
        return None;
    }
    let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
    if buf.len() < 5 + len {
        return None;
    }
    let flags = buf[0];
    let payload = buf[5..5 + len].to_vec();
    buf.drain(..5 + len);
    Some((flags, payload))
}

fn decode(payload: &[u8]) -> Msg {
    serde_json::from_slice(payload).expect("response message is JSON")
}

// ── Tests ────────────────────────────────────────────────────────────────

/// The client sends one message and keeps its request open. The handler must
/// see that message *now* — a server that collects the body first never does,
/// because the body has not ended.
#[test]
fn client_stream_message_reaches_handler_before_request_ends() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 0), CLIENT_STREAM, "application/connect+json");

    h.send_data(envelope(&msg("a")));
    let dl = Instant::now() + BUDGET;
    while seen.messages().is_empty() {
        h.step();
        assert!(
            Instant::now() < dl,
            "handler never saw the first message while the request was still open: \
             the request body is being collected before the handler runs"
        );
    }
    assert_eq!(seen.messages(), vec!["a"]);
    assert!(
        h.result.is_none(),
        "the call cannot finish before the request does"
    );

    h.send_data(envelope(&msg("b")));
    h.send_data(envelope(&msg("c")));
    h.finish_request();
    h.finish_call().expect("dispatch_h3");

    assert_eq!(h.response_status(), "200");
    let mut buf = Vec::new();
    let (flags, payload) = h.next_envelope(&mut buf);
    assert_eq!(flags, 0);
    assert_eq!(decode(&payload), msg("a+b+c"));
    let (flags, _) = h.next_envelope(&mut buf);
    assert_eq!(
        flags & END_STREAM_FLAG,
        END_STREAM_FLAG,
        "end-of-stream envelope"
    );
    assert_eq!(seen.messages(), vec!["a", "b", "c"]);
}

/// Full duplex over h3: read each echo before sending the next message, with
/// the request direction open throughout.
#[test]
fn bidi_interleaves_send_and_receive_over_h3() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 0), BIDI, "application/connect+json");

    let mut buf = Vec::new();
    let mut status = None;
    for text in ["x", "y", "z"] {
        h.send_data(envelope(&msg(text)));
        if status.is_none() {
            status = Some(h.response_status());
        }
        let (flags, payload) = h.next_envelope(&mut buf);
        assert_eq!(flags, 0);
        assert_eq!(decode(&payload), msg(&format!("re:{text}")));
    }
    assert_eq!(status.as_deref(), Some("200"));

    h.finish_request();
    let (flags, _) = h.next_envelope(&mut buf);
    assert_eq!(
        flags & END_STREAM_FLAG,
        END_STREAM_FLAG,
        "end-of-stream envelope"
    );
    assert!(
        h.response_end().is_none(),
        "response ends after end-of-stream"
    );
    h.finish_call().expect("dispatch_h3");
}

/// The body passes `read_max_bytes` and the client never finishes it. The server
/// must answer 413 from what it has, and tell the client to stop sending.
#[test]
fn unary_over_read_max_bytes_gets_413_without_reading_whole_body() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 32), UNARY, "application/json");

    h.send_data(Bytes::from(vec![b'x'; 64]));
    // No FIN: a server that waits for the end of the body never answers.
    h.finish_call().expect("dispatch_h3");
    assert_eq!(h.response_status(), "413");
    assert!(h.response_end().is_none(), "413 carries no body");

    // STOP_SENDING(H3_NO_ERROR): further writes are refused by the peer.
    let dl = Instant::now() + BUDGET;
    let err = loop {
        let mut frame = Req::encode_data(Bytes::from(vec![b'x'; 1024]));
        let res = loop {
            if let VStream::Next(res) = h.client.poll_send_data(&mut frame) {
                break res;
            }
            h.step();
            assert!(Instant::now() < dl, "timed out writing after 413");
        };
        if let Err(e) = res {
            break e;
        }
        h.step();
        assert!(
            Instant::now() < dl,
            "server never sent STOP_SENDING after 413"
        );
    };
    assert!(
        matches!(
            err,
            H3Error::Stream(QuicStreamError::Terminated { code }) if code == error_code::H3_NO_ERROR
        ),
        "expected STOP_SENDING with H3_NO_ERROR, got {err}"
    );
}

/// A unary body within the cap is served as before.
#[test]
fn unary_within_read_max_bytes_still_works() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 1024), UNARY, "application/json");

    h.send_data(Bytes::from(serde_json::to_vec(&msg("hi")).unwrap()));
    h.finish_request();
    h.finish_call().expect("dispatch_h3");

    assert_eq!(h.response_status(), "200");
    let mut body = Vec::new();
    while let Some(chunk) = h.response_end() {
        body.extend_from_slice(&chunk);
    }
    assert_eq!(decode(&body), msg("echo:hi"));
}

/// The whole client stream is on the wire before the server runs at all.
#[test]
fn client_stream_sent_in_full_still_works() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 0), CLIENT_STREAM, "application/connect+json");

    for text in ["a", "b", "c"] {
        h.send_data(envelope(&msg(text)));
    }
    h.finish_request();
    h.finish_call().expect("dispatch_h3");

    assert_eq!(h.response_status(), "200");
    let mut buf = Vec::new();
    let (flags, payload) = h.next_envelope(&mut buf);
    assert_eq!(flags, 0);
    assert_eq!(decode(&payload), msg("a+b+c"));
    let (flags, _) = h.next_envelope(&mut buf);
    assert_eq!(flags & END_STREAM_FLAG, END_STREAM_FLAG);
}

/// The `grpc-status` trailer, once the response body has ended.
fn grpc_status(h: &Harness) -> Option<String> {
    h.client.trailers().and_then(|fields| {
        fields
            .iter()
            .find(|(n, _)| &n[..] == b"grpc-status")
            .map(|(_, v)| String::from_utf8_lossy(v).to_string())
    })
}

/// gRPC over HTTP/3 ends a unary call with a `grpc-status` trailer. The h3
/// server used to drop the response's trailers, so every gRPC client read the
/// call as failed (`Unknown`, "missing grpc-status").
#[test]
fn grpc_unary_over_h3_ends_with_grpc_status() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 0), UNARY, "application/grpc+json");
    h.send_data(envelope(&msg("hi")));
    h.finish_request();
    h.finish_call().expect("dispatch_h3");

    assert_eq!(h.response_status(), "200");
    let mut buf = Vec::new();
    let (_, payload) = h.next_envelope(&mut buf);
    assert_eq!(decode(&payload), msg("echo:hi"));
    assert_eq!(h.response_end(), None, "no more DATA after the message");
    assert_eq!(grpc_status(&h).as_deref(), Some("0"), "unary gRPC must end in grpc-status");
}

/// A streaming gRPC call over HTTP/3 ends with a `grpc-status` trailer, not a
/// bare FIN.
#[test]
fn grpc_stream_over_h3_ends_with_grpc_status() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 0), BIDI, "application/grpc+json");
    h.send_data(envelope(&msg("x")));
    h.finish_request();
    h.finish_call().expect("dispatch_h3");

    assert_eq!(h.response_status(), "200");
    let mut buf = Vec::new();
    let (_, payload) = h.next_envelope(&mut buf);
    assert_eq!(decode(&payload), msg("re:x"));
    assert_eq!(h.response_end(), None, "no more DATA after the message");
    assert_eq!(grpc_status(&h).as_deref(), Some("0"), "streaming gRPC must end in grpc-status");
}

/// The client resets its request mid-stream. That is not a clean end of the
/// body: the call is cancelled, no successful response goes out, and the
/// server's half of the stream is reset.
#[test]
fn peer_reset_mid_client_stream_cancels_the_call() {
    let seen = Arc::new(Seen::default());
    let mut h = Harness::open(router(&seen, 0), CLIENT_STREAM, "application/connect+json");

    h.send_data(envelope(&msg("a")));
    let dl = Instant::now() + BUDGET;
    while seen.messages().is_empty() {
        h.step();
        assert!(Instant::now() < dl, "handler never saw the first message");
    }

    h.client.reset(error_code::H3_REQUEST_CANCELLED);
    let err = h
        .finish_call()
        .expect_err("a reset request must fail the call");
    assert_eq!(err.kind(), io::ErrorKind::ConnectionAborted, "{err}");

    let cancel = seen
        .cancel_signal()
        .expect("handler stored its cancel signal");
    assert!(
        cancel.is_canceled(),
        "the call's Ctx is cancelled on a peer reset"
    );

    // The server reset its response half: the client sees an error, never a
    // successful response folded from a truncated request.
    let dl = Instant::now() + BUDGET;
    let res = loop {
        if let VStream::Next(res) = h.client.poll_headers() {
            break res;
        }
        h.step();
        assert!(
            Instant::now() < dl,
            "timed out waiting for the server reset"
        );
    };
    match res {
        Err(H3Error::Stream(QuicStreamError::Terminated { code })) => {
            assert_eq!(code, error_code::H3_REQUEST_CANCELLED);
        }
        other => panic!("expected the response stream to be reset, got {other:?}"),
    }
}

// ── Production path: the real H3 server on the valtron pool ──────────────

/// Serve the test router with `foundation_http`'s real HTTP/3 server — the
/// QUIC driver and accept tasks on the valtron pool, `dispatch_h3` spawned as a
/// `FutureTask` per request — and return its address and shutdown signal.
fn start_h3_server(seen: &Arc<Seen>) -> (std::net::SocketAddr, Arc<OnSignal>) {
    let rpc: Arc<dyn H3Serve> = Arc::new(ConnectRpcServeH3::new(router(seen, 0).into_handler()));
    let mut app = HttpApp::new_h3_serve();
    app.route_any_h3("/*", rpc);
    let shutdown = Arc::new(OnSignal::new());
    let addr = serve_h3(
        "127.0.0.1:0".parse().unwrap(),
        CERT_PEM,
        KEY_PEM,
        Arc::new(app),
        Arc::clone(&shutdown),
    )
    .expect("start h3 server");
    (addr, shutdown)
}

fn h3_client(addr: std::net::SocketAddr, procedure: &str) -> Client<Msg, Msg> {
    let cfg = client_config_trusting_pem(CA_PEM).expect("client config");
    let transport: Arc<dyn Transport> =
        Arc::new(H3Transport::new(cfg).with_server_name("localhost"));
    Client::new(
        transport,
        &format!("https://{addr}{procedure}"),
        codecs(),
        ClientOptions::new().with_codec("json"),
    )
    .expect("build client")
}

fn call_ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

/// Unary and client-stream through the production server. `dispatch_h3` runs
/// as a valtron `FutureTask`; an H3 poll that is not ready must not park it
/// forever (QUIC streams register no waker), or no call ever completes.
#[valtron_test]
async fn unary_and_client_stream_over_real_h3_server() {
    let seen = Arc::new(Seen::default());
    let (addr, shutdown) = start_h3_server(&seen);

    let resp = h3_client(addr, UNARY)
        .unary(call_ctx(), Request::new(msg("hello")))
        .await
        .expect("unary over h3");
    assert_eq!(resp.msg, msg("echo:hello"));

    let outbound = futures::stream::iter(vec![msg("a"), msg("b"), msg("c")]);
    let resp = h3_client(addr, CLIENT_STREAM)
        .client_stream(call_ctx(), outbound)
        .await
        .expect("client stream over h3");
    assert_eq!(resp.msg, msg("a+b+c"));

    shutdown.turn_on();
}

/// Interleaved bidi through the production server: each echo is read before
/// the next message is sent, with the request direction open throughout.
#[valtron_test]
async fn bidi_interleaves_over_real_h3_server() {
    let seen = Arc::new(Seen::default());
    let (addr, shutdown) = start_h3_server(&seen);

    let mut stream = h3_client(addr, BIDI)
        .bidi_stream(call_ctx(), futures::stream::iter(Vec::<Msg>::new()))
        .await
        .expect("open bidi over h3");
    for text in ["x", "y", "z"] {
        stream.send(&msg(text)).await.expect("send");
        let echo = stream
            .receive()
            .await
            .expect("receive")
            .expect("an echo per message, before the request closes");
        assert_eq!(echo, msg(&format!("re:{text}")));
    }
    stream.close_request().await.expect("close request");
    assert!(stream.receive().await.expect("receive").is_none());

    shutdown.turn_on();
}
