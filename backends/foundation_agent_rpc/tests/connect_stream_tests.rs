//! T7 — `ChatStream` over Connect on real loopback TCP, proven unbuffered.
//!
//! WHY: a long answer must reach the client token by token, never assembled
//! server-side first.
//!
//! HOW: a gated model emits token `k + 1` only after the *client* has received
//! token `k`. A server (or client) that buffers the stream before forwarding
//! never opens the gate, the gate times out, and the turn fails.
//!
//! The gate never blocks a thread: the model yields `Stream::Delayed` until it
//! may continue. And the unbuffered proof runs Connect over h2c: the HTTP/1.1
//! server blocks its valtron worker in `poll(2)` for a whole call, and a client
//! task parked on that same worker (client and server share the pool in a
//! test) then stalls until the call ends — a known starvation bug (LEARNINGS
//! 01 T7, backlog), not buffering. HTTP/1.1 streaming is still round-tripped
//! below with an ungated producer.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event;
use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_agent_rpc::proto::v1::{ChatRequest, CreateInstanceRequest};
use foundation_agent_rpc::{AgentRpcServer, ModelHandle};
use foundation_ai::agentic::testing::{mock_text, zero_usage, MockModelProvider};
use foundation_ai::errors::GenerationResult;
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelParams, ModelProviderDescriptor, ModelSpec,
    ModelState, ModelStreamBox, TextBasedFormatter, ToolFormatter, UsageReport,
};
use foundation_connectrpc::{
    ClientOptions, Code, Ctx, H1Transport, H2Transport, Request, Transport,
};
use foundation_core::valtron::{valtron_test, Stream, WakeableStream};
use foundation_netio::http::NativeHttpClient;

const TOKENS: [&str; 5] = ["alpha ", "bravo ", "charlie ", "delta ", "echo"];
const GATE_TIMEOUT: Duration = Duration::from_secs(10);

/// Counts the tokens the client has received.
#[derive(Default)]
struct Gate {
    seen: AtomicUsize,
}

impl Gate {
    fn client_saw_one(&self) {
        self.seen.fetch_add(1, Ordering::SeqCst);
    }

    fn client_has_seen(&self, n: usize) -> bool {
        self.seen.load(Ordering::SeqCst) >= n
    }
}

#[derive(Clone)]
struct GatedModel {
    gate: Arc<Gate>,
}

impl Model for GatedModel {
    fn spec(&self) -> ModelSpec {
        ModelSpec {
            name: "gated".into(),
            id: ModelId::Name("gated".into(), None),
            devices: None,
            model_location: None,
            lora_location: None,
        }
    }

    fn tool_formatter(&self) -> Box<dyn ToolFormatter> {
        Box::new(TextBasedFormatter)
    }

    fn descriptor(&self) -> Option<ModelProviderDescriptor> {
        None
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        Ok(zero_usage())
    }

    fn generate(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        Ok(vec![mock_text(&TOKENS.concat())])
    }

    fn stream(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<ModelStreamBox> {
        Ok(Box::new(GatedStream {
            gate: Arc::clone(&self.gate),
            next: 0,
            started: false,
            done: false,
            waiting_since: None,
        }))
    }
}

struct GatedStream {
    gate: Arc<Gate>,
    next: usize,
    started: bool,
    done: bool,
    waiting_since: Option<Instant>,
}

impl WakeableStream for GatedStream {}

impl Iterator for GatedStream {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if !self.started {
            self.started = true;
            return Some(Stream::Init);
        }
        if self.next == TOKENS.len() {
            self.done = true;
            return Some(Stream::Pending(ModelState::Finished));
        }
        if self.next > 0 && !self.gate.client_has_seen(self.next) {
            let since = *self.waiting_since.get_or_insert_with(Instant::now);
            if since.elapsed() > GATE_TIMEOUT {
                self.done = true;
                return Some(Stream::Pending(ModelState::Error(format!(
                    "client never received token {} — the stream is buffered",
                    self.next
                ))));
            }
            // Not yet: ask to be polled again shortly, without blocking a worker.
            return Some(Stream::Delayed(Duration::from_millis(2)));
        }
        self.waiting_since = None;
        let token = TOKENS[self.next];
        self.next += 1;
        Some(Stream::Next(mock_text(token)))
    }
}

fn client(base_url: &str) -> AgentServiceClient {
    let transport: Arc<dyn Transport> =
        Arc::new(H1Transport::new(Arc::new(NativeHttpClient::from_system())));
    AgentServiceClient::new(transport, base_url, ClientOptions::new().with_connect())
        .expect("client")
}

/// Connect over HTTP/2 cleartext.
fn h2_client(base_url: &str) -> AgentServiceClient {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    AgentServiceClient::new(transport, base_url, ClientOptions::new().with_connect())
        .expect("client")
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(30))
}

#[valtron_test(threads = 12)]
async fn chat_stream_delivers_each_token_before_the_next_is_produced() {
    let gate = Arc::new(Gate::default());
    let model = ModelHandle::from_model(
        "gated",
        GatedModel {
            gate: Arc::clone(&gate),
        },
    );
    let server = AgentRpcServer::builder()
        .with_model(model)
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves");
    let client = h2_client(&server.base_url());

    client
        .create_instance(
            ctx(),
            Request::new(CreateInstanceRequest {
                name: "alpha".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("create");

    let mut stream = client
        .chat_stream(
            ctx(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "talk to me".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("open stream");

    let mut texts = Vec::new();
    let mut done = None;
    while let Some(event) = stream.receive().await.expect("receive") {
        match event.event {
            Some(Event::Text(text)) => {
                texts.push(text);
                gate.client_saw_one();
            }
            Some(Event::Done(summary)) => done = Some(summary),
            Some(Event::Failed(failed)) => panic!("turn failed: {}", failed.message),
            _ => {}
        }
    }

    assert_eq!(
        texts,
        TOKENS.map(String::from).to_vec(),
        "every token, in order"
    );
    let done = done.expect("a terminal Done event");
    assert!(done.message_count > 0);

    server.graceful_shutdown().expect("clean shutdown");
}

/// Connect streaming over HTTP/1.1: every event arrives, then `Done`.
#[valtron_test(threads = 8)]
async fn chat_stream_round_trips_over_http1() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("one "), mock_text("two")]);
    let server = AgentRpcServer::builder()
        .with_model(ModelHandle::from_router("mock", "mock", mock.into_router()))
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves");
    let client = client(&server.base_url());
    client
        .create_instance(
            ctx(),
            Request::new(CreateInstanceRequest {
                name: "alpha".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("create");
    let mut stream = client
        .chat_stream(
            ctx(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "count".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("open stream");
    let mut texts = Vec::new();
    let mut done = false;
    while let Some(event) = stream.receive().await.expect("receive") {
        match event.event {
            Some(Event::Text(t)) => texts.push(t),
            Some(Event::Done(_)) => done = true,
            Some(Event::Failed(f)) => panic!("turn failed: {}", f.message),
            _ => {}
        }
    }
    assert_eq!(texts, vec!["one ".to_string(), "two".to_string()]);
    assert!(done, "a terminal Done event");
    server.graceful_shutdown().expect("clean shutdown");
}

/// The same gate without the network: the service's event stream itself
/// yields each token as it is produced.
#[valtron_test(threads = 8)]
async fn the_service_stream_is_unbuffered_in_process() {
    use foundation_agent_rpc::proto::v1::agent_service::AgentService;
    use futures::StreamExt;

    let gate = Arc::new(Gate::default());
    let server = AgentRpcServer::builder()
        .with_model(ModelHandle::from_model(
            "gated",
            GatedModel {
                gate: Arc::clone(&gate),
            },
        ))
        .insecure_plaintext()
        .build()
        .expect("server builds");
    let svc = Arc::clone(server.service());
    svc.create_instance(
        ctx(),
        Request::new(CreateInstanceRequest {
            name: "alpha".into(),
            ..Default::default()
        }),
    )
    .await
    .expect("create");
    let mut events = svc
        .chat_stream(
            ctx(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("stream");
    let mut texts = Vec::new();
    while let Some(event) = events.next().await {
        match event.expect("event").event {
            Some(Event::Text(t)) => {
                texts.push(t);
                gate.client_saw_one();
            }
            Some(Event::Failed(f)) => panic!("turn failed: {}", f.message),
            _ => {}
        }
    }
    assert_eq!(texts.len(), TOKENS.len());
}

#[valtron_test(threads = 8)]
async fn chat_stream_on_a_missing_instance_is_not_found() {
    let gate = Arc::new(Gate::default());
    let server = AgentRpcServer::builder()
        .with_model(ModelHandle::from_model("gated", GatedModel { gate }))
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves");
    let client = client(&server.base_url());

    let opened = client
        .chat_stream(
            ctx(),
            Request::new(ChatRequest {
                instance: "nobody".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await;
    let err = match opened {
        Err(err) => err,
        Ok(mut stream) => stream.receive().await.expect_err("stream must fail"),
    };
    assert_eq!(err.current_context().code(), Code::NotFound);

    server.graceful_shutdown().expect("clean shutdown");
}
