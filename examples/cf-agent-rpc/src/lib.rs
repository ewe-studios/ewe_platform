//! `agent-rpc` on Cloudflare Workers (spec 83 feature 05, T21).
//!
//! WHY: the Worker is the public endpoint, and one Durable Object holds each
//! instance. This crate is the thin platform glue: it builds the host once per
//! isolate and forwards each Durable Object call to
//! [`AgentInstanceObject`](foundation_agent_rpc::host::worker::object::AgentInstanceObject).
//!
//! WHAT: the `fetch` entry, the `AgentInstance` Durable Object and the valtron
//! pool of the isolate.
//!
//! HOW: the JavaScript runtime polls `fetch` and the Durable Object's `fetch`.
//! The single-threaded engine is initialized once. The host reads the Worker
//! vars and bindings; no state is kept between requests except the host and the
//! engine.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use foundation_agent_rpc::host::worker::entry::WorkerHost;
use foundation_agent_rpc::host::worker::object::AgentInstanceObject;
use foundation_agent_rpc::AgentRpcError;
use foundation_context::Ctx;
use foundation_core::valtron::{PoolGuard, initialize_pool};
use wasm_bindgen::prelude::*;
use worker::{DurableObject, Env, Request, Response, State, durable_object};

thread_local! {
    /// The host of this isolate. One Worker isolate serves many requests.
    static HOST: RefCell<Option<Arc<WorkerHost>>> = const { RefCell::new(None) };
}

/// The Worker's valtron pool, once per isolate.
static POOL: OnceLock<PoolGuard> = OnceLock::new();

/// The `fetch` entry of the Worker.
///
/// # Errors
/// A JavaScript error when the host cannot be built or a request fails.
#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, env: JsValue) -> Result<web_sys::Response, JsValue> {
    console_error_panic_hook::set_once();
    let env = env.unchecked_ref::<Env>().clone();
    let host = host(&env)
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let response = host
        .fetch(Request::from(req))
        .await
        .map_err(|error| JsValue::from_str(&format!("{error:?}")))?;
    Ok(web_sys::Response::from(response))
}

/// Build the host once, then reuse it.
async fn host(env: &Env) -> Result<Arc<WorkerHost>, AgentRpcError> {
    if let Some(host) = HOST.with(|slot| slot.borrow().as_ref().cloned()) {
        return Ok(host);
    }
    POOL.get_or_init(|| initialize_pool(42, None));
    let ctx = Ctx::host();
    let _ = foundation_core::env::install(&ctx, foundation_core::env::system);
    let host = Arc::new(
        WorkerHost::new(env.clone(), ctx)
            .await
            .map_err(|error| error.current_context().clone())?,
    );
    HOST.with(|slot| *slot.borrow_mut() = Some(Arc::clone(&host)));
    Ok(host)
}

/// One instance: a Durable Object with its own SQL database.
#[durable_object]
pub struct AgentInstance {
    inner: AgentInstanceObject,
}

impl DurableObject for AgentInstance {
    fn new(state: State, env: Env) -> Self {
        POOL.get_or_init(|| initialize_pool(42, None));
        Self {
            inner: AgentInstanceObject::new(state, env),
        }
    }

    async fn fetch(&self, req: Request) -> worker::Result<Response> {
        self.inner.fetch(req).await
    }

    async fn alarm(&self) -> worker::Result<Response> {
        self.inner.alarm().await
    }
}
