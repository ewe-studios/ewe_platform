#![allow(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

//! `foundation_ai` deployed on Cloudflare Workers, calling OpenRouter through
//! `FetchHttpClient` (wasm `fetch`) and streaming the reply back as
//! Server-Sent Events (spec 65 T5, `cf-cloud-provider`).
//!
//! Endpoints:
//! - `GET /` — a small HTML page that POSTs a prompt to `/generate`.
//! - `POST /generate` — streams the agentic turn as an SSE body.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use futures::StreamExt;
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use foundation_ai::agentic::{AgentSession, Turn};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{ModelId, ModelProvider};
use foundation_core::valtron::{initialize_pool, PoolGuard, StreamIteratorExt};
use foundation_netio::wasm::client::FetchHttpClient;

/// OpenRouter free model (see `foundation_ai/examples/hello_openrouter.rs`).
///
/// `google/gemma-4-26b-a4b-it:free` (the original T5 default) is frequently
/// rate-limited on OpenRouter's shared free pool, so we default to
/// `google/gemma-4-31b-it:free`, a plain chat model that returns `delta.content`
/// (no reasoning-only output). Override at runtime with the `MODEL` binding.
const DEFAULT_MODEL: &str = "google/gemma-4-31b-it:free";
const SYSTEM_PROMPT: &str = "You are a concise, helpful assistant.";

/// valtron single-threaded executor, initialised once per isolate. The
/// `js-wasmbindgen` feature selects `JSThreadYielder`, so parked tasks yield to
/// the JS event loop via `setTimeout` instead of busy-spinning.
static POOL_GUARD: OnceLock<PoolGuard> = OnceLock::new();

thread_local! {
    static SESSION: RefCell<Option<AgentSession>> = RefCell::new(None);
}

/// Read the OpenRouter key from a Worker secret, falling back to an env var.
fn read_api_key(env: &worker::Env) -> String {
    env.secret("OPENROUTER_API_KEY")
        .map(|s| s.to_string())
        .or_else(|_| env.var("OPENROUTER_API_KEY").map(|v| v.to_string()))
        .unwrap_or_default()
}

/// Optional vendor base-URL override (for the miniflare smoke test's mocked
/// vendor `fetch`). When unset, `OpenAIConfig::openrouter` keeps its real
/// `https://openrouter.ai/api/v1` base URL.
fn read_base_url(env: &worker::Env) -> Option<String> {
    let url = env
        .secret("OPENROUTER_BASE_URL")
        .map(|s| s.to_string())
        .or_else(|_| env.var("OPENROUTER_BASE_URL").map(|v| v.to_string()))
        .unwrap_or_default();
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

/// Model id from the `MODEL` binding, falling back to [`DEFAULT_MODEL`].
fn read_model(env: &worker::Env) -> String {
    let model = env
        .var("MODEL")
        .map(|v| v.to_string())
        .unwrap_or_default();
    if model.trim().is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        model.trim().to_string()
    }
}

/// Build (or reuse) the OpenRouter-backed session for this isolate.
///
/// `AgentSession` is an `Arc` clone, so returning an owned clone is cheap and
/// avoids holding the `RefCell` borrow across an `.await`.
fn get_or_build_session(env: &worker::Env) -> AgentSession {
    SESSION.with(|cell| {
        if let Some(s) = cell.borrow().as_ref() {
            return s.clone();
        }
        let key = read_api_key(env);
        let mut config = OpenAIConfig::openrouter(key);
        if let Some(base_url) = read_base_url(env) {
            config = config.with_base_url(base_url);
        }
        let provider = OpenAIProvider::with_http_client(Arc::new(FetchHttpClient::new()))
            .create(Some(config))
            .expect("failed to build OpenRouter provider");
        let session = AgentSession::builder(provider)
            .with_model(ModelId::from(read_model(env)))
            .with_system_prompt(SYSTEM_PROMPT)
            .build()
            .expect("failed to build agent session");
        *cell.borrow_mut() = Some(session.clone());
        session
    })
}

fn html_response(body: &str, status: u16) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(status);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "text/html; charset=utf-8").unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some(body), &init).unwrap()
}

fn text_response(body: &str, status: u16) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(status);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "text/plain; charset=utf-8").unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some(body), &init).unwrap()
}

async fn read_prompt(req: &web_sys::Request) -> Result<String, JsValue> {
    let text_promise = req.text()?;
    let body_js = JsFuture::from(text_promise).await?;
    Ok(body_js.as_string().unwrap_or_default())
}

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>OpenRouter via Cloudflare Workers</title></head>
<body>
<h1>foundation_ai → OpenRouter</h1>
<textarea id="prompt" rows="4" cols="70" placeholder="Ask something...">What is the capital of France?</textarea><br>
<button id="go">Generate</button>
<pre id="output"></pre>
<script>
document.getElementById('go').onclick = async () => {
  const prompt = document.getElementById('prompt').value;
  const output = document.getElementById('output');
  output.textContent = '';
  const res = await fetch('/generate', { method: 'POST', body: prompt });
  if (!res.ok) { output.textContent = 'error: ' + res.status; return; }
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    const lines = buf.split('\n');
    buf = lines.pop();
    for (const line of lines) {
      if (line.startsWith('data: ')) output.textContent += line.slice(6);
    }
  }
};
</script>
</body>
</html>"#;

async fn handle_generate(env: &worker::Env, req: web_sys::Request) -> web_sys::Response {
    web_sys::console::log_1(&"handle_generate: start".into());
    let prompt = match read_prompt(&req).await {
        Ok(p) if !p.trim().is_empty() => p,
        Ok(_) => return text_response("Send a prompt in the request body.", 400),
        Err(_) => return text_response("failed to read request body", 400),
    };

    web_sys::console::log_1(&"handle_generate: building session".into());
    let session = get_or_build_session(env);
    web_sys::console::log_1(&"handle_generate: session ready, running turn".into());
    let turn = match session.run_turn_stream(prompt) {
        Ok(t) => t,
        Err(e) => return text_response(&format!("error: {e}"), 500),
    };
    web_sys::console::log_1(&"handle_generate: turn created".into());

    // `into_next_stream` (not `into_future_stream`) is the wasm bridge that
    // parks on valtron's in-band Wait/Pending states, yielding to the JS event
    // loop so the in-flight fetch can resolve, and surfaces `Next` payloads
    // directly (`Item = Turn`). `into_future_stream` also parks but yields the
    // full `Stream<Turn, _>` item type, so `into_next_stream` is the simpler
    // drain for a byte/record consumer.
    let stream = Rc::new(RefCell::new(Some(turn.into_inner().into_next_stream())));

    let start_callback = Closure::wrap(Box::new(move |ctrl: JsValue| {
        let controller = ctrl.unchecked_into::<web_sys::ReadableStreamDefaultController>();
        let stream = stream.borrow_mut().take().unwrap();
        let promise = future_to_promise(async move {
            let mut stream = stream;
            let mut n = 0usize;
            // HTTP backends stream snapshots (each record re-sends the text so
            // far); local ones stream deltas. Send the client only what is new.
            let mut sent = String::new();
            while let Some(record) = stream.next().await {
                let text = Turn::text_of(&record);
                let delta = if let Some(rest) = text.strip_prefix(sent.as_str()) {
                    let delta = rest.to_string();
                    sent = text;
                    delta
                } else {
                    sent.push_str(&text);
                    text
                };
                if !delta.is_empty() {
                    n += 1;
                    let chunk = format!("data: {}\n\n", delta.replace('\n', " "));
                    let bytes = Uint8Array::from(chunk.as_bytes());
                    if let Err(e) = controller.enqueue_with_chunk(&bytes) {
                        // The client went away: stop pulling the model stream.
                        web_sys::console::warn_1(&e);
                        break;
                    }
                }
            }
            web_sys::console::log_1(
                &format!("handle_generate: stream done, {n} chunks").into(),
            );
            let _ = controller.close();
            Ok(JsValue::NULL)
        });
        JsValue::from(promise)
    }) as Box<dyn FnMut(JsValue) -> JsValue>);

    let underlying_source = web_sys::UnderlyingSource::new();
    underlying_source.set_start(start_callback.as_ref().unchecked_ref::<js_sys::Function>());
    start_callback.forget();

    let rs = web_sys::ReadableStream::new_with_underlying_source(&underlying_source).unwrap();
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "text/event-stream").unwrap();
    let init = web_sys::ResponseInit::new();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_readable_stream_and_init(Some(&rs), &init).unwrap()
}

fn path_of(req: &web_sys::Request) -> String {
    let url = req.url();
    if let Some(pos) = url.find("//") {
        let rest = &url[pos + 2..];
        if let Some(pos2) = rest.find('/') {
            rest[pos2..].to_string()
        } else {
            "/".to_string()
        }
    } else {
        url
    }
}

fn init_logs() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = console_log::init_with_level(log::Level::Trace);
        let filter = tracing_subscriber::EnvFilter::new("foundation_ai=trace,foundation_netio=trace");
        let _ = tracing_subscriber::fmt()
            .without_time()
            .with_max_level(tracing::Level::TRACE)
            .with_env_filter(filter)
            .try_init();
    });
}

#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, env: worker::Env) -> web_sys::Response {
    console_error_panic_hook::set_once();
    init_logs();
    POOL_GUARD.get_or_init(|| initialize_pool(42, None));

    let method = req.method();
    let path = path_of(&req);
    match (method.as_str(), path.as_str()) {
        ("GET", "/") => html_response(INDEX_HTML, 200),
        ("POST", "/generate") => handle_generate(&env, req).await,
        _ => html_response("Not Found", 404),
    }
}
