//! Spec 65 feature 00 T6: the Cloudflare Worker examples under miniflare
//! (`wrangler dev --local`), against a mocked vendor — no live keys.
//!
//! WHY: the examples run `foundation_ai` inside a single-threaded wasm isolate,
//! where a blocking call or a busy-polling task freezes the whole Worker (spec 65
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
            Vendor::WorkersAi => serde_json::json!({ "response": so_far }),
        };
        if out.write_all(format!("data: {data}\n\n").as_bytes()).is_err() {
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

/// `cf-cloud-provider`: `OpenRouter` via `FetchHttpClient`, reply streamed as SSE.
#[test]
fn cf_cloud_provider_streams_one_generation_and_stays_responsive() {
    let dir = example_dir("cf-cloud-provider");
    worker_build(&dir);
    let vendor = MockVendor::start(Vendor::OpenRouter);

    let worker = Miniflare::builder()
        .worker_dir(&dir)
        .var("OPENROUTER_API_KEY", "mock")
        .var("OPENROUTER_BASE_URL", format!("http://127.0.0.1:{}/api", vendor.port))
        .startup_timeout(Duration::from_secs(90))
        .build()
        .expect("start cf-cloud-provider under miniflare");

    let (status, body) = worker
        .post("/generate", "What is the capital of France?", PROMPT_TIMEOUT)
        .expect("POST /generate");
    assert_eq!(status, 200, "body: {body}\nworker stderr:\n{}", worker.stderr_tail());
    assert_eq!(
        sse_text(&body),
        ANSWER,
        "the streamed deltas must add up to the answer once\nbody: {body}"
    );
    assert_eq!(vendor.requests(), 1, "one prompt must be one model call");

    let (status, _) = worker.get("/").expect("GET / after a generate");
    assert_eq!(status, 200, "the isolate must keep serving after a generate");
}

/// `cf-agent-loop`: Workers AI via `FetchHttpClient`, reply returned whole.
#[test]
fn cf_agent_loop_answers_one_generation_and_stays_responsive() {
    let dir = example_dir("cf-agent-loop");
    worker_build(&dir);
    let vendor = MockVendor::start(Vendor::WorkersAi);

    let worker = Miniflare::builder()
        .worker_dir(&dir)
        .var("WORKERS_AI_API_TOKEN", "mock")
        .var("CLOUDFLARE_ACCOUNT_ID", "mock-account")
        .var("WORKERS_AI_BASE_URL", format!("http://127.0.0.1:{}", vendor.port))
        .startup_timeout(Duration::from_secs(90))
        .build()
        .expect("start cf-agent-loop under miniflare");

    let (status, body) = worker
        .post("/", "What is the capital of France?", PROMPT_TIMEOUT)
        .expect("POST /");
    assert_eq!(status, 200, "body: {body}\nworker stderr:\n{}", worker.stderr_tail());
    assert_eq!(body, ANSWER, "the reply must be the answer once");
    assert_eq!(vendor.requests(), 1, "one prompt must be one model call");

    let (status, _) = worker.get("/").expect("GET / after a prompt");
    assert_eq!(status, 200, "the isolate must keep serving after a prompt");
}
