//! Conformance tests (spec-41 F28): Phase 1 Connect + gRPC-Web over HTTP/1.1.
//!
//! Tests start real servers on loopback sockets and drive them with H1Transport.
//! Follows the pattern from server_socket_tests.rs (server setup) and
//! router_dispatch_tests.rs (router patterns).

use std::sync::Arc;

use bytes::{Buf, BufMut, Bytes};
use futures::StreamExt;

use buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};

use foundation_core::synca::OnSignal;
use foundation_core::url::Uri;
use foundation_core::valtron::valtron_test;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;

use foundation_netio::simple_http::client::SimpleHttpClient;
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleUrl, Status,
};

use foundation_connectrpc::codec::{CodecFor, JsonCodec, ProcedureCodecs, ProtoCodec};
use foundation_connectrpc::protocol::connect::streaming_content_type;
use foundation_connectrpc::protocol::grpc_web::{
    constants as grpc_web_consts, content_type as grpc_web_content_type, parse_trailer_frame_body,
};
use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::H1Transport;
use foundation_connectrpc::{
    ConnectResult, ConnectRpcServe, HandlerOptions, IdempotencyLevel, Request, Response, Router,
    Code, ConnectError,
};

// ─────────────────────────────────────────────────────────────────────────────
// Test message
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Procedure paths
// ─────────────────────────────────────────────────────────────────────────────

const ECHO_PROCEDURE: &str = "/test.EchoService/Echo";
const LIST_PROCEDURE: &str = "/test.ListService/Items";

// ─────────────────────────────────────────────────────────────────────────────
// Server helpers
// ─────────────────────────────────────────────────────────────────────────────

struct TestServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<OnSignal>,
}

/// Wait for the server to accept connections.
fn wait_for_server(addr: std::net::SocketAddr) {
    for _ in 0..50 {
        if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("could not connect to test server at {addr}");
}

/// Build an echo unary handler.
fn echo_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        ECHO_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Build an echo unary handler that is POST-only (NoSideEffects NOT set, so GET
/// is not automatically enabled).
fn echo_post_only_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        ECHO_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Build an error-returning unary handler.
fn error_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        ECHO_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, _req: Request<TestMsg>| async move {
            Err(ConnectError::permission_denied("not allowed").into())
        },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Build a server-streaming list handler.
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

/// Build a server-streaming handler that fails after the first frame.
fn error_stream_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.server_stream(
        LIST_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move {
            let items: Vec<ConnectResult<TestMsg>> = vec![
                Ok(TestMsg { id: req.msg.id, name: "first".to_string() }),
                Err(ConnectError::internal("stream error after first frame").into()),
            ];
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Build a client-streaming echo handler (collects all messages, returns count).
fn client_stream_echo_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.client_stream(
        ECHO_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, reqs: foundation_connectrpc::RequestStream<TestMsg>| async move {
            let mut count = 0u32;
            let mut last = TestMsg::default();
            use futures::StreamExt;
            let mut s = reqs;
            while let Some(msg) = s.next().await {
                match msg {
                    Ok(m) => { count += 1; last = m; }
                    Err(_) => break,
                }
            }
            Ok(Response::new(TestMsg {
                id: count as i32,
                name: last.name,
            }))
        },
        HandlerOptions::new(),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Build an echo handler forwarding request headers to response.
fn echo_with_header_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        ECHO_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move {
            let msg = req.msg.clone();
            let mut resp = Response::new(msg);
            // Echo all request headers prefixed with "X-Req-" into response headers.
            for (h, vals) in req.headers() {
                if h.to_string().starts_with("x-custom-") {
                    resp.headers_mut().insert(h.clone(), vals.clone());
                }
            }
            Ok(resp)
        },
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Build an echo handler with custom response headers and trailers.
fn echo_with_trailer_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        ECHO_PROCEDURE,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move {
            let mut resp = Response::new(req.msg);
            resp.headers_mut().insert(
                SimpleHeader::from("x-response-custom".to_string()),
                vec!["header-value".to_string()],
            );
            resp.trailers_mut().insert(
                SimpleHeader::from("x-trailer-custom".to_string()),
                vec!["trailer-value".to_string()],
            );
            Ok(resp)
        },
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

fn start_server(serve: Arc<dyn Serve>) -> TestServer {
    let mut app = HttpApp::new_serve();
    app.router.add_route_any(ECHO_PROCEDURE, &serve);
    app.router.add_route_any(LIST_PROCEDURE, &serve);
    app.router.add_route_any("/test.EchoService/Echo", &serve);

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

    // Give the server a moment to start accepting.
    wait_for_server(addr);

    TestServer { addr, shutdown }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parse helpers
// ─────────────────────────────────────────────────────────────────────────────

const FLAG_END_STREAM: u8 = 0x02;

/// Parse a Connect-protocol envelope stream into `(messages, saw_end_stream)`.
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

const TRAILER_FLAG: u8 = 0x80;

/// Parse a gRPC-Web response body into `(messages, trailers)`.
fn parse_grpc_web_body(body: &[u8]) -> (Vec<Vec<u8>>, SimpleHeaders) {
    let mut messages = Vec::new();
    let mut trailers = SimpleHeaders::new();
    let mut pos = 0;
    while pos + 5 <= body.len() {
        let flags = body[pos];
        let len = u32::from_be_bytes([body[pos + 1], body[pos + 2], body[pos + 3], body[pos + 4]])
            as usize;
        let start = pos + 5;
        let stop = (start + len).min(body.len());
        let payload = Vec::from(&body[start..stop]);
        if flags & TRAILER_FLAG != 0 {
            trailers = parse_trailer_frame_body(&payload);
        } else {
            messages.push(payload);
        }
        pos = stop;
    }
    (messages, trailers)
}

/// Build a gRPC envelope for a request message.
fn grpc_envelope(body: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8]; // uncompressed flag
    buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
    buf.extend_from_slice(body);
    buf
}

/// Build a Connect streaming envelope for a request message.
fn connect_envelope(body: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8]; // flags = 0
    buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
    buf.extend_from_slice(body);
    buf
}

fn json_bytes(msg: &TestMsg) -> Vec<u8> {
    JsonCodec.marshal(msg).expect("json marshal").to_vec()
}

fn proto_bytes(msg: &TestMsg) -> Vec<u8> {
    ProtoCodec.marshal(msg).expect("proto marshal").to_vec()
}

fn connect_transport() -> (H1Transport, TestServer) {
    let serve = echo_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));
    (transport, server)
}

fn build_request_descriptor(
    addr: std::net::SocketAddr,
    procedure: &str,
    content_type: &str,
    method: SimpleMethod,
) -> RequestDescriptor {
    let url = format!("http://127.0.0.1:{}{}", addr.port(), procedure);
    let uri = Uri::parse(&url).expect("valid URI");
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::CONTENT_TYPE, vec![content_type.to_string()]);
    RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url),
        request_uri: uri,
        headers,
        method,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 1: Connect — Unary POST Proto Success
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Connect unary via proto codec is the most basic RPC shape.
/// WHAT: POST a proto-encoded message, verify 200 + matching echo + content-type.
#[valtron_test(seed = 28101, threads = 4)]
async fn connect_unary_post_proto_success() {
    let (transport, server) = connect_transport();

    let msg = TestMsg { id: 42, name: "proto-unary".into() };
    let body = proto_bytes(&msg);

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        "application/proto",
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "unary proto POST returns 200");

    let ct = headers.get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(ct, "application/proto", "content-type is proto");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let echoed: TestMsg = ProtoCodec
        .unmarshal(Bytes::from(body_bytes))
        .expect("decode proto response");
    assert_eq!(echoed, msg, "echoed message matches request");
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 2: Connect — Unary POST JSON Success
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: JSON codec is the second mandatory codec (Connect spec).
/// WHAT: POST a JSON-encoded message, verify 200 + matching echo + content-type.
#[valtron_test(seed = 28102, threads = 4)]
async fn connect_unary_post_json_success() {
    let (transport, server) = connect_transport();

    let msg = TestMsg { id: 99, name: "json-unary".into() };
    let body = json_bytes(&msg);

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        "application/json",
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "unary json POST returns 200");

    let ct = headers.get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(ct, "application/json", "content-type is json");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let echoed: TestMsg = JsonCodec
        .unmarshal(Bytes::from(body_bytes))
        .expect("decode json response");
    assert_eq!(echoed, msg, "echoed message matches request");
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 3: Connect — Unary Error
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Handler errors must be rendered correctly per Connect spec.
/// WHAT: Handler returns permission_denied, verify HTTP 403 + JSON error body.
#[valtron_test(seed = 28103, threads = 4)]
async fn connect_unary_error() {
    let serve = error_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 1, name: "error".into() };
    let body = json_bytes(&msg);

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        "application/json",
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    // permission_denied → 403
    assert_eq!(status, Status::Forbidden, "permission_denied maps to 403");

    // Error body is JSON.
    let ct = headers.get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(ct, "application/json", "error content-type is json");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let err_body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("valid json error body");
    assert_eq!(err_body["code"], "permission_denied");
    assert_eq!(err_body["message"], "not allowed");

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 4: Connect — Unary GET Success
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Connect GET for NoSideEffects endpoints serves idempotent RPCs.
/// WHAT: GET with message in query params, verify 200 + correct echo.
#[valtron_test(seed = 28104, threads = 4)]
async fn connect_unary_get_success() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;

    let serve = echo_serve(); // NoSideEffects
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 77, name: "get-test".into() };
    let json_body = json_bytes(&msg);
    let encoded_msg = URL_SAFE_NO_PAD.encode(&json_body);

    let query = format!(
        "connect=v1&encoding=json&base64=1&message={}",
        encoded_msg
    );
    let url = format!(
        "http://127.0.0.1:{}{}?{}",
        server.addr.port(),
        ECHO_PROCEDURE,
        query
    );
    let uri = Uri::parse(&url).expect("valid URI");

    let desc = RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url),
        request_uri: uri,
        headers: SimpleHeaders::new(),
        method: SimpleMethod::GET,
    };

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.close(); // no body for GET

    let (status, headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "unary GET returns 200");

    let ct = headers.get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(ct, "application/json", "content-type is json for json encoding");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let echoed: TestMsg = JsonCodec
        .unmarshal(Bytes::from(body_bytes))
        .expect("decode json response");
    assert_eq!(echoed, msg, "GET echo matches request");

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 5: Connect — Unary Unknown Path 404
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Unregistered paths must return 404.
/// WHAT: POST to an unregistered path.
#[valtron_test(seed = 28105, threads = 4)]
async fn connect_unary_unknown_path_404() {
    let (transport, server) = connect_transport();

    let msg = TestMsg { id: 0, name: "".into() };
    let body = json_bytes(&msg);

    // Use a non-existent procedure path.
    let desc = build_request_descriptor(
        server.addr,
        "/test.Nonexistent/Missing",
        "application/json",
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, _headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::NotFound, "unregistered path returns 404");

    // Drain body.
    while let Some(chunk) = stream.recv_body.next().await {
        drop(chunk);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 6: Connect — Unary Wrong Method 405
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Wrong HTTP method must return 405 with Allow header.
/// WHAT: GET to a POST-only endpoint.
#[valtron_test(seed = 28106, threads = 4)]
async fn connect_unary_wrong_method_405() {
    let serve = echo_post_only_serve(); // NoSideEffects NOT set
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        "application/json",
        SimpleMethod::GET,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.close(); // GET has no body

    let (status, headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::MethodNotAllowed, "GET to POST-only returns 405");

    let allow = headers.get(&SimpleHeader::from("allow".to_string()))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert!(allow.contains("POST"), "Allow header contains POST: {allow}");

    // Drain body.
    while let Some(chunk) = stream.recv_body.next().await {
        drop(chunk);
    }

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 7: Connect — Server Streaming Success
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Server-streaming is a fundamental Connect streaming pattern.
/// WHAT: Send one request, receive multiple response frames with EndStream.
#[valtron_test(seed = 28107, threads = 4)]
async fn connect_server_stream_success() {
    let serve = list_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 10, name: "seed".into() };
    let json = json_bytes(&msg);
    let envelope = connect_envelope(&json);

    let desc = build_request_descriptor(
        server.addr,
        LIST_PROCEDURE,
        &streaming_content_type("json"),
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(envelope)).expect("send body");
    stream.send_body.close();

    let (status, headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "server-stream returns 200");

    let ct = headers.get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(ct, "application/connect+json", "streaming content-type");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let (messages, saw_end_stream) = parse_connect_envelope_stream(&body_bytes);
    assert_eq!(messages.len(), 3, "three server-stream items");
    assert!(saw_end_stream, "EndStream frame present");

    for (i, msg_data) in messages.iter().enumerate() {
        let item: TestMsg = JsonCodec
            .unmarshal(Bytes::from(msg_data.clone()))
            .expect("decode item");
        assert_eq!(item.id, 10 + i as i32, "item {i} id");
        assert_eq!(item.name, format!("item-{i}"), "item {i} name");
    }

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 8: Connect — Client Streaming Success
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Client-streaming lets callers send multiple request messages.
/// WHAT: Send two messages, get a single response with count.
#[valtron_test(seed = 28108, threads = 4)]
async fn connect_client_stream_success() {
    let serve = client_stream_echo_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        &streaming_content_type("json"),
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");

    // Send two messages.
    let msg1 = TestMsg { id: 1, name: "first".into() };
    let msg2 = TestMsg { id: 2, name: "second".into() };
    let enc1 = connect_envelope(&json_bytes(&msg1));
    let enc2 = connect_envelope(&json_bytes(&msg2));

    stream.send_body.try_send(Bytes::from(enc1)).expect("send msg1");
    stream.send_body.try_send(Bytes::from(enc2)).expect("send msg2");
    stream.send_body.close();

    let (status, _headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "client-stream returns 200");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    // Client streaming returns a single message (the count + last name).
    // The response is a single Connect envelope.
    let (messages, saw_end) = parse_connect_envelope_stream(&body_bytes);
    assert_eq!(messages.len(), 1, "one response message");
    assert!(saw_end, "EndStream present");

    let result: TestMsg = JsonCodec
        .unmarshal(Bytes::from(messages[0].clone()))
        .expect("decode response");
    assert_eq!(result.id, 2, "count = 2 messages received");
    assert_eq!(result.name, "second", "last message name");

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 9: Connect — Server Streaming Error
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Streaming errors must be reported via EndStream frame.
/// WHAT: Handler sends first frame then fails; verify error in EndStream.
#[valtron_test(seed = 28109, threads = 4)]
async fn connect_server_stream_error() {
    let serve = error_stream_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 1, name: "trigger".into() };
    let envelope = connect_envelope(&json_bytes(&msg));

    let desc = build_request_descriptor(
        server.addr,
        LIST_PROCEDURE,
        &streaming_content_type("json"),
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(envelope)).expect("send body");
    stream.send_body.close();

    let (status, _headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "streaming error returns 200 (error rides payload)");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    // First frame is a data frame, second is the EndStream error frame.
    let (messages, saw_end) = parse_connect_envelope_stream(&body_bytes);
    assert_eq!(messages.len(), 1, "one data frame before error");
    assert!(saw_end, "EndStream error frame present");

    let first: TestMsg = JsonCodec
        .unmarshal(Bytes::from(messages[0].clone()))
        .expect("decode first item");
    assert_eq!(first.name, "first");

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 10: gRPC-Web — Unary Proto Success
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: gRPC-Web unary is how browsers call RPCs.
/// WHAT: Send gRPC-Web enveloped proto request, verify grpc-status trailer.
#[valtron_test(seed = 28110, threads = 4)]
async fn grpc_web_unary_proto_success() {
    let (transport, server) = connect_transport();

    let msg = TestMsg { id: 55, name: "grpc-web".into() };
    let proto = proto_bytes(&msg);
    let envelope = grpc_envelope(&proto);

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        &grpc_web_content_type("proto", false),
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(envelope)).expect("send body");
    stream.send_body.close();

    let (status, _headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    // gRPC-Web always returns 200; status rides the trailer frame.
    assert_eq!(status, Status::OK, "grpc-web returns 200");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let (messages, trailers) = parse_grpc_web_body(&body_bytes);
    assert_eq!(messages.len(), 1, "one data frame in grpc-web response");

    let echoed: TestMsg = ProtoCodec
        .unmarshal(Bytes::from(messages[0].clone()))
        .expect("decode proto response");
    assert_eq!(echoed, msg, "grpc-web echo matches");

    // Verify grpc-status=0 in trailers.
    let grpc_status = trailers
        .get(&SimpleHeader::from(grpc_web_consts::TRAILER_STATUS.to_string()))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(grpc_status, "0", "grpc-status is 0 (success)");
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 11: gRPC-Web — Unary Error
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: gRPC-Web errors must carry grpc-status + grpc-message in trailers.
/// WHAT: Handler returns error, verify grpc-status + grpc-message in body.
#[valtron_test(seed = 28111, threads = 4)]
async fn grpc_web_unary_error() {
    let serve = error_serve(); // returns permission_denied
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 1, name: "grpc-error".into() };
    let proto = proto_bytes(&msg);
    let envelope = grpc_envelope(&proto);

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        &grpc_web_content_type("proto", false),
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(envelope)).expect("send body");
    stream.send_body.close();

    let (status, _headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "grpc-web error returns 200 (error rides trailers)");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let (_messages, trailers) = parse_grpc_web_body(&body_bytes);

    // permission_denied → grpc-status=7
    let grpc_status = trailers
        .get(&SimpleHeader::from(grpc_web_consts::TRAILER_STATUS.to_string()))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(grpc_status, "7", "grpc-status is 7 (PermissionDenied)");

    // grpc-message should contain the error message
    let grpc_message = trailers
        .get(&SimpleHeader::from(grpc_web_consts::TRAILER_MESSAGE.to_string()))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(grpc_message, "not allowed", "grpc-message is 'not allowed'");

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 12: gRPC-Web — Server Stream Success
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: gRPC-Web server-streaming with 0x80 trailer frame.
/// WHAT: Enveloped frames with trailer, verify all items + grpc-status.
#[valtron_test(seed = 28112, threads = 4)]
async fn grpc_web_server_stream_success() {
    let serve = list_serve(); // 3 items
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 20, name: "stream".into() };
    let json = json_bytes(&msg);
    let envelope = grpc_envelope(&json); // gRPC envelope for request

    let desc = build_request_descriptor(
        server.addr,
        LIST_PROCEDURE,
        &grpc_web_content_type("json", false),
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(envelope)).expect("send body");
    stream.send_body.close();

    let (status, _headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK, "grpc-web streaming returns 200");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let (messages, trailers) = parse_grpc_web_body(&body_bytes);
    assert_eq!(messages.len(), 3, "three grpc-web stream items");

    for (i, msg_data) in messages.iter().enumerate() {
        let item: TestMsg = JsonCodec
            .unmarshal(Bytes::from(msg_data.clone()))
            .expect("decode item");
        assert_eq!(item.id, 20 + i as i32, "item {i} id");
        assert_eq!(item.name, format!("item-{i}"), "item {i} name");
    }

    // Verify grpc-status=0 in trailer frame.
    let grpc_status = trailers
        .get(&SimpleHeader::from(grpc_web_consts::TRAILER_STATUS.to_string()))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert_eq!(grpc_status, "0", "grpc-status is 0 (success)");

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 13: Request Headers Reach Handler
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Custom request headers must be visible to the handler.
/// WHAT: Send custom x-custom-* headers, verify they echo back.
#[valtron_test(seed = 28113, threads = 4)]
async fn request_headers_reach_handler() {
    let serve = echo_with_header_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 1, name: "headers".into() };
    let body = json_bytes(&msg);

    let url = format!("http://127.0.0.1:{}{}", server.addr.port(), ECHO_PROCEDURE);
    let uri = Uri::parse(&url).expect("valid URI");
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/json".to_string()]);
    headers.insert(
        SimpleHeader::from("x-custom-foo".to_string()),
        vec!["bar".to_string()],
    );
    headers.insert(
        SimpleHeader::from("x-custom-baz".to_string()),
        vec!["qux".to_string()],
    );

    let desc = RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url),
        request_uri: uri,
        headers,
        method: SimpleMethod::POST,
    };

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, response_headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK);

    // Verify custom headers were echoed back from the handler.
    let foo_val = response_headers
        .get(&SimpleHeader::from("x-custom-foo".to_string()))
        .and_then(|v| v.first())
        .cloned();
    assert_eq!(foo_val.as_deref(), Some("bar"), "x-custom-foo echoed back");

    let baz_val = response_headers
        .get(&SimpleHeader::from("x-custom-baz".to_string()))
        .and_then(|v| v.first())
        .cloned();
    assert_eq!(baz_val.as_deref(), Some("qux"), "x-custom-baz echoed back");

    // Drain body.
    while let Some(chunk) = stream.recv_body.next().await {
        drop(chunk);
    }

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 14: Response Headers Reach Client
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Headers set by the handler must be visible to the client.
/// WHAT: Verify custom response header from handler is present.
#[valtron_test(seed = 28114, threads = 4)]
async fn response_headers_reach_client() {
    let serve = echo_with_trailer_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 1, name: "resp-headers".into() };
    let body = json_bytes(&msg);

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        "application/json",
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, response_headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK);

    // Verify custom response header from handler.
    let val = response_headers
        .get(&SimpleHeader::from("x-response-custom".to_string()))
        .and_then(|v| v.first())
        .cloned();
    assert_eq!(val.as_deref(), Some("header-value"), "response custom header present");

    // Drain body.
    while let Some(chunk) = stream.recv_body.next().await {
        drop(chunk);
    }

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 15: Response Trailers Reach Client
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Trailers set by the handler must be visible to the client (via Trailer-
/// prefixed headers for Connect unary).
/// WHAT: Verify custom trailer from handler is present as trailer-* header.
#[valtron_test(seed = 28115, threads = 4)]
async fn response_trailers_reach_client() {
    let serve = echo_with_trailer_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 1, name: "trailers".into() };
    let body = json_bytes(&msg);

    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        "application/json",
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, response_headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(status, Status::OK);

    // For Connect unary, trailers are rendered as Trailer-* prefixed headers.
    let trailer_val = response_headers
        .get(&SimpleHeader::from("trailer-x-trailer-custom".to_string()))
        .and_then(|v| v.first())
        .cloned();
    assert_eq!(
        trailer_val.as_deref(),
        Some("trailer-value"),
        "trailer rendered as Trailer-* header"
    );

    // Drain body.
    while let Some(chunk) = stream.recv_body.next().await {
        drop(chunk);
    }

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 16: Error Code to HTTP Status
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Each Connect error code must map to the correct HTTP status (Decision 03).
/// WHAT: Verify the Code::http_status() mapping for all codes.
#[valtron_test(seed = 28116, threads = 4)]
async fn error_code_to_http_status() {
    // Test code → HTTP status mappings (Decision 03 table).
    let cases: Vec<(Code, u16, &str)> = vec![
        (Code::Canceled, 499, "canceled"),
        (Code::Unknown, 500, "unknown"),
        (Code::InvalidArgument, 400, "invalid_argument"),
        (Code::DeadlineExceeded, 504, "deadline_exceeded"),
        (Code::NotFound, 404, "not_found"),
        (Code::AlreadyExists, 409, "already_exists"),
        (Code::PermissionDenied, 403, "permission_denied"),
        (Code::ResourceExhausted, 429, "resource_exhausted"),
        (Code::FailedPrecondition, 400, "failed_precondition"),
        (Code::Aborted, 409, "aborted"),
        (Code::OutOfRange, 400, "out_of_range"),
        (Code::Unimplemented, 501, "unimplemented"),
        (Code::Internal, 500, "internal"),
        (Code::Unavailable, 503, "unavailable"),
        (Code::DataLoss, 500, "data_loss"),
        (Code::Unauthenticated, 401, "unauthenticated"),
    ];

    for (code, expected_http, name) in &cases {
        let http = code.http_status();
        assert_eq!(
            http, *expected_http,
            "Code::{} maps to HTTP {expected_http}, got {http}",
            name,
        );
    }

    // Also verify the reverse: http_status → code_name strings as JSON.
    for (code, _expected_http, name) in &cases {
        let err: foundation_errstacks::ErrorTrace<ConnectError> =
            ConnectError::new(*code, "test").into();
        let body = foundation_connectrpc::protocol::connect::unary_error_body(&err).expect("error body");
        let parsed: serde_json::Value =
            serde_json::from_slice(&body).expect("valid json");
        assert_eq!(parsed["code"], *name, "error body has correct code name");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 17: Error EndStream
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Streaming EndStreamResponse must carry the error correctly.
/// WHAT: Handler returns error, verify EndStream frame with error details.
#[valtron_test(seed = 28117, threads = 4)]
async fn error_end_stream() {
    let serve = error_stream_serve();
    let server = start_server(serve);
    let transport = H1Transport::new(Arc::new(SimpleHttpClient::from_system()));

    let msg = TestMsg { id: 1, name: "endstream".into() };
    let envelope = connect_envelope(&json_bytes(&msg));

    let desc = build_request_descriptor(
        server.addr,
        LIST_PROCEDURE,
        &streaming_content_type("json"),
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");

    stream.send_body.try_send(Bytes::from(envelope)).expect("send body");

    stream.send_body.close();

    let (status, _headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");


    assert_eq!(status, Status::OK, "error EndStream returns 200 (error rides payload)");

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        let chunk = chunk.expect("body chunk");
        body_bytes.extend_from_slice(&chunk);
    }

    // Parse the stream: first data frame, then EndStream error frame.
    let mut pos = 0;
    let mut data_frames = 0;
    let mut saw_error = false;
    while pos + 5 <= body_bytes.len() {
        let flags = body_bytes[pos];
        let len = u32::from_be_bytes([
            body_bytes[pos + 1],
            body_bytes[pos + 2],
            body_bytes[pos + 3],
            body_bytes[pos + 4],
        ]) as usize;
        let start = pos + 5;
        let stop = (start + len).min(body_bytes.len());
        let payload = &body_bytes[start..stop];
        if flags & FLAG_END_STREAM != 0 {
            saw_error = true;
            // The EndStream payload is JSON with nested error field:
            // {"error":{"code":"internal",...},"metadata":{...}}
            if !payload.is_empty() {
                let json: serde_json::Value =
                    serde_json::from_slice(payload).expect("valid EndStream JSON");
                let code_str = json["error"]["code"].as_str().unwrap_or("unknown");
                assert_eq!(code_str, "internal", "EndStream error code is 'internal'");
            }
        } else {
            data_frames += 1;
        }
        pos = stop;
    }


    assert!(data_frames >= 1, "at least one data frame before EndStream");
    assert!(saw_error, "EndStream error frame present");

    server.shutdown.turn_on();
}

// ═════════════════════════════════════════════════════════════════════════════
// TEST 18: Unsupported Codec 415
// ═════════════════════════════════════════════════════════════════════════════

/// WHY: Unknown codecs must return 415 with Accept-Post listing supported codecs.
/// WHAT: POST with application/xml (unsupported), verify 415 + Accept-Post.
#[valtron_test(seed = 28118, threads = 4)]
async fn unsupported_codec_415() {
    let (transport, server) = connect_transport();

    let msg = TestMsg { id: 0, name: "".into() };
    let body = json_bytes(&msg);

    // application/xml is not a recognized codec — it parses as a Connect
    // content type with codec name "xml", which is not registered.
    let desc = build_request_descriptor(
        server.addr,
        ECHO_PROCEDURE,
        "application/xml",
        SimpleMethod::POST,
    );

    let mut stream = transport.open(desc).expect("open transport");
    stream.send_body.try_send(Bytes::from(body)).expect("send body");
    stream.send_body.close();

    let (status, headers) = stream.head.next().await
        .expect("response head")
        .expect("head ok");

    assert_eq!(
        status,
        Status::UnsupportedMediaType,
        "unsupported codec returns 415"
    );

    // Accept-Post header lists supported codecs.
    let accept = headers
        .get(&SimpleHeader::from("accept-post".to_string()))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    assert!(
        accept.contains("application/proto") || accept.contains("application/json"),
        "Accept-Post lists supported codecs: {accept}"
    );

    // Drain body.
    while let Some(chunk) = stream.recv_body.next().await {
        drop(chunk);
    }
}
