//! Test support for the `foundation_connectrpc` wasm cases.
//!
//! WHY: the wasm testbed builds one test target per crate (spec 88 rule R10),
//! and the case file needs a pass-through codec, a recording response sink, a
//! request builder and a deterministic clock. Those helpers live here, so the
//! cases stay in `tests/` (spec 88 rule R9) and the crate has one target.
//!
//! WHAT: [`IdentityCodec`], [`RecordingSink`], [`Recording`], [`request`],
//! [`enveloped`], [`parse_envelopes`], [`SimulatedClock`], [`SimulatedTimer`]
//! and the body helpers.
//!
//! HOW: the sink records every call before it returns, and each write future
//! is ready at once, like `CollectingSink` (`shared/router/dispatch.rs`).
//! The clock never reads real time; a case moves it by hand.

use std::future::Future;
use std::io;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use foundation_compact::{Duration, Instant, SystemTime};
use foundation_connectrpc::shared::transport::BoxFuture;
use foundation_connectrpc::{
    Codec, CodecError, CodecFor, DeadlineTimer, EnvelopeWriter, RequestBody, ResponseSink,
};
use foundation_context::{Clock, Ctx};
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse,
    Status,
};

// ── the pass-through codec ──────────────────────────────────────────────────

/// A codec that copies bytes unchanged. Its wire name is `"proto"`, the default
/// `ClientOptions` codec name (copied from `tests/client_core_tests.rs`).
pub struct IdentityCodec;

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

// ── the recording sink ──────────────────────────────────────────────────────

/// One call the sink received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    /// A complete response (`whole`), with its status and body.
    Whole {
        status: Option<Status>,
        body: Vec<u8>,
    },
    /// A stream head (`open`), with its status.
    Open { status: Option<Status> },
    /// One streamed frame (`chunk`).
    Chunk(Vec<u8>),
    /// The end of a stream (`finish`).
    Finish,
}

impl Call {
    /// The call name, for an ordered comparison in a failure message.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Call::Whole { .. } => "whole",
            Call::Open { .. } => "open",
            Call::Chunk(_) => "chunk",
            Call::Finish => "finish",
        }
    }
}

/// The shared list a [`RecordingSink`] appends to.
pub type Recording = Arc<Mutex<Vec<Call>>>;

/// A [`ResponseSink`] that records each call and keeps the bytes.
///
/// `fail_at` is the 1-based call number that fails. `None` never fails.
pub struct RecordingSink {
    calls: Recording,
    fail_at: Option<usize>,
    next: usize,
}

impl RecordingSink {
    /// A sink that records into `calls` and never fails.
    #[must_use]
    pub fn new(calls: Recording) -> Self {
        Self {
            calls,
            fail_at: None,
            next: 0,
        }
    }

    /// A sink that fails on call number `n` (1-based), before it records it.
    #[must_use]
    pub fn failing_on(calls: Recording, n: usize) -> Self {
        Self {
            calls,
            fail_at: Some(n),
            next: 0,
        }
    }

    fn record(&mut self, call: Call) -> io::Result<()> {
        self.next += 1;
        if self.fail_at == Some(self.next) {
            return Err(io::Error::other(format!(
                "the sink fails on call {}",
                self.next
            )));
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

/// The bytes of a response body that is already in memory.
#[must_use]
pub fn response_body(response: &SimpleOutgoingResponse) -> Vec<u8> {
    match &response.body {
        Some(SendSafeBody::Bytes(bytes)) => bytes.clone(),
        Some(SendSafeBody::Text(text)) => text.clone().into_bytes(),
        _ => Vec::new(),
    }
}

// ── requests and envelopes ──────────────────────────────────────────────────

/// Build a request with the Connect content type and an in-memory body.
///
/// `content_type` is `application/proto` for a unary call and
/// `application/connect+proto` for a streaming call.
#[must_use]
pub fn request(
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

/// A Connect streaming request envelope around one message.
#[must_use]
pub fn enveloped(message: &[u8]) -> Vec<u8> {
    EnvelopeWriter::new(None, 0, 0)
        .write(Bytes::copy_from_slice(message))
        .expect("the envelope encodes")
}

/// The frames of a Connect streaming body, as `(flags, payload)`.
///
/// Flags `0x00` is a message; flags `0x02` is the end-stream frame.
#[must_use]
pub fn parse_envelopes(mut body: &[u8]) -> Vec<(u8, Vec<u8>)> {
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

/// An in-memory request body for `dispatch_to`.
pub fn body(bytes: Vec<u8>) -> RequestBody<'static> {
    foundation_connectrpc::in_memory_body(Some(SendSafeBody::Bytes(bytes)))
}

// ── the deterministic clock and timer ───────────────────────────────────────

/// A [`Clock`] that never reads real time. A case moves it by hand.
///
/// The monotonic reading starts at `origin` and moves only through
/// [`Self::advance`]. The wall clock starts at the UNIX epoch. The impl mirrors
/// the shape of `foundation_core`'s `SimulatedClock`, without its executor wake.
pub struct SimulatedClock {
    base: Instant,
    offset: Mutex<Duration>,
}

impl SimulatedClock {
    /// A clock whose monotonic reading starts at zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            // A real `Instant` has no public constructor. Code only compares
            // differences of `Instant`s, so the origin does not affect a run.
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
        }
    }

    /// Move the monotonic reading forward by `by`.
    pub fn advance(&self, by: Duration) {
        let mut offset = self
            .offset
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *offset += by;
    }
}

impl Default for SimulatedClock {
    fn default() -> Self {
        Self::new()
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

/// A [`DeadlineTimer`] that reads the injected clock.
///
/// It resolves once `deadline` has passed on `ctx.clock()`. A clock that never
/// moves keeps the future pending, which is correct: only the test moves the
/// deterministic clock (root rule 11). The cases that must fire the deadline
/// set a zero timeout, so the deadline equals the current reading.
pub struct SimulatedTimer;

impl DeadlineTimer for SimulatedTimer {
    fn sleep_until(&self, deadline: Instant, ctx: &Ctx) -> BoxFuture<'_, ()> {
        // Read the injected clock now. The future keeps no borrow of `ctx`, so
        // it can outlive this call, and a clock that never moves keeps it
        // pending (root rule 11).
        let now = ctx.clock().now();
        Box::pin(async move {
            if now >= deadline {
                return;
            }
            std::future::pending::<()>().await;
        })
    }
}

/// A context with the real host clock, for a case that reads no clock.
#[must_use]
pub fn ctx() -> Ctx {
    Ctx::host()
}

/// A deterministic context with a fixed seed and its own simulated clock.
#[must_use]
pub fn simulated_ctx() -> (Ctx, Arc<SimulatedClock>) {
    let clock = Arc::new(SimulatedClock::new());
    (Ctx::with_seed(clock.clone(), 7), clock)
}

// ── host detection and the transport base URL ───────────────────────────────

/// The host a case runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// Embedded Deno or the Deno `bindgen-deno` runner.
    Deno,
    /// The Cloudflare worker host.
    Workerd,
    /// A browser page (Chromium).
    Browser,
}

/// Detect the host from a global the runtime supplies.
#[must_use]
pub fn host() -> Host {
    let global = js_sys::global();
    if js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str("__CF_TEST_ENV__"))
        .map(|value| !value.is_undefined())
        .unwrap_or(false)
    {
        return Host::Workerd;
    }
    if js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str("Deno"))
        .map(|value| !value.is_undefined())
        .unwrap_or(false)
    {
        return Host::Deno;
    }
    Host::Browser
}

/// The origin of the Rust testbed server, when the host has one.
///
/// A browser page loads from the Rust server, so `location.origin` is the
/// endpoint. The web template also sets `__FWT_BASE_URL__` to that origin.
/// The Deno and workerd templates point that global at a different server, so
/// the transport cases skip those hosts and name the issue.
pub fn rpc_base_url() -> Option<String> {
    match host() {
        Host::Browser => {
            let global = js_sys::global();
            js_sys::Reflect::get(
                &global,
                &wasm_bindgen::JsValue::from_str("__FWT_BASE_URL__"),
            )
            .ok()
            .and_then(|value| value.as_string())
            .or_else(|| web_sys::window().and_then(|window| window.location().origin().ok()))
        }
        Host::Deno | Host::Workerd => None,
    }
}

// ── a small bounded wait, for the client to settle ──────────────────────────

/// Yield to the host once, so its fetch or timer callbacks can run.
pub async fn yield_host_turn() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let set_timeout =
            js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str("setTimeout"))
                .expect("setTimeout")
                .dyn_into::<js_sys::Function>()
                .expect("setTimeout is a function");
        set_timeout
            .call2(&global, &resolve, &wasm_bindgen::JsValue::from_f64(0.0))
            .expect("setTimeout");
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

// A helper the crate uses, kept here so the case file does not repeat it.
use wasm_bindgen::JsCast as _;

/// Await `future` while also driving the valtron engine, on the wasm host.
///
/// WHY: `H1Transport::open` schedules its exchange task on the valtron engine
/// (`valtron::send`). An async test future alone does not drive that engine.
/// `valtron::execute` cannot drive it from a wasm-bindgen test body: it hangs
/// under the Deno runner (foundation_netio `tests/wasm/websocket.rs`, #565).
///
/// HOW: poll the future, step the engine with `run_once`, then yield to the
/// host. The host turn lets a fetch promise settle. A no-op waker is enough
/// because the loop polls every turn.
///
/// # Errors
/// Returns the engine's scheduling error, or a timeout message.
pub async fn drive_on_valtron<T: 'static>(
    future: impl Future<Output = T> + 'static,
) -> Result<T, String> {
    use foundation_core::valtron::ProgressIndicator;
    use std::task::{Context, Poll};

    let _guard = foundation_core::valtron::initialize_pool(7, None);
    let mut future = Box::pin(future);
    let waker = futures::task::noop_waker();
    let mut task_cx = Context::from_waker(&waker);

    // Each turn is one host timer. 4,000 turns is far more than a loopback
    // fetch needs, and it still fails instead of hanging (rule R4).
    for _ in 0..4_000 {
        if let Poll::Ready(value) = future.as_mut().poll(&mut task_cx) {
            return Ok(value);
        }
        for _ in 0..64 {
            match foundation_core::valtron::single::run_once() {
                ProgressIndicator::NoWork | ProgressIndicator::Wait => break,
                ProgressIndicator::CanProgress(_) | ProgressIndicator::SpinWait(_) => {}
            }
        }
        yield_host_turn().await;
    }
    Err("the transport call did not finish within 4,000 host turns".to_string())
}

/// The remaining time to a call deadline, in milliseconds, as the handler reads
/// it (case 13).
#[must_use]
pub fn remaining_ms(call: &foundation_connectrpc::RpcCtx) -> Option<u64> {
    let deadline = call.request.deadline?;
    let left = deadline.saturating_duration_since(call.ctx.clock().now());
    Some(u64::try_from(left.as_millis()).unwrap_or(u64::MAX))
}
