//! `foundation_ai` on Cloudflare Workers: a chat page backed by an agent
//! session that calls OpenRouter through the platform client from
//! `HttpClientBuilder` (the fetch client on wasm) and
//! streams each reply back as it is generated (spec 65 T5).
//!
//! - `GET /` — the chat page.
//! - `POST /chat` — the prompt as the body; the reply as Server-Sent Events,
//!   one event per new piece of text.
//!
//! The session is built on the first request and reused by this isolate.

use std::cell::RefCell;
use std::sync::OnceLock;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use foundation_ai::agentic::{AgentSession, TurnEvent};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{ModelId, ModelProvider};
use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_netio::HttpClientBuilder;
use foundation_netio::wasm::response::sse_response;

use futures_lite::StreamExt;

/// Default OpenRouter model; override with the `MODEL` binding.
const DEFAULT_MODEL: &str = "google/gemma-4-31b-it:free";
const SYSTEM_PROMPT: &str = "You are a concise, helpful assistant.";
const CHAT_HTML: &str = include_str!("chat.html");

static POOL: OnceLock<PoolGuard> = OnceLock::new();

thread_local! {
    static SESSION: RefCell<Option<AgentSession>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, env: worker::Env) -> web_sys::Response {
    console_error_panic_hook::set_once();
    POOL.get_or_init(|| initialize_pool(42, None));

    match (req.method().as_str(), path(&req).as_str()) {
        ("GET", "/") => respond(200, "text/html; charset=utf-8", CHAT_HTML),
        ("POST", "/chat") => chat(&env, &req).await.unwrap_or_else(|message| {
            respond(500, "text/plain; charset=utf-8", &message)
        }),
        _ => respond(404, "text/plain; charset=utf-8", "not found"),
    }
}

/// Run one turn of the session and stream its events to the client.
///
/// Each Server-Sent Event carries one line of JSON describing a turn event:
/// `{"type":"text","content":…}` for text, `{"type":"thinking","content":…}`
/// for reasoning, `{"type":"activity","message":…}` for tool work or retries,
/// and a terminal `{"type":"error","message":…}` or `{"type":"done"}`. The
/// client shows a loading state until the first event, then text or the error.
async fn chat(env: &worker::Env, req: &web_sys::Request) -> Result<web_sys::Response, String> {
    let prompt = body_text(req).await?;
    if prompt.trim().is_empty() {
        return Ok(respond(400, "text/plain; charset=utf-8", "send a prompt"));
    }
    let turn = session(env)
        .await?
        .run_turn_stream(prompt)
        .map_err(|e| e.to_string())?;
    sse_response(turn.events().map(event_frame)).map_err(|e| format!("{e:?}"))
}

/// One turn event as a single-line JSON payload for the client.
fn event_frame(event: TurnEvent) -> String {
    match event {
        TurnEvent::Text(text) => json_object(&[("type", "text"), ("content", &text)]),
        TurnEvent::Thinking(thinking) => {
            json_object(&[("type", "thinking"), ("content", &thinking)])
        }
        TurnEvent::ToolCall { name, .. } => {
            let message = format!("calling {name}");
            json_object(&[("type", "activity"), ("message", &message)])
        }
        TurnEvent::ToolResult { name, .. } => {
            let message = format!("{name} done");
            json_object(&[("type", "activity"), ("message", &message)])
        }
        TurnEvent::Retract { reason } => {
            let message = format!("retrying: {reason}");
            json_object(&[("type", "activity"), ("message", &message)])
        }
        TurnEvent::Progress(_) => "{\"type\":\"activity\"}".to_string(),
        TurnEvent::Failed(error) => {
            json_object(&[("type", "error"), ("message", &error.to_string())])
        }
        TurnEvent::Done(_) => "{\"type\":\"done\"}".to_string(),
        _ => "{\"type\":\"activity\"}".to_string(),
    }
}

/// A small JSON object with string values, for the SSE frames above. Keys are
/// fixed and known-safe; only the values are escaped.
fn json_object(fields: &[(&str, &str)]) -> String {
    let mut out = String::from("{");
    for (i, (key, value)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(key);
        out.push_str("\":\"");
        out.push_str(&json_escape(value));
        out.push('"');
    }
    out.push('}');
    out
}

/// Escape a string for inclusion in a JSON string value.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// The isolate's session, built on first use.
///
/// Building is async (`AgentSessionBuilder::build`), so the slot is not
/// borrowed across it; if another request stored a session meanwhile, that
/// one is kept and reused.
async fn session(env: &worker::Env) -> Result<AgentSession, String> {
    if let Some(session) = SESSION.with(|slot| slot.borrow().clone()) {
        return Ok(session);
    }
    let mut config = OpenAIConfig::openrouter(setting(env, "OPENROUTER_API_KEY").unwrap_or_default());
    if let Some(base_url) = setting(env, "OPENROUTER_BASE_URL") {
        config = config.with_base_url(base_url);
    }
    let provider = OpenAIProvider::with_http_client(HttpClientBuilder::new().build())
        .create(Some(config))
        .map_err(|e| e.to_string())?;
    let model = setting(env, "MODEL").unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let session = AgentSession::builder(provider)
        .with_model(ModelId::from(model))
        .with_system_prompt(SYSTEM_PROMPT)
        .build()
        .await
        .map_err(|e| e.to_string())?;
    Ok(SESSION.with(|slot| slot.borrow_mut().get_or_insert(session).clone()))
}

/// A non-empty secret or var named `name`.
fn setting(env: &worker::Env, name: &str) -> Option<String> {
    env.secret(name)
        .map(|s| s.to_string())
        .or_else(|_| env.var(name).map(|v| v.to_string()))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

async fn body_text(req: &web_sys::Request) -> Result<String, String> {
    let text = req.text().map_err(|e| format!("{e:?}"))?;
    let body = JsFuture::from(text).await.map_err(|e| format!("{e:?}"))?;
    Ok(body.as_string().unwrap_or_default())
}

fn respond(status: u16, content_type: &str, body: &str) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(status);
    if let Ok(headers) = web_sys::Headers::new() {
        let _ = headers.set("Content-Type", content_type);
        init.set_headers(&headers);
    }
    web_sys::Response::new_with_opt_str_and_init(Some(body), &init)
        .expect("a string response is always constructible")
}

fn path(req: &web_sys::Request) -> String {
    web_sys::Url::new(&req.url()).map_or_else(|_| "/".to_string(), |url| url.pathname())
}
