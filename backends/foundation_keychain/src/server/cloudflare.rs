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
//! `apply_schema()` — real async (JS Promises), zero valtron. The context
//! (including the JWT signing key) is cached in a per-isolate static so tokens
//! minted by login survive across requests. The Ed25519 signing key is persisted
//! in KV so tokens also survive isolate recycles.

use std::sync::{Arc, Mutex};

use foundation_auth::shared::jwt::JwtSigningKey;
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use worker::{event, Context, Env, Request, Response, Result as WorkerResult};

use crate::core::context::KeychainContext;
use crate::core::router::route;
use crate::core::store::apply_schema;

/// Per-isolate context cache — built once and reused across requests.
static CONTEXT: Mutex<Option<Arc<KeychainContext>>> = Mutex::new(None);

/// D1 binding name in `wrangler.toml`.
const D1_BINDING: &str = "KEYCHAIN_DB";
/// KV binding name in `wrangler.toml` — stores the JWT signing key.
const KV_BINDING: &str = "KEYCHAIN_KV";
/// KV key for the JWT signing key PEM.
const SIGNING_KEY_KV_KEY: &str = "jwt_signing_key";

/// The Workers HTTP entry point.
///
/// Builds a D1-backed [`KeychainContext`] from the env binding, initialises the
/// key-value schema (async — no valtron pool needed), and dispatches every
/// request through the portable [`route`](crate::core::router::route) function.
#[event(fetch)]
pub async fn fetch(mut req: Request, env: Env, _ctx: Context) -> WorkerResult<Response> {
    // Reuse the cached context when warm, initialise on first request.
    let ctx = match get_or_init_context(&env).await {
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

/// Return the cached context, building it on first call.
///
/// The JWT signing key is loaded from KV on cold start (so tokens survive
/// isolate recycles) or generated fresh if no key exists yet (first deploy).
/// Both the context and signing key are cached in-process — subsequent
/// requests within the same isolate clone the `Arc`.
async fn get_or_init_context(env: &Env) -> Result<Arc<KeychainContext>, String> {
    // Fast path: already initialised in this isolate.
    if let Some(ctx) = CONTEXT.lock().unwrap().as_ref() {
        return Ok(Arc::clone(ctx));
    }

    // Cold start: init D1, apply schema, load-or-generate signing key.
    let ctx = Arc::new(build_context(env).await?);

    let mut guard = CONTEXT.lock().unwrap();
    if let Some(existing) = guard.as_ref() {
        return Ok(Arc::clone(existing));
    }
    *guard = Some(Arc::clone(&ctx));
    Ok(ctx)
}

/// Build a D1-backed [`KeychainContext`], loading the JWT signing key from
/// KV (or generating + persisting it on first deploy).
async fn build_context(env: &Env) -> Result<KeychainContext, String> {
    // ── D1 storage ──────────────────────────────────────────────────────
    let d1 = env.d1(D1_BINDING).map_err(|e| e.to_string())?;
    let db: foundation_db::D1Database = d1.into();
    let provider = StorageProvider::new(StorageBackend::D1Wasm {
        db: Arc::new(db),
        table_prefix: String::new(),
    })
    .map_err(|e| e.to_string())?;

    provider
        .init_schema_async()
        .await
        .map_err(|e| format!("schema init: {e:?}"))?;

    let store: Arc<dyn AsyncQueryStore> = Arc::new(provider);
    apply_schema(store.as_ref())
        .await
        .map_err(|e| format!("vault schema: {e}"))?;

    // ── JWT signing key (KV-backed) ────────────────────────────────────
    let signing_key = load_or_create_signing_key(env).await?;

    Ok(KeychainContext::with_signing_key(store, signing_key))
}

/// Load the Ed25519 JWT signing key from KV, or generate a new one and
/// persist it so tokens survive isolate recycles.
async fn load_or_create_signing_key(env: &Env) -> Result<JwtSigningKey, String> {
    let kv = env.kv(KV_BINDING).map_err(|e| format!("kv bind: {e}"))?;

    // Try loading the existing key from KV.
    if let Ok(Some(pem)) = kv.get(SIGNING_KEY_KV_KEY).text().await {
        if let Ok(key) = JwtSigningKey::from_pem(&pem) {
            return Ok(key);
        }
        // Corrupt entry — overwrite below.
    }

    // Generate a fresh Ed25519 key and store it in KV.
    let key = JwtSigningKey::generate_ed25519();
    let pem = key.to_pem().map_err(|e| format!("key pem: {e}"))?;
    kv.put(SIGNING_KEY_KV_KEY, pem)
        .map_err(|e| format!("kv put: {e}"))?
        .execute()
        .await
        .map_err(|e| format!("kv store: {e}"))?;

    Ok(key)
}
