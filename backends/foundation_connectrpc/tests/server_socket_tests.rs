//! Walking-skeleton tests (spec-41 F44 / Decision 11 §Connection ownership) —
//! prove the end-to-end spine over a **real loopback TCP socket**.
//!
//! # Two test styles, two connection owners (Decision 11 §Connection ownership)
//!
//! The connection-owner contract (Decision 11) splits responsibility into two halves:
//!
//! | Half | Owner | What it proves |
//! |---|---|---|
//! | **Server** | Per-connection task spawned by the listener on `accept` | The fd owner
//!   (foundation_http `HttpServer`) reads the request off the wire, hands it to
//!   `ConnectRpcServe::serve()`, which dispatches through the router and writes the
//!   response back. |
//! | **Client** | `Transport::open()` (Decision 11 §Transport) | The connection owner
//!   spawns the byte pump (request → wire, wire → response) on the executor, hands the
//!   caller `send_body`/`recv_body` pipe halves, and returns immediately so the caller
//!   never deadlocks on a bounded pipe. |
//!
//! ## Style A — Raw TCP (server-spine proof)
//!
//! `unary_round_trip_over_real_socket` and `server_stream_over_real_socket` hand-craft
//! HTTP/1.1 requests byte-by-byte over a raw `TcpStream` and hand-parse the raw HTTP
//! response. These tests prove the **server connection owner in isolation** — they
//! validate that `ConnectRpcServe` → dispatch → `Http11::response()` → wire works
//! end-to-end **without depending on the client transport working correctly**. If
//! these fail, the server spine is broken.
//!
//! ## Style B — H1Transport (client-spine proof)
//!
//! `h1_transport_over_real_socket` uses `H1Transport::open()` to push request bytes
//! into `send_body` (a `PipeSender<Bytes>`), await the response head from `response`
//! (a `BoxFuture`), and drain response bytes from `recv_body` (a `PipeReceiver<Bytes>`).
//! This test proves the **client connection owner** works — the valtron task spawned
//! inside `open()` drives the network I/O concurrently with the caller's pushes,
//! satisfying the half-duplex deadlock-avoidance contract.
//!
//! ## Why not merge them?
//!
//! If the only test used `H1Transport` and it failed, you wouldn't know whether the
//! server or the client broke. The raw-TCP tests pin the server; the `H1Transport`
//! test pins the client. Together they bound the failure surface.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use tracing_test::traced_test;

use buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};
use bytes::{Buf, BufMut, Bytes};

use foundation_core::synca::OnSignal;
use foundation_core::valtron::initialize_pool;
use foundation_core::valtron::valtron_test;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;

use foundation_connectrpc::shared::codec::CodecFor;
use foundation_connectrpc::{
    ConnectResult, ConnectRpcServe, HandlerOptions, IdempotencyLevel, JsonCodec, ProcedureCodecs,
    Request, Response, Router,
};

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

const PROCEDURE: &str = "/test.EchoService/Echo";

fn echo_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Read an HTTP/1.1 response off `stream`: `(status_code, body_bytes)`. Reads the
/// header block, then exactly `Content-Length` body bytes (no reliance on EOF).
fn read_response(stream: &mut TcpStream) -> (u16, Vec<u8>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    // Read until the header terminator is present.
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
    let content_length = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().eq_ignore_ascii_case("content-length"))
                .then(|| v.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or_else(|| {
            panic!(
                "no content-length; raw head:\n{head}\n---body-so-far---\n{}",
                String::from_utf8_lossy(&buf[header_end..])
            )
        });

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).expect("read body");
        assert!(n > 0, "connection closed before body completed");
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);
    (status, body)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

// ── Style A: Server-spine proof (raw TCP) ────────────────────────────────────
//
// These tests hand-craft HTTP/1.1 over a raw TcpStream — they validate the server
// connection owner in isolation. No client transport is involved; a raw-TCP client
// is the simplest possible peer, so any failure here is a server bug.

/// Prove the **server** connection owner for a unary RPC:
///
/// 1. `HttpServer` (the fd owner) accepts a connection and hands it to
///    `ConnectRpcServe::serve()`.
/// 2. `serve()` calls `ConnectRpcHandler::dispatch()`, which routes to the
///    unary handler, runs it to completion via `block_on`, and gets back a
///    `SimpleOutgoingResponse`.
/// 3. `serve()` frames the response as HTTP/1.1 (`Http11::response()`) and writes
///    it to the `SharedByteBufferStream<RawStream>` — the same `RawStream` the fd
///    owner holds.
///
/// The client side is a **raw TCP stream** — we hand-write the HTTP request and
/// hand-parse the HTTP response. This is deliberately not using `H1Transport`
/// because we want to prove the server works independent of the client transport.
#[test]
fn unary_round_trip_over_real_socket() {
    let _guard = initialize_pool(41, Some(4));

    // Server: mount the ConnectRpc adapter as the connection owner's handler.
    let mut app = HttpApp::new_serve();
    let serve = echo_serve();
    app.router.add_route_any(PROCEDURE, &serve);

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

    // Client: raw HTTP/1.1 POST over the loopback socket.
    let msg = TestMsg {
        id: 99,
        name: "socket".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal").to_vec();
    let request = format!(
        "POST {PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        json.len()
    );

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(request.as_bytes()).expect("write head");
    stream.write_all(&json).expect("write body");
    stream.flush().unwrap();

    let (status, body) = read_response(&mut stream);
    assert_eq!(status, 200, "unary over socket returns 200");
    let echoed: TestMsg = JsonCodec.unmarshal(Bytes::from(body)).expect("decode echo");
    assert_eq!(echoed, msg, "server echoed the request over the wire");

    shutdown.turn_on();
}

const LIST_PROCEDURE: &str = "/test.ListService/Items";

fn list_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.server_stream(
        LIST_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move {
            let base = req.msg.id;
            let items: Vec<ConnectResult<TestMsg>> = (0..3)
                .map(|i| {
                    Ok(TestMsg {
                        id: base + i,
                        name: format!("item-{i}"),
                    })
                })
                .collect();
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Prove the **server** connection owner for a server-streaming RPC:
///
/// Same raw-TCP pattern as the unary test, but exercises the streaming dispatch
/// path. The handler returns 3 items via `futures::stream::iter`; `dispatch()`
/// runs the streaming join (Decision 08) to completion, frames the output as a
/// Connect-protocol envelope stream, and writes it to the wire.
///
/// The client parses the Connect envelope framing (5-byte header: 1 flag + 4 byte
/// length, per the Connect streaming protocol) and asserts all 3 items arrived
/// followed by a terminating EndStream frame.
#[test]
fn server_stream_over_real_socket() {
    let _guard = initialize_pool(42, Some(4));

    let mut app = HttpApp::new_serve();
    let serve = list_serve();
    app.router.add_route_any(LIST_PROCEDURE, &serve);

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

    // Build a Connect streaming request: POST with application/connect+json + enveloped body.
    let msg = TestMsg {
        id: 10,
        name: "seed".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal").to_vec();
    let envelope = {
        let mut buf = Vec::new();
        // Flags=0, length as 4-be bytes
        buf.push(0u8);
        buf.extend_from_slice(&(json.len() as u32).to_be_bytes());
        buf.extend_from_slice(&json);
        buf
    };

    let request = format!(
        "POST {LIST_PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: application/connect+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        envelope.len()
    );

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(request.as_bytes()).expect("write head");
    stream.write_all(&envelope).expect("write body");
    stream.flush().unwrap();

    let (status, body) = read_response(&mut stream);
    assert_eq!(status, 200, "server-stream over socket returns 200");

    // Parse the Connect streaming response.
    let (messages, saw_end_stream) = parse_connect_envelope_stream(&body);
    assert_eq!(messages.len(), 3, "three server-stream items");
    assert!(saw_end_stream, "EndStream frame present");
    for (i, msg_bytes) in messages.iter().enumerate() {
        let item: TestMsg = JsonCodec
            .unmarshal(Bytes::from(msg_bytes.clone()))
            .expect("decode item");
        assert_eq!(item.id, 10 + i as i32, "item {i} id");
        assert_eq!(item.name, format!("item-{i}"), "item {i} name");
    }

    shutdown.turn_on();
}

/// Parse a Connect-protocol envelope stream into `(messages, saw_end_stream)`.
const FLAG_END_STREAM: u8 = 0x02;
fn parse_connect_envelope_stream(body: &[u8]) -> (Vec<Vec<u8>>, bool) {
    let mut messages = Vec::new();
    let mut end = false;
    let mut pos = 0;
    while pos + 5 <= body.len() {
        let flags = body[pos];
        let len = u32::from_be_bytes([body[pos + 1], body[pos + 2], body[pos + 3], body[pos + 4]])
            as usize;
        let start = pos + 5;
        let stop = (start + len).min(body.len());
        let payload = body[start..stop].to_vec();
        if flags & FLAG_END_STREAM != 0 {
            end = true;
        } else {
            messages.push(payload);
        }
        pos = stop;
    }
    (messages, end)
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

// ── H1Transport compile-time contract proof ──────────────────────────────────

/// Prove `H1Transport` implements `Transport` with the correct capabilities at
/// compile time.
#[test]
fn h1_transport_capabilities_are_correct() {
    use foundation_connectrpc::shared::transport::Transport;
    use foundation_connectrpc::H1Transport;
    use foundation_netio::http::NativeHttpClient;
    use foundation_netio::shared::http::Proto;

    let client = NativeHttpClient::from_system();
    let transport = H1Transport::new(Arc::new(client));
    let caps = transport.capabilities();

    assert!(caps.request_streaming, "h1 supports chunked upload");
    assert!(!caps.full_duplex, "h1 is half-duplex");
    assert!(!caps.h2_trailers);
    assert_eq!(caps.http_versions, &[Proto::HTTP11]);
    assert!(!caps.multiplexed);
}

// ── Style B: Client-spine proof (H1Transport) ───────────────────────────────
//
// This test uses `H1Transport::open()` over a real loopback socket — the client
// pushes request body bytes into `send_body`, the valtron task inside `open()`
// spawns the network I/O, and the caller reads response bytes from `recv_body`.
// Proves the client connection-owner end-to-end (not a compile-time assertion).

/// Prove the **client** connection owner over a real loopback TCP socket:
///
/// 1. Server: `ConnectRpcServe` handles an echo unary RPC.
/// 2. Client: `H1Transport::open()` spawns the pump on the valtron pool and
///    returns `TransportStream` synchronously — `send_body`, `head`, `recv_body`.
/// 3. The caller pushes an enveloped Connect unary request into `send_body`,
///    polls `head` for the response status, and drains `recv_body`.
///
#[valtron_test(seed = 44, threads = 8)]
#[traced_test]
async fn h1_client_transport_over_real_socket() {
    use foundation_connectrpc::shared::transport::Transport;
    use foundation_connectrpc::H1Transport;
    use foundation_netio::http::NativeHttpClient;
    use foundation_netio::shared::http::{
        Proto, RequestDescriptor, SimpleHeaders, SimpleMethod, SimpleUrl,
    };
    use foundation_core::url::Uri;

    // Server.
    let mut app = HttpApp::new_serve();
    let serve = echo_serve();
    app.router.add_route_any(PROCEDURE, &serve);

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

    // Wait for server.
    let _ = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).expect("server ready");

    // Client: H1Transport. `open()` is synchronous — spawns the pump on the pool
    // and returns the three caller-facing pipe halves immediately.
    let transport = H1Transport::new(Arc::new(NativeHttpClient::from_system()));

    let url = format!("http://127.0.0.1:{}{PROCEDURE}", addr.port());
    let uri = Uri::parse(&url).expect("parse URL");

    let descriptor = RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url.clone()),
        request_uri: uri,
        headers: {
            // Connect *unary*: bare message body, `application/json` content-type
            // (the enveloped `application/connect+json` form is for streaming).
            let mut h = SimpleHeaders::new();
            h.insert(
                foundation_netio::shared::http::SimpleHeader::CONTENT_TYPE,
                vec!["application/json".to_string()],
            );
            h
        },
        method: SimpleMethod::POST,
    };

    let stream = transport.open(descriptor).expect("open");
    let mut head = stream.head;
    let mut recv_body = stream.recv_body;

    // Push the bare unary request message into send_body (no envelope).
    let msg = TestMsg {
        id: 77,
        name: "transport".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal");
    stream
        .send_body
        .try_send(Bytes::from(json.to_vec()))
        .expect("send request body");
    // Unary: one message then EOF. Close the send pipe immediately so the
    // chunked request-body renderer emits its terminating `0\r\n\r\n` and the
    // server finishes reading the request. Without this the pushable pipe stays
    // open-but-empty (`Data::Retry` forever) and the upload never completes.
    // (Dropping the sender would also close it, but `stream.head`/`recv_body`
    // were partially moved out, so `stream.send_body` lives to end of scope —
    // hence the explicit close here.)
    stream.send_body.close();

    // F45 Part D: head and recv_body are both futures Streams; RPC takes the one head.
    let (status, _headers) = head
        .next()
        .await
        .expect("response head chunk")
        .expect("response head ok");
    assert_eq!(status, foundation_netio::shared::http::Status::OK);

    let body_bytes = recv_body
        .next()
        .await
        .expect("response body chunk")
        .expect("response body ok");

    // Decode the echoed message.
    let echoed: TestMsg = JsonCodec.unmarshal(body_bytes).expect("decode echo");
    assert_eq!(
        echoed, msg,
        "H1Transport: echoed message matches request over real socket"
    );

    shutdown.turn_on();
}

/// Prove the **client** connection owner for a *server-streaming* RPC over a real
/// loopback socket — the one acceptance phrase the unary client test does not
/// cover.
///
/// 1. Server: `ConnectRpcServe` runs a `server_stream` handler emitting 3 items.
/// 2. Client: `H1Transport::open()` sends a Connect streaming request
///    (`application/connect+json`, one enveloped seed message) and returns
///    `TransportStream` synchronously.
/// 3. The caller drains `recv_body` to end of stream, reassembles the Connect
///    envelope frames, and asserts all 3 items plus the terminating EndStream
///    frame arrived — proving the H1 client consumes a multi-frame server stream,
///    not just a single unary body.
#[valtron_test(seed = 45, threads = 8)]
#[traced_test]
async fn h1_client_server_stream_over_real_socket() {
    use foundation_connectrpc::shared::transport::Transport;
    use foundation_connectrpc::H1Transport;
    use foundation_core::url::Uri;
    use foundation_netio::http::NativeHttpClient;
    use foundation_netio::shared::http::{
        Proto, RequestDescriptor, SimpleHeaders, SimpleMethod, SimpleUrl,
    };

    // Server: the same server-streaming handler the raw-TCP test pins.
    let mut app = HttpApp::new_serve();
    let serve = list_serve();
    app.router.add_route_any(LIST_PROCEDURE, &serve);

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

    // Wait for the server to be accepting.
    let _ = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).expect("server ready");

    // Client: H1Transport, Connect streaming content-type.
    let transport = H1Transport::new(Arc::new(NativeHttpClient::from_system()));
    let url = format!("http://127.0.0.1:{}{LIST_PROCEDURE}", addr.port());
    let uri = Uri::parse(&url).expect("parse URL");
    let descriptor = RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url.clone()),
        request_uri: uri,
        headers: {
            let mut h = SimpleHeaders::new();
            h.insert(
                foundation_netio::shared::http::SimpleHeader::CONTENT_TYPE,
                vec!["application/connect+json".to_string()],
            );
            h
        },
        method: SimpleMethod::POST,
    };

    let stream = transport.open(descriptor).expect("open");
    let mut head = stream.head;
    let mut recv_body = stream.recv_body;

    // Push one enveloped seed message (flags=0, 4-byte length prefix), then EOF.
    let seed = TestMsg {
        id: 10,
        name: "seed".to_string(),
    };
    let json = JsonCodec.marshal(&seed).expect("marshal").to_vec();
    let mut envelope = Vec::with_capacity(5 + json.len());
    envelope.push(0u8);
    envelope.extend_from_slice(&(json.len() as u32).to_be_bytes());
    envelope.extend_from_slice(&json);
    stream
        .send_body
        .try_send(Bytes::from(envelope))
        .expect("send request envelope");
    stream.send_body.close();

    // Head first.
    let (status, _headers) = head
        .next()
        .await
        .expect("response head chunk")
        .expect("response head ok");
    assert_eq!(status, foundation_netio::shared::http::Status::OK);

    // Drain the whole streamed body (many BodyChunk frames), reassemble, parse.
    let mut body = Vec::new();
    while let Some(chunk) = recv_body.next().await {
        let bytes = chunk.expect("body chunk ok");
        body.extend_from_slice(&bytes);
    }

    let (messages, saw_end_stream) = parse_connect_envelope_stream(&body);
    assert_eq!(
        messages.len(),
        3,
        "H1 client received all 3 server-stream items"
    );
    assert!(saw_end_stream, "H1 client saw the EndStream frame");
    for (i, msg_bytes) in messages.iter().enumerate() {
        let item: TestMsg = JsonCodec
            .unmarshal(Bytes::from(msg_bytes.clone()))
            .expect("decode item");
        assert_eq!(item.id, 10 + i as i32, "item {i} id over H1 client");
        assert_eq!(item.name, format!("item-{i}"), "item {i} name over H1 client");
    }

    shutdown.turn_on();
}
