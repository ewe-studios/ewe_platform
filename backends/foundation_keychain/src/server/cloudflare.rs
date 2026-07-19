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
//! `worker::D1Database` into a [`StorageBackend::D1Wasm`]. The key-value and
//! vault schemas are applied at cold-start via `init_schema_async()` +
//! `apply_schema()` — real async (JS Promises), zero valtron. All crypto/JWT is
//! `foundation_auth` (decision 01).
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
use crate::core::store::apply_schema;

/// D1 binding name in `wrangler.toml`.
const D1_BINDING: &str = "KEYCHAIN_DB";

/// The Workers HTTP entry point.
///
/// Builds a D1-backed [`KeychainContext`] from the env binding, initialises the
/// key-value schema (async — no valtron pool needed), and dispatches every
/// request through the portable [`route`](crate::core::router::route) function.
#[event(fetch)]
pub async fn fetch(mut req: Request, env: Env, _ctx: Context) -> WorkerResult<Response> {
    // Build the D1 storage provider + keychain context.
    let ctx = match build_context(&env).await {
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

/// Build a D1-backed [`KeychainContext`] from the Worker env.
///
/// Calls [`StorageProvider::init_schema_async`] (which creates `kv_store` if
/// absent) and then creates the context with a fresh ephemeral Ed25519 JWT
/// signing key.
async fn build_context(env: &Env) -> Result<KeychainContext, String> {
    let d1 = env.d1(D1_BINDING).map_err(|e| e.to_string())?;
    let db: foundation_db::D1Database = d1.into();
    let provider = StorageProvider::new(StorageBackend::D1Wasm {
        db: Arc::new(db),
        table_prefix: String::new(),
    })
    .map_err(|e| e.to_string())?;

    // Schema init — uses JS Promise (no valtron pool needed).
    provider
        .init_schema_async()
        .await
        .map_err(|e| format!("schema init: {e:?}"))?;

    let store: Arc<dyn AsyncQueryStore> = Arc::new(provider);

    // Apply the keychain vault schema (accounts, ciphers, folders, etc.).
    apply_schema(store.as_ref())
        .await
        .map_err(|e| format!("vault schema: {e}"))?;

    Ok(KeychainContext::new(store))
}
