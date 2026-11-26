//! Spec 83 feature 00 T6: the Cloudflare Worker examples under miniflare
//! (`wrangler dev --local`), against a mocked vendor — no live keys.
//!
//! WHY: the examples run `foundation_ai` inside a single-threaded wasm isolate,
//! where a blocking call or a busy-polling task freezes the whole Worker (spec 83
//! feature 06 T5). Compile gates cannot see that; only running the Worker can.
//!
//! WHAT: for each example, build it with `worker-build --release`, start an
//! in-process mock of the vendor API that streams a fixed answer over SSE, run the
//! Worker under [`Miniflare`] pointed at the mock, send a prompt, and check that
//! - the answer comes back,
//! - the vendor was called exactly once (one generation per prompt), and
//! - the Worker still serves `GET /` afterwards (the isolate is not wedged).
//!
//! Opt-in (`--features cf-worker-tests`): needs `worker-build`
//! (`cargo install worker-build`), the `wasm32-unknown-unknown` target, and
//! `wrangler` (or `npx`) on `PATH`.
//!
//! ```bash
//! cargo test -p foundation_toolings --features cf-worker-tests --test cf_examples
//! ```

#![cfg(feature = "cf-worker-tests")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_toolings::harness::Miniflare;

/// The answer every mock streams, one word per SSE event.
const WORDS: [&str; 7] = ["The", " capital", " of", " France", " is", " Paris", "."];
const ANSWER: &str = "The capital of France is Paris.";

/// How long one example may take to answer a prompt under miniflare.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(60);

/// Which vendor wire format the mock speaks.
#[derive(Clone, Copy)]
enum Vendor {
    /// OpenAI-compatible chat completions (`delta.content` chunks, `[DONE]`).
    OpenRouter,
    /// Workers AI REST (`{"response": "<text so far>"}` chunks, ends on EOF).
    WorkersAi,
}

/// A mock vendor API on `127.0.0.1:<port>`: answers every POST with [`WORDS`]
/// streamed as SSE, and counts the requests.
struct MockVendor {
    port: u16,
    requests: Arc<AtomicUsize>,
}

impl MockVendor {
    fn start(vendor: Vendor) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock vendor");
        let port = listener.local_addr().expect("mock vendor addr").port();
        let requests = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let count = Arc::clone(&count);
                std::thread::spawn(move || serve(&stream, vendor, &count));
            }
        });
        Self { port, requests }
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

/// Read one request (head + `Content-Length` body), then stream the answer.
fn serve(stream: &TcpStream, vendor: Vendor, requests: &AtomicUsize) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone mock stream"));
    let mut content_length = 0usize;
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; content_length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    if !request_line.starts_with("POST ") {
        let _ = (&*stream).write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    }
    requests.fetch_add(1, Ordering::SeqCst);

    let mut out = stream;
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                Cache-Control: no-cache\r\nConnection: close\r\n\r\n";
    if out.write_all(head.as_bytes()).is_err() {
        return;
    }
    let mut so_far = String::new();
    for word in WORDS {
        so_far.push_str(word);
        let data = match vendor {
            Vendor::OpenRouter => serde_json::json!({
                "id": "mock-1",
                "object": "chat.completion.chunk",
                "created": 0,
                "model": "mock",
                "choices": [{"index": 0, "delta": {"content": word}, "finish_reason": null}],
            }),
            Vendor::WorkersAi => serde_json::json!({ "response": word }),
        };
        if out
            .write_all(format!("data: {data}\n\n").as_bytes())
            .is_err()
        {
            return;
        }
        let _ = out.flush();
        // Spread the chunks out so the Worker really waits on the network.
        std::thread::sleep(Duration::from_millis(50));
    }
    if let Vendor::OpenRouter = vendor {
        let done = serde_json::json!({
            "id": "mock-1",
            "object": "chat.completion.chunk",
            "created": 0,
            "model": "mock",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 7, "total_tokens": 12},
        });
        let _ = out.write_all(format!("data: {done}\n\ndata: [DONE]\n\n").as_bytes());
    }
    if let Vendor::WorkersAi = vendor {
        // Real Workers AI closes the stream with an empty `response` delta and
        // a `[DONE]` sentinel.
        let _ = out.write_all(b"data: {\"response\":\"\"}\n\ndata: [DONE]\n\n");
    }
    let _ = out.flush();
}

fn example_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

/// `worker-build --release` in `dir`, failing the test with its output.
fn worker_build(dir: &Path) {
    let output = Command::new("worker-build")
        .arg("--release")
        .current_dir(dir)
        .output()
        .expect("run worker-build (cargo install worker-build)");
    assert!(
        output.status.success(),
        "worker-build failed in {}:\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The concatenated `data:` payloads of an SSE body.
fn sse_text(body: &str) -> String {
    body.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .collect()
}

/// `cf-cloud-provider`: `OpenRouter` via the fetch client (`HttpClientBuilder`), reply streamed as SSE.
#[test]
fn cf_cloud_provider_streams_one_generation_and_stays_responsive() {
    let dir = example_dir("cf-cloud-provider");
    worker_build(&dir);
    let vendor = MockVendor::start(Vendor::OpenRouter);

    let worker = Miniflare::builder()
        .worker_dir(&dir)
        .var("OPENROUTER_API_KEY", "mock")
        .var(
            "OPENROUTER_BASE_URL",
            format!("http://127.0.0.1:{}/api", vendor.port),
        )
        .startup_timeout(Duration::from_secs(300))
        .build(Ctx::host())
        .expect("start cf-cloud-provider under miniflare");

    let (status, body) = worker
        .post("/chat", "What is the capital of France?", PROMPT_TIMEOUT)
        .expect("POST /chat");
    assert_eq!(
        status,
        200,
        "body: {body}\nworker stderr:\n{}",
        worker.stderr_tail()
    );
    assert_eq!(
        sse_text(&body),
        ANSWER,
        "the streamed deltas must add up to the answer once\nbody: {body}"
    );
    assert_eq!(vendor.requests(), 1, "one prompt must be one model call");

    let (status, _) = worker.get("/").expect("GET / after a chat turn");
    assert_eq!(
        status, 200,
        "the isolate must keep serving after a chat turn"
    );
}

/// `cf-agent-loop`: Workers AI via the fetch client (`HttpClientBuilder`), reply returned whole.
#[test]
fn cf_agent_loop_answers_one_generation_and_stays_responsive() {
    let dir = example_dir("cf-agent-loop");
    worker_build(&dir);
    let vendor = MockVendor::start(Vendor::WorkersAi);

    let worker = Miniflare::builder()
        .worker_dir(&dir)
        .var("WORKERS_AI_API_TOKEN", "mock")
        .var("CLOUDFLARE_ACCOUNT_ID", "mock-account")
        .var(
            "WORKERS_AI_BASE_URL",
            format!("http://127.0.0.1:{}", vendor.port),
        )
        .startup_timeout(Duration::from_secs(300))
        .build(Ctx::host())
        .expect("start cf-agent-loop under miniflare");

    let (status, body) = worker
        .post("/", "What is the capital of France?", PROMPT_TIMEOUT)
        .expect("POST /");
    assert_eq!(
        status,
        200,
        "body: {body}\nworker stderr:\n{}",
        worker.stderr_tail()
    );
    assert_eq!(body, ANSWER, "the reply must be the answer once");
    assert_eq!(vendor.requests(), 1, "one prompt must be one model call");

    let (status, _) = worker.get("/").expect("GET / after a prompt");
    assert_eq!(status, 200, "the isolate must keep serving after a prompt");
}

/// One protobuf length-delimited string field.
fn proto_string(field: u8, value: &str) -> Vec<u8> {
    let mut out = vec![(field << 3) | 2];
    let mut len = value.len();
    loop {
        let mut byte = u8::try_from(len & 0x7f).unwrap_or(0x7f);
        len >>= 7;
        if len != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if len == 0 {
            break;
        }
    }
    out.extend_from_slice(value.as_bytes());
    out
}

/// One Connect envelope (flag 0, big-endian length, message).
fn connect_envelope(message: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8];
    out.extend_from_slice(&u32::try_from(message.len()).unwrap_or(0).to_be_bytes());
    out.extend_from_slice(message);
    out
}

/// A bearer access secret for the test host, and its public key.
fn access_secret(ctx: &Ctx) -> (String, String) {
    foundation_core::env::install(ctx, foundation_core::env::system).expect("install the Env");
    use base64::Engine;
    use foundation_auth::secret::{SecretDomain, SecretIssuer, SecretRequest, SigningKey};
    use foundation_compact::Duration;
    let key = SigningKey::from_seed([7u8; 32], Some("k1".to_string()));
    let public =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.verifying_key().as_bytes());
    let issuer = SecretIssuer::new(
        SecretDomain::AGENT_RPC,
        key,
        ctx.clone(),
        Duration::from_hours(24),
    );
    let token = issuer
        .issue(&SecretRequest::access(
            Duration::from_hours(1),
            "https://agents.example.com",
        ))
        .expect("issue the access secret")
        .encode()
        .to_string();
    (token, public)
}

/// `cf-agent-rpc`: the Worker starts, answers CORS, refuses a forged header,
/// then serves a full lifecycle with a bearer secret: health, create, stream,
/// list, destroy.
#[test]
fn cf_agent_rpc_serves_connect() {
    if which::which("worker-build").is_err() || which::which("wrangler").is_err() {
        eprintln!("cf-agent-rpc: worker-build or wrangler is absent; skipping");
        return;
    }
    let dir = example_dir("cf-agent-rpc");
    worker_build(&dir);

    let vendor = MockVendor::start(Vendor::WorkersAi);
    let ctx = Ctx::host();
    let (token, public_key) = access_secret(&ctx);
    let models = format!(
        "[{{\"name\":\"mock\",\"provider\":\"workers-ai\",\"model_id\":\"@cf/meta/llama-3.1-8b-instruct\",\"api_key_secret\":\"WORKERS_AI_API_TOKEN\",\"account_id\":\"mock\",\"base_url\":\"http://127.0.0.1:{}\"}}]",
        vendor.port
    );
    let worker = Miniflare::builder()
        .worker_dir(&dir)
        .compatibility_date("2025-09-01")
        .var("AGENT_RPC_PUBLIC_URL", "https://agents.example.com")
        .var("AGENT_RPC_MODELS", models)
        .var(
            "AGENT_RPC_SECRET_PUBLIC_KEYS",
            format!("[{{\"kid\":\"k1\",\"alg\":\"ed25519\",\"public_key\":\"{public_key}\"}}]"),
        )
        .var("AGENT_RPC_CORS_ORIGINS", "https://app.example.com")
        .secret("WORKERS_AI_API_TOKEN", "mock")
        .secret("CLOUDFLARE_ACCOUNT_ID", "mock")
        .startup_timeout(Duration::from_secs(300))
        .build(ctx)
        .expect("start cf-agent-rpc under miniflare");

    let (status, head, _body) = worker
        .request(
            "OPTIONS",
            "/ewe.agent_rpc.v1.AgentService/Health",
            &[
                ("Origin", "https://app.example.com"),
                ("Access-Control-Request-Method", "POST"),
            ],
            "",
            Duration::from_secs(10),
        )
        .expect("the CORS preflight");
    assert_eq!(
        status,
        204,
        "the preflight status\nworker stderr:\n{}",
        worker.stderr_tail()
    );
    let head = head.to_ascii_lowercase();
    assert!(
        head.contains("access-control-allow-origin: https://app.example.com"),
        "the preflight must name the origin; head:\n{head}"
    );

    let (status, _head, body) = worker
        .request(
            "POST",
            "/ewe.agent_rpc.v1.AgentService/Health",
            &[
                ("Origin", "https://app.example.com"),
                ("Content-Type", "application/proto"),
                ("ewe-forwarded-principal", "evil"),
            ],
            "",
            Duration::from_secs(20),
        )
        .expect("the forged-principal call");
    assert_eq!(
        status,
        401,
        "a forged forwarded header must not authenticate; body: {body}\nworker stderr:\n{}",
        worker.stderr_tail()
    );

    let bearer = format!("Bearer {token}");
    let (status, _head, body) = worker
        .request_bytes(
            "POST",
            "/ewe.agent_rpc.v1.AgentService/Health",
            &[
                ("Content-Type", "application/proto"),
                ("Authorization", bearer.as_str()),
            ],
            &[],
            Duration::from_secs(20),
        )
        .expect("Health with the access secret");
    assert_eq!(
        status,
        200,
        "the bearer secret authenticates; body: {body:?}\nworker stderr:\n{}",
        worker.stderr_tail()
    );

    let mut create = proto_string(1, "case-one");
    create.extend(proto_string(2, "mock"));

    let (created_status, _head, created_body) = worker
        .request_bytes(
            "POST",
            "/ewe.agent_rpc.v1.AgentService/CreateInstance",
            &[
                ("Content-Type", "application/proto"),
                ("Authorization", bearer.as_str()),
            ],
            &create,
            Duration::from_secs(30),
        )
        .expect("CreateInstance");
    assert_eq!(
        created_status,
        200,
        "the instance is created; body: {created_body:?}\nworker stderr:\n{}",
        worker.stderr_tail()
    );

    let (list_status, _head, list_body) = worker
        .request_bytes(
            "POST",
            "/ewe.agent_rpc.v1.AgentService/ListInstances",
            &[
                ("Content-Type", "application/proto"),
                ("Authorization", bearer.as_str()),
            ],
            &[],
            Duration::from_secs(20),
        )
        .expect("ListInstances");
    assert_eq!(list_status, 200, "ListInstances succeeds");
    assert!(
        list_body.windows(8).any(|window| window == b"case-one"),
        "the list names the instance; body: {list_body:?}"
    );

    let mut chat = proto_string(1, "case-one");
    chat.extend(proto_string(2, "What is the capital of France?"));
    let envelope = connect_envelope(&chat);
    let (stream_status, _head, stream_body) = worker
        .request_bytes(
            "POST",
            "/ewe.agent_rpc.v1.AgentService/ChatStream",
            &[
                ("Content-Type", "application/connect+proto"),
                ("Authorization", bearer.as_str()),
            ],
            &envelope,
            Duration::from_secs(60),
        )
        .expect("ChatStream");
    assert_eq!(
        stream_status,
        200,
        "ChatStream succeeds; body: {stream_body:?}\nworker stderr:\n{}",
        worker.stderr_tail()
    );
    assert!(
        !stream_body.is_empty(),
        "the turn streamed events; stderr:\n{}",
        worker.stderr_tail()
    );

    let destroy = proto_string(1, "case-one");
    let (destroy_status, _head, destroy_body) = worker
        .request_bytes(
            "POST",
            "/ewe.agent_rpc.v1.AgentService/DestroyInstance",
            &[
                ("Content-Type", "application/proto"),
                ("Authorization", bearer.as_str()),
            ],
            &destroy,
            Duration::from_secs(30),
        )
        .expect("DestroyInstance");
    assert_eq!(
        destroy_status,
        200,
        "the instance is destroyed; body: {destroy_body:?}\nworker stderr:\n{}",
        worker.stderr_tail()
    );
}
