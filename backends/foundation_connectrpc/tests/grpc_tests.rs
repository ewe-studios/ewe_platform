//! gRPC protocol tests over HTTP/2 cleartext (spec-41 F31).
//!
//! WHY: Feature 31 gRPC is implemented (handler, client, wire tasks) but has zero
//! tests. The acceptance criteria require all four RPC modes over h2c with
//! envelope framing.
//!
//! WHAT: Transport-level tests (manual gRPC envelope construction) verify the
//! handler wire path: envelope framing, HTTP 200 always, `Content-Type:
//! application/grpc+{codec}`. Typed-Client streaming tests exercise the
//! `GrpcClient`'s reader/writer tasks end-to-end.
//!
//! HOW: Transport-level unary/streaming tests build gRPC envelopes manually
//! (flags 0x00 + 4-byte BE length + payload) and send them through
//! `H2Transport`. Typed-Client tests use `Client::new(...).with_grpc()` — the
//! reader/writer tasks handle envelopes transparently.
//!
//! NOTE: `grpc-status` trailers are not yet verifiable. HTTP/2 trailing HEADERS
//! are consumed by the h2 pump but not forwarded to the transport-level caller
//! nor plumbed into the seam's `ClientConn`. When the h2 transport gains a
//! trailer pipe, every test should also assert `grpc-status` in the response.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron_test;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::serve::h2::H2Serve;

use foundation_connectrpc::error::ConnectResult;
use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, HandlerOptions, JsonCodec,
    ProcedureCodecs, Request, RequestStream, Response, Router,
};
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleUrl, Status,
};

// ── Shared test types & server ───────────────────────────────────────────────

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
const CLIENT_STREAM: &str = "/t.Svc/ClientStream";
const BIDI: &str = "/t.Svc/Bidi";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

fn build_router() -> Router {
    let mut router = Router::new();

    router.unary(
        UNARY,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move {
            Ok(Response::new(Msg::of(format!("echo:{}", req.msg.text))))
        },
        HandlerOptions::new(),
    );

    router.server_stream(
        SERVER_STREAM,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move {
            let items: Vec<ConnectResult<Msg>> = (0..3)
                .map(|i| Ok(Msg::of(format!("{}-{i}", req.msg.text))))
                .collect();
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );

    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        |_ctx: Ctx, mut reqs: RequestStream<Msg>| async move {
            let mut parts = Vec::new();
            while let Some(msg) = reqs.next().await {
                parts.push(msg?.text);
            }
            Ok(Response::new(Msg::of(parts.join("+"))))
        },
        HandlerOptions::new(),
    );

    router.bidi_stream(
        BIDI,
        codecs(),
        |_ctx: Ctx, reqs: RequestStream<Msg>| async move {
            Ok(reqs.map(|msg| msg.map(|m| Msg::of(format!("re:{}", m.text)))))
        },
        HandlerOptions::new(),
    );

    router
}

/// Start an HTTP/2 server with Connect and gRPC handlers both wired.
/// The router's protocol-agnostic dispatch picks gRPC based on `Content-Type`.
fn start_h2_server() -> (String, Arc<OnSignal>) {
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(build_router().into_handler()));

    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(UNARY, rpc.clone());
    app.route_any_h2(SERVER_STREAM, rpc.clone());
    app.route_any_h2(CLIENT_STREAM, rpc.clone());
    app.route_any_h2(BIDI, rpc);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(OnSignal::new());

    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    (addr.to_string(), shutdown)
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

// ── gRPC envelope helpers ────────────────────────────────────────────────────

/// Fixed 5-byte gRPC envelope header size.
const ENVELOPE_HEADER_LEN: usize = 5;

/// Wrap `payload` in a gRPC envelope: flags=0 | 4-byte BE len | payload.
fn grpc_envelope(payload: &[u8]) -> Vec<u8> {
    let mut envelope = vec![0u8; ENVELOPE_HEADER_LEN];
    envelope[0] = 0x00; // flags: none
    envelope[1..ENVELOPE_HEADER_LEN]
        .copy_from_slice(&(payload.len() as u32).to_be_bytes());
    envelope.extend_from_slice(payload);
    envelope
}

/// Parse enveloped body bytes into messages (flag byte 0x00 only).
fn parse_envelope_messages(body: &[u8]) -> Vec<Vec<u8>> {
    let mut messages = Vec::new();
    let mut offset = 0;
    while offset + ENVELOPE_HEADER_LEN <= body.len() {
        let flags = body[offset];
        let len = u32::from_be_bytes([
            body[offset + 1],
            body[offset + 2],
            body[offset + 3],
            body[offset + 4],
        ]) as usize;
        let end = (offset + ENVELOPE_HEADER_LEN + len).min(body.len());
        let payload = &body[offset + ENVELOPE_HEADER_LEN..end];
        if flags == 0x00 {
            messages.push(payload.to_vec());
        }
        offset = offset + ENVELOPE_HEADER_LEN + len;
    }
    messages
}

/// Build request headers for a gRPC call with the given codec.
fn grpc_headers(codec: &str) -> SimpleHeaders {
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec![format!("application/grpc+{codec}")],
    );
    headers.insert(
        SimpleHeader::from("te".to_string()),
        vec!["trailers".to_string()],
    );
    headers
}

fn grpc_content_type(codec: &str) -> String {
    format!("application/grpc+{codec}")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Transport-level tests (manual envelope — exercises the gRPC handler wire path)
// ═══════════════════════════════════════════════════════════════════════════════

/// WHY: gRPC unary wraps both request and response in a single envelope.
/// This verifies the server-side `GrpcHandler.decode_unary_request` and
/// `encode_unary_response` work at the wire level.
///
/// WHAT: Manually envelope the request, send through `H2Transport`, verify
/// status=200, Content-Type=application/grpc+json, single enveloped response.
#[valtron_test]
async fn grpc_unary_over_h2c_transport() {
    let (addr, shutdown) = start_h2_server();

    let transport = H2Transport::new();
    let url = format!("http://{addr}{UNARY}");
    let uri = foundation_core::url::Uri::parse(&url).expect("valid uri");

    let msg = serde_json::to_vec(&Msg::of("grpc-unary")).expect("encode");
    let body = grpc_envelope(&msg);

    let mut stream = transport
        .open(RequestDescriptor {
            proto: Proto::HTTP20,
            request_url: SimpleUrl::url_only(&url),
            request_uri: uri,
            headers: grpc_headers("json"),
            method: SimpleMethod::POST,
        })
        .expect("open h2 stream");

    stream
        .send_body
        .try_send(Bytes::from(body))
        .expect("send request body");
    stream.send_body.close();

    let (status, headers) = stream
        .head
        .next()
        .await
        .expect("response head")
        .expect("head ok");

    // gRPC always returns HTTP 200.
    assert_eq!(status, Status::OK, "gRPC response must be 200 OK");

    let ct = headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .expect("response must carry Content-Type");
    assert_eq!(ct, &grpc_content_type("json"), "Content-Type mismatch");

    // Response body is a single envelope containing the echoed message.
    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let messages = parse_envelope_messages(&body_bytes);
    assert_eq!(messages.len(), 1, "gRPC unary response is exactly one envelope");

    let echoed: Msg =
        serde_json::from_slice(&messages[0]).expect("decode gRPC unary response message");
    assert_eq!(echoed, Msg::of("echo:grpc-unary"));

    shutdown.turn_on();
}

/// WHY: gRPC server-streaming: one enveloped request, multiple enveloped
/// response messages. The handler yields N messages; each must arrive as a
/// separate gRPC envelope.
#[valtron_test]
async fn grpc_server_stream_over_h2c_transport() {
    let (addr, shutdown) = start_h2_server();

    let transport = H2Transport::new();
    let url = format!("http://{addr}{SERVER_STREAM}");
    let uri = foundation_core::url::Uri::parse(&url).expect("valid uri");

    let msg = serde_json::to_vec(&Msg::of("items")).expect("encode");
    let body = grpc_envelope(&msg);

    let mut stream = transport
        .open(RequestDescriptor {
            proto: Proto::HTTP20,
            request_url: SimpleUrl::url_only(&url),
            request_uri: uri,
            headers: grpc_headers("json"),
            method: SimpleMethod::POST,
        })
        .expect("open h2 stream");

    stream
        .send_body
        .try_send(Bytes::from(body))
        .expect("send request body");
    stream.send_body.close();

    let (status, _headers) = stream
        .head
        .next()
        .await
        .expect("response head")
        .expect("head ok");
    assert_eq!(status, Status::OK);

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let messages = parse_envelope_messages(&body_bytes);
    assert_eq!(messages.len(), 3, "server stream yields exactly 3 messages");

    let mut got = Vec::new();
    for m in &messages {
        let parsed: Msg = serde_json::from_slice(m).expect("decode message");
        got.push(parsed.text);
    }
    assert_eq!(got, vec!["items-0", "items-1", "items-2"]);

    shutdown.turn_on();
}

/// WHY: gRPC client-streaming: the request sends multiple envelopes; the
/// handler folds them into one response envelope.
#[valtron_test]
async fn grpc_client_stream_over_h2c_transport() {
    let (addr, shutdown) = start_h2_server();

    let transport = H2Transport::new();
    let url = format!("http://{addr}{CLIENT_STREAM}");
    let uri = foundation_core::url::Uri::parse(&url).expect("valid uri");

    // Send multiple enveloped request messages.
    let mut body = Vec::new();
    for text in ["a", "b", "c"] {
        let msg = serde_json::to_vec(&Msg::of(text)).expect("encode");
        body.extend_from_slice(&grpc_envelope(&msg));
    }

    let mut stream = transport
        .open(RequestDescriptor {
            proto: Proto::HTTP20,
            request_url: SimpleUrl::url_only(&url),
            request_uri: uri,
            headers: grpc_headers("json"),
            method: SimpleMethod::POST,
        })
        .expect("open h2 stream");

    stream
        .send_body
        .try_send(Bytes::from(body))
        .expect("send request body");
    stream.send_body.close();

    let (status, _headers) = stream
        .head
        .next()
        .await
        .expect("response head")
        .expect("head ok");
    assert_eq!(status, Status::OK);

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk.expect("body chunk"));
    }

    let messages = parse_envelope_messages(&body_bytes);
    assert_eq!(messages.len(), 1, "client stream folded into one response");

    let folded: Msg = serde_json::from_slice(&messages[0]).expect("decode");
    assert_eq!(folded, Msg::of("a+b+c"));

    shutdown.turn_on();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Typed-Client streaming tests (GrpcClient + reader/writer tasks)
// ═══════════════════════════════════════════════════════════════════════════════

/// Build a typed `Client` configured for gRPC over h2c with JSON codec.
fn grpc_client_for(addr: &str, procedure: &str) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_grpc().with_codec("json"),
    )
    .expect("build gRPC client")
}

/// WHY: gRPC server-streaming through the typed Client exercises the
/// `GrpcClient`'s reader/writer tasks (envelope/de-envelope) and the
/// `ServerStream` facade (typed decode).
///
/// WHAT: One request → 3 messages received via `stream.receive()`.
#[valtron_test]
async fn grpc_server_stream_via_typed_client() {
    let (addr, shutdown) = start_h2_server();

    let client = grpc_client_for(&addr, SERVER_STREAM);
    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("typed")))
        .await
        .expect("open server stream");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    assert_eq!(got, vec!["typed-0", "typed-1", "typed-2"]);

    shutdown.turn_on();
}

/// WHY: gRPC client-streaming through the typed Client: multiple enveloped
/// request messages → single enveloped response. The `GrpcClient` writer
/// task envelopes each outgoing message; the reader task de-envelopes the
/// response.
#[valtron_test]
async fn grpc_client_stream_via_typed_client() {
    let (addr, shutdown) = start_h2_server();

    let client = grpc_client_for(&addr, CLIENT_STREAM);
    let outbound = futures::stream::iter(vec![Msg::of("x"), Msg::of("y"), Msg::of("z")]);
    let resp = client
        .client_stream(ctx(), outbound)
        .await
        .expect("client stream");

    assert_eq!(resp.msg, Msg::of("x+y+z"));

    shutdown.turn_on();
}

/// WHY: gRPC bidi-streaming with interleaved send/receive — the most demanding
/// mode. Proves the reader/writer tasks in both `GrpcClient` and `GrpcHandler`
/// work concurrently without deadlocking.
#[valtron_test]
async fn grpc_bidi_stream_via_typed_client() {
    let (addr, shutdown) = start_h2_server();

    let client = grpc_client_for(&addr, BIDI);
    let mut stream = client
        .bidi_stream(ctx(), futures::stream::iter(Vec::<Msg>::new()))
        .await
        .expect("open bidi");

    for text in ["x", "y", "z"] {
        stream.send(&Msg::of(text)).await.expect("send");
        let echo = stream
            .receive()
            .await
            .expect("receive")
            .expect("an echo per message");
        assert_eq!(echo, Msg::of(format!("re:{text}")));
    }

    stream.close_request().await.expect("close request");
    assert!(
        stream.receive().await.expect("receive").is_none(),
        "stream ends once the request direction closes"
    );

    shutdown.turn_on();
}

/// WHY: gRPC error responses must still return HTTP 200 (status rides
/// `grpc-status` trailers, never the HTTP status line). This test verifies
/// the handler-level error is rendered correctly at the transport level.
#[valtron_test]
async fn grpc_handler_error_returns_http_200() {
    const ERR_PATH: &str = "/t.Svc/Error";
    let mut router = build_router();
    router.unary(
        ERR_PATH,
        codecs(),
        |_ctx: Ctx, _req: Request<Msg>| async move {
            Err(foundation_connectrpc::error::ConnectError::permission_denied("not allowed").into())
        },
        HandlerOptions::new(),
    );

    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(ERR_PATH, rpc);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(OnSignal::new());
    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    let addr_str = addr.to_string();

    let transport = H2Transport::new();
    let url = format!("http://{addr_str}{ERR_PATH}");
    let uri = foundation_core::url::Uri::parse(&url).expect("valid uri");

    let body = grpc_envelope(&[]);

    let mut stream = transport
        .open(RequestDescriptor {
            proto: Proto::HTTP20,
            request_url: SimpleUrl::url_only(&url),
            request_uri: uri,
            headers: grpc_headers("json"),
            method: SimpleMethod::POST,
        })
        .expect("open h2 stream");

    stream
        .send_body
        .try_send(Bytes::from(body))
        .expect("send body");
    stream.send_body.close();

    let (status, _headers) = stream
        .head
        .next()
        .await
        .expect("response head")
        .expect("head ok");

    // gRPC errors MUST return HTTP 200 — errors ride trailers, not status.
    assert_eq!(
        status,
        Status::OK,
        "gRPC error response must be 200 OK, got {status}"
    );

    shutdown.turn_on();
}

/// WHY: gRPC defines `grpc-timeout` header encoding (Decision 05 §Protocol 2):
/// duration → `<value><unit>`. The `GrpcHandler.parse_timeout()` parses this,
/// and the dispatch sets `ctx.with_deadline()` from it. Sending a call with
/// a reasonable timeout must not cause a spurious failure.
#[valtron_test]
async fn grpc_timeout_header_is_accepted() {
    let (addr, shutdown) = start_h2_server();

    let transport = H2Transport::new();
    let url = format!("http://{addr}{UNARY}");
    let uri = foundation_core::url::Uri::parse(&url).expect("valid uri");

    let msg = serde_json::to_vec(&Msg::of("timeout-test")).expect("encode");
    let body = grpc_envelope(&msg);

    let mut headers = grpc_headers("json");
    headers.insert(
        SimpleHeader::from("grpc-timeout".to_string()),
        vec!["1S".to_string()],
    );

    let mut stream = transport
        .open(RequestDescriptor {
            proto: Proto::HTTP20,
            request_url: SimpleUrl::url_only(&url),
            request_uri: uri,
            headers,
            method: SimpleMethod::POST,
        })
        .expect("open h2 stream");

    stream
        .send_body
        .try_send(Bytes::from(body))
        .expect("send body");
    stream.send_body.close();

    let (status, _headers) = stream
        .head
        .next()
        .await
        .expect("response head")
        .expect("head ok");
    assert_eq!(status, Status::OK, "call with grpc-timeout must succeed");

    shutdown.turn_on();
}
