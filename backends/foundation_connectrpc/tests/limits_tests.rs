//! Request size limits: decompression bombs, oversized envelopes, and the
//! default read limit.
//!
//! WHY: decompression ran with no output cap (`decompress(.., 0)`), streaming
//! readers never received the procedure's limit, and `read_max_bytes`
//! defaulted to unlimited — so one small request could claim any amount of
//! memory. These tests pin each cap.
//!
//! HOW: requests go through `ConnectRpcHandler::dispatch` in memory, driven by
//! `futures::executor::block_on` (no connection or valtron pool needed).

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use flate2::write::GzEncoder;
use flate2::Compression;
use futures::StreamExt;

use foundation_http::shared::context::ContextBag;
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse,
};

use foundation_connectrpc::{
    ConnectRpcHandler, Ctx, HandlerOptions, JsonCodec, ProcedureCodecs, Request, RequestStream,
    Response, Router, SizeLimits,
};

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

const UNARY: &str = "/t.Svc/Unary";
const CLIENT_STREAM: &str = "/t.Svc/ClientStream";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

fn handler(options: impl Fn() -> HandlerOptions) -> ConnectRpcHandler {
    let mut router = Router::new();
    router.unary(
        UNARY,
        codecs(),
        |_ctx: Ctx, req: Request<Msg>| async move { Ok(Response::new(req.msg)) },
        options(),
    );
    router.client_stream(
        CLIENT_STREAM,
        codecs(),
        |_ctx: Ctx, mut reqs: RequestStream<Msg>| async move {
            let mut count = 0;
            while let Some(msg) = reqs.next().await {
                msg?;
                count += 1;
            }
            Ok(Response::new(Msg {
                text: count.to_string(),
            }))
        },
        options(),
    );
    router.into_handler()
}

fn request(path: &str, headers: &[(&str, &str)], body: Vec<u8>) -> SimpleIncomingRequest {
    let mut builder = SimpleIncomingRequest::builder()
        .with_parsed_url(path)
        .with_proto(Proto::HTTP11)
        .with_method(SimpleMethod::POST)
        .with_body(SendSafeBody::Bytes(body));
    for (name, value) in headers {
        builder = builder.add_header_raw(SimpleHeader::from((*name).to_string()), *value);
    }
    builder.build().expect("request builds")
}

fn dispatch(handler: &ConnectRpcHandler, request: SimpleIncomingRequest) -> SimpleOutgoingResponse {
    futures::executor::block_on(handler.dispatch(Arc::new(ContextBag::new()), request))
}

fn body_text(response: &SimpleOutgoingResponse) -> String {
    match &response.body {
        Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(b).to_string(),
        Some(SendSafeBody::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// `len` bytes of JSON (a string of spaces) gzipped down to a few KiB.
fn gzip_bomb(len: usize) -> Vec<u8> {
    let mut json = b"{\"text\":\"".to_vec();
    json.resize(len, b' ');
    json.extend_from_slice(b"\"}");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&json).unwrap();
    encoder.finish().unwrap()
}

fn envelope(flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = vec![flags];
    buf.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

const LIMIT: usize = 64 * 1024;

fn limited() -> HandlerOptions {
    HandlerOptions::new().with_read_max_bytes(LIMIT)
}

/// A small compressed Connect unary body that inflates past the limit is
/// `resource_exhausted`, not inflated without bound.
#[test]
fn compressed_unary_body_over_the_limit_is_resource_exhausted() {
    let handler = handler(limited);
    let bomb = gzip_bomb(8 * 1024 * 1024);
    assert!(
        bomb.len() < LIMIT,
        "the compressed body itself is under the limit"
    );

    let response = dispatch(
        &handler,
        request(
            UNARY,
            &[
                ("content-type", "application/json"),
                ("content-encoding", "gzip"),
            ],
            bomb,
        ),
    );
    let body = body_text(&response);
    assert_eq!(response.status.clone().into_usize(), 429, "body: {body}");
    assert!(body.contains("resource_exhausted"), "body: {body}");
}

/// The same within the limit still decodes.
#[test]
fn compressed_unary_body_within_the_limit_decodes() {
    let handler = handler(limited);
    let response = dispatch(
        &handler,
        request(
            UNARY,
            &[
                ("content-type", "application/json"),
                ("content-encoding", "gzip"),
            ],
            gzip_bomb(1024),
        ),
    );
    assert_eq!(
        response.status.clone().into_usize(),
        200,
        "body: {}",
        body_text(&response)
    );
}

/// A compressed gRPC-Web unary envelope is capped the same way.
#[test]
fn compressed_grpc_web_unary_over_the_limit_is_resource_exhausted() {
    let handler = handler(limited);
    let response = dispatch(
        &handler,
        request(
            UNARY,
            &[
                ("content-type", "application/grpc-web+json"),
                ("grpc-encoding", "gzip"),
            ],
            envelope(1, &gzip_bomb(8 * 1024 * 1024)),
        ),
    );
    // gRPC-Web reports the status in its trailers.
    let text = format!("{:?} {}", response.headers, body_text(&response));
    assert!(text.contains("grpc-status: 8"), "response: {text}");
}

/// A compressed streaming envelope is capped too.
#[test]
fn compressed_streaming_envelope_over_the_limit_is_resource_exhausted() {
    let handler = handler(limited);
    let response = dispatch(
        &handler,
        request(
            CLIENT_STREAM,
            &[
                ("content-type", "application/connect+json"),
                ("connect-content-encoding", "gzip"),
            ],
            envelope(1, &gzip_bomb(8 * 1024 * 1024)),
        ),
    );
    let body = body_text(&response);
    assert!(body.contains("resource_exhausted"), "body: {body}");
}

/// An envelope whose length prefix claims 4 GiB is rejected from its 5-byte
/// header, before any payload is waited for or buffered.
#[test]
fn oversized_envelope_prefix_is_rejected_from_its_header() {
    let handler = handler(limited);
    let start = Instant::now();
    let response = dispatch(
        &handler,
        request(
            CLIENT_STREAM,
            &[("content-type", "application/connect+json")],
            vec![0, 0xFF, 0xFF, 0xFF, 0xFF],
        ),
    );
    let body = body_text(&response);
    assert!(body.contains("resource_exhausted"), "body: {body}");
    assert!(body.contains("exceeds read_max_bytes"), "body: {body}");
    assert!(start.elapsed() < Duration::from_secs(1));
}

/// `read_max_bytes` has a 4 MiB default, and it applies to a plain unary body.
#[test]
fn the_default_read_limit_applies() {
    assert_eq!(SizeLimits::default().read_max_bytes, 4 * 1024 * 1024);
    assert_eq!(HandlerOptions::new().limits.read_max_bytes, 4 * 1024 * 1024);
    assert_eq!(
        foundation_connectrpc::shared::client::SizeLimits::default().read_max_bytes,
        4 * 1024 * 1024
    );

    let handler = handler(HandlerOptions::new);
    let mut json = b"{\"text\":\"".to_vec();
    json.resize(5 * 1024 * 1024, b' ');
    json.extend_from_slice(b"\"}");
    let response = dispatch(
        &handler,
        request(UNARY, &[("content-type", "application/json")], json),
    );
    assert_eq!(response.status.clone().into_usize(), 413);

    // It caps each streaming envelope as well.
    let response = dispatch(
        &handler,
        request(
            CLIENT_STREAM,
            &[("content-type", "application/connect+json")],
            vec![0, 0xFF, 0xFF, 0xFF, 0xFF],
        ),
    );
    assert!(body_text(&response).contains("resource_exhausted"));

    // `0` still opts out.
    let handler = handler_unlimited();
    let mut json = b"{\"text\":\"".to_vec();
    json.resize(5 * 1024 * 1024, b' ');
    json.extend_from_slice(b"\"}");
    let response = dispatch(
        &handler,
        request(UNARY, &[("content-type", "application/json")], json),
    );
    assert_eq!(response.status.clone().into_usize(), 200);
}

fn handler_unlimited() -> ConnectRpcHandler {
    handler(|| HandlerOptions::new().with_read_max_bytes(0))
}
