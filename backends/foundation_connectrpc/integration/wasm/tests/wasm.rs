// A capability constant is compared on purpose: the case asserts the value the
// module exports.
#![allow(clippy::assertions_on_constants)]

//! Wasm cases for foundation_connectrpc (specs 85 and 88 feature 03).
//!
//! WHY: the router, the response sink, the error model, the deadline and the
//! HTTP/1.1 client are the request path of the product, and no case ran them on
//! a wasm host. The crate's own test targets cannot build for wasm32 (issue
//! #576), so this standalone crate runs them in the wasm testbed.
//!
//! WHAT: the spec 85 streaming-sink cases (`inbound_body_streams`,
//! `error_status_streams`, `whole_response_keeps_status`, `cancel_drops_source`,
//! `capability_constants`) and the spec 88 feature 03 cases 1 to 17.
//!
//! HOW: the dispatch cases drive the real `Router` in memory; the transport
//! cases build `H1Transport` over the host's real fetch and call the testbed's
//! `/connect` route on the same origin. Every case names its oracle in a
//! comment (rule R2) and its failure names the case and the first unexpected
//! value (rule R3).

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use foundation_connectrpc::shared::client::{Client, ClientOptions};
use foundation_connectrpc::shared::codec::ProcedureCodecs;
use foundation_connectrpc::shared::protocol::connect::constants as connect_consts;
use foundation_connectrpc::shared::transport::h1::H1Transport;
use foundation_connectrpc::shared::transport::wasm::WASM_FETCH_CAPABILITIES;
use foundation_connectrpc::{
    CallHooks, Code, ConnectError, ConnectResult, ConnectSink, HandlerOptions, Request,
    Response as RpcResponse, ResponseSink, Router, RpcCtx, Transport,
    request_body_from_stream,
};
use foundation_connectrpc_wasm_tests::{
    Call, IdentityCodec, RecordingSink, SimulatedTimer, body, ctx, enveloped, host,
    parse_envelopes, remaining_ms, request, response_body, rpc_base_url, simulated_ctx,
};
use foundation_core::valtron::Stream;
use foundation_http::wasm::bridge::cf::request_from_cf;
use foundation_http::wasm::cf_conn::CfConn;
use foundation_netio::HttpClientBuilder;
use futures::StreamExt;
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse,
    Status,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::{JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use wasm_bindgen_test::wasm_bindgen_test;
use web_sys::{
    ReadableStream, ReadableStreamDefaultController, ReadableStreamDefaultReader, RequestInit,
    Response, UnderlyingSource,
};

async fn yield_now() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .expect("setTimeout")
            .dyn_into::<js_sys::Function>()
            .expect("setTimeout is a function");
        set_timeout
            .call2(&global, &resolve, &JsValue::from_f64(0.0))
            .expect("setTimeout");
    });
    let _ = JsFuture::from(promise).await;
}

/// A JS `ReadableStream` that enqueues one chunk on each pull, up to `total`,
/// and counts its pulls.
fn counting_stream(total: u32, size: usize) -> (ReadableStream, Rc<Cell<u32>>) {
    let pulls = Rc::new(Cell::new(0u32));
    let sent = Rc::new(Cell::new(0u32));

    let pull = {
        let pulls = Rc::clone(&pulls);
        let sent = Rc::clone(&sent);
        Closure::<dyn FnMut(JsValue)>::new(move |controller: JsValue| {
            let controller: ReadableStreamDefaultController = controller.unchecked_into();
            pulls.set(pulls.get() + 1);
            let n = sent.get();
            if n >= total {
                let _ = controller.close();
                return;
            }
            sent.set(n + 1);
            let chunk = js_sys::Uint8Array::new_with_length(u32::try_from(size).unwrap_or(1));
            chunk.fill(7, 0, u32::try_from(size).unwrap_or(1));
            let _ = controller.enqueue_with_chunk(&chunk);
        })
    };
    let cancel = Closure::<dyn FnMut(JsValue)>::new(|_reason: JsValue| {});

    let underlying = UnderlyingSource::new();
    underlying.set_pull(pull.into_js_value().unchecked_ref::<js_sys::Function>());
    underlying.set_cancel(cancel.into_js_value().unchecked_ref::<js_sys::Function>());
    (
        ReadableStream::new_with_underlying_source(&underlying).expect("stream"),
        pulls,
    )
}

#[wasm_bindgen_test]
async fn inbound_body_streams() {
    // A 1000-chunk request body. Reading one chunk must not read the rest.
    let (stream, pulls) = counting_stream(1000, 16);
    let init = RequestInit::new();
    init.set_method("POST");
    // A browser requires `duplex: "half"` for a streaming request body
    // (measured on Chromium 157, spec 88 feature 03).
    let _ = js_sys::Reflect::set(
        init.as_ref(),
        &JsValue::from_str("duplex"),
        &JsValue::from_str("half"),
    );
    init.set_body(&JsValue::from(stream));
    let request =
        web_sys::Request::new_with_str_and_init("http://example.com/rpc", &init).expect("request");

    let incoming = request_from_cf(&request, 8 * 1024 * 1024, 16).expect("request_from_cf");
    let body = incoming.body.expect("the body is a stream");
    assert!(
        matches!(body, SendSafeBody::Stream(_)),
        "the inbound body must be a stream, not buffered text"
    );

    let mut iter = SendSafeBodyBytesIterator::new(body);
    let mut first = None;
    for _ in 0..200 {
        match iter.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(chunk))) => {
                first = Some(chunk);
                break;
            }
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(error))) => {
                panic!("body stream failed: {error}")
            }
            Some(_) => yield_now().await,
            None => break,
        }
    }
    assert!(first.is_some(), "the handler reads one chunk");
    // The reader may fill its bounded queue (default 16), but it must not read
    // the whole 1000-chunk body for one chunk.
    assert!(
        pulls.get() < 100,
        "one chunk must not read the whole body: {} pulls",
        pulls.get()
    );
}

fn head(status: Status) -> SimpleOutgoingResponse {
    SimpleOutgoingResponse::builder()
        .with_proto(Proto::HTTP11)
        .with_status(status)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/connect+proto")
        .build()
        .expect("valid response head")
}

async fn read_one_chunk(response: &Response) -> Vec<u8> {
    let stream = response.body().expect("a streaming body");
    let reader: ReadableStreamDefaultReader = stream
        .get_reader()
        .unchecked_into::<ReadableStreamDefaultReader>();
    let result = JsFuture::from(reader.read()).await.expect("read");
    let value = js_sys::Reflect::get(&result, &JsValue::from_str("value")).expect("value");
    js_sys::Uint8Array::new(&value).to_vec()
}

#[wasm_bindgen_test]
async fn error_status_streams() {
    let (mut sink, handle) = ConnectSink::new_pair();
    sink.open(head(Status::ServiceUnavailable))
        .await
        .expect("open");
    sink.chunk(Bytes::from_static(b"frame-one"))
        .await
        .expect("chunk");
    sink.finish().await.expect("finish");

    let response = handle.into_response().expect("a response");
    assert_eq!(response.status(), 503, "the sink keeps the error status");
    assert_eq!(read_one_chunk(&response).await, b"frame-one");
}

#[wasm_bindgen_test]
async fn whole_response_keeps_status() {
    let (mut sink, handle) = ConnectSink::new_pair();
    let response = SimpleOutgoingResponse::builder()
        .with_proto(Proto::HTTP11)
        .with_status(Status::ServiceUnavailable)
        .with_body_bytes(b"unavailable".to_vec())
        .build()
        .expect("response");
    sink.whole(response).await.expect("whole");

    let response = handle.into_response().expect("a response");
    assert_eq!(response.status(), 503);
    assert_eq!(read_one_chunk(&response).await, b"unavailable");
}

#[wasm_bindgen_test]
async fn cancel_drops_source() {
    let (mut sink, handle) = ConnectSink::new_pair();
    sink.open(head(Status::OK)).await.expect("open");
    sink.chunk(Bytes::from_static(b"first"))
        .await
        .expect("chunk");

    // The client goes away: cancelling the response body drops the source.
    let response = handle.into_response().expect("a response");
    let stream = response.body().expect("a streaming body");
    let reader: ReadableStreamDefaultReader = stream
        .get_reader()
        .unchecked_into::<ReadableStreamDefaultReader>();
    JsFuture::from(reader.cancel()).await.expect("cancel");

    // The next frame cannot be delivered: the source is gone.
    let result = sink.chunk(Bytes::from_static(b"second")).await;
    assert!(
        result.is_err(),
        "a cancelled response must end the call, not accept a frame"
    );
}

#[wasm_bindgen_test]
fn capability_constants() {
    assert!(
        WASM_FETCH_CAPABILITIES.request_streaming,
        "fetch streams a request body"
    );
    assert!(!WASM_FETCH_CAPABILITIES.full_duplex);
    assert!(!WASM_FETCH_CAPABILITIES.h2_trailers);
    assert!(!WASM_FETCH_CAPABILITIES.multiplexed);
}

// ══════════════════════════════════════════════════════════════════════════
// Spec 88 feature 03: the router, the error model, the sink and the deadline
// ══════════════════════════════════════════════════════════════════════════

const UNARY: &str = "http://t/case.CaseService/Unary";
const STREAM: &str = "http://t/case.CaseService/Stream";
const FAIL: &str = "http://t/case.CaseService/Fail";
const WAIT: &str = "http://t/case.CaseService/Wait";
const SECOND: &str = "http://t/case.SecondService/Unary";

/// The three frames every server-stream case emits.
const FRAMES: [&[u8]; 3] = [b"frame-0", b"frame-1", b"frame-2"];

/// A unary procedure that prefixes the request with `echo:` and counts its
/// calls. Oracle: the round-trip case compares the body, the missing-procedure
/// case compares the count.
fn echo_router(calls: Arc<AtomicUsize>) -> Router {
    let mut router = Router::new();
    router.unary(
        UNARY,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, req: Request<Vec<u8>>| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move {
                let mut out = b"echo:".to_vec();
                out.extend_from_slice(&req.msg);
                Ok(RpcResponse::new(out))
            }
        },
        HandlerOptions::new(),
    );
    router
}

/// A server-stream procedure that emits the three fixed frames and counts how
/// many frames the handler produced.
fn stream_router(produced: Arc<AtomicUsize>) -> Router {
    let mut router = Router::new();
    router.server_stream(
        STREAM,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, _req: Request<Vec<u8>>| {
            let produced = produced.clone();
            async move {
                let frames: Vec<ConnectResult<Vec<u8>>> = FRAMES
                    .iter()
                    .map(|frame| {
                        produced.fetch_add(1, Ordering::SeqCst);
                        Ok(frame.to_vec())
                    })
                    .collect();
                Ok(futures::stream::iter(frames))
            }
        },
        HandlerOptions::new(),
    );
    router
}

/// A unary procedure that returns `invalid_argument` (case 4).
fn fail_router() -> Router {
    let mut router = Router::new();
    router.unary(
        FAIL,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |_ctx, _req: Request<Vec<u8>>| async move {
            Err(ConnectError::invalid_argument("the handler rejected the call").into())
        },
        HandlerOptions::new(),
    );
    router
}

/// A unary procedure that never returns (the deadline cases).
fn wait_router() -> Router {
    let mut router = Router::new();
    router.unary(
        WAIT,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |_ctx, _req: Request<Vec<u8>>| async move {
            std::future::pending::<()>().await;
            unreachable!("the waiting handler never returns")
        },
        HandlerOptions::new(),
    );
    router
}

/// A request with one JSON-unary body and the default `application/proto` type.
fn unary_request(url: &str, body: &[u8]) -> SimpleIncomingRequest {
    request(url, SimpleMethod::POST, "application/proto", body.to_vec())
}

/// A streaming request with envelopes and the Connect streaming content type.
fn stream_request(url: &str, body: Vec<u8>) -> SimpleIncomingRequest {
    request(url, SimpleMethod::POST, "application/connect+proto", body)
}

/// A request with a call-timeout header.
fn request_with_timeout(url: &str, body: &[u8], millis: u64) -> SimpleIncomingRequest {
    let mut req = unary_request(url, body);
    req.headers.insert(
        SimpleHeader::from(connect_consts::HEADER_TIMEOUT.to_string()),
        vec![millis.to_string()],
    );
    req
}

fn calls_snapshot(calls: &Arc<Mutex<Vec<Call>>>) -> Vec<Call> {
    calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

fn call_kinds(calls: &[Call]) -> Vec<&'static str> {
    calls.iter().map(Call::kind).collect()
}

// ── case 1 ─────────────────────────────────────────────────────────────────

/// Case 1, oracle example: a unary call returns 200, the Connect content type
/// and the handler's message.
#[wasm_bindgen_test]
async fn unary_round_trip_through_the_router() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls.clone()).into_handler();
    let response = handler.dispatch(ctx(), unary_request(UNARY, b"ping")).await;

    assert_eq!(response.status, Status::OK, "case 1: status");
    let content_type = response
        .headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|values| values.first())
        .cloned();
    assert_eq!(
        content_type.as_deref(),
        Some("application/proto"),
        "case 1: content type"
    );
    assert_eq!(
        response_body(&response),
        b"echo:ping".to_vec(),
        "case 1: the handler's message"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "case 1: the handler ran once"
    );
}

// ── case 2 ─────────────────────────────────────────────────────────────────

/// Case 2, oracle expected outcome plus counter: a procedure the router does
/// not have answers the path-miss status (404) and runs no handler.
///
/// Decision 03 maps a non-Connect 404 to `unimplemented`; that is the code a
/// Connect client reports for this answer. The router's own dispatch flow
/// documents 404 for a path miss (`shared/router/dispatch.rs`, step 1). Issue
/// #755 records that the Connect client's fallback maps it to `NotFound`.
#[wasm_bindgen_test]
async fn unknown_service_returns_unimplemented() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls.clone()).into_handler();
    let response = handler
        .dispatch(ctx(), unary_request("http://t/missing.Procedure", b"ping"))
        .await;

    assert_eq!(
        response.status,
        Status::NotFound,
        "case 2: the path-miss status"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "case 2: a missing procedure runs no handler"
    );
    assert_eq!(
        Code::from_http_status(404),
        Code::Unimplemented,
        "case 2: Decision 03 maps a non-Connect 404 to unimplemented"
    );
}

// ── case 3 ─────────────────────────────────────────────────────────────────

/// Case 3, oracle example measured first: a wrong method answers 405 and an
/// `Allow` header that names POST.
#[wasm_bindgen_test]
async fn wrong_method_reports_the_documented_status() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls).into_handler();
    let response = handler
        .dispatch(
            ctx(),
            request(UNARY, SimpleMethod::PUT, "application/proto", Vec::new()),
        )
        .await;

    assert_eq!(
        response.status,
        Status::MethodNotAllowed,
        "case 3: a wrong method answers 405"
    );
    let allow = response
        .headers
        .get(&SimpleHeader::from("allow".to_string()))
        .and_then(|values| values.first())
        .cloned()
        .unwrap_or_default();
    assert!(
        allow.contains("POST"),
        "case 3: Allow names POST, got {allow:?}"
    );
}

// ── case 4 ─────────────────────────────────────────────────────────────────

/// Case 4, oracle example: a handler error keeps its code and message in the
/// Connect unary error body.
#[wasm_bindgen_test]
async fn handler_error_keeps_its_code_and_message() {
    let handler = fail_router().into_handler();
    let response = handler.dispatch(ctx(), unary_request(FAIL, b"ping")).await;

    assert_eq!(
        response.status,
        Status::BadRequest,
        "case 4: invalid_argument maps to HTTP 400"
    );
    let wire: serde_json::Value =
        serde_json::from_slice(&response_body(&response)).expect("case 4: a JSON error body");
    assert_eq!(
        wire.get("code").and_then(serde_json::Value::as_str),
        Some("invalid_argument"),
        "case 4: the code: {wire}"
    );
    assert_eq!(
        wire.get("message").and_then(serde_json::Value::as_str),
        Some("the handler rejected the call"),
        "case 4: the message: {wire}"
    );
}

// ── case 5 ─────────────────────────────────────────────────────────────────

/// Case 5, oracle model: the sink receives open, the three frames and the
/// end-stream frame, then finish; the frames keep their order.
///
/// The Connect protocol ends a stream with its own `0x02` EndStream frame, so
/// the sink sees four chunks: three messages and the end-stream frame.
#[wasm_bindgen_test]
async fn server_stream_frames_arrive_in_order() {
    let produced = Arc::new(AtomicUsize::new(0));
    let handler = stream_router(produced).into_handler();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(calls.clone());
    let req = stream_request(STREAM, enveloped(b"seed"));

    handler
        .dispatch_to(
            ctx(),
            req,
            body(enveloped(b"seed")),
            Box::new(sink),
            CallHooks::default(),
        )
        .await
        .expect("case 5: the sink accepts every frame");

    let calls = calls_snapshot(&calls);
    assert_eq!(
        calls.first().map(Call::kind),
        Some("open"),
        "case 5: the first call is open; got {:?}",
        call_kinds(&calls)
    );
    assert_eq!(
        calls.last().map(Call::kind),
        Some("finish"),
        "case 5: the last call is finish; got {:?}",
        call_kinds(&calls)
    );

    let mut chunks = Vec::new();
    for call in &calls {
        if let Call::Chunk(bytes) = call {
            chunks.extend_from_slice(bytes);
        }
    }
    let frames = parse_envelopes(&chunks);
    assert_eq!(frames.len(), 4, "case 5: three messages and one end-stream");
    for (index, expected) in FRAMES.iter().enumerate() {
        assert_eq!(frames[index].0, 0x00, "case 5: frame {index} is a message");
        assert_eq!(
            frames[index].1.as_slice(),
            *expected,
            "case 5: frame {index} in order"
        );
    }
    assert_eq!(frames[3].0, 0x02, "case 5: the last frame ends the stream");
}

// ── case 6 ─────────────────────────────────────────────────────────────────

/// Case 6, oracle fault: a sink that fails stops the call, dispatch returns
/// the error, and the handler produces no fourth frame.
#[wasm_bindgen_test]
async fn server_stream_stops_when_the_sink_fails() {
    let produced = Arc::new(AtomicUsize::new(0));
    let handler = stream_router(produced.clone()).into_handler();
    let calls = Arc::new(Mutex::new(Vec::new()));
    // Call 1 is open, call 2 the first chunk, call 3 the second chunk.
    let sink = RecordingSink::failing_on(calls.clone(), 3);
    let req = stream_request(STREAM, enveloped(b"seed"));

    let outcome = handler
        .dispatch_to(
            ctx(),
            req,
            body(enveloped(b"seed")),
            Box::new(sink),
            CallHooks::default(),
        )
        .await;

    assert!(
        outcome.is_err(),
        "case 6: dispatch returns the sink's error"
    );
    let calls = calls_snapshot(&calls);
    assert_eq!(
        call_kinds(&calls),
        vec!["open", "chunk"],
        "case 6: the sink failed on its third call and recorded two"
    );
    assert!(
        produced.load(Ordering::SeqCst) < 4,
        "case 6: the handler produced no fourth frame"
    );
}

// ── case 7 ─────────────────────────────────────────────────────────────────

/// Case 7, oracle model: a unary response uses `whole` and never `open`.
#[wasm_bindgen_test]
async fn unary_uses_whole_and_never_open() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls).into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());

    handler
        .dispatch_to(
            ctx(),
            unary_request(UNARY, b"ping"),
            body(b"ping".to_vec()),
            Box::new(sink),
            CallHooks::default(),
        )
        .await
        .expect("case 7: the sink accepts the response");

    let calls = calls_snapshot(&recorded);
    assert_eq!(
        call_kinds(&calls),
        vec!["whole"],
        "case 7: a unary response is one whole call"
    );
    match &calls[0] {
        Call::Whole { status, body } => {
            assert_eq!(*status, Some(Status::OK), "case 7: status");
            assert_eq!(body.as_slice(), b"echo:ping", "case 7: body");
        }
        other => panic!("case 7: expected whole, got {other:?}"),
    }
}

// ── case 8 ─────────────────────────────────────────────────────────────────

/// Case 8, oracle example: three request frames reach a client-stream handler
/// in order.
#[wasm_bindgen_test]
async fn client_stream_reads_every_frame() {
    use futures::StreamExt as _;

    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_by_handler = seen.clone();
    let mut router = Router::new();
    router.client_stream(
        "http://t/case.CaseService/Collect",
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, mut reqs: foundation_connectrpc::RequestStream<Vec<u8>>| {
            let seen = seen_by_handler.clone();
            async move {
                let mut count = 0usize;
                while let Some(item) = reqs.next().await {
                    seen.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(item?);
                    count += 1;
                }
                Ok(RpcResponse::new(format!("count:{count}").into_bytes()))
            }
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();

    let mut payload = Vec::new();
    for frame in [b"one".as_slice(), b"two", b"three"] {
        payload.extend_from_slice(&enveloped(frame));
    }
    let req = stream_request("http://t/case.CaseService/Collect", payload.clone());
    let response = handler.dispatch(ctx(), req).await;

    assert_eq!(response.status, Status::OK, "case 8: status");
    let seen = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        seen,
        vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()],
        "case 8: the handler reads the three frames in order"
    );
}

// ── case 9 ─────────────────────────────────────────────────────────────────

/// Case 9, oracle example: a bidi handler echoes the three request frames in
/// request order. The request claims HTTP/2, because full duplex needs it.
#[wasm_bindgen_test]
async fn bidi_echoes_in_order() {
    use futures::StreamExt as _;

    let mut router = Router::new();
    router.bidi_stream(
        "http://t/case.CaseService/Chat",
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |_ctx, mut reqs: foundation_connectrpc::RequestStream<Vec<u8>>| async move {
            let mut echoes = Vec::new();
            while let Some(item) = reqs.next().await {
                let message = item?;
                let mut echo = b"echo:".to_vec();
                echo.extend_from_slice(&message);
                echoes.push(Ok(echo));
            }
            Ok(futures::stream::iter(echoes))
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();

    let mut payload = Vec::new();
    for frame in [b"one".as_slice(), b"two", b"three"] {
        payload.extend_from_slice(&enveloped(frame));
    }
    let mut req = stream_request("http://t/case.CaseService/Chat", payload);
    req.proto = Proto::HTTP20;
    let listener = handler.dispatch(ctx(), req).await;

    // `dispatch` collects the stream into the response body. A bidi call over
    // HTTP/2 is a real stream, so the body holds the Connect frames.
    assert_eq!(listener.status, Status::OK, "case 9: status");
    let frames = parse_envelopes(&response_body(&listener));
    let messages: Vec<&[u8]> = frames
        .iter()
        .filter(|(flags, _)| *flags == 0x00)
        .map(|(_, payload)| payload.as_slice())
        .collect();
    assert_eq!(
        messages,
        vec![b"echo:one".as_slice(), b"echo:two", b"echo:three"],
        "case 9: the echoes keep request order"
    );
}

// ── case 10 ────────────────────────────────────────────────────────────────

/// Case 10, oracle example: two services share one router, and each procedure
/// reaches its own handler.
#[wasm_bindgen_test]
async fn two_services_keep_their_procedures_apart() {
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));

    let mut router = echo_router(first.clone());
    let second_calls = second.clone();
    router.unary(
        SECOND,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, _req: Request<Vec<u8>>| {
            second_calls.fetch_add(1, Ordering::SeqCst);
            async move { Ok(RpcResponse::new(b"second".to_vec())) }
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();

    let one = handler.dispatch(ctx(), unary_request(UNARY, b"a")).await;
    let two = handler.dispatch(ctx(), unary_request(SECOND, b"b")).await;

    assert_eq!(response_body(&one), b"echo:a".to_vec(), "case 10: first");
    assert_eq!(response_body(&two), b"second".to_vec(), "case 10: second");
    assert_eq!(first.load(Ordering::SeqCst), 1, "case 10: first count");
    assert_eq!(second.load(Ordering::SeqCst), 1, "case 10: second count");
}

// ── case 11 ────────────────────────────────────────────────────────────────

/// Case 11, oracle expected outcome: a deadline that has passed ends the call
/// with `deadline_exceeded`, on the injected clock.
///
/// A zero `connect-timeout-ms` makes the deadline equal the current reading of
/// the simulated clock, so the timer resolves on its first poll.
#[wasm_bindgen_test]
async fn past_deadline_ends_the_call() {
    let handler = wait_router().into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());
    let (app_ctx, _clock) = simulated_ctx();
    let req = request_with_timeout(WAIT, b"slow", 0);

    handler
        .dispatch_to(
            app_ctx,
            req,
            body(b"slow".to_vec()),
            Box::new(sink),
            CallHooks::default().with_timer(&SimulatedTimer),
        )
        .await
        .expect("case 11: the deadline answers through the sink");

    let calls = calls_snapshot(&recorded);
    assert_eq!(
        call_kinds(&calls),
        vec!["whole"],
        "case 11: one complete error response"
    );
    match &calls[0] {
        Call::Whole { status, body } => {
            assert_eq!(
                *status,
                Some(Status::GatewayTimeout),
                "case 11: DeadlineExceeded maps to 504"
            );
            let wire: serde_json::Value =
                serde_json::from_slice(body).expect("case 11: a JSON error body");
            assert_eq!(
                wire.get("code").and_then(serde_json::Value::as_str),
                Some("deadline_exceeded"),
                "case 11: the code: {wire}"
            );
        }
        other => panic!("case 11: expected whole, got {other:?}"),
    }
}

// ── case 12 ────────────────────────────────────────────────────────────────

/// Case 12, oracle example: a future deadline lets a handler that answers
/// finish normally.
#[wasm_bindgen_test]
async fn future_deadline_lets_the_handler_finish() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls).into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());
    let (app_ctx, _clock) = simulated_ctx();
    let req = request_with_timeout(UNARY, b"fast", 60_000);

    handler
        .dispatch_to(
            app_ctx,
            req,
            body(b"fast".to_vec()),
            Box::new(sink),
            CallHooks::default().with_timer(&SimulatedTimer),
        )
        .await
        .expect("case 12: the handler answers before the deadline");

    let calls = calls_snapshot(&recorded);
    match &calls[0] {
        Call::Whole { status, body } => {
            assert_eq!(*status, Some(Status::OK), "case 12: status");
            assert_eq!(
                body.as_slice(),
                b"echo:fast",
                "case 12: the handler's message"
            );
        }
        other => panic!("case 12: expected whole, got {other:?}"),
    }
}

// ── case 13 ────────────────────────────────────────────────────────────────

/// Case 13, oracle example: the handler reads the remaining time from the
/// deadline the injected clock measured.
#[wasm_bindgen_test]
async fn handler_sees_the_remaining_deadline() {
    let mut router = Router::new();
    router.unary(
        WAIT,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |call, _req: Request<Vec<u8>>| async move {
            let left = remaining_ms(&call).unwrap_or_default();
            Ok(RpcResponse::new(left.to_string().into_bytes()))
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());
    let (app_ctx, _clock) = simulated_ctx();
    let req = request_with_timeout(WAIT, b"x", 5_000);

    handler
        .dispatch_to(
            app_ctx,
            req,
            body(b"x".to_vec()),
            Box::new(sink),
            CallHooks::default().with_timer(&SimulatedTimer),
        )
        .await
        .expect("case 13: the handler answers");

    let calls = calls_snapshot(&recorded);
    match &calls[0] {
        Call::Whole { body, .. } => {
            let left = String::from_utf8_lossy(body);
            assert_eq!(left, "5000", "case 13: the handler reads the 5 s timeout");
        }
        other => panic!("case 13: expected whole, got {other:?}"),
    }
}

// ── cases 14 to 16: the transport ──────────────────────────────────────────

/// The Connect base URL the host serves, or a skip that names the reason.
///
/// Every wasm host serves the endpoint (issue #752): the browser loads from the
/// Rust testbed server, the Deno template answers `/connect` from its own
/// server, and the worker answers it from its own fetch handler. A host that
/// sets no base URL skips with the reason instead of failing.
fn transport_base() -> Option<String> {
    let base = rpc_base_url();
    if base.is_none() {
        web_sys::console::log_1(&JsValue::from_str(&format!(
            "skipped: the {:?} host set no Connect base URL (__FWT_RPC_BASE_URL__)",
            host()
        )));
    }
    base
}

fn build_client(base: &str, procedure: &str) -> Client<Vec<u8>, Vec<u8>> {
    let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);
    Client::new(
        Arc::new(H1Transport::new(HttpClientBuilder::new().build())),
        &format!("{base}/connect/{procedure}"),
        codecs,
        ClientOptions::default(),
    )
    .expect("case: the client builds")
}

fn transport_ctx() -> RpcCtx {
    RpcCtx::client(ctx())
}

/// Case 14, oracle example: `H1Transport` reaches the testbed's `/connect`
/// route over the host's real fetch and decodes the handler's message.
#[wasm_bindgen_test]
async fn h1_transport_unary_over_the_host_fetch() {
    let Some(base) = transport_base() else {
        return;
    };
    let client = build_client(&base, "case.CaseService/Unary");
    let ctx = transport_ctx();
    let request = Request::new(b"ping".to_vec());

    let response = foundation_connectrpc_wasm_tests::drive_on_valtron(async move {
        client.unary(ctx, request).await
    })
    .await
    .expect("case 14: the valtron engine runs the call")
    .expect("case 14: the endpoint answers");

    assert_eq!(
        response.msg,
        b"echo:ping".to_vec(),
        "case 14: the handler's message"
    );
}

/// Case 15, oracle expected outcome: the endpoint answers a non-200 status and
/// the client returns a typed `ConnectError` with the handler's code.
#[wasm_bindgen_test]
async fn h1_transport_error_status_is_typed() {
    let Some(base) = transport_base() else {
        return;
    };
    let client = build_client(&base, "case.CaseService/Fail");
    let ctx = transport_ctx();
    let request = Request::new(b"ping".to_vec());

    let error = foundation_connectrpc_wasm_tests::drive_on_valtron(async move {
        client.unary(ctx, request).await
    })
    .await
    .expect("case 15: the valtron engine runs the call")
    .expect_err("case 15: the endpoint rejects the call");

    assert_eq!(
        error.current_context().code(),
        Code::InvalidArgument,
        "case 15: the handler's code"
    );
    assert!(
        error.to_string().contains("the handler rejected the call"),
        "case 15: the handler's message: {error}"
    );
}

/// Case 16, oracle example: a server stream crosses the transport and every
/// frame arrives in order.
#[wasm_bindgen_test]
async fn h1_transport_server_stream_delivers_frames() {
    let Some(base) = transport_base() else {
        return;
    };
    let client = build_client(&base, "case.CaseService/Stream");
    let ctx = transport_ctx();
    let request = Request::new(b"seed".to_vec());

    let messages = foundation_connectrpc_wasm_tests::drive_on_valtron(async move {
        receive_all(client, ctx, request).await
    })
    .await
    .expect("case 16: the valtron engine runs the call")
    .expect("case 16: the stream arrives");

    assert_eq!(
        messages,
        vec![
            b"frame-0".to_vec(),
            b"frame-1".to_vec(),
            b"frame-2".to_vec()
        ],
        "case 16: every frame arrives, in order"
    );
}

/// Read every decoded message of a server stream.
async fn receive_all(
    client: Client<Vec<u8>, Vec<u8>>,
    ctx: RpcCtx,
    request: Request<Vec<u8>>,
) -> ConnectResult<Vec<Vec<u8>>> {
    let mut stream = client.server_stream(ctx, request).await?;
    let mut messages = Vec::new();
    while let Some(message) = stream.receive().await? {
        messages.push(message);
    }
    Ok(messages)
}

// ── case 17 ────────────────────────────────────────────────────────────────

/// Case 17, oracle expected outcome: the transport advertises exactly what the
/// host's fetch can do.
#[wasm_bindgen_test]
fn wasm_fetch_capabilities_match_the_transport() {
    let transport = H1Transport::new(HttpClientBuilder::new().build());
    let actual = transport.capabilities();
    assert_eq!(
        actual.request_streaming, WASM_FETCH_CAPABILITIES.request_streaming,
        "case 17: request streaming"
    );
    assert_eq!(
        actual.full_duplex, WASM_FETCH_CAPABILITIES.full_duplex,
        "case 17: full duplex"
    );
    assert_eq!(
        actual.h2_trailers, WASM_FETCH_CAPABILITIES.h2_trailers,
        "case 17: h2 trailers"
    );
    assert_eq!(
        actual.multiplexed, WASM_FETCH_CAPABILITIES.multiplexed,
        "case 17: multiplexing"
    );
}

// ── CfConn streaming (#927) and the async request body (#928) ─────────────

/// Read every chunk of a response body, with one reader for the whole body.
async fn read_all_chunks(reader: &ReadableStreamDefaultReader) -> Vec<Vec<u8>> {
    let mut chunks = Vec::new();
    loop {
        let result = JsFuture::from(reader.read()).await.expect("read");
        let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if done {
            break;
        }
        let value = js_sys::Reflect::get(&result, &JsValue::from_str("value")).expect("value");
        chunks.push(js_sys::Uint8Array::new(&value).to_vec());
    }
    chunks
}

fn streaming_reader(response: &Response) -> ReadableStreamDefaultReader {
    response
        .body()
        .expect("a streaming body")
        .get_reader()
        .unchecked_into::<ReadableStreamDefaultReader>()
}

/// #927: the first frame reaches the client while the producer still runs.
///
/// The handler writes frame one, then waits on a gate. The test reads frame
/// one through a `CfConn`-carried response, asserts the producer has not
/// finished, then releases the gate and reads the rest.
#[wasm_bindgen_test]
async fn cf_conn_streams_the_first_frame_while_the_producer_runs() {
    let (release, gate) = futures::channel::oneshot::channel::<()>();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let produced = Arc::new(AtomicUsize::new(0));

    let mut router = Router::new();
    let handler_produced = Arc::clone(&produced);
    let handler_gate = Arc::clone(&gate);
    router.server_stream(
        STREAM,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, _req: Request<Vec<u8>>| {
            let produced = Arc::clone(&handler_produced);
            let gate = Arc::clone(&handler_gate);
            async move {
                produced.fetch_add(1, Ordering::SeqCst);
                let first: Vec<ConnectResult<Vec<u8>>> = vec![Ok(b"first".to_vec())];
                let first = futures::stream::iter(first);
                let second = futures::stream::once(async move {
                    let receiver = gate.lock().unwrap().take().expect("the gate");
                    let _ = receiver.await;
                    produced.fetch_add(1, Ordering::SeqCst);
                    Ok(b"second".to_vec())
                });
                Ok(first.chain(second))
            }
        },
        HandlerOptions::new(),
    );

    let handler = Arc::new(router.into_handler());
    let (sink, handle) = ConnectSink::new_pair_with_capacity(1);
    spawn_local(async move {
        let _ = handler
            .dispatch_to(
                ctx(),
                stream_request(STREAM, enveloped(b"seed")),
                body(enveloped(b"seed")),
                Box::new(sink),
                CallHooks::default(),
            )
            .await;
    });

    let response = handle.wait_response().await.expect("the response head");
    let mut conn = CfConn::new();
    conn.set_response(response);
    let response = conn.into_response().expect("a streaming response");
    let reader = streaming_reader(&response);

    let first = JsFuture::from(reader.read()).await.expect("first read");
    let value = js_sys::Reflect::get(&first, &JsValue::from_str("value")).expect("value");
    let first = js_sys::Uint8Array::new(&value).to_vec();
    assert_eq!(
        parse_envelopes(&first),
        vec![(0u8, b"first".to_vec())],
        "#927: the first frame arrives before the producer ends"
    );
    assert_eq!(
        produced.load(Ordering::SeqCst),
        1,
        "#927: the producer is still running after the first frame"
    );

    release.send(()).expect("release the gate");
    let rest = read_all_chunks(&reader).await;
    let mut all = first;
    for chunk in rest {
        all.extend_from_slice(&chunk);
    }
    let payloads: Vec<Vec<u8>> = parse_envelopes(&all)
        .into_iter()
        .filter(|(flag, _)| *flag == 0)
        .map(|(_, payload)| payload)
        .collect();
    assert_eq!(
        payloads,
        vec![b"first".to_vec(), b"second".to_vec()],
        "#927: both frames arrive in order"
    );
}

/// #927: a 10 MiB stream arrives in many chunks, not one buffer.
#[wasm_bindgen_test]
async fn cf_conn_streams_ten_mib_in_chunks() {
    const CHUNK: usize = 64 * 1024;
    const COUNT: usize = 160; // 10 MiB

    let mut router = Router::new();
    router.server_stream(
        STREAM,
        ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, _req: Request<Vec<u8>>| async move {
            let frames: Vec<ConnectResult<Vec<u8>>> =
                (0..COUNT).map(|_| Ok(vec![b'x'; CHUNK])).collect();
            Ok(futures::stream::iter(frames))
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();
    let (sink, handle) = ConnectSink::new_pair_with_capacity(4);
    spawn_local(async move {
        let _ = handler
            .dispatch_to(
                ctx(),
                stream_request(STREAM, enveloped(b"seed")),
                body(enveloped(b"seed")),
                Box::new(sink),
                CallHooks::default(),
            )
            .await;
    });

    let response = handle.wait_response().await.expect("the response head");
    let mut conn = CfConn::new();
    conn.set_response(response);
    let response = conn.into_response().expect("a streaming response");
    let chunks = read_all_chunks(&streaming_reader(&response)).await;
    let total: usize = chunks.iter().map(Vec::len).sum();
    assert!(
        chunks.len() > 1,
        "#927: a 10 MiB body arrives in chunks, got {}",
        chunks.len()
    );
    assert!(
        total >= 10 * 1024 * 1024,
        "#927: ten MiB arrive, got {total} bytes"
    );
}

/// #928: a JS `ReadableStream` becomes an async request body.
///
/// The body parks on a retry and yields every chunk; a `WouldBlock` error
/// would fail the test.
#[wasm_bindgen_test]
async fn a_js_stream_becomes_an_async_request_body() {
    const CHUNK: usize = 32 * 1024;
    const COUNT: u32 = 8;

    let (stream, _pulls) = counting_stream(COUNT, CHUNK);
    let mut body = request_body_from_stream(stream, 2, Some(COUNT as usize * CHUNK));
    let mut chunks = 0usize;
    let mut total = 0usize;
    while let Some(item) = body.next().await {
        let bytes = item.expect("#928: a retry parks, it is not a WouldBlock error");
        chunks += 1;
        total += bytes.len();
    }
    assert_eq!(total, COUNT as usize * CHUNK, "#928: every byte arrives");
    assert!(chunks > 1, "#928: the body arrives in chunks");
}
