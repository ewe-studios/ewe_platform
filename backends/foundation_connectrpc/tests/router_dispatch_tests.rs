//! Router & dispatch tests (spec-41 F22 / Decision 08): the 404 → 405 → 415 →
//! 505 decision flow with correct `Allow`/`Accept-Post`, unary POST + GET
//! execution, server-streaming execution through the seam, multi-service
//! coexistence, and `into_handler` freeze.
//!
//! Dispatch is one async future; a local cooperative `block_on` drives it (no
//! valtron pool needed — the streaming path joins its tasks inside the dispatch
//! future).

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};
use bytes::{Buf, BufMut, Bytes};

use foundation_http::shared::context::ContextBag;
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod, Status,
};

use foundation_connectrpc::shared::codec::CodecFor;
use foundation_connectrpc::{
    ConnectResult, EnvelopeWriter, HandlerOptions, JsonCodec, ProcedureCodecs, Request, Response,
    Router,
};

// ── tiny cooperative executor ─────────────────────────────────────────────────

struct ThreadWaker(std::thread::Thread);
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

// ── a real, minimal buffa message (id @1, name @2) ────────────────────────────

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
    fn compute_size(&self, _cache: &mut SizeCache) -> u32 {
        0
    }
    fn write_to(&self, _cache: &mut SizeCache, buf: &mut impl BufMut) {
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
        _ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        match tag.field_number() {
            1 => {
                self.id = decode_varint(buf)? as i32;
                Ok(())
            }
            2 => {
                let len = decode_varint(buf)? as usize;
                let mut bytes = vec![0u8; len];
                buf.copy_to_slice(&mut bytes);
                self.name = String::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
                Ok(())
            }
            _ => skip_field(tag, buf),
        }
    }
    fn clear(&mut self) {
        *self = Self::default();
    }
}

// ── request builders ──────────────────────────────────────────────────────────

const ECHO: &str = "http://t/test.EchoService/Echo";

fn request(
    url: &str,
    method: SimpleMethod,
    content_type: Option<&str>,
    body: Option<Vec<u8>>,
) -> SimpleIncomingRequest {
    let mut builder = SimpleIncomingRequest::builder()
        .with_parsed_url(url)
        .with_proto(Proto::HTTP11)
        .with_method(method);
    if let Some(ct) = content_type {
        builder = builder.add_header_raw(SimpleHeader::CONTENT_TYPE, ct);
    }
    if let Some(body) = body {
        builder = builder.with_body(SendSafeBody::Bytes(body));
    }
    builder.build().expect("request builds")
}

fn json_bytes(msg: &TestMsg) -> Vec<u8> {
    JsonCodec.marshal(msg).expect("json marshal").to_vec()
}

fn enveloped(msg: &TestMsg) -> Vec<u8> {
    // A single Connect streaming request frame (no compression).
    EnvelopeWriter::new(None, 0, 0)
        .write(Bytes::from(json_bytes(msg)))
        .expect("envelope")
}

fn body_bytes(response: &foundation_netio::shared::http::SimpleOutgoingResponse) -> Vec<u8> {
    match &response.body {
        Some(SendSafeBody::Bytes(b)) => b.clone(),
        Some(SendSafeBody::Text(t)) => t.clone().into_bytes(),
        _ => Vec::new(),
    }
}

fn header(
    response: &foundation_netio::shared::http::SimpleOutgoingResponse,
    name: &str,
) -> Option<String> {
    response
        .headers
        .get(&SimpleHeader::from(name.to_string()))
        .and_then(|v| v.first())
        .cloned()
}

fn bag() -> Arc<ContextBag> {
    Arc::new(ContextBag::new())
}

fn echo_router() -> Router {
    let mut router = Router::new();
    router.unary(
        ECHO,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new().with_idempotency(foundation_connectrpc::IdempotencyLevel::NoSideEffects),
    );
    router
}

// ── decision flow ─────────────────────────────────────────────────────────────

#[test]
fn unregistered_path_is_404() {
    let handler = echo_router().into_handler();
    let req = request(
        "http://t/test.EchoService/Missing",
        SimpleMethod::POST,
        Some("application/json"),
        Some(json_bytes(&TestMsg::default())),
    );
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(response.status, Status::NotFound);
}

#[test]
fn wrong_method_is_405_with_allow() {
    let handler = echo_router().into_handler();
    let req = request(ECHO, SimpleMethod::PUT, Some("application/json"), None);
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(response.status, Status::MethodNotAllowed);
    let allow = header(&response, "allow").expect("Allow header present");
    assert!(allow.contains("POST"), "Allow lists POST: {allow}");
    assert!(allow.contains("GET"), "Allow lists GET (Connect unary): {allow}");
}

#[test]
fn unknown_codec_is_415_with_accept_post() {
    let handler = echo_router().into_handler();
    // `application/xml` parses as the Connect codec name "xml" — not registered.
    let req = request(ECHO, SimpleMethod::POST, Some("application/xml"), Some(vec![]));
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(response.status, Status::UnsupportedMediaType);
    let accept = header(&response, "accept-post").expect("Accept-Post present");
    assert!(accept.contains("application/proto"), "{accept}");
    assert!(accept.contains("application/json"), "{accept}");
}

#[test]
fn bidi_over_http1_is_505() {
    let mut router = Router::new();
    router.bidi_stream(
        "http://t/test.EchoService/Chat",
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, _reqs: foundation_connectrpc::RequestStream<TestMsg>| async move {
            Ok(futures::stream::empty::<ConnectResult<TestMsg>>())
        },
        HandlerOptions::new(),
    );
    let handler = router.into_handler();
    let req = request(
        "http://t/test.EchoService/Chat",
        SimpleMethod::POST,
        Some("application/connect+json"),
        Some(vec![]),
    );
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(
        response.status,
        Status::HttpVersionNotSupported,
        "bidi requires full-duplex (HTTP/2+); HTTP/1.1 → 505"
    );
}

// ── execution ─────────────────────────────────────────────────────────────────

#[test]
fn unary_post_json_roundtrips() {
    let handler = echo_router().into_handler();
    let msg = TestMsg {
        id: 7,
        name: "hi".to_string(),
    };
    let req = request(
        ECHO,
        SimpleMethod::POST,
        Some("application/json"),
        Some(json_bytes(&msg)),
    );
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(response.status, Status::OK);
    let echoed: TestMsg = JsonCodec
        .unmarshal(Bytes::from(body_bytes(&response)))
        .expect("decode echo");
    assert_eq!(echoed, msg);
}

#[test]
fn unary_get_is_routed_and_decoded() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;

    let handler = echo_router().into_handler();
    let msg = TestMsg {
        id: 42,
        name: "get".to_string(),
    };
    let message = URL_SAFE_NO_PAD.encode(json_bytes(&msg));
    let url = format!(
        "http://t/test.EchoService/Echo?connect=v1&encoding=json&base64=1&message={message}"
    );
    let req = request(&url, SimpleMethod::GET, None, None);
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(response.status, Status::OK, "unary GET routes to the handler");
    let echoed: TestMsg = JsonCodec
        .unmarshal(Bytes::from(body_bytes(&response)))
        .expect("decode echo");
    assert_eq!(echoed, msg);
}

#[test]
fn server_stream_emits_all_responses() {
    let mut router = Router::new();
    router.server_stream(
        "http://t/test.EchoService/List",
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
    let handler = router.into_handler();
    let req = request(
        "http://t/test.EchoService/List",
        SimpleMethod::POST,
        Some("application/connect+json"),
        Some(enveloped(&TestMsg {
            id: 10,
            name: "seed".to_string(),
        })),
    );
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(response.status, Status::OK);
    assert_eq!(
        header(&response, "content-type").as_deref(),
        Some("application/connect+json")
    );

    let (messages, end_stream) = parse_connect_stream(&body_bytes(&response));
    assert_eq!(messages.len(), 3, "three server-stream responses");
    assert!(end_stream, "terminating EndStream frame present");
    let first: TestMsg = JsonCodec
        .unmarshal(Bytes::from(messages[0].clone()))
        .expect("decode first");
    assert_eq!(first.id, 10);
}

#[test]
fn require_connect_protocol_header_is_enforced() {
    let mut router = Router::new();
    router.unary(
        ECHO,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move { Ok(Response::new(req.msg)) },
        HandlerOptions::new().with_require_connect_protocol_header(true),
    );
    let handler = router.into_handler();

    // Missing the version header → rejected.
    let req = request(
        ECHO,
        SimpleMethod::POST,
        Some("application/json"),
        Some(json_bytes(&TestMsg::default())),
    );
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(
        response.status,
        Status::BadRequest,
        "missing Connect-Protocol-Version → invalid_argument (400)"
    );

    // With the header present → accepted.
    let mut req = request(
        ECHO,
        SimpleMethod::POST,
        Some("application/json"),
        Some(json_bytes(&TestMsg::default())),
    );
    req.headers.insert(
        SimpleHeader::from("connect-protocol-version".to_string()),
        vec!["1".to_string()],
    );
    let response = block_on(handler.dispatch(bag(), req));
    assert_eq!(response.status, Status::OK);
}

// ── coexistence + freeze ──────────────────────────────────────────────────────

#[test]
fn multiple_services_coexist() {
    let mut router = echo_router();
    router.unary(
        "http://t/test.MathService/Inc",
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, req: Request<TestMsg>| async move {
            Ok(Response::new(TestMsg {
                id: req.msg.id + 1,
                name: req.msg.name,
            }))
        },
        HandlerOptions::new(),
    );
    assert_eq!(router.len(), 2, "two procedures registered");
    let handler = router.into_handler();

    let inc = request(
        "http://t/test.MathService/Inc",
        SimpleMethod::POST,
        Some("application/json"),
        Some(json_bytes(&TestMsg {
            id: 4,
            name: "n".to_string(),
        })),
    );
    let response = block_on(handler.dispatch(bag(), inc));
    let out: TestMsg = JsonCodec
        .unmarshal(Bytes::from(body_bytes(&response)))
        .expect("decode");
    assert_eq!(out.id, 5);

    // The other service still routes independently.
    let echo = request(
        ECHO,
        SimpleMethod::POST,
        Some("application/json"),
        Some(json_bytes(&TestMsg {
            id: 9,
            name: "e".to_string(),
        })),
    );
    let response = block_on(handler.dispatch(bag(), echo));
    assert_eq!(response.status, Status::OK);
}

#[test]
fn into_handler_exposes_frozen_router() {
    let handler = echo_router().into_handler();
    // The frozen router reflects registrations; there is no post-build mutation API.
    assert_eq!(handler.router().len(), 1);
    assert!(!handler.router().is_empty());
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Parse a Connect streaming response body into `(messages, saw_end_stream)`.
fn parse_connect_stream(body: &[u8]) -> (Vec<Vec<u8>>, bool) {
    const FLAG_END_STREAM: u8 = 0x02;
    let mut messages = Vec::new();
    let mut end = false;
    let mut i = 0;
    while i + 5 <= body.len() {
        let flags = body[i];
        let len = u32::from_be_bytes([body[i + 1], body[i + 2], body[i + 3], body[i + 4]]) as usize;
        let start = i + 5;
        let stop = (start + len).min(body.len());
        let payload = body[start..stop].to_vec();
        if flags & FLAG_END_STREAM != 0 {
            end = true;
        } else {
            messages.push(payload);
        }
        i = stop;
    }
    (messages, end)
}
