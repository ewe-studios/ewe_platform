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
/// header block, then the body: exactly `Content-Length` bytes, or the decoded
/// chunks of a `Transfer-Encoding: chunked` body (no reliance on EOF).
fn read_response(stream: &mut TcpStream) -> (u16, Vec<u8>) {
    let (status, head, mut rest) = read_head(stream);
    if is_chunked(&head) {
        let mut body = Vec::new();
        while let Some(chunk) = read_chunk(stream, &mut rest) {
            body.extend_from_slice(&chunk);
        }
        return (status, body);
    }
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
                String::from_utf8_lossy(&rest)
            )
        });

    let mut tmp = [0u8; 1024];
    while rest.len() < content_length {
        let n = stream.read(&mut tmp).expect("read body");
        assert!(n > 0, "connection closed before body completed");
        rest.extend_from_slice(&tmp[..n]);
    }
    rest.truncate(content_length);
    (status, rest)
}

/// Read the status line and headers: `(status, head_text, bytes_read_past_the_head)`.
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

/// Read one HTTP/1.1 chunk, using `rest` as the read-ahead buffer. `None` at the
/// terminating zero-length chunk.
fn read_chunk(stream: &mut TcpStream, rest: &mut Vec<u8>) -> Option<Vec<u8>> {
    let mut tmp = [0u8; 4096];
    let line_end = loop {
        if let Some(pos) = find_subsequence(rest, b"\r\n") {
            break pos;
        }
        let n = stream.read(&mut tmp).expect("read chunk size");
        assert!(n > 0, "connection closed inside a chunk size line");
        rest.extend_from_slice(&tmp[..n]);
    };
    let size_text = String::from_utf8_lossy(&rest[..line_end]).to_string();
    let size = usize::from_str_radix(size_text.split(';').next().unwrap().trim(), 16)
        .expect("chunk size");
    let needed = line_end + 2 + size + 2;
    while rest.len() < needed {
        let n = stream.read(&mut tmp).expect("read chunk data");
        assert!(n > 0, "connection closed inside a chunk");
        rest.extend_from_slice(&tmp[..n]);
    }
    let data = rest[line_end + 2..line_end + 2 + size].to_vec();
    rest.drain(..needed);
    (size > 0).then_some(data)
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
    use foundation_netio::shared::http::Proto;

    let transport = H1Transport::new(HttpClientBuilder::new().build());
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
async fn h1_client_transport_over_real_socket() {
    use foundation_connectrpc::shared::transport::Transport;
    use foundation_connectrpc::H1Transport;
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
    let transport = H1Transport::new(HttpClientBuilder::new().build());

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
async fn h1_client_server_stream_over_real_socket() {
    use foundation_connectrpc::shared::transport::Transport;
    use foundation_connectrpc::H1Transport;
    use foundation_core::url::Uri;
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
    let transport = H1Transport::new(HttpClientBuilder::new().build());
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

// ── Streaming responses and request bodies over a raw socket ─────────────────
//
// WHY: the HTTP/1.1 server used to collect a streaming RPC's whole response in
// memory and write it only after the handler finished, and it pushed request
// bodies through a 4-slot pipe with `try_send`, silently dropping every chunk
// after the fourth. These tests pin down that frames reach the client as they
// are produced, that a stream is held back by a slow client rather than
// buffered, and that request bodies of any chunk count arrive whole.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};

use foundation_connectrpc::{Code, ConnectError};
use foundation_netio::HttpClientBuilder;

/// Start an `HttpServer` serving `serve` at `route`; returns its address and the
/// shutdown switch.
fn start_server(route: &str, serve: Arc<dyn Serve>) -> (std::net::SocketAddr, Arc<OnSignal>) {
    let mut app = HttpApp::new_serve();
    app.router.add_route_any(route, &serve);
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

/// One Connect envelope: flags byte, 4-byte big-endian length, payload.
fn envelope(flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5 + payload.len());
    buf.push(flags);
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

fn json_envelope(msg: &TestMsg) -> Vec<u8> {
    envelope(0, &JsonCodec.marshal(msg).expect("marshal"))
}

/// Read the next whole envelope `(flags, payload)` from a chunked body, carrying
/// partial envelopes across chunks in `carry`. `None` once the body ends.
fn read_envelope(
    stream: &mut TcpStream,
    rest: &mut Vec<u8>,
    carry: &mut Vec<u8>,
) -> Option<(u8, Vec<u8>)> {
    loop {
        if carry.len() >= 5 {
            let len = u32::from_be_bytes([carry[1], carry[2], carry[3], carry[4]]) as usize;
            if carry.len() >= 5 + len {
                let flags = carry[0];
                let payload = carry[5..5 + len].to_vec();
                carry.drain(..5 + len);
                return Some((flags, payload));
            }
        }
        let chunk = read_chunk(stream, rest)?;
        carry.extend_from_slice(&chunk);
    }
}

/// Send a Connect streaming POST whose body is `body`, as one chunk per entry.
fn send_chunked_request(
    stream: &mut TcpStream,
    procedure: &str,
    content_type: &str,
    body: &[Vec<u8>],
) {
    let head = format!(
        "POST {procedure} HTTP/1.1\r\nHost: t\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
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

/// Pending until `flag` is set. Busy-wakes, which is fine for a test gate.
struct WaitFor(Arc<AtomicBool>);

impl Future for WaitFor {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn open_stream_request(addr: std::net::SocketAddr, procedure: &str) -> (TcpStream, Vec<u8>) {
    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let seed = TestMsg {
        id: 1,
        name: "seed".to_string(),
    };
    send_chunked_request(
        &mut stream,
        procedure,
        "application/connect+json",
        &[json_envelope(&seed)],
    );
    let (status, head, rest) = read_head(&mut stream);
    assert_eq!(status, 200, "stream opens with 200; head:\n{head}");
    assert!(is_chunked(&head), "a stream is sent chunked; head:\n{head}");
    (stream, rest)
}

const GATED_PROCEDURE: &str = "/test.StreamService/Gated";

/// The first frame must reach the client while the handler is still running:
/// the handler emits one item, then waits until the client has received it.
/// A server that collects the stream before writing never sends that item, and
/// the client's read times out.
#[test]
fn first_frame_arrives_before_the_handler_finishes() {
    let _guard = initialize_pool(46, Some(4));
    let released = Arc::new(AtomicBool::new(false));

    let gate = released.clone();
    let mut router = Router::new();
    router.server_stream(
        GATED_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        move |_ctx, _req: Request<TestMsg>| {
            let gate = gate.clone();
            async move {
                Ok(futures::stream::unfold(0u8, move |step| {
                    let gate = gate.clone();
                    async move {
                        match step {
                            0 => Some((
                                Ok(TestMsg {
                                    id: 0,
                                    name: "first".into(),
                                }),
                                1,
                            )),
                            1 => {
                                WaitFor(gate).await;
                                Some((
                                    Ok(TestMsg {
                                        id: 1,
                                        name: "second".into(),
                                    }),
                                    2,
                                ))
                            }
                            _ => None,
                        }
                    }
                }))
            }
        },
        HandlerOptions::new(),
    );
    let (addr, shutdown) = start_server(
        GATED_PROCEDURE,
        Arc::new(ConnectRpcServe::new(router.into_handler())),
    );

    let (mut stream, mut rest) = open_stream_request(addr, GATED_PROCEDURE);
    let mut carry = Vec::new();

    let (flags, payload) = read_envelope(&mut stream, &mut rest, &mut carry)
        .expect("first frame arrives while the handler waits");
    assert_eq!(flags, 0);
    let first: TestMsg = JsonCodec.unmarshal(Bytes::from(payload)).expect("decode");
    assert_eq!(first.name, "first");
    released.store(true, Ordering::SeqCst);

    let (_, payload) = read_envelope(&mut stream, &mut rest, &mut carry).expect("second frame");
    let second: TestMsg = JsonCodec.unmarshal(Bytes::from(payload)).expect("decode");
    assert_eq!(second.name, "second");
    let (flags, _) = read_envelope(&mut stream, &mut rest, &mut carry).expect("end of stream");
    assert_eq!(flags & FLAG_END_STREAM, FLAG_END_STREAM);
    assert!(
        read_envelope(&mut stream, &mut rest, &mut carry).is_none(),
        "body ends"
    );

    shutdown.turn_on();
}

const ENDLESS_PROCEDURE: &str = "/test.StreamService/Endless";

/// Sets its flag when dropped: tells the test the handler's stream was released.
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// An endless stream is held back by a client that stops reading (no frames
/// pile up in server memory), and is released once the client goes away.
#[test]
fn endless_stream_follows_the_client_and_stops_when_it_leaves() {
    let _guard = initialize_pool(47, Some(4));
    let produced = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    // Large frames fill the socket buffers after a few items, so backpressure
    // shows up as the producer stalling.
    let big = "x".repeat(64 * 1024);

    let (count, drop_flag) = (produced.clone(), dropped.clone());
    let mut router = Router::new();
    router.server_stream(
        ENDLESS_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        move |_ctx, _req: Request<TestMsg>| {
            let (count, drop_flag, big) = (count.clone(), drop_flag.clone(), big.clone());
            async move {
                let guard = DropFlag(drop_flag);
                Ok(futures::stream::unfold(guard, move |guard| {
                    let n = count.fetch_add(1, Ordering::SeqCst);
                    let big = big.clone();
                    async move {
                        Some((
                            Ok(TestMsg {
                                id: n as i32,
                                name: big,
                            }),
                            guard,
                        ))
                    }
                }))
            }
        },
        HandlerOptions::new(),
    );
    let (addr, shutdown) = start_server(
        ENDLESS_PROCEDURE,
        Arc::new(ConnectRpcServe::new(router.into_handler())),
    );

    let (mut stream, mut rest) = open_stream_request(addr, ENDLESS_PROCEDURE);
    let mut carry = Vec::new();
    for i in 0..5 {
        let (flags, payload) =
            read_envelope(&mut stream, &mut rest, &mut carry).expect("frame from endless stream");
        assert_eq!(flags, 0);
        let msg: TestMsg = JsonCodec.unmarshal(Bytes::from(payload)).expect("decode");
        assert_eq!(msg.id, i, "frames arrive in order");
    }

    // Stop reading: the producer must stall once the pipes and socket buffers
    // are full instead of running ahead into memory. How long the buffers take
    // to fill depends on the kernel's socket buffer sizes and the machine's
    // load, so wait for the count to hold still for a whole window rather than
    // sampling once after a fixed delay.
    let window = Duration::from_millis(300);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut stalled_at = produced.load(Ordering::SeqCst);
    loop {
        std::thread::sleep(window);
        let now = produced.load(Ordering::SeqCst);
        if now == stalled_at {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the producer is held back while the client is not reading; \
             it was still producing ({stalled_at} -> {now}) after 10s"
        );
        stalled_at = now;
    }
    assert!(
        stalled_at < 500,
        "only a bounded number of frames is in flight, got {stalled_at}"
    );

    // Leave: the server's next write fails, which must release the handler.
    drop(stream);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !dropped.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "the handler's stream was not released after the client left"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    shutdown.turn_on();
}

const FAILING_PROCEDURE: &str = "/test.StreamService/FailsAfterOne";

/// An error after frames have gone out travels in-band in the end-of-stream
/// frame, under the 200 already sent.
#[test]
fn error_after_frames_is_sent_in_band() {
    let _guard = initialize_pool(48, Some(4));
    let mut router = Router::new();
    router.server_stream(
        FAILING_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, _req: Request<TestMsg>| async move {
            let items: Vec<ConnectResult<TestMsg>> = vec![
                Ok(TestMsg {
                    id: 1,
                    name: "ok".into(),
                }),
                Err(ConnectError::new(Code::ResourceExhausted, "out of widgets").into()),
            ];
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );
    let (addr, shutdown) = start_server(
        FAILING_PROCEDURE,
        Arc::new(ConnectRpcServe::new(router.into_handler())),
    );

    let (mut stream, mut rest) = open_stream_request(addr, FAILING_PROCEDURE);
    let mut carry = Vec::new();
    let (flags, _) = read_envelope(&mut stream, &mut rest, &mut carry).expect("data frame");
    assert_eq!(flags, 0);
    let (flags, payload) = read_envelope(&mut stream, &mut rest, &mut carry).expect("end frame");
    assert_eq!(flags & FLAG_END_STREAM, FLAG_END_STREAM);
    let end = String::from_utf8_lossy(&payload);
    assert!(
        end.contains("resource_exhausted"),
        "end-stream carries the code: {end}"
    );
    assert!(
        end.contains("out of widgets"),
        "end-stream carries the message: {end}"
    );

    shutdown.turn_on();
}

const REFUSING_PROCEDURE: &str = "/test.StreamService/Refuses";

/// A handler that fails before producing a stream still gets a well-formed
/// response carrying its error code.
#[test]
fn error_before_any_frame_is_reported() {
    let _guard = initialize_pool(49, Some(4));
    let mut router = Router::new();
    router.server_stream(
        REFUSING_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, _req: Request<TestMsg>| async move {
            Err::<futures::stream::Empty<ConnectResult<TestMsg>>, _>(
                ConnectError::new(Code::PermissionDenied, "not yours").into(),
            )
        },
        HandlerOptions::new(),
    );
    let (addr, shutdown) = start_server(
        REFUSING_PROCEDURE,
        Arc::new(ConnectRpcServe::new(router.into_handler())),
    );

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let seed = TestMsg::default();
    send_chunked_request(
        &mut stream,
        REFUSING_PROCEDURE,
        "application/connect+json",
        &[json_envelope(&seed)],
    );
    let (status, body) = read_response(&mut stream);
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("permission_denied") && text.contains("not yours"),
        "status {status}, body: {text}"
    );

    shutdown.turn_on();
}

/// A unary request body sent as many chunks arrives whole. It used to be cut
/// off after the fourth chunk.
#[test]
fn unary_request_body_in_many_chunks_arrives_whole() {
    let _guard = initialize_pool(50, Some(4));
    let (addr, shutdown) = start_server(PROCEDURE, echo_serve());

    let msg = TestMsg {
        id: 7,
        name: "a name long enough to split into many small chunks".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal").to_vec();
    let pieces: Vec<Vec<u8>> = json.chunks(3).map(<[u8]>::to_vec).collect();
    assert!(pieces.len() > 10);

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    send_chunked_request(&mut stream, PROCEDURE, "application/json", &pieces);
    let (status, body) = read_response(&mut stream);
    assert_eq!(status, 200, "body: {}", String::from_utf8_lossy(&body));
    let echoed: TestMsg = JsonCodec.unmarshal(Bytes::from(body)).expect("decode echo");
    assert_eq!(echoed, msg);

    shutdown.turn_on();
}

const COUNT_PROCEDURE: &str = "/test.StreamService/Count";

/// Every message of a client stream sent as many chunks reaches the handler.
#[test]
fn client_stream_in_many_chunks_delivers_every_message() {
    let _guard = initialize_pool(51, Some(4));
    let mut router = Router::new();
    router.client_stream(
        COUNT_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, mut reqs: foundation_connectrpc::RequestStream<TestMsg>| async move {
            let mut count = 0;
            while let Some(msg) = reqs.next().await {
                msg?;
                count += 1;
            }
            Ok(Response::new(TestMsg {
                id: count,
                name: "counted".into(),
            }))
        },
        HandlerOptions::new(),
    );
    let (addr, shutdown) = start_server(
        COUNT_PROCEDURE,
        Arc::new(ConnectRpcServe::new(router.into_handler())),
    );

    let pieces: Vec<Vec<u8>> = (0..12)
        .map(|i| {
            json_envelope(&TestMsg {
                id: i,
                name: format!("m{i}"),
            })
        })
        .collect();

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    send_chunked_request(
        &mut stream,
        COUNT_PROCEDURE,
        "application/connect+json",
        &pieces,
    );
    let (status, body) = read_response(&mut stream);
    assert_eq!(status, 200);
    let (messages, saw_end) = parse_connect_envelope_stream(&body);
    assert!(saw_end, "end-stream frame present");
    assert_eq!(messages.len(), 1);
    let reply: TestMsg = JsonCodec
        .unmarshal(Bytes::from(messages[0].clone()))
        .expect("decode");
    assert_eq!(reply.id, 12, "every message reached the handler");

    shutdown.turn_on();
}

/// Write `head`, then each body piece with a pause before it, so the server
/// reads the body faster than it arrives and its non-blocking socket reports
/// `WouldBlock` between pieces.
fn send_slowly(stream: &mut TcpStream, head: &str, pieces: &[Vec<u8>], pause: Duration) {
    stream.write_all(head.as_bytes()).expect("write head");
    stream.flush().expect("flush head");
    for piece in pieces {
        std::thread::sleep(pause);
        stream.write_all(piece).expect("write piece");
        stream.flush().expect("flush piece");
    }
}

/// A request body that arrives slowly is waited for, not failed: every
/// message of a client stream whose chunks trickle in reaches the handler.
#[test]
fn client_stream_body_arriving_slowly_is_waited_for() {
    let _guard = initialize_pool(52, Some(4));
    let mut router = Router::new();
    router.client_stream(
        COUNT_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, mut reqs: foundation_connectrpc::RequestStream<TestMsg>| async move {
            let mut count = 0;
            while let Some(msg) = reqs.next().await {
                msg?;
                count += 1;
            }
            Ok(Response::new(TestMsg {
                id: count,
                name: "counted".into(),
            }))
        },
        HandlerOptions::new(),
    );
    let (addr, shutdown) = start_server(
        COUNT_PROCEDURE,
        Arc::new(ConnectRpcServe::new(router.into_handler())),
    );

    let mut pieces: Vec<Vec<u8>> = (0..5)
        .map(|i| {
            let env = json_envelope(&TestMsg {
                id: i,
                name: format!("m{i}"),
            });
            let mut chunk = format!("{:x}\r\n", env.len()).into_bytes();
            chunk.extend_from_slice(&env);
            chunk.extend_from_slice(b"\r\n");
            chunk
        })
        .collect();
    pieces.push(b"0\r\n\r\n".to_vec());

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let head = format!(
        "POST {COUNT_PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: application/connect+json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    send_slowly(&mut stream, &head, &pieces, Duration::from_millis(50));
    let (status, body) = read_response(&mut stream);
    assert_eq!(status, 200, "body: {}", String::from_utf8_lossy(&body));
    let (messages, saw_end) = parse_connect_envelope_stream(&body);
    assert!(saw_end, "end-stream frame present");
    assert_eq!(messages.len(), 1);
    let reply: TestMsg = JsonCodec
        .unmarshal(Bytes::from(messages[0].clone()))
        .expect("decode");
    assert_eq!(reply.id, 5, "every message reached the handler");

    shutdown.turn_on();
}

/// A sized (`Content-Length`) unary body that arrives in slow pieces is
/// waited for and arrives whole.
#[test]
fn unary_body_arriving_slowly_is_waited_for() {
    let _guard = initialize_pool(53, Some(4));
    let (addr, shutdown) = start_server(PROCEDURE, echo_serve());

    let msg = TestMsg {
        id: 9,
        name: "sent a few bytes at a time".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal").to_vec();
    let pieces: Vec<Vec<u8>> = json.chunks(8).map(<[u8]>::to_vec).collect();

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let head = format!(
        "POST {PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        json.len()
    );
    send_slowly(&mut stream, &head, &pieces, Duration::from_millis(50));
    let (status, body) = read_response(&mut stream);
    assert_eq!(status, 200, "body: {}", String::from_utf8_lossy(&body));
    let echoed: TestMsg = JsonCodec.unmarshal(Bytes::from(body)).expect("decode echo");
    assert_eq!(echoed, msg);

    shutdown.turn_on();
}

/// Send a unary echo request on a kept-alive connection and check the reply.
fn keep_alive_unary(stream: &mut TcpStream, id: i32) {
    let msg = TestMsg {
        id,
        name: "again".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal").to_vec();
    let head = format!(
        "POST {PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        json.len()
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.write_all(&json).expect("write body");
    let (status, body) = read_response(stream);
    assert_eq!(status, 200, "body: {}", String::from_utf8_lossy(&body));
    let echoed: TestMsg = JsonCodec.unmarshal(Bytes::from(body)).expect("decode echo");
    assert_eq!(echoed, msg);
}

/// A kept-alive connection serves a unary call, then a streamed response,
/// then another unary call: each response is framed so the next request is
/// read from the right place.
#[test]
fn keep_alive_connection_serves_unary_and_streams_in_turn() {
    let _guard = initialize_pool(54, Some(4));
    let mut app = HttpApp::new_serve();
    let echo = echo_serve();
    let list = list_serve();
    app.router.add_route_any(PROCEDURE, &echo);
    app.router.add_route_any(LIST_PROCEDURE, &list);
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

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    keep_alive_unary(&mut stream, 1);

    let seed = json_envelope(&TestMsg {
        id: 10,
        name: "seed".to_string(),
    });
    let head = format!(
        "POST {LIST_PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: application/connect+json\r\nContent-Length: {}\r\n\r\n",
        seed.len()
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.write_all(&seed).expect("write body");
    let (status, body) = read_response(&mut stream);
    assert_eq!(status, 200);
    let (messages, saw_end) = parse_connect_envelope_stream(&body);
    assert_eq!(messages.len(), 3);
    assert!(saw_end);

    keep_alive_unary(&mut stream, 2);

    shutdown.turn_on();
}

/// A request rejected before its body is read leaves that body on the
/// connection. The server must close the connection after answering instead
/// of parsing those bytes as the next request.
#[test]
fn rejected_request_with_unread_body_closes_the_connection() {
    let _guard = initialize_pool(55, Some(4));
    let (addr, shutdown) = start_server(PROCEDURE, echo_serve());

    let mut stream = connect_with_retry(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    // Not an RPC content type: answered with 415 without reading the body,
    // and the body reads as a request to anyone who parses it as one.
    let body = b"GET /smuggled HTTP/1.1\r\nHost: t\r\n\r\n";
    let head = format!(
        "POST {PROCEDURE} HTTP/1.1\r\nHost: t\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.write_all(body).expect("write body");
    let (status, _) = read_response(&mut stream);
    assert_eq!(status, 415);

    let mut rest = Vec::new();
    let read = stream.read_to_end(&mut rest);
    assert!(
        read.is_ok() && rest.is_empty(),
        "the connection closes after the response and nothing else is answered: {read:?}, {}",
        String::from_utf8_lossy(&rest)
    );

    shutdown.turn_on();
}
