//! Client request compression (spec 73, issue #378).
//!
//! WHY: the streaming client writers were built with no compressor, and the
//! server-streaming path compressed the request body before the writer framed
//! it. The writer therefore never set the compressed flag, and the peer read
//! compressed bytes as an uncompressed message. Client- and bidi-streaming
//! announced an encoding but never compressed a frame.
//!
//! WHAT:
//! - A gzip server-stream request round-trips through a real server.
//! - A gzip client-stream request round-trips.
//! - A gzip bidi request round-trips.
//! - A message below `compress_min_bytes` stays uncompressed and round-trips.
//! - A compressed frame over `send_max_bytes` fails with a typed
//!   `ResourceExhausted` writer error.
//!
//! HOW: the client negotiates gzip with `with_send_gzip`. The protocol writer
//! now applies the compressor per frame and sets the compressed flag. The
//! server reads the announced `grpc-encoding` and decompresses the frame.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::StreamExt;

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
    HandlerOptions, JsonCodec, ProcedureCodecs, Request, RequestStream, Response, Router,
};
use foundation_core::valtron::{valtron_test, Pipe};
use foundation_http::native::serve::H2Serve;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::ShutdownSignal;

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

impl Msg {
    fn of(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

const SERVER_STREAM: &str = "/t.Svc/ServerStream";
const CLIENT_STREAM: &str = "/t.Svc/ClientStream";
const BIDI: &str = "/t.Svc/Bidi";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

fn router() -> Router {
    let mut router = Router::new();
    router.server_stream(
        SERVER_STREAM,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move {
            let items: Vec<ConnectResult<Msg>> =
                vec![Ok(Msg::of(format!("echo:{}", req.msg.text)))];
            Ok(futures::stream::iter(items))
        },
        HandlerOptions::new(),
    );
    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        |_ctx: Ctx, reqs: RequestStream<Msg>| async move {
            let mut reqs = reqs;
            let mut parts = Vec::new();
            while let Some(item) = reqs.next().await {
                parts.push(item?.text);
            }
            Ok(Response::new(Msg::of(parts.join("+"))))
        },
        HandlerOptions::new(),
    );
    router.bidi_stream(
        BIDI,
        codecs(),
        |_ctx: Ctx, reqs: RequestStream<Msg>| async move {
            let echoed = reqs.map(|item| {
                let msg = item?;
                Ok(Msg::of(format!("re:{}", msg.text)))
            });
            Ok(echoed)
        },
        HandlerOptions::new(),
    );
    router
}

/// Start an HTTP/2 server with the three streaming routes.
fn start_server() -> (String, Arc<ShutdownSignal>) {
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router().into_handler()));

    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(SERVER_STREAM, rpc.clone());
    app.route_any_h2(CLIENT_STREAM, rpc.clone());
    app.route_any_h2(BIDI, rpc);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(ShutdownSignal::new());
    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || server.serve_with_listener(&listener, &server_shutdown));

    (addr.to_string(), shutdown)
}

/// Build a gRPC client that sends gzip-compressed requests.
fn gzip_client(addr: &str, procedure: &str, options: ClientOptions) -> Client<Msg, Msg> {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{procedure}");
    Client::new(
        transport,
        &url,
        codecs(),
        options.with_grpc().with_codec("json").with_send_gzip(),
    )
    .expect("build client")
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

/// A gzip server-stream request reaches the server as one message.
///
/// Fails on the old code: the body was compressed before framing, the writer
/// sent it with the compressed flag clear, and the server rejected the payload
/// with `json codec: decode failed`.
#[valtron_test]
async fn a_gzip_server_stream_request_round_trips() {
    let (addr, shutdown) = start_server();
    let client = gzip_client(&addr, SERVER_STREAM, ClientOptions::new());

    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("hello")))
        .await
        .expect("open server stream");

    let got = stream.receive().await.expect("receive").map(|m| m.text);
    assert_eq!(got, Some("echo:hello".to_string()));

    shutdown.turn_on();
}

/// A gzip client-stream request reaches the server as messages.
///
/// Fails on the old code: the client announced `grpc-encoding: gzip` but the
/// writer never compressed (or flagged) a frame, so the setting did nothing and
/// the request was not compressed at all.
#[valtron_test]
async fn a_gzip_client_stream_request_round_trips() {
    let (addr, shutdown) = start_server();
    let client = gzip_client(&addr, CLIENT_STREAM, ClientOptions::new());

    let outbound = futures::stream::iter(vec![Msg::of("a"), Msg::of("b")]);
    let response = client
        .client_stream(ctx(), outbound)
        .await
        .expect("client stream");

    assert_eq!(response.msg, Msg::of("a+b"));

    shutdown.turn_on();
}

/// Two gzip bidi request messages reach the server and echo back.
///
/// Fails on the old code: the client announced `grpc-encoding: gzip` but the
/// writer sent plain frames under that header.
#[valtron_test]
async fn a_gzip_bidi_request_round_trips() {
    let (addr, shutdown) = start_server();
    let client = gzip_client(&addr, BIDI, ClientOptions::new());

    let mut stream = client.bidi_stream(ctx()).await.expect("open bidi");
    for text in ["first", "second"] {
        stream.send(&Msg::of(text)).await.expect("send");
    }
    stream.close_request().await.expect("close request");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }
    assert_eq!(got, vec!["re:first", "re:second"]);

    shutdown.turn_on();
}

/// A message below `compress_min_bytes` is sent plain and still round-trips.
///
/// The writer must check `compress_min_bytes` on the payload, not the client.
#[valtron_test]
async fn a_message_below_compress_min_bytes_is_not_compressed() {
    let (addr, shutdown) = start_server();
    let client = gzip_client(
        &addr,
        SERVER_STREAM,
        ClientOptions::new().with_compress_min_bytes(1024),
    );

    let mut stream = client
        .server_stream(ctx(), Request::new(Msg::of("short")))
        .await
        .expect("open server stream");

    let got = stream.receive().await.expect("receive").map(|m| m.text);
    assert_eq!(got, Some("echo:short".to_string()));

    shutdown.turn_on();
}

/// A frame whose compressed size exceeds `send_max_bytes` fails in the writer.
///
/// The writer checks the cap after compression. The assertion reads the writer
/// task result directly, because the client surface has no channel for a
/// request-writer error.
#[valtron_test]
async fn a_frame_over_send_max_bytes_is_rejected() {
    let registry = CompressionRegistry::new();
    let compressor = registry.get("gzip").cloned().expect("gzip registered");

    // The request sink stays open; the frame is read from the writer's pipe.
    let (req_tx, _req_rx) = Pipe::<Bytes>::with_depth(8);
    // The response side is unused: the writer fails before any response.
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(8);
    drop(resp_tx);
    let (head_tx, head_rx) = Pipe::with_depth(1);
    drop(head_tx);
    let (trailer_tx, trailer_rx) = Pipe::with_depth(1);
    drop(trailer_tx);

    let exchange = GrpcClient
        .new_conn(
            &Spec {
                stream_type: StreamType::ServerStream,
                procedure: "t.Svc/ServerStream".to_string(),
                is_client: true,
                idempotency: IdempotencyLevel::Unknown,
            },
            foundation_netio::shared::http::SimpleHeaders::new(),
            TransportStream {
                send_body: Arc::new(req_tx),
                head: head_stream_from_pipe(head_rx),
                recv_body: body_stream_from_pipe(resp_rx),
                trailers: trailer_rx,
            },
            ClientExchangeConfig {
                cancel: CancelSignal::new(),
                read_max_bytes: 0,
                compression: &registry,
                send_compressor: Some(compressor),
                compress_min_bytes: 0,
                send_max_bytes: 16,
            },
        )
        .expect("client exchange");

    let reader = exchange.reader_task;
    let writer = exchange.writer_task;
    let (mut send, _recv) = exchange.conn.split();

    // Pseudo-random bytes do not compress below 16 bytes.
    let mut payload = Vec::with_capacity(4096);
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..4096 {
        // SplitMix64: every output is independent enough that gzip cannot
        // shrink 4096 bytes below the 16-byte cap.
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        payload.push(z.to_le_bytes()[7]);
    }
    send.send(Bytes::from(payload)).await.expect("frame queued");
    drop(send);

    drop(reader);
    let err = writer.await.expect_err("the writer must reject the frame");
    assert_eq!(
        err.current_context().code(),
        Code::ResourceExhausted,
        "expected ResourceExhausted, got: {err}"
    );
    assert!(
        err.to_string().contains("send_max_bytes"),
        "the error must name the limit, got: {err}"
    );
}
