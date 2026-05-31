---
feature: "Wasm OAuth Manager (wasm-bindgen)"
description: "Implement OAuthManager for wasm32 using browser/worker fetch API, shared OAuth types, and feature-gated wasm-bindgen dependencies"
status: "implemented"
priority: "high"
depends_on: ["02-auth-wasm-compat"]
estimated_effort: "medium"
created: 2026-05-17
last_updated: 2026-05-31
author: "Main Agent"
tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
---

# Feature: Wasm OAuth Manager (wasm-bindgen)

## Overview

Implement a wasm-compatible `OAuthManager` in `foundation_auth` that uses the browser/worker `fetch()` API via `wasm-bindgen` instead of `SimpleHttpClient`. The shared OAuth types (`OAuthConfig`, `OAuthError`, `PkceChallenge`, `OAuthToken`) already live in `shared/oauth.rs` and `shared/oauth_token.rs` — only the HTTP transport layer differs between native and wasm.

The `wasm-bindgen-oauth` feature flag gates all `wasm-bindgen`, `js-sys`, and `web-sys` dependencies so they are only pulled in when explicitly needed. This keeps the base wasm build lightweight and avoids pulling in ~30 extra crates unless the user specifically wants OAuth flows on wasm.

## Architecture

### Module Layout

```
foundation_auth/src/
├── lib.rs                          # Re-exports, cfg-gated OAuthManager
├── shared/
│   ├── oauth.rs                    # OAuthConfig, OAuthError, PkceChallenge, OAuthHttpClient trait
│   ├── oauth_token.rs              # OAuthToken data type
│   └── ...                         # Other shared auth modules
├── native/
│   └── oauth.rs                    # OAuthManager using SimpleHttpClient (non-wasm)
└── wasm_bindgen/
    ├── mod.rs                      # #[cfg(feature = "wasm-bindgen-oauth")]
    └── oauth.rs                    # OAuthManager using web_sys::fetch (wasm only)
```

### OAuthManager Export Strategy

`lib.rs` re-exports `OAuthManager` from the appropriate backend:

```rust
// Always available (shared types)
pub use shared::oauth::{OAuthConfig, OAuthError, OAuthConfigBuilder, PkceChallenge};
pub use shared::oauth_token::OAuthToken;

// Native OAuthManager
#[cfg(not(target_arch = "wasm32"))]
pub use native::oauth::OAuthManager;

// Wasm OAuthManager (requires wasm-bindgen-oauth feature)
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-oauth"))]
pub use wasm_bindgen::oauth::OAuthManager;
```

This means:
- **Native builds**: get `OAuthManager` automatically (no feature needed)
- **Wasm builds without `wasm-bindgen-oauth`**: get shared types but no `OAuthManager`
- **Wasm builds with `wasm-bindgen-oauth`**: get full `OAuthManager` via fetch API

## Implementation Details

### Feature Flag and Dependencies

`foundation_auth/Cargo.toml`:

```toml
[features]
wasm = ["uuid/js", "chrono/wasmbind", "dep:getrandom", "getrandom/js"]
wasm-bindgen-oauth = [
  "dep:wasm-bindgen",
  "dep:wasm-bindgen-futures",
  "dep:js-sys",
  "dep:web-sys",
]

[dependencies]
wasm-bindgen = { version = "0.2", optional = true }
wasm-bindgen-futures = { version = "0.4", optional = true }
js-sys = { version = "0.3", optional = true }
web-sys = { version = "0.3", features = [
  "Request",
  "RequestInit",
  "RequestMode",
  "Response",
  "Headers",
  "AbortController",
  "ServiceWorkerGlobalScope",
  "Window",
  "crypto",
], optional = true }
```

Why not `[target.'cfg(target_arch = "wasm32")'.dependencies]`:
- `cfg`-conditional deps are harder to reason about and can't be toggled independently
- A feature flag makes it explicit: `--features wasm-bindgen-oauth` or nothing
- Allows native builds to compile even if `wasm-bindgen` has issues on certain platforms
- Matches the existing `wasm` feature pattern already in use

### Fetch Implementation

#### Runtime Detection (Service Worker vs Browser)

Cloudflare Workers run in a `ServiceWorkerGlobalScope`, not a browser `Window`. The fetch API is available in both, but the global object differs. Following reqwest's pattern:

```rust
fn js_fetch(req: &web_sys::Request) -> js_sys::Promise {
    let global = js_sys::global();

    // Check for service worker context first
    if let Ok(true) = js_sys::Reflect::has(
        &global,
        &JsValue::from_str("ServiceWorkerGlobalScope"),
    ) {
        global
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .fetch_with_request(req)
    } else {
        // Browser window context
        web_sys::window()
            .expect("fetch: no window and no service worker")
            .fetch_with_request(req)
    }
}
```

#### Promise Resolution via wasm-bindgen-futures

We use `wasm_bindgen_futures::JsFuture` to convert the JS `Promise` into a Rust `Future`:

```rust
use wasm_bindgen_futures::JsFuture;

async fn fetch_response(req: &web_sys::Request) -> Result<web_sys::Response, OAuthError> {
    let promise = js_fetch(req);
    let js_value = JsFuture::from(promise)
        .await
        .map_err(|e| OAuthError::TokenRequestFailed(format!("fetch failed: {e:?}")))?;
    js_value
        .dyn_into::<web_sys::Response>()
        .map_err(|_| OAuthError::TokenRequestFailed("fetch returned non-Response".into()))
}
```

#### Request Construction

```rust
fn build_request(url: &str, body: &str) -> Result<web_sys::Request, OAuthError> {
    let mut init = web_sys::RequestInit::new();
    init.method("POST");
    init.body(Some(&JsValue::from_str(body)));

    let headers = web_sys::Headers::new()
        .map_err(|e| OAuthError::TokenRequestFailed(format!("headers creation failed: {e:?}")))?;
    headers
        .append("Content-Type", "application/x-www-form-urlencoded")
        .map_err(|e| OAuthError::TokenRequestFailed(format!("header append failed: {e:?}")))?;
    init.headers(&headers.into());

    // Disable CORS for same-origin requests
    init.mode(web_sys::RequestMode::Cors);

    web_sys::Request::new_with_str_and_init(url, &init)
        .map_err(|e| OAuthError::TokenRequestFailed(format!("request creation failed: {e:?}")))
}
```

### Token Exchange Flow

The three OAuth methods (`exchange_code`, `client_credentials`, `refresh_token`) all follow the same pattern:

1. Validate `OAuthConfig`
2. Build URL-encoded form body
3. Create `web_sys::Request` with POST + Content-Type header
4. Call `js_fetch()` → `JsFuture::from(promise).await`
5. Check response status
6. Parse JSON body into `TokenResponse` → `OAuthToken`

The body-building logic is identical between native and wasm — only the HTTP call differs.

### TokenResponse (Private Deserialization)

Both native and wasm define the same private struct:

```rust
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    scope: Option<String>,
    id_token: Option<String>,
}
```

This struct maps exactly to the OAuth 2.0 token response spec and is used internally only. No need to expose it in shared/.

## What Stays in shared/

| Type | File | Description |
|------|------|-------------|
| `OAuthConfig` | `shared/oauth.rs` | Provider configuration (URLs, client ID, scopes) |
| `OAuthConfigBuilder` | `shared/oauth.rs` | Builder for `OAuthConfig` |
| `OAuthError` | `shared/oauth.rs` | All OAuth-related error variants |
| `PkceChallenge` | `shared/oauth.rs` | PKCE code verifier/challenge pair |
| `OAuthToken` | `shared/oauth_token.rs` | Token data with `into_jwt_token()` and `is_expired()` |

## What Goes in native/oauth.rs

| Item | Description |
|------|-------------|
| `OAuthManager` | OAuth flow orchestrator using `SimpleHttpClient` |
| `TokenResponse` | Private JSON deserialization struct |
| `OAuthManager::exchange_code` | POST via `SimpleHttpClient::post()` |
| `OAuthManager::client_credentials` | POST via `SimpleHttpClient::post()` |
| `OAuthManager::refresh_token` | POST via `SimpleHttpClient::post()` |

## What Goes in wasm_bindgen/oauth.rs

| Item | Description |
|------|-------------|
| `OAuthManager` | OAuth flow orchestrator using `web_sys::fetch` |
| `TokenResponse` | Private JSON deserialization struct (identical to native) |
| `js_fetch()` | Runtime-detecting fetch (service worker vs browser) |
| `build_request()` | Construct `web_sys::Request` with headers and body |
| `OAuthManager::exchange_code` | Async POST via `JsFuture` |
| `OAuthManager::client_credentials` | Async POST via `JsFuture` |
| `OAuthManager::refresh_token` | Async POST via `JsFuture` |

## OAuthManager API (identical for both targets)

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(config: OAuthConfig) -> Self` | Create from shared config |
| `config` | `fn config(&self) -> &OAuthConfig` | Borrow config |
| `generate_state` | `fn generate_state() -> String` | Random CSRF state |
| `get_authorization_url` | `fn get_authorization_url(&self, state: &str) -> Result<(String, Option<PkceChallenge>), OAuthError>` | Build redirect URL with query params |
| `validate_state` | `fn validate_state(expected: &str, actual: &str) -> bool` | Constant-time CSRF check |
| `exchange_code` | `fn exchange_code(&self, code: &str, code_verifier: Option<&str>) -> Result<OAuthToken, OAuthError>` | Authorization code flow |
| `client_credentials` | `fn client_credentials(&self, scopes: Option<Vec<String>>) -> Result<OAuthToken, OAuthError>` | Service-to-service auth |
| `refresh_token` | `fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken, OAuthError>` | Token refresh |

## Security Considerations

### PKCE
- Code verifier is 32 random bytes, base64-encoded (43 chars) — meets RFC 7636 minimum of 43 chars
- Code challenge is SHA-256 hash, base64-encoded (43 chars) — always uses S256 method
- Verifier is sent in the token exchange, never exposed in URLs

### State Parameter
- 32 random bytes, base64-encoded — resistant to guessing
- Constant-time comparison via byte equality check prevents timing attacks

### Token Storage
- Tokens should be stored via `CredentialStore` (already in shared/)
- In browser contexts, avoid `localStorage` (XSS-exposed) — prefer secure, HttpOnly cookies via server-side session management
- Cloudflare Workers can use KV bindings or D1 for token storage

### CORS
- OAuth token endpoints typically don't support CORS from browser contexts
- For browser OAuth flows, the token exchange should go through a backend proxy, not directly from the browser
- Cloudflare Workers (service worker context) don't have CORS restrictions for server-side fetch

## Error Mapping

The wasm fetch API returns `Promise<Error>` on network failures. Map these to `OAuthError`:

| JS Error Shape | OAuthError Variant |
|----------------|-------------------|
| Network failure (offline, DNS) | `TokenRequestFailed("fetch failed: ...")` |
| CORS blocked | `TokenRequestFailed("fetch failed: ...")` |
| Timeout (via AbortController) | `TokenRequestFailed("fetch timed out")` |
| Non-2xx response | `TokenEndpointError { status, message }` |
| Invalid JSON response | `TokenParseError("...")` |

## Testing Strategy

### Unit Tests (native only)
- `OAuthConfig::builder()` chaining
- `OAuthConfig::validate()` error conditions
- `PkceChallenge::generate()` format validation
- `OAuthManager::generate_state()` uniqueness
- `OAuthManager::validate_state()` correctness
- `OAuthManager::get_authorization_url()` URL structure

### Integration Tests (requires live OAuth provider)
- Full authorization code flow with mock token endpoint
- Client credentials flow
- Token refresh flow

### Wasm Tests
- Wasm tests require `wasm-pack test` or `wasm-bindgen-test`
- Mock `fetch` via `jsdom` or test against a local mock server
- Cloudflare Workers: use `miniflare` for local testing

## Tasks

1. [x] Add `wasm-bindgen-oauth` feature flag with wasm-bindgen, js-sys, web-sys deps to `Cargo.toml`
2. [ ] Create `wasm_bindgen/` directory with `mod.rs`
3. [ ] Implement `wasm_bindgen/oauth.rs` with `OAuthManager`, `js_fetch()`, `build_request()`
4. [ ] Update `lib.rs` to conditionally export `wasm_bindgen::oauth::OAuthManager`
5. [ ] Verify native compilation still works (no wasm-bindgen deps pulled in)
6. [ ] Verify wasm compilation with `--features wasm-bindgen-oauth`
7. [ ] Add basic tests for wasm OAuth manager

## Verification

```bash
# Native build (no wasm-bindgen deps)
cargo check -p foundation_auth

# Wasm build without OAuth (lightweight)
cargo check -p foundation_auth --target wasm32-unknown-unknown --features wasm

# Wasm build with OAuth support
cargo check -p foundation_auth --target wasm32-unknown-unknown --features wasm,wasm-bindgen-oauth

# Run tests
cargo test -p foundation_auth
```

## Dependency Impact

| Crate | Size Impact | When Pulled In |
|-------|-------------|----------------|
| `wasm-bindgen` | ~100 KB | Only with `wasm-bindgen-oauth` |
| `wasm-bindgen-futures` | ~10 KB | Only with `wasm-bindgen-oauth` |
| `js-sys` | ~200 KB | Only with `wasm-bindgen-oauth` |
| `web-sys` | ~500 KB+ | Only with `wasm-bindgen-oauth` |
| `foundation_auth` (base) | ~50 KB | Always |

Without `wasm-bindgen-oauth`, the wasm build of `foundation_auth` stays at ~50 KB with only the shared types.

---

_Created: 2026-05-17, Updated: 2026-05-18_
