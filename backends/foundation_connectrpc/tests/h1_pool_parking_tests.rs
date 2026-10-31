//! Feature 08 — long-lived HTTP/1.1 RPCs do not hold a valtron worker.
//!
//! WHY: before Feature 08, `ConnectRpcServe::serve` ran the whole call with
//! `io.block_on(select(dispatch, hangup))`. A server-streaming RPC that waited
//! between frames held its worker for the life of the stream. Enough open
//! streams used up a small pool, and every other request queued behind them.
//!
//! WHAT: this test pins the fix. A server with a 2-worker pool serves 8
//! server-streaming RPCs at once, each pausing 1 s between frames. While all 8
//! are open, a unary call on a ninth connection must still complete in under
//! 100 ms. On the old code the unary call waits until a stream ends (or times
//! out), because the two workers are blocked for the life of two of the streams.
//!
//! HOW: raw `TcpStream` clients, like the server-spine proofs in
//! `server_socket_tests.rs`. Each streaming client opens a chunked Connect
//! request and reads the 200 head, which proves its handler is running and
//! paused between frames. The unary client then times a full round trip.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};
use bytes::{Buf, BufMut, Bytes};

use foundation_http::ShutdownSignal;
use foundation_core::valtron::{initialize_pool, timer};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;

use foundation_connectrpc::shared::codec::CodecFor;
use foundation_connectrpc::{
    ConnectRpcServe, HandlerOptions, IdempotencyLevel, JsonCodec, ProcedureCodecs, Request,
    Response, Router,
};

const STREAM_PROCEDURE: &str = "/test.PoolService/Stream";
const PING_PROCEDURE: &str = "/test.PoolService/Ping";

/// How many server-streaming RPCs stay open at once.
const OPEN_STREAMS: usize = 8;
/// Longest the unary call may take while the streams are open.
const UNARY_BUDGET: Duration = Duration::from_millis(100);

// ── test message (id @1, name @2) ─────────────────────────────────────────────

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct TestMsg {
    id: i32,
    name: String,
}

impl DefaultInstance for TestMsg {
    fn default_instance() -> &'static Self {
        static INST: std::sync::OnceLock<TestMsg> = std::sync::OnceLock::new();
        INST.get_or_init(TestMsg::default)
    }
}

impl Message for TestMsg {
    fn compute_size(&self, _c: &mut SizeCache) -> u32 {
        0
    }
    fn write_to(&self, _c: &mut SizeCache, buf: &mut impl BufMut) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.id as u64, buf);
        Tag::new(2, WireType::LengthDelimited).encode(buf);
        encode_varint(self.name.len() as u64, buf);
        buf.put_slice(self.name.as_bytes());
    }
    fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        _c: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        match tag.field_number() {
            1 => {
                self.id = decode_varint(buf)? as i32;
                Ok(())
            }
            2 => {
                let len = decode_varint(buf)? as usize;
                let mut b = vec![0u8; len];
                buf.copy_to_slice(&mut b);
                self.name = String::from_utf8(b).map_err(|_| DecodeError::InvalidUtf8)?;
                Ok(())
            }
            _ => skip_field(tag, buf),
        }
    }
    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A router with one server-streaming procedure (a frame, then 1 s between each
/// of the next four) and one unary ping that echoes immediately.
fn pool_router() -> Router {
    let mut router = Router::new();

    router.server_stream(
        STREAM_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move {
            let base = req.msg.id;
            Ok(futures::stream::unfold(0usize, move |i| async move {
                if i == 5 {
                    return None;
                }
                if i > 0 {
                    // Park between frames, without blocking a thread, so a
                    // stream that only held its worker would keep it busy here.
                    timer::sleep_for(Duration::from_secs(1)).await;
                }
                Some((
                    Ok(TestMsg {
                        id: base + i as i32,
                        name: format!("msg-{i}"),
                    }),
                    i + 1,
                ))
            }))
        },
        HandlerOptions::new(),
    );

    router.unary(
        PING_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );

    router
}

fn start_server(serve: Arc<dyn Serve>) -> (std::net::SocketAddr, Arc<ShutdownSignal>) {
    let mut app = HttpApp::new_serve();
    app.router.add_route_any(STREAM_PROCEDURE, &serve);
    app.router.add_route_any(PING_PROCEDURE, &serve);

    let shutdown = Arc::new(ShutdownSignal::new());
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

fn connect_with_retry(addr: std::net::SocketAddr) -> TcpStream {
    for _ in 0..50 {
        if let Ok(s) = TcpStream::connect(addr) {
            return s;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("could not connect to test server at {addr}");
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read the status line and headers: `(status, head_text, bytes past the head)`.
fn read_head(stream: &mut TcpStream) -> (u16, String, Vec<u8>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let header_end = loop {
        if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut tmp).expect("read headers");
        assert!(n > 0, "connection closed before headers completed");
        buf.extend_from_slice(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .expect("status line");
    (status, head, buf[header_end..].to_vec())
}

fn is_chunked(head: &str) -> bool {
    head.lines().any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
        })
    })
}

/// One Connect envelope: flags byte, 4-byte big-endian length, payload.
fn json_envelope(msg: &TestMsg) -> Vec<u8> {
    let json = JsonCodec.marshal(msg).expect("marshal").to_vec();
    let mut buf = Vec::with_capacity(5 + json.len());
    buf.push(0u8);
    buf.extend_from_slice(&(json.len() as u32).to_be_bytes());
    buf.extend_from_slice(&json);
    buf
}

/// Send a Connect streaming POST whose body is `body`, as one chunk per entry.
fn send_chunked_request(stream: &mut TcpStream, procedure: &str, body: &[Vec<u8>]) {
    let head = format!(
        "POST {procedure} HTTP/1.1\r\nHost: t\r\nContent-Type: application/connect+json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).expect("write head");
    for piece in body {
        write!(stream, "{:x}\r\n", piece.len()).expect("chunk size");
        stream.write_all(piece).expect("chunk data");
        stream.write_all(b"\r\n").expect("chunk end");
    }
    stream.write_all(b"0\r\n\r\n").expect("last chunk");
    stream.flush().expect("flush");
}

/// Open one streaming RPC and read its 200 head; the handler is now running and
/// paused between frames. Returns the live stream and bytes read past the head.
fn open_stream(addr: std::net::SocketAddr, id: i32) -> (TcpStream, Vec<u8>) {
    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let seed = TestMsg {
        id,
        name: "seed".to_string(),
    };
    send_chunked_request(&mut stream, STREAM_PROCEDURE, &[json_envelope(&seed)]);
    let (status, head, rest) = read_head(&mut stream);
    assert_eq!(status, 200, "stream {id} opens with 200; head:\n{head}");
    assert!(
        is_chunked(&head),
        "stream {id} is sent chunked; head:\n{head}"
    );
    (stream, rest)
}

/// Send a unary ping and read its whole `Content-Length` body, timing the trip.
fn ping(addr: std::net::SocketAddr) -> (Duration, TestMsg) {
    let msg = TestMsg {
        id: 777,
        name: "ping".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal").to_vec();
    let request = format!(
        "POST {PING_PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        json.len()
    );

    let t0 = Instant::now();
    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout");
    stream.write_all(request.as_bytes()).expect("write head");
    stream.write_all(&json).expect("write body");
    stream.flush().expect("flush");

    let (_status, head, mut rest) = read_head(&mut stream);
    let content_length = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().eq_ignore_ascii_case("content-length"))
                .then(|| v.trim().parse::<usize>().ok())
                .flatten()
        })
        .expect("content-length present");
    let mut tmp = [0u8; 1024];
    while rest.len() < content_length {
        let n = stream.read(&mut tmp).expect("read body");
        assert!(n > 0, "connection closed before body completed");
        rest.extend_from_slice(&tmp[..n]);
    }
    rest.truncate(content_length);
    let elapsed = t0.elapsed();
    let echoed: TestMsg = JsonCodec.unmarshal(Bytes::from(rest)).expect("decode echo");
    (elapsed, echoed)
}

/// While 8 server-streaming RPCs sit open on a 2-worker pool, a unary call
/// still completes immediately — the streams no longer hold a worker each.
#[test]
#[foundation_macros::timeout(60000)]
fn h1_streaming_calls_do_not_exhaust_the_pool() {
    // Two workers is the whole point: eight open streams would queue up the
    // ninth call on the old blocking code.
    let _guard = initialize_pool(71, Some(2));

    let serve = Arc::new(ConnectRpcServe::new(pool_router().into_handler()));
    let (addr, shutdown) = start_server(serve);

    // Open all eight streams; each read of the 200 head proves its handler is
    // running (and now paused between frames) before the unary call starts.
    let mut open = Vec::with_capacity(OPEN_STREAMS);
    for id in 0..OPEN_STREAMS as i32 {
        open.push(open_stream(addr, id));
    }
    // Give the last stream a moment to settle into its pause.
    std::thread::sleep(Duration::from_millis(20));

    let (elapsed, echoed) = ping(addr);
    assert_eq!(echoed.id, 777, "unary echoed the request");
    assert_eq!(echoed.name, "ping", "unary echoed the request name");
    assert!(
        elapsed < UNARY_BUDGET,
        "unary call must finish while {OPEN_STREAMS} streams are open, \
         took {elapsed:?} (budget {UNARY_BUDGET:?})"
    );

    // The streams are still alive; drop them before stopping the server.
    drop(open);
    shutdown.turn_on();
}
