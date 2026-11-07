//! Client response decompression (spec 73 feature 02, issue #259).
//!
//! WHY: before this change every client reader passed `None` as its
//! decompressor, and the unary decoder refused a compressed frame. A response
//! with the compressed flag set therefore failed even when the client accepted
//! the encoding. The client now reads the response head and picks the
//! decompressor from it.
//!
//! WHAT:
//! - gRPC streaming and unary responses compressed with gzip decode.
//! - Connect streaming responses compressed with gzip decode.
//! - An unregistered response encoding is a typed `Code::Unimplemented` error
//!   that names it.
//!
//! HOW: the server compresses its responses when the client advertises gzip in
//! the protocol's accept-encoding header. The client sends no compressed
//! request, so only the response path is under test.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bytes::Bytes;

use foundation_connectrpc::shared::context::{CancelSignal, IdempotencyLevel, Spec, StreamType};
use foundation_connectrpc::shared::error::ConnectResult;
use foundation_connectrpc::shared::protocol::grpc::GrpcClient;
use foundation_connectrpc::shared::protocol::{ClientExchangeConfig, ProtocolClient};
use foundation_connectrpc::shared::transport::{
    body_stream_from_pipe, head_stream_from_pipe, Transport, TransportStream,
};
use foundation_connectrpc::{
    Client, ClientOptions, Code, CompressionRegistry, ConnectRpcServeH2, Ctx, H2Transport,
    HandlerOptions, JsonCodec, ProcedureCodecs, Request, Response, Router,
};
use foundation_core::valtron::{valtron_test, Pipe};
use foundation_http::native::serve::H2Serve;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::ShutdownSignal;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders, Status};

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
const BIG_UNARY: &str = "/t.Svc/BigUnary";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

fn router() -> Router {
    let mut router = Router::new();
    router.unary(
        UNARY,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move {
            Ok(Response::new(Msg::of(format!("echo:{}", req.msg.text))))
        },
        HandlerOptions::new(),
    );
    router.unary(
        BIG_UNARY,
        codecs(),
        |_ctx: Ctx, _req: Request<Msg>| async move {
            Ok(Response::new(Msg::of("a".repeat(64 * 1024))))
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
    router
}

/// Start an HTTP/2 server with the unary, big-unary and server-streaming routes.
fn start_server() -> (String, Arc<ShutdownSignal>) {
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router().into_handler()));

    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(UNARY, rpc.clone());
    app.route_any_h2(BIG_UNARY, rpc.clone());
    app.route_any_h2(SERVER_STREAM, rpc);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(ShutdownSignal::new());
    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || server.serve_with_listener(&listener, &server_shutdown));

    (addr.to_string(), shutdown)
}

fn client(addr: &str, procedure: &str, grpc: bool, extra: &[(&str, &str)]) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    let mut options = ClientOptions::new().with_codec("json");
    if grpc {
        options = options.with_grpc();
    } else {
        options = options.with_connect();
    }
    for (key, value) in extra {
        options = options.with_header(key.to_string(), value.to_string());
    }
    Client::new(transport, &url, codecs(), options).expect("build client")
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

/// A gzip response frame on the gRPC streaming path decodes.
///
/// Fails on the old code: the gRPC reader passed `None`, so the compressed
/// frame failed with "compressed frame received but no decompressor was
/// negotiated".
#[valtron_test]
async fn a_compressed_grpc_response_is_decompressed() {
    let (addr, shutdown) = start_server();
    let client = client(
        &addr,
        SERVER_STREAM,
        true,
        &[("grpc-accept-encoding", "gzip")],
    );

    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("z")))
        .await
        .expect("open server stream");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    assert_eq!(got, vec!["z-0", "z-1", "z-2"]);

    shutdown.turn_on();
}

/// A gzip unary gRPC response decodes.
///
/// Fails on the old code: `decode_unary_response` returned an internal error
/// for any compressed frame.
#[valtron_test]
async fn a_compressed_unary_grpc_response_is_decompressed() {
    let (addr, shutdown) = start_server();
    let client = client(&addr, UNARY, true, &[("grpc-accept-encoding", "gzip")]);

    let response = client
        .unary(ctx(), Request::new(Msg::of("hi")))
        .await
        .expect("the compressed unary response must decode");
    assert_eq!(response.msg, Msg::of("echo:hi"));

    shutdown.turn_on();
}

/// A compressed unary response that inflates past `read_max_bytes` is
/// `ResourceExhausted`, not inflated without bound.
#[valtron_test]
async fn a_compressed_unary_response_over_the_limit_is_resource_exhausted() {
    let (addr, shutdown) = start_server();
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{BIG_UNARY}");
    let client = Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new()
            .with_grpc()
            .with_codec("json")
            .with_read_max_bytes(1024)
            .with_header("grpc-accept-encoding".to_string(), "gzip"),
    )
    .expect("build client");

    let err = client
        .unary(ctx(), Request::new(Msg::of("bomb")))
        .await
        .expect_err("a decompression bomb must be bounded");
    assert_eq!(
        err.current_context().code(),
        Code::ResourceExhausted,
        "expected ResourceExhausted, got: {err}"
    );

    shutdown.turn_on();
}

/// A gzip response frame on the Connect streaming path decodes.
///
/// Fails on the old code: the Connect reader passed `None`, so the compressed
/// frame failed with "compressed frame received but no decompressor was
/// negotiated".
#[valtron_test]
async fn a_compressed_connect_stream_response_is_decompressed() {
    let (addr, shutdown) = start_server();
    let client = client(
        &addr,
        SERVER_STREAM,
        false,
        &[("connect-accept-encoding", "gzip")],
    );

    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("c")))
        .await
        .expect("open server stream");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    assert_eq!(got, vec!["c-0", "c-1", "c-2"]);

    shutdown.turn_on();
}

/// An unregistered response encoding is `Code::Unimplemented` and names it.
///
/// Fails on the old code: the head's `grpc-encoding` was ignored, so the empty
/// body read as a clean end of stream instead of an error.
#[valtron_test(timeout = 5_000)]
async fn an_unknown_response_encoding_is_unimplemented() {
    let registry = CompressionRegistry::new();

    let (req_tx, _req_rx) = Pipe::<Bytes>::with_depth(8);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(8);
    // No response body: the encoding is rejected before any frame is read.
    drop(resp_tx);

    let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::with_depth(1);
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::from("content-type".to_string()),
        vec!["application/grpc+json".to_string()],
    );
    headers.insert(
        SimpleHeader::from("grpc-encoding".to_string()),
        vec!["snappy".to_string()],
    );
    head_tx
        .try_send((Status::OK, headers))
        .expect("response head");

    let exchange = GrpcClient
        .new_conn(
            &Spec {
                stream_type: StreamType::ServerStream,
                procedure: "t.Svc/Unary".to_string(),
                is_client: true,
                idempotency: IdempotencyLevel::Unknown,
            },
            SimpleHeaders::new(),
            TransportStream {
                send_body: Arc::new(req_tx),
                head: head_stream_from_pipe(head_rx),
                recv_body: body_stream_from_pipe(resp_rx),
                trailers: {
                    let (tx, rx) = Pipe::with_depth(1);
                    let _ = tx.close();
                    rx
                },
            },
            ClientExchangeConfig {
                cancel: CancelSignal::new(),
                read_max_bytes: 0,
                compression: &registry,
                send_compressor: None,
                compress_min_bytes: 0,
                send_max_bytes: 0,
            },
        )
        .expect("client exchange");

    let (send, mut recv) = exchange.conn.split();
    drop(send);
    // The request direction is not part of this test.
    drop(exchange.writer_task);
    let reader = exchange.reader_task;

    let (_reader_result, received) = futures::join!(reader, async { recv.receive().await });
    let err = received.expect_err("an unknown encoding must fail the call");
    assert_eq!(
        err.current_context().code(),
        Code::Unimplemented,
        "expected Unimplemented, got: {err}"
    );
    assert!(
        err.to_string().contains("snappy"),
        "the error must name the encoding, got: {err}"
    );
}
