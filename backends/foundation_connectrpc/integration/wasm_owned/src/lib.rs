//! The `foundation_connectrpc` pure-Rust wasm cases on the OWNED harness
//! (spec 88 feature 03, T7).
//!
//! WHY: the router, the response sink, the error model and the deadline are
//! pure in-memory paths. This package builds them without the fetch client, so
//! its wasm module imports no wasm-bindgen glue and runs on Deno and Chromium
//! through the owned harness (rules R9, R15).
//!
//! WHAT: cases 1 to 13 of feature 03. The transport cases (14 to 17) stay on
//! the wasm-bindgen harness, because they need the fetch client.
//!
//! HOW: each case awaits the real `dispatch` or `dispatch_to` future. The
//! deadline cases use a deterministic clock this package supplies (root rule
//! 11). The cases live in the lib, not `tests/`: the owned runner reads the
//! lib's `__fwt_` exports (spec 88 section 5).

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use foundation_compact::{Duration, Instant, SystemTime};
use foundation_connectrpc::shared::transport::BoxFuture;
use foundation_connectrpc::{
    CallHooks, Code, Codec, CodecError, CodecFor, ConnectError, ConnectResult, DeadlineTimer,
    EnvelopeWriter, HandlerOptions, Request, RequestBody, RequestStream, Response, ResponseSink,
    Router, RpcCtx,
};
use foundation_context::{Clock, Ctx};
use foundation_macros::wasm_test;
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse,
    Status,
};

const UNARY: &str = "http://t/case.CaseService/Unary";
const STREAM: &str = "http://t/case.CaseService/Stream";
const FAIL: &str = "http://t/case.CaseService/Fail";
const WAIT: &str = "http://t/case.CaseService/Wait";
const SECOND: &str = "http://t/case.SecondService/Unary";

const FRAMES: [&[u8]; 3] = [b"frame-0", b"frame-1", b"frame-2"];

// ── test support ────────────────────────────────────────────────────────────

/// A pass-through codec. Its wire name is `"proto"`.
struct IdentityCodec;

impl Codec for IdentityCodec {
    fn name(&self) -> &'static str {
        "proto"
    }
    fn is_binary(&self) -> bool {
        true
    }
}

impl CodecFor<Vec<u8>> for IdentityCodec {
    fn marshal(&self, message: &Vec<u8>) -> Result<Bytes, CodecError> {
        Ok(Bytes::copy_from_slice(message))
    }
    fn unmarshal(&self, data: Bytes) -> Result<Vec<u8>, CodecError> {
        if data.is_empty() {
            return Err(CodecError::ZeroLength { codec: "identity" });
        }
        Ok(data.to_vec())
    }
    fn marshal_stable(&self, message: &Vec<u8>) -> Result<Bytes, CodecError> {
        Ok(Bytes::copy_from_slice(message))
    }
    fn marshal_append(&self, buf: &mut Vec<u8>, message: &Vec<u8>) -> Result<(), CodecError> {
        buf.extend_from_slice(message);
        Ok(())
    }
}

/// One recorded sink call.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Whole {
        status: Option<Status>,
        body: Vec<u8>,
    },
    Open {
        status: Option<Status>,
    },
    Chunk(Vec<u8>),
    Finish,
}

impl Call {
    fn kind(&self) -> &'static str {
        match self {
            Call::Whole { .. } => "whole",
            Call::Open { .. } => "open",
            Call::Chunk(_) => "chunk",
            Call::Finish => "finish",
        }
    }
}

type Recording = Arc<Mutex<Vec<Call>>>;

/// A response sink that records each call. `fail_at` fails that 1-based call.
struct RecordingSink {
    calls: Recording,
    fail_at: Option<usize>,
    next: usize,
}

impl RecordingSink {
    fn new(calls: Recording) -> Self {
        Self {
            calls,
            fail_at: None,
            next: 0,
        }
    }

    fn failing_on(calls: Recording, n: usize) -> Self {
        Self {
            calls,
            fail_at: Some(n),
            next: 0,
        }
    }

    fn record(&mut self, call: Call) -> io::Result<()> {
        self.next += 1;
        if self.fail_at == Some(self.next) {
            return Err(io::Error::other("the sink fails"));
        }
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
        Ok(())
    }
}

impl ResponseSink for RecordingSink {
    fn whole(&mut self, response: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        let result = self.record(Call::Whole {
            status: Some(response.status.clone()),
            body: response_body(&response),
        });
        Box::pin(std::future::ready(result))
    }

    fn open(&mut self, head: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        let result = self.record(Call::Open {
            status: Some(head.status.clone()),
        });
        Box::pin(std::future::ready(result))
    }

    fn chunk(&mut self, bytes: Bytes) -> BoxFuture<'_, io::Result<()>> {
        let result = self.record(Call::Chunk(bytes.to_vec()));
        Box::pin(std::future::ready(result))
    }

    fn finish(&mut self) -> BoxFuture<'_, io::Result<()>> {
        let result = self.record(Call::Finish);
        Box::pin(std::future::ready(result))
    }
}

fn response_body(response: &SimpleOutgoingResponse) -> Vec<u8> {
    match &response.body {
        Some(SendSafeBody::Bytes(bytes)) => bytes.clone(),
        Some(SendSafeBody::Text(text)) => text.clone().into_bytes(),
        _ => Vec::new(),
    }
}

fn request(
    url: &str,
    method: SimpleMethod,
    content_type: &str,
    body: Vec<u8>,
) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_parsed_url(url)
        .with_proto(Proto::HTTP11)
        .with_method(method)
        .add_header_raw(SimpleHeader::CONTENT_TYPE, content_type)
        .with_body(SendSafeBody::Bytes(body))
        .build()
        .expect("the request builds")
}

fn unary_request(url: &str, body: &[u8]) -> SimpleIncomingRequest {
    request(url, SimpleMethod::POST, "application/proto", body.to_vec())
}

fn stream_request(url: &str, body: Vec<u8>) -> SimpleIncomingRequest {
    request(url, SimpleMethod::POST, "application/connect+proto", body)
}

fn request_with_timeout(url: &str, body: &[u8], millis: u64) -> SimpleIncomingRequest {
    let mut req = unary_request(url, body);
    req.headers.insert(
        SimpleHeader::from("connect-timeout-ms".to_string()),
        vec![millis.to_string()],
    );
    req
}

fn enveloped(message: &[u8]) -> Vec<u8> {
    EnvelopeWriter::new(None, 0, 0)
        .write(Bytes::copy_from_slice(message))
        .expect("the envelope encodes")
}

fn body(bytes: Vec<u8>) -> RequestBody<'static> {
    foundation_connectrpc::in_memory_body(Some(SendSafeBody::Bytes(bytes)))
}

fn parse_envelopes(mut body: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut frames = Vec::new();
    while body.len() >= 5 {
        let flags = body[0];
        let len = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        if body.len() < 5 + len {
            break;
        }
        frames.push((flags, body[5..5 + len].to_vec()));
        body = &body[5 + len..];
    }
    frames
}

fn calls_snapshot(calls: &Recording) -> Vec<Call> {
    calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

fn call_kinds(calls: &[Call]) -> Vec<&'static str> {
    calls.iter().map(Call::kind).collect()
}

/// A clock that never reads real time. Only the test moves it.
struct SimulatedClock {
    base: Instant,
    offset: Mutex<Duration>,
}

impl SimulatedClock {
    fn new() -> Self {
        Self {
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
        }
    }
}

impl Clock for SimulatedClock {
    fn now(&self) -> Instant {
        self.base
            + *self
                .offset
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn elapsed(&self) -> Duration {
        *self
            .offset
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn wall_now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
    }
}

/// A deadline timer that reads the injected clock at call time.
struct SimulatedTimer;

impl DeadlineTimer for SimulatedTimer {
    fn sleep_until(&self, deadline: Instant, ctx: &Ctx) -> BoxFuture<'_, ()> {
        let now = ctx.clock().now();
        Box::pin(async move {
            if now >= deadline {
                return;
            }
            std::future::pending::<()>().await;
        })
    }
}

fn simulated_ctx() -> Ctx {
    Ctx::with_seed(Arc::new(SimulatedClock::new()), 7)
}

fn remaining_ms(call: &RpcCtx) -> Option<u64> {
    let deadline = call.request.deadline?;
    let left = deadline.saturating_duration_since(call.ctx.clock().now());
    Some(u64::try_from(left.as_millis()).unwrap_or(u64::MAX))
}

fn echo_router(calls: Arc<AtomicUsize>) -> Router {
    let mut router = Router::new();
    router.unary(
        UNARY,
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, req: Request<Vec<u8>>| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move {
                let mut out = b"echo:".to_vec();
                out.extend_from_slice(&req.msg);
                Ok(Response::new(out))
            }
        },
        HandlerOptions::new(),
    );
    router
}

fn stream_router(produced: Arc<AtomicUsize>) -> Router {
    let mut router = Router::new();
    router.server_stream(
        STREAM,
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
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

fn fail_router() -> Router {
    let mut router = Router::new();
    router.unary(
        FAIL,
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |_ctx, _req: Request<Vec<u8>>| async move {
            Err(ConnectError::invalid_argument("the handler rejected the call").into())
        },
        HandlerOptions::new(),
    );
    router
}

fn wait_router() -> Router {
    let mut router = Router::new();
    router.unary(
        WAIT,
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |_ctx, _req: Request<Vec<u8>>| async move {
            std::future::pending::<()>().await;
            unreachable!("the waiting handler never returns")
        },
        HandlerOptions::new(),
    );
    router
}

// ── case 1 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn unary_round_trip_through_the_router() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls.clone()).into_handler();
    let response = handler
        .dispatch(simulated_ctx(), unary_request(UNARY, b"ping"))
        .await;
    assert_eq!(response.status, Status::OK, "case 1: status");
    assert_eq!(
        response_body(&response),
        b"echo:ping".to_vec(),
        "case 1: the handler's message"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "case 1: one call");
}

// ── case 2 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn unknown_service_returns_unimplemented() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls.clone()).into_handler();
    let response = handler
        .dispatch(
            simulated_ctx(),
            unary_request("http://t/missing.Procedure", b"ping"),
        )
        .await;
    assert_eq!(response.status, Status::NotFound, "case 2: path miss");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "case 2: no handler");
    assert_eq!(
        Code::from_http_status(404),
        Code::Unimplemented,
        "case 2: Decision 03 maps a non-Connect 404 to unimplemented"
    );
}

// ── case 3 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn wrong_method_reports_the_documented_status() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls).into_handler();
    let response = handler
        .dispatch(
            simulated_ctx(),
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
    assert!(allow.contains("POST"), "case 3: Allow names POST");
}

// ── case 4 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn handler_error_keeps_its_code_and_message() {
    let handler = fail_router().into_handler();
    let response = handler
        .dispatch(simulated_ctx(), unary_request(FAIL, b"ping"))
        .await;
    assert_eq!(response.status, Status::BadRequest, "case 4: status");
    let wire: serde_json::Value =
        serde_json::from_slice(&response_body(&response)).expect("case 4: JSON");
    assert_eq!(
        wire.get("code").and_then(serde_json::Value::as_str),
        Some("invalid_argument"),
        "case 4: code"
    );
    assert_eq!(
        wire.get("message").and_then(serde_json::Value::as_str),
        Some("the handler rejected the call"),
        "case 4: message"
    );
}

// ── case 5 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn server_stream_frames_arrive_in_order() {
    let produced = Arc::new(AtomicUsize::new(0));
    let handler = stream_router(produced).into_handler();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(calls.clone());
    handler
        .dispatch_to(
            simulated_ctx(),
            stream_request(STREAM, enveloped(b"seed")),
            body(enveloped(b"seed")),
            Box::new(sink),
            CallHooks::default(),
        )
        .await
        .expect("case 5: the sink accepts every frame");

    let calls = calls_snapshot(&calls);
    assert_eq!(calls.first().map(Call::kind), Some("open"), "case 5: open");
    assert_eq!(
        calls.last().map(Call::kind),
        Some("finish"),
        "case 5: finish"
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
    assert_eq!(frames[3].0, 0x02, "case 5: the end-stream frame");
}

// ── case 6 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn server_stream_stops_when_the_sink_fails() {
    let produced = Arc::new(AtomicUsize::new(0));
    let handler = stream_router(produced.clone()).into_handler();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::failing_on(calls.clone(), 3);
    let outcome = handler
        .dispatch_to(
            simulated_ctx(),
            stream_request(STREAM, enveloped(b"seed")),
            body(enveloped(b"seed")),
            Box::new(sink),
            CallHooks::default(),
        )
        .await;
    assert!(outcome.is_err(), "case 6: dispatch returns the error");
    assert_eq!(
        call_kinds(&calls_snapshot(&calls)),
        vec!["open", "chunk"],
        "case 6: the sink failed on its third call"
    );
    assert!(
        produced.load(Ordering::SeqCst) < 4,
        "case 6: no fourth frame"
    );
}

// ── case 7 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn unary_uses_whole_and_never_open() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls).into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());
    handler
        .dispatch_to(
            simulated_ctx(),
            unary_request(UNARY, b"ping"),
            body(b"ping".to_vec()),
            Box::new(sink),
            CallHooks::default(),
        )
        .await
        .expect("case 7: the sink accepts the response");
    let calls = calls_snapshot(&recorded);
    assert_eq!(call_kinds(&calls), vec!["whole"], "case 7: one whole call");
}

// ── case 8 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn client_stream_reads_every_frame() {
    use futures::StreamExt as _;

    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_by_handler = seen.clone();
    let mut router = Router::new();
    router.client_stream(
        "http://t/case.CaseService/Collect",
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, mut reqs: RequestStream<Vec<u8>>| {
            let seen = seen_by_handler.clone();
            async move {
                while let Some(item) = reqs.next().await {
                    seen.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(item?);
                }
                Ok(Response::new(b"done".to_vec()))
            }
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();
    let mut payload = Vec::new();
    for frame in [b"one".as_slice(), b"two", b"three"] {
        payload.extend_from_slice(&enveloped(frame));
    }
    let response = handler
        .dispatch(
            simulated_ctx(),
            stream_request("http://t/case.CaseService/Collect", payload),
        )
        .await;
    assert_eq!(response.status, Status::OK, "case 8: status");
    let seen = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        seen,
        vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()],
        "case 8: three frames in order"
    );
}

// ── case 9 ─────────────────────────────────────────────────────────────────

#[wasm_test]
async fn bidi_echoes_in_order() {
    use futures::StreamExt as _;

    let mut router = Router::new();
    router.bidi_stream(
        "http://t/case.CaseService/Chat",
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |_ctx, mut reqs: RequestStream<Vec<u8>>| async move {
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
    let response = handler.dispatch(simulated_ctx(), req).await;
    assert_eq!(response.status, Status::OK, "case 9: status");
    let frames = parse_envelopes(&response_body(&response));
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

#[wasm_test]
async fn two_services_keep_their_procedures_apart() {
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    let mut router = echo_router(first.clone());
    let second_calls = second.clone();
    router.unary(
        SECOND,
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        move |_ctx, _req: Request<Vec<u8>>| {
            second_calls.fetch_add(1, Ordering::SeqCst);
            async move { Ok(Response::new(b"second".to_vec())) }
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();
    let one = handler
        .dispatch(simulated_ctx(), unary_request(UNARY, b"a"))
        .await;
    let two = handler
        .dispatch(simulated_ctx(), unary_request(SECOND, b"b"))
        .await;
    assert_eq!(response_body(&one), b"echo:a".to_vec(), "case 10: first");
    assert_eq!(response_body(&two), b"second".to_vec(), "case 10: second");
    assert_eq!(first.load(Ordering::SeqCst), 1, "case 10: first count");
    assert_eq!(second.load(Ordering::SeqCst), 1, "case 10: second count");
}

// ── case 11 ────────────────────────────────────────────────────────────────

#[wasm_test]
async fn past_deadline_ends_the_call() {
    let handler = wait_router().into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());
    handler
        .dispatch_to(
            simulated_ctx(),
            request_with_timeout(WAIT, b"slow", 0),
            body(b"slow".to_vec()),
            Box::new(sink),
            CallHooks::default().with_timer(&SimulatedTimer),
        )
        .await
        .expect("case 11: the deadline answers through the sink");
    let calls = calls_snapshot(&recorded);
    match &calls[0] {
        Call::Whole { status, body } => {
            assert_eq!(*status, Some(Status::GatewayTimeout), "case 11: status");
            let wire: serde_json::Value = serde_json::from_slice(body).expect("case 11: JSON");
            assert_eq!(
                wire.get("code").and_then(serde_json::Value::as_str),
                Some("deadline_exceeded"),
                "case 11: code"
            );
        }
        other => panic!("case 11: expected whole, got {other:?}"),
    }
}

// ── case 12 ────────────────────────────────────────────────────────────────

#[wasm_test]
async fn future_deadline_lets_the_handler_finish() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = echo_router(calls).into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());
    handler
        .dispatch_to(
            simulated_ctx(),
            request_with_timeout(UNARY, b"fast", 60_000),
            body(b"fast".to_vec()),
            Box::new(sink),
            CallHooks::default().with_timer(&SimulatedTimer),
        )
        .await
        .expect("case 12: the handler answers");
    let calls = calls_snapshot(&recorded);
    match &calls[0] {
        Call::Whole { status, body } => {
            assert_eq!(*status, Some(Status::OK), "case 12: status");
            assert_eq!(body.as_slice(), b"echo:fast", "case 12: message");
        }
        other => panic!("case 12: expected whole, got {other:?}"),
    }
}

// ── case 13 ────────────────────────────────────────────────────────────────

#[wasm_test]
async fn handler_sees_the_remaining_deadline() {
    let mut router = Router::new();
    router.unary(
        WAIT,
        foundation_connectrpc::ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec),
        |call, _req: Request<Vec<u8>>| async move {
            let left = remaining_ms(&call).unwrap_or_default();
            Ok(Response::new(left.to_string().into_bytes()))
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink::new(recorded.clone());
    handler
        .dispatch_to(
            simulated_ctx(),
            request_with_timeout(WAIT, b"x", 5_000),
            body(b"x".to_vec()),
            Box::new(sink),
            CallHooks::default().with_timer(&SimulatedTimer),
        )
        .await
        .expect("case 13: the handler answers");
    let calls = calls_snapshot(&recorded);
    match &calls[0] {
        Call::Whole { body, .. } => {
            assert_eq!(
                String::from_utf8_lossy(body),
                "5000",
                "case 13: the handler reads the 5 s timeout"
            );
        }
        other => panic!("case 13: expected whole, got {other:?}"),
    }
}
