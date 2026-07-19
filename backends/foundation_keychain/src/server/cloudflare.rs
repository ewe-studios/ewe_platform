//! Cloudflare Workers backend transport (spec-57, F008 Stage 3).
//!
//! WHY: serves the Bitwarden API on the edge. Unlike the native backend (sync
//! `Serve` over local Turso), the Workers `fetch` handler is genuinely `async`
//! (D1 is a network round-trip), so it `.await`s the shared
//! [`route`](crate::core::router::route) directly — no single-poll bridge.
//!
//! WHAT: the `#[event(fetch)]` entry point builds a D1-backed [`KeychainContext`]
//! from the env binding, extracts (method, path, body, bearer) from the
//! `worker::Request`, routes, and renders the `(status, json)` to a
//! `worker::Response`.
//!
//! HOW: `foundation_db`'s `workers-rs` interop converts the env's
//! `worker::D1Database` into a [`StorageBackend::D1Wasm`]. All crypto/JWT is
//! `foundation_auth` (decision 01). The schema is applied by wrangler D1
//! migrations (not per-request).
//!
//! NOTE (Stage-3 remainder): the JWT signing key is currently generated per cold
//! start — tokens don't survive an isolate recycle. Production must persist it in
//! KV / a secret. The SignalR notifications Durable Object (decision 09, reusing
//! `foundation_netio::websocket::shared` framing) is also deferred.

use std::sync::Arc;

use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use worker::{event, Context, Env, Request, Response, Result as WorkerResult};

use crate::core::context::KeychainContext;
use crate::core::router::route;

/// D1 binding name in `wrangler.toml`.
const D1_BINDING: &str = "KEYCHAIN_DB";

/// The Workers HTTP entry point.
#[event(fetch)]
pub async fn fetch(mut req: Request, env: Env, _ctx: Context) -> WorkerResult<Response> {
    let ctx = match build_context(&env) {
        Ok(ctx) => ctx,
        Err(e) => return Response::error(format!("keychain init failed: {e}"), 500),
    };

    let method = req.method().to_string().to_uppercase();
    let path = req.path();
    let bearer = req
        .headers()
        .get("Authorization")
        .ok()
        .flatten()
        .and_then(|h| h.strip_prefix("Bearer ").map(|t| t.trim().to_string()));
    let body = req.text().await.unwrap_or_default();

    let (status, json) = match route(&ctx, &method, &path, &body, bearer.as_deref()).await {
        Ok((status, json)) => (status, json),
        Err(err) => (err.status(), err.to_json()),
    };

    Ok(Response::from_json(&json)?.with_status(status))
}

/// Build a D1-backed context from the Worker env.
fn build_context(env: &Env) -> Result<KeychainContext, String> {
    let d1 = env.d1(D1_BINDING).map_err(|e| e.to_string())?;
    let db: foundation_db::D1Database = d1.into();
    let provider = StorageProvider::new(StorageBackend::D1Wasm {
        db: Arc::new(db),
        table_prefix: String::new(),
    })
    .map_err(|e| e.to_string())?;
    let store: Arc<dyn AsyncQueryStore> = Arc::new(provider);
    Ok(KeychainContext::new(store))
}
