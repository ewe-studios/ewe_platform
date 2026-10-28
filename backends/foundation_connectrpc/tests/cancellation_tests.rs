//! Call cancellation and deadlines on the server side.
//!
//! WHY: a handler waiting on `ctx.cancelled()` must stop when the call is over
//! for its peer: an h2 RST_STREAM, a stream the connection drops, an HTTP/1.1
//! client that closes its connection, or the call's deadline passing. Nothing
//! fired the `Ctx` cancel signal before, and an h2 reset read like a clean end
//! of the request body.
//!
//! HOW: each handler parks its `Ctx` in a [`Slot`] and awaits `cancelled()`.
//! The h2 tests drive `ConnectRpcHandler::dispatch_h2` directly over its frame
//! pipes, standing in for the h2 connection; the HTTP/1.1 tests use a real
//! `HttpServer` over a loopback socket.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::{valtron_test, Pipe, PipeReceiver, PipeSender};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::Serve;
use foundation_netio::http2::frame::ErrorCode;
use foundation_netio::http2::types::{header_from_hpack, H2Frame, H2IncomingFrame};
use foundation_netio::netcap::ConnectionContext;

use foundation_connectrpc::{
    ConnectError, ConnectRpcHandler, ConnectRpcServe, Ctx, HandlerOptions, JsonCodec,
    ProcedureCodecs, Request, RequestStream, Response, Router,
};

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

const UNARY: &str = "/t.Svc/Unary";
const CLIENT_STREAM: &str = "/t.Svc/ClientStream";
const SERVER_STREAM: &str = "/t.Svc/ServerStream";

/// Where a handler leaves its `Ctx` for the test to watch.
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

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

/// Every procedure parks its `Ctx` in `slot` and waits to be cancelled.
fn router(slot: &Slot) -> Router {
    let mut router = Router::new();
    let s = slot.clone();
    router.unary(
        UNARY,
        codecs(),
        move |ctx: Ctx, _req: Request<Msg>| {
            let s = s.clone();
            async move {
                s.put(&ctx);
                ctx.cancelled().await;
                Err::<Response<Msg>, _>(ConnectError::canceled("cancelled").into())
            }
        },
        HandlerOptions::new(),
    );
    let s = slot.clone();
    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        move |ctx: Ctx, _reqs: RequestStream<Msg>| {
            let s = s.clone();
            async move {
                s.put(&ctx);
                ctx.cancelled().await;
                Err::<Response<Msg>, _>(ConnectError::canceled("cancelled").into())
            }
        },
        HandlerOptions::new(),
    );
    let s = slot.clone();
    router.server_stream(
        SERVER_STREAM,
        codecs(),
        move |ctx: Ctx, _req: Request<Msg>| {
            let s = s.clone();
            async move {
                s.put(&ctx);
                ctx.cancelled().await;
                Err::<futures::stream::Empty<_>, _>(ConnectError::canceled("cancelled").into())
            }
        },
        HandlerOptions::new(),
    );
    router
}

/// One Connect envelope around a JSON `Msg`.
fn json_envelope(text: &str) -> Bytes {
    let payload = serde_json::to_vec(&Msg {
        text: text.to_string(),
    })
    .unwrap();
    let mut buf = vec![0u8];
    buf.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    buf.extend_from_slice(&payload);
    Bytes::from(buf)
}

// ── HTTP/2 ──────────────────────────────────────────────────────────────────

/// An h2 stream as the connection hands it to `dispatch_h2`.
struct H2Stream {
    body: PipeSender<H2IncomingFrame>,
    response: PipeReceiver<H2Frame>,
    done: std::sync::mpsc::Receiver<std::io::Result<()>>,
}

fn open_h2(handler: Arc<ConnectRpcHandler>, path: &str, extra: &[(&str, &str)]) -> H2Stream {
    let mut pairs: Vec<(Bytes, Bytes)> = vec![
        (Bytes::from_static(b":method"), Bytes::from_static(b"POST")),
        (Bytes::from_static(b":scheme"), Bytes::from_static(b"http")),
        (Bytes::from_static(b":authority"), Bytes::from_static(b"t")),
        (Bytes::from_static(b":path"), Bytes::from(path.to_string())),
    ];
    for (name, value) in extra {
        pairs.push((
            Bytes::from(name.to_string()),
            Bytes::from(value.to_string()),
        ));
    }
    let header = header_from_hpack(&pairs, Arc::new(ConnectionContext::default())).unwrap();
    let (body_tx, body_rx) = Pipe::<H2IncomingFrame>::with_depth(8);
    let (resp_tx, resp_rx) = Pipe::<H2Frame>::with_depth(8);
    let (done_tx, done) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let fut = handler.dispatch_h2(Arc::new(ContextBag::new()), header, body_rx, resp_tx);
        let _ = done_tx.send(futures::executor::block_on(fut));
    });
    H2Stream {
        body: body_tx,
        response: resp_rx,
        done,
    }
}

const STREAMING_JSON: (&str, &str) = ("content-type", "application/connect+json");

/// A peer RST_STREAM mid-request cancels the handler's `Ctx` instead of reading
/// as a clean end of the request stream.
#[test]
fn h2_reset_cancels_the_handlers_ctx() {
    let slot = Slot::default();
    let handler = Arc::new(router(&slot).into_handler());
    let stream = open_h2(handler, CLIENT_STREAM, &[STREAMING_JSON]);

    stream
        .body
        .try_send(H2IncomingFrame::Data(json_envelope("a")))
        .unwrap();
    let ctx = slot.wait_entered();
    assert!(!ctx.is_canceled());

    stream
        .body
        .try_send(H2IncomingFrame::Reset(ErrorCode::Cancel))
        .unwrap();
    stream.body.close();
    assert_cancelled_within(&ctx, Duration::from_secs(5));
    let outcome = stream
        .done
        .recv_timeout(Duration::from_secs(5))
        .expect("dispatch ends once the stream is reset");
    assert!(
        outcome.is_ok(),
        "a reset stream has nothing to answer: {outcome:?}"
    );
    assert!(
        stream.response.try_recv().is_err(),
        "nothing is sent on a reset stream"
    );
}

/// A streaming handler that awaits its own cancellation sees it complete: the
/// call's cancel fires before the handler future is dropped, and the handler
/// is polled once more. The cancel used to fire only after the handler was
/// dropped, so code after `ctx.cancelled().await` never ran on a peer reset.
#[test]
fn h2_reset_lets_a_streaming_handler_observe_its_cancellation() {
    let observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let entered = Slot::default();
    let mut router = Router::new();
    let (seen, slot) = (Arc::clone(&observed), entered.clone());
    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        move |ctx: Ctx, _reqs: RequestStream<Msg>| {
            let (seen, slot) = (Arc::clone(&seen), slot.clone());
            async move {
                slot.put(&ctx);
                ctx.cancelled().await;
                seen.store(true, std::sync::atomic::Ordering::SeqCst);
                Err::<Response<Msg>, _>(ConnectError::canceled("cancelled").into())
            }
        },
        HandlerOptions::new(),
    );
    let stream = open_h2(Arc::new(router.into_handler()), CLIENT_STREAM, &[STREAMING_JSON]);

    stream
        .body
        .try_send(H2IncomingFrame::Data(json_envelope("a")))
        .unwrap();
    entered.wait_entered();
    stream
        .body
        .try_send(H2IncomingFrame::Reset(ErrorCode::Cancel))
        .unwrap();
    stream.body.close();
    stream
        .done
        .recv_timeout(Duration::from_secs(5))
        .expect("dispatch ends once the stream is reset")
        .expect("a reset stream has nothing to answer");
    assert!(
        observed.load(std::sync::atomic::Ordering::SeqCst),
        "the handler's `ctx.cancelled().await` never completed"
    );
}

/// A stream the connection drops after the request ended (its response pipe
/// closes under the handler — a reset after `END_STREAM`, or the connection
/// going away) cancels the call.
#[test]
fn h2_dropped_stream_after_request_end_cancels_the_call() {
    let slot = Slot::default();
    let handler = Arc::new(router(&slot).into_handler());
    let stream = open_h2(handler, SERVER_STREAM, &[STREAMING_JSON]);

    stream
        .body
        .try_send(H2IncomingFrame::Data(json_envelope("a")))
        .unwrap();
    stream.body.close(); // END_STREAM
    let ctx = slot.wait_entered();
    assert!(!ctx.is_canceled());

    drop(stream.response);
    assert_cancelled_within(&ctx, Duration::from_secs(5));
    // Ok, or the failed write to the gone stream: either way it ends.
    let _ended = stream
        .done
        .recv_timeout(Duration::from_secs(5))
        .expect("dispatch ends once the stream is dropped");
}

/// The same for unary: the handler runs after the body is read, so only the
/// response pipe can tell it the stream is gone.
#[test]
fn h2_dropped_unary_stream_cancels_the_call() {
    let slot = Slot::default();
    let handler = Arc::new(router(&slot).into_handler());
    let stream = open_h2(handler, UNARY, &[("content-type", "application/json")]);

    stream
        .body
        .try_send(H2IncomingFrame::Data(Bytes::from_static(
            b"{\"text\":\"a\"}",
        )))
        .unwrap();
    stream.body.close();
    let ctx = slot.wait_entered();

    drop(stream.response);
    assert_cancelled_within(&ctx, Duration::from_secs(5));
}

/// A `connect-timeout-ms` deadline is enforced: the handler's `Ctx` is
/// cancelled when it passes and the call is answered `deadline_exceeded`.
#[test]
fn h2_deadline_cancels_and_answers_deadline_exceeded() {
    let slot = Slot::default();
    let handler = Arc::new(router(&slot).into_handler());
    let stream = open_h2(
        handler,
        UNARY,
        &[
            ("content-type", "application/json"),
            ("connect-timeout-ms", "200"),
        ],
    );
    stream
        .body
        .try_send(H2IncomingFrame::Data(Bytes::from_static(
            b"{\"text\":\"a\"}",
        )))
        .unwrap();
    stream.body.close();

    let ctx = slot.wait_entered();
    assert_cancelled_within(&ctx, Duration::from_secs(5));
    stream
        .done
        .recv_timeout(Duration::from_secs(5))
        .expect("dispatch ends at the deadline")
        .expect("the timeout is answered");

    let mut status = None;
    let mut body = Vec::new();
    while let Ok(frame) = stream.response.try_recv() {
        match frame {
            H2Frame::Headers { status: s, .. } => status = Some(s),
            H2Frame::Data { payload, .. } => body.extend_from_slice(&payload),
            _ => {}
        }
    }
    assert_eq!(status, Some(504));
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("deadline_exceeded"), "body: {body}");
}

// ── HTTP/1.1 ────────────────────────────────────────────────────────────────

fn start_h1_server(slot: &Slot) -> (std::net::SocketAddr, Arc<OnSignal>) {
    let serve: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(router(slot).into_handler()));
    let mut app = HttpApp::new_serve();
    app.router.add_route_any(UNARY, &serve);
    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = HttpServer::with_config(
        app,
        &format!("127.0.0.1:{}", addr.port()),
        ServerConfig::defaults(),
    );
    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &shutdown_thread));
    (addr, shutdown)
}

fn send_unary(addr: std::net::SocketAddr, extra_headers: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let body = b"{\"text\":\"a\"}";
    let request = format!(
        "POST {UNARY} HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n{extra_headers}\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream
}

/// An HTTP/1.1 client that closes its connection mid-call cancels the call.
#[valtron_test]
fn h1_client_disconnect_cancels_the_call() {
    let slot = Slot::default();
    let (addr, shutdown) = start_h1_server(&slot);

    let stream = send_unary(addr, "");
    let ctx = slot.wait_entered();
    assert!(!ctx.is_canceled());

    drop(stream);
    assert_cancelled_within(&ctx, Duration::from_secs(5));
    shutdown.turn_on();
}

/// An HTTP/1.1 call's deadline cancels it and is answered `deadline_exceeded`.
#[valtron_test]
fn h1_deadline_cancels_and_answers_deadline_exceeded() {
    let slot = Slot::default();
    let (addr, shutdown) = start_h1_server(&slot);

    let mut stream = send_unary(addr, "Connect-Timeout-Ms: 200\r\n");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let ctx = slot.wait_entered();

    let mut response = Vec::new();
    let mut buf = [0u8; 1024];
    while !String::from_utf8_lossy(&response).contains("deadline_exceeded") {
        let n = stream.read(&mut buf).expect("read the timeout response");
        assert!(
            n > 0,
            "connection closed: {}",
            String::from_utf8_lossy(&response)
        );
        response.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&response);
    assert!(text.starts_with("HTTP/1.1 504"), "response: {text}");
    assert!(ctx.is_canceled());
    shutdown.turn_on();
}
