//! Shared test support: a manual clock, a recording webhook sender and a harness.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, ContentType, CreateSpec, DurableStream, DurableStreamStore,
    DurableStreamVfs, MemoryVfs, StreamName,
};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::durable;
use foundation_event_stream::shared::{
    AckRequest, CreateRequest, SubscriptionConfig, SubscriptionEngine, WebhookFuture, WebhookInput,
    WebhookRequest, WebhookResponse, WebhookSender, WebhookSigningKey,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// A clock that the test moves by hand.
#[derive(Clone)]
pub struct TestClock {
    millis: Arc<AtomicI64>,
}

impl TestClock {
    /// Start at UNIX time 1 700 000 000 000 milliseconds.
    pub fn new() -> Self {
        Self {
            millis: Arc::new(AtomicI64::new(1_700_000_000_000)),
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
}

impl foundation_context::Clock for TestClock {
    fn now(&self) -> foundation_compact::Instant {
        foundation_compact::Instant::now()
    }

    fn elapsed(&self) -> core::time::Duration {
        core::time::Duration::ZERO
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
pub struct Harness {
    /// The subscription engine.
    pub engine: SubscriptionEngine,
    /// The manual clock.
    pub clock: Arc<TestClock>,
    /// The recording webhook sender.
    pub sender: Arc<RecordingSender>,
    /// The Durable Streams store over the observed virtual file system.
    pub stream: DurableStream,
}

/// Build a harness with an in-memory virtual file system.
pub fn harness() -> Harness {
    let clock = Arc::new(TestClock::new());
    let ctx = Ctx::fixed(clock.clone());
    let vfs: Arc<dyn DurableStreamVfs> = Arc::new(MemoryVfs::new());
    let sender = RecordingSender::new();
    let signing = WebhookSigningKey::from_seed([7_u8; 32]);
    let config = SubscriptionConfig::new("v1/stream");
    let engine = SubscriptionEngine::new(vfs, ctx.clone(), signing, sender.clone(), config);
    let stream = DurableStream::with_ctx(engine.observed_vfs(), &ctx);
    Harness {
        engine,
        clock,
        sender,
        stream,
    }
}

/// Create a stream with an optional initial body.
pub fn create_stream(harness: &Harness, path: &str, json: bool, body: Option<&[u8]>) {
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
pub fn append_stream(harness: &Harness, path: &str, body: &[u8]) {
    let name = StreamName::parse(path).expect("valid stream name");
    block_on_future_scoped(
        harness
            .stream
            .append(&name, AppendRequest::new(Chunk::new(body.to_vec()))),
    )
    .expect("append stream");
}

/// Read a stream's tail offset as wire text.
pub fn tail_offset(harness: &Harness, path: &str) -> String {
    let name = StreamName::parse(path).expect("valid stream name");
    let head = block_on_future_scoped(harness.stream.head(&name)).expect("head stream");
    durable::wire::offset_text(&head.tail)
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
