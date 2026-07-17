# F01: Cloudflare Backend

## Goal

Implement the Cloudflare Workers backend using `foundation_deployment_cloudflare::workers` (Durable Objects, WebSocket, Env) + `foundation_db`'s WASM backends (`D1Wasm`, `R2Wasm`, `KVWasm`) + `foundation_auth`'s `D1CredentialStore` and `pbkdf2`. Prove everything works end-to-end on `wasm32-unknown-unknown`.

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

The `#[event(fetch)]` and `#[event(scheduled)]` entry points via `foundation_deployment_cloudflare::workers`:

```rust
use foundation_deployment_cloudflare::workers::{env, context::WorkersContext, durable_object::DurableObjectHandle};

#[event(fetch)]
async fn main(req: Request, e: Env, _ctx: Context) -> Result<Response> {
    let env = env::WorkersEnv::new(e)?;
    let ctx = WorkersContext::new(env.clone())?;
    // Route setup — wire api/ handlers to workers-rs Router
    // ...
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
