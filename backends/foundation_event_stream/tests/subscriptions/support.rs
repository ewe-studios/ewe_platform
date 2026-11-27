//! Shared test support: a manual clock, a recording webhook sender and a harness.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, ContentType, CreateSpec, DurableStream, DurableStreamStore,
    DurableStreamVfs, MemoryVfs, StreamName,
};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::shared::{
    AckRequest, CreateRequest, SubscriptionConfig, SubscriptionEngine, WebhookFuture, WebhookInput,
    WebhookRequest, WebhookResponse, WebhookSender, WebhookSigningKey,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// A clock that the test moves by hand.
///
/// The monotonic `now()` moves with `advance`, so a timer on the injected clock fires only after
/// the test moves time. No test waits on real time.
#[derive(Clone)]
pub struct TestClock {
    origin: foundation_compact::Instant,
    millis: Arc<AtomicI64>,
}

/// The wall-clock start of every test, in UNIX milliseconds.
const TEST_CLOCK_ORIGIN_MS: i64 = 1_700_000_000_000;

impl TestClock {
    /// Start at UNIX time 1 700 000 000 000 milliseconds.
    pub fn new() -> Self {
        Self {
            origin: foundation_compact::Instant::now(),
            millis: Arc::new(AtomicI64::new(TEST_CLOCK_ORIGIN_MS)),
        }
    }

    /// Move the clock forward.
    pub fn advance(&self, millis: i64) {
        self.millis.fetch_add(millis, Ordering::SeqCst);
    }

    /// The current time in UNIX milliseconds.
    pub fn millis(&self) -> i64 {
        self.millis.load(Ordering::SeqCst)
    }

    /// The time moved since the clock started, in milliseconds.
    fn moved_ms(&self) -> u64 {
        u64::try_from((self.millis() - TEST_CLOCK_ORIGIN_MS).max(0)).unwrap_or(0)
    }
}

impl foundation_context::Clock for TestClock {
    fn now(&self) -> foundation_compact::Instant {
        self.origin + core::time::Duration::from_millis(self.moved_ms())
    }

    fn elapsed(&self) -> core::time::Duration {
        core::time::Duration::from_millis(self.moved_ms())
    }

    fn wall_now(&self) -> foundation_context::SystemTime {
        foundation_compact::time::from_unix_millis(self.millis())
    }
}

/// One scripted webhook outcome.
#[derive(Clone, Debug)]
pub enum WebhookOutcome {
    /// Answer with a status and a body.
    Response(u16, Vec<u8>),
    /// Fail before a response.
    TransportError,
}

/// A webhook sender that records every request and replays scripted outcomes.
pub struct RecordingSender {
    outcomes: Mutex<VecDeque<WebhookOutcome>>,
    requests: Mutex<Vec<WebhookRequest>>,
}

impl RecordingSender {
    /// Build an empty sender. The default outcome is `200 {}`.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            outcomes: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
        })
    }

    /// Queue one outcome.
    pub fn push(&self, outcome: WebhookOutcome) {
        self.outcomes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(outcome);
    }

    /// Every request, in order.
    pub fn requests(&self) -> Vec<WebhookRequest> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The last request.
    pub fn last_request(&self) -> Option<WebhookRequest> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last()
            .cloned()
    }
}

impl WebhookSender for RecordingSender {
    fn send(&self, request: WebhookRequest) -> WebhookFuture<'_> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request);
        let outcome = self
            .outcomes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| WebhookOutcome::Response(200, b"{}".to_vec()));
        Box::pin(async move {
            match outcome {
                WebhookOutcome::Response(status, body) => Ok(WebhookResponse { status, body }),
                WebhookOutcome::TransportError => Err("transport error".to_string()),
            }
        })
    }
}

/// One engine and the stream store over the same observed virtual file system.
pub struct Harness<S: WebhookSender = RecordingSender> {
    /// The subscription engine.
    pub engine: SubscriptionEngine,
    /// The manual clock.
    pub clock: Arc<TestClock>,
    /// The webhook sender.
    pub sender: Arc<S>,
    /// The Durable Streams store over the observed virtual file system.
    pub stream: DurableStream,
}

/// Build a harness with an in-memory virtual file system and a recording sender.
pub fn harness() -> Harness {
    harness_with(SubscriptionConfig::new("v1/stream"), RecordingSender::new())
}

/// Build a harness with a configuration and a webhook sender.
pub fn harness_with(config: SubscriptionConfig, sender: Arc<RecordingSender>) -> Harness {
    let clock = Arc::new(TestClock::new());
    harness_with_clock(clock, config, sender)
}

/// Build a harness over a clock that the caller shares with a custom sender.
pub fn harness_with_clock<S: WebhookSender + 'static>(
    clock: Arc<TestClock>,
    config: SubscriptionConfig,
    sender: Arc<S>,
) -> Harness<S> {
    let ctx = Ctx::fixed(clock.clone());
    // The delivery timeout needs the injected timer. The test environment uses the valtron
    // timer, which reads the manual clock, so no test waits on real time.
    let env = foundation_core::env::simulated(&ctx, 7).expect("the simulated effect environment");
    ctx.store(env);
    let vfs: Arc<dyn DurableStreamVfs> = Arc::new(MemoryVfs::new());
    let signing = WebhookSigningKey::from_seed([7_u8; 32]);
    let sender_trait: Arc<dyn WebhookSender> = sender.clone();
    let engine = SubscriptionEngine::new(vfs, ctx.clone(), signing, sender_trait, config);
    let stream = DurableStream::with_ctx(engine.observed_vfs(), &ctx);
    Harness {
        engine,
        clock,
        sender,
        stream,
    }
}

/// Create a stream with an optional initial body.
pub fn create_stream<S: WebhookSender>(
    harness: &Harness<S>,
    path: &str,
    json: bool,
    body: Option<&[u8]>,
) {
    let name = StreamName::parse(path).expect("valid stream name");
    let content_type = ContentType::parse(if json {
        "application/json"
    } else {
        "application/octet-stream"
    })
    .expect("valid content type");
    let spec = CreateSpec {
        content_type: Some(content_type),
        initial: body.map(|body| Chunk::new(body.to_vec())),
        ..CreateSpec::default()
    };
    block_on_future_scoped(harness.stream.create(name, spec)).expect("create stream");
}

/// Append one body to a stream.
pub fn append_stream<S: WebhookSender>(harness: &Harness<S>, path: &str, body: &[u8]) {
    let name = StreamName::parse(path).expect("valid stream name");
    block_on_future_scoped(
        harness
            .stream
            .append(&name, AppendRequest::new(Chunk::new(body.to_vec()))),
    )
    .expect("append stream");
}

/// Build a webhook create request.
pub fn webhook_request(pattern: &str, url: &str, lease_ttl_ms: Option<u64>) -> CreateRequest {
    CreateRequest {
        kind: "webhook".to_string(),
        pattern: Some(pattern.to_string()),
        streams: Vec::new(),
        webhook: Some(WebhookInput {
            url: url.to_string(),
        }),
        wake_stream: None,
        lease_ttl_ms,
        description: None,
    }
}

/// Build a pull-wake create request.
pub fn pull_request(pattern: &str, wake_stream: &str, lease_ttl_ms: Option<u64>) -> CreateRequest {
    CreateRequest {
        kind: "pull-wake".to_string(),
        pattern: Some(pattern.to_string()),
        streams: Vec::new(),
        webhook: None,
        wake_stream: Some(wake_stream.to_string()),
        lease_ttl_ms,
        description: None,
    }
}

/// Build an ack body.
pub fn ack_request(
    wake_id: &str,
    generation: u64,
    stream: &str,
    offset: &str,
    done: bool,
) -> AckRequest {
    AckRequest {
        wake_id: Some(wake_id.to_string()),
        generation: Some(generation),
        acks: Some(vec![foundation_event_stream::shared::AckEntry {
            stream: Some(stream.to_string()),
            path: None,
            offset: offset.to_string(),
        }]),
        done,
    }
}
