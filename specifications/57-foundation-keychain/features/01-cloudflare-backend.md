# F01: Cloudflare Backend

## Goal

Implement the Cloudflare Workers backend using `foundation_db`'s WASM backends (`D1Wasm`, `R2Wasm`, `KVWasm`) + Web Crypto for PBKDF2 + `foundation_auth`'s `D1CredentialStore`. Prove everything works end-to-end on `wasm32-unknown-unknown`.

## Work

### 1. Cloudflare Crypto (`cloudflare/crypto.rs`)

Port OrangeVault's `crypto/` module — Web Crypto via `web_sys::SubtleCrypto`:

- `pbkdf2_sha256(password, salt, iterations, key_len) -> Vec<u8>` — via `crypto.subtle.importKey` + `deriveBits` (PBKDF2-HMAC-SHA256). Max 100K iterations (WebCrypto limit).
- `constant_time_eq(a, b) -> bool` — timing-safe comparison
- `random_bytes(len) -> Vec<u8>` — via `crypto.getRandomValues`
- `hmac_sha256(key, data) -> Vec<u8>` — via SubtleCrypto
- `hmac_sha1(key, data) -> Vec<u8>` — for TOTP

TOTP uses `foundation_auth::two_factor::TOTPSecret` — the Web Crypto backend only provides HMAC-SHA1 for the TOTP verification in the identity flow.

### 2. Server Bootstrap (`server/cloudflare.rs`)

The `#[event(fetch)]` and `#[event(scheduled)]` entry points:

```rust
#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Initialize foundation_db StorageProvider
    let db = StorageProvider::new(StorageBackend::D1Wasm {
        db: Arc::new(env.d1("DB")?),
        table_prefix: "orangevault".into(),
    })?;
    let blob = StorageProvider::new(StorageBackend::R2Wasm {
        bucket: env.bucket("FILES")?,
        prefix: "".into(),
    })?;
    let kv = StorageProvider::new(StorageBackend::KVWasm {
        kv: env.kv("CACHE")?,
        prefix: "".into(),
    })?;

    // Initialize foundation_auth JWT signing key (stored in KV)
    let signing_key = load_or_create_signing_key(&kv).await?;

    // Build request context with storage + auth
    let ctx = KeychainContext { db, blob, kv, signing_key, env: env.clone() };

    // Route setup — wire api/ handlers to workers-rs Router
    let router = Router::with_data(ctx)
        .get("/alive", |_, _| Response::ok(""))
        .get("/api/alive", |_, _| Response::ok(""))
        .get_async("/api/config", api::accounts::get_config)
        .post_async("/accounts/prelogin", api::accounts::prelogin)
        // ... all ~150 routes ...
        .run(req, env).await;

    // CORS + security headers on response
    finalize_response(router)
}

#[event(scheduled)]
async fn scheduled(event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let db = StorageProvider::new(StorageBackend::D1Wasm { ... })?;
    let blob = StorageProvider::new(StorageBackend::R2Wasm { ... })?;
    match event.cron().as_str() {
        "0 */6 * * *" => purge_expired_sends(&db, &blob).await,
        "0 0 * * *" => purge_trashed_ciphers(&db).await,
        other => console_log!("cron: unknown schedule {other}"),
    }
}
```

### 3. Cloudflare Notifications (`cloudflare/notifier.rs`)

Port OrangeVault's `notifications.rs` — Durable Object `UserNotifier`:

- `#[durable_object]` struct with WebSocket accept, SignalR handshake, MessagePack framing, 15s ping alarm
- Uses portable `core::notifications::serialize_msgpack` and `create_notification`
- Internal `/notify` endpoint for triggering notifications from API handlers
- `send_notification(env, user_uuid, update_type, context_id, payload)` — fire-and-forget DO fetch

### 4. Integration with foundation_auth

- **JWT**: Use `foundation_auth::shared::jwt::JwtSigningKey` for signing. On WASM, we need a way to persist the key in KV — either serialize as PEM/JWK and store, or use `foundation_auth`'s existing key management.
- **CredentialStore**: Use `foundation_auth::wasm_bindgen::D1CredentialStore` directly for user credential lookup during login.
- **Middleware**: Use `foundation_auth::shared::middleware::extract_bearer_token` for token extraction. After `foundation_auth` JWT verify, run our `check_security_stamp` adapter.
- **TOTP**: Use `foundation_auth::shared::two_factor::TOTPSecret` for TOTP generation/verification.
- **Rate limiting**: Use `foundation_db::RateLimiterStore` (already implemented on `StorageProvider`) for login brute-force protection.

### Success Criterion

- `cargo check --features backend-cloudflare --target wasm32-unknown-unknown` passes
- Compiles with `worker-build --release` without errors
- Binary size under 5MB compressed
- All OrangeVault integration tests pass against the built WASM binary in Miniflare
- SignalR MessagePack notifications work (WebSocket handshake + binary frames)
