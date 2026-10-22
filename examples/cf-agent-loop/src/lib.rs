#![allow(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

//! `foundation_ai` agentic layer on Cloudflare Workers, served through the
//! `foundation_http` `dispatch_cf` path + a shared `Router`, with Workers AI as
//! the model provider (spec 65 T5, `cf-agent-loop`).
//!
//! `CfServe`/`CfConn` buffer the response (no streaming at this layer), so the
//! handler runs the turn to completion and returns the accumulated assistant
//! text. The streaming variant lives in `examples/cf-cloud-provider`.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use wasm_bindgen::prelude::*;

use foundation_ai::agentic::AgentSession;
use foundation_ai::backends::workers_ai::{WorkersAiConfig, WorkersAiProvider};
use foundation_ai::types::{ModelId, ModelProvider};
use foundation_core::valtron::{initialize_pool, PoolGuard, StreamIteratorExt};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::wasm::bridge::cf::CfHttpApp;
use foundation_http::wasm::cf_conn::{CfConn, CfConnectionResult};
use foundation_http::wasm::serve_cf::{CfServe, CfServeFactory};
use foundation_http::wasm::CfHttpAppSingleton;
use foundation_http::{SendSafeBody, SimpleIncomingRequest};
use foundation_netio::wasm::client::FetchHttpClient;

/// Default Workers AI model.
const DEFAULT_MODEL: &str = "@cf/meta/llama-3.1-8b-instruct";
const SYSTEM_PROMPT: &str = "You are a concise, helpful assistant.";

/// valtron single-threaded executor, initialised once per isolate.
static POOL_GUARD: OnceLock<PoolGuard> = OnceLock::new();

/// Credentials read from the Worker env, cached for the isolate's lifetime.
#[derive(Clone)]
struct WorkersAiCreds {
    api_token: String,
    account_id: String,
    model: String,
    base_url: Option<String>,
}

thread_local! {
    static CREDS: RefCell<Option<WorkersAiCreds>> = RefCell::new(None);
    static SESSION: RefCell<Option<AgentSession>> = RefCell::new(None);
}

/// Read a Worker secret, falling back to an env var.
fn secret_or_var(env: &worker::Env, name: &str) -> String {
    env.secret(name)
        .map(|s| s.to_string())
        .or_else(|_| env.var(name).map(|v| v.to_string()))
        .unwrap_or_default()
}

/// Cache the Workers AI credentials from the CF env.
fn store_creds(env: &JsValue) {
    let env = env.unchecked_ref::<worker::Env>();
    let base_url = secret_or_var(env, "WORKERS_AI_BASE_URL");
    let creds = WorkersAiCreds {
        api_token: secret_or_var(env, "WORKERS_AI_API_TOKEN"),
        account_id: secret_or_var(env, "CLOUDFLARE_ACCOUNT_ID"),
        model: env
            .var("MODEL")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
        base_url: if base_url.is_empty() {
            None
        } else {
            Some(base_url)
        },
    };
    CREDS.with(|c| *c.borrow_mut() = Some(creds));
}

/// Build (or reuse) the Workers-AI-backed session for this isolate.
///
/// Building is async (`AgentSessionBuilder::build`), so the cell is not
/// borrowed across it; if another request stored a session meanwhile, that
/// one is kept and reused.
async fn get_or_build_session() -> Result<AgentSession, String> {
    if let Some(session) = SESSION.with(|cell| cell.borrow().clone()) {
        return Ok(session);
    }
    let creds = CREDS
        .with(|c| c.borrow().clone())
        .ok_or_else(|| "no credentials stored".to_string())?;
    let mut config = WorkersAiConfig::api_token(creds.api_token).with_account_id(creds.account_id);
    if let Some(base_url) = creds.base_url {
        config = config.with_base_url(base_url);
    }
    let provider = WorkersAiProvider::with_http_client(Arc::new(FetchHttpClient::new()))
        .create(Some(config))
        .map_err(|e| e.to_string())?;
    let session = AgentSession::builder(provider)
        .with_model(ModelId::from(creds.model.as_str()))
        .with_system_prompt(SYSTEM_PROMPT)
        .build()
        .await
        .map_err(|e| e.to_string())?;
    Ok(SESSION.with(|cell| cell.borrow_mut().get_or_insert(session).clone()))
}

/// Run one agentic turn and return the accumulated assistant text.
async fn run_prompt(prompt: &str) -> Result<String, String> {
    if prompt.trim().is_empty() {
        return Ok("Send a prompt in the request body.".to_string());
    }
    // `ask` waits by parking, yielding to the JS event loop while the
    // Workers AI fetch is in flight; snapshot/delta chunks are already joined.
    let answer = get_or_build_session()
        .await?
        .ask(prompt.to_string())
        .await
        .map_err(|e| e.to_string())?;
    answer.into_result().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// CfServe handler (buffered)
// ---------------------------------------------------------------------------

struct AgentHandler;

impl CfServeFactory for AgentHandler {
    fn create(_bag: &ContextBag) -> Self {
        AgentHandler
    }
}

#[async_trait::async_trait(?Send)]
impl CfServe for AgentHandler {
    async fn serve_cf(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        let prompt = match req.body {
            Some(SendSafeBody::Text(t)) => t,
            Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(&b).into_owned(),
            _ => String::new(),
        };

        match run_prompt(&prompt).await {
            Ok(text) => {
                conn.set_status(200);
                conn.set_header("Content-Type", "text/plain; charset=utf-8");
                conn.set_body(text.into_bytes());
            }
            Err(e) => {
                conn.set_status(500);
                conn.set_header("Content-Type", "text/plain; charset=utf-8");
                conn.set_body(format!("error: {e}").into_bytes());
            }
        }
        CfConnectionResult::Ok
    }
}

fn build_app() -> CfHttpApp {
    let mut app = HttpApp::<Arc<dyn CfServe>>::new_cf();
    app.route_any_cf::<AgentHandler>("/");
    CfHttpApp::from_app(app)
}

fn init_logs() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = console_log::init_with_level(log::Level::Trace);
        let filter = tracing_subscriber::EnvFilter::new("foundation_ai=trace,foundation_http=trace");
        let _ = tracing_subscriber::fmt()
            .without_time()
            .with_max_level(tracing::Level::TRACE)
            .with_env_filter(filter)
            .try_init();
    });
}

#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, env: JsValue) -> Result<web_sys::Response, JsValue> {
    console_error_panic_hook::set_once();
    init_logs();
    POOL_GUARD.get_or_init(|| initialize_pool(42, None));

    store_creds(&env);
    let guard = CfHttpAppSingleton::get_or_init(|_bag| build_app());
    guard
        .fetch(req, env)
        .await
        .map_err(|e| JsValue::from_str(&format!("{e:?}")))
}
