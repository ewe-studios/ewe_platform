---
feature: "Wasm OAuth Manager (wasm-bindgen)"
description: "Implement OAuthManager for wasm32 using browser/worker fetch API, shared OAuth types, and configurable HTTP client trait"
status: "planned"
priority: "high"
depends_on: ["02-auth-wasm-compat"]
estimated_effort: "medium"
created: 2026-05-17
last_updated: 2026-05-17
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature: Wasm OAuth Manager (wasm-bindgen)

## Overview

Implement a wasm-compatible `OAuthManager` in `foundation_auth` that uses the browser/worker `fetch()` API instead of `SimpleHttpClient`. The shared OAuth types (`OAuthConfig`, `OAuthError`, `PkceChallenge`, `OAuthToken`) already live in `shared/oauth.rs` and `shared/oauth_token.rs` — only the HTTP transport layer differs between native and wasm.

## Design

### Shared Trait: OAuthHttpClient

Define a trait in `shared/oauth.rs` that abstracts over the HTTP client:

```rust
/// Minimal HTTP client interface for OAuth token exchange.
pub trait OAuthHttpClient {
    /// POST URL-encoded body, return (status_code, body_string).
    fn post_form(
        url: &str,
        body: &str,
    ) -> Result<(u16, String), OAuthError>;
}
```

### Native Implementation

`native/oauth.rs` implements `OAuthHttpClient` using `SimpleHttpClient`:

```rust
pub struct NativeOAuthClient;

impl OAuthHttpClient for NativeOAuthClient {
    fn post_form(url: &str, body: &str) -> Result<(u16, String), OAuthError> {
        let client = SimpleHttpClient::from_system();
        // ... existing post logic ...
        Ok((status, body_text))
    }
}
```

### Wasm Implementation

Create `wasm_bindgen/oauth.rs` gated behind `#[cfg(target_arch = "wasm32")]` and `feature = "wasm-oauth"`:

```rust
use wasm_bindgen::prelude::*;
use web_sys::{Request, RequestInit, RequestMode, Response};

pub struct WasmOAuthClient;

impl OAuthHttpClient for WasmOAuthClient {
    fn post_form(url: &str, body: &str) -> Result<(u16, String), OAuthError> {
        // Use wasm_bindgen_futures or synchronous-ish fetch pattern
        // depending on whether we're in a service worker or browser context
    }
}
```

Inspiration from `reqwest/src/wasm/client.rs` — the `js_fetch` function detects the runtime environment:

```rust
fn js_fetch(req: &web_sys::Request) -> Promise {
    let global = js_sys::global();
    if js_sys::Reflect::has(&global, &JsValue::from_str("ServiceWorkerGlobalScope")) == Ok(true) {
        global.unchecked_into::<ServiceWorkerGlobalScope>().fetch_with_request(req)
    } else {
        fetch_with_request(req)
    }
}
```

This dual-detection is critical for Cloudflare Workers (service worker context) vs browser OAuth flows.

### Generic OAuthManager

Make `OAuthManager` generic over the client, or use a cfg-based internal implementation:

```rust
// In shared/oauth.rs — the OAuthManager struct definition lives here
// with cfg-gated method bodies that call the appropriate client.

#[cfg(not(target_arch = "wasm32"))]
pub use crate::native::oauth::OAuthManager;

#[cfg(target_arch = "wasm32")]
pub use crate::wasm_bindgen::oauth::OAuthManager;
```

## Requirements

### Feature Flag

Add to `foundation_auth/Cargo.toml`:

```toml
[features]
wasm = ["uuid/js", "chrono/wasmbind", "dep:getrandom", "getrandom/js"]
wasm-oauth = ["wasm", "wasm-bindgen", "wasm-bindgen-futures", "js-sys", "web-sys"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
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
], optional = true }
```

### OAuthManager API (identical for both targets)

The following methods must be available on both native and wasm `OAuthManager`:

| Method | Description |
|--------|-------------|
| `new(config: OAuthConfig)` | Create from shared config |
| `config() -> &OAuthConfig` | Borrow config |
| `generate_state() -> String` | Random CSRF state |
| `get_authorization_url(state) -> (String, Option<PkceChallenge>)` | Build redirect URL |
| `validate_state(expected, actual) -> bool` | CSRF check |
| `exchange_code(code, code_verifier) -> OAuthToken` | Token exchange via fetch |
| `client_credentials(scopes) -> OAuthToken` | Service-to-service auth |
| `refresh_token(refresh_token) -> OAuthToken` | Token refresh |

### What Stays in shared/

| Type | File |
|------|------|
| `OAuthConfig`, `OAuthConfigBuilder` | `shared/oauth.rs` |
| `OAuthError` | `shared/oauth.rs` |
| `PkceChallenge` | `shared/oauth.rs` |
| `OAuthToken` | `shared/oauth_token.rs` |
| `OAuthHttpClient` trait | `shared/oauth.rs` |

### What Goes in native/oauth.rs

- `OAuthManager` struct with `SimpleHttpClient`-based HTTP calls
- `TokenResponse` deserialization (private)

### What Goes in wasm_bindgen/oauth.rs

- `OAuthManager` struct with `web_sys::fetch`-based HTTP calls
- `TokenResponse` deserialization (private, same struct)
- `js_fetch()` runtime detection (service worker vs browser)
- Request body handling via `web_sys::RequestInit`

## Tasks

1. [ ] Add `OAuthHttpClient` trait to `shared/oauth.rs`
2. [ ] Refactor `native/oauth.rs` to implement `OAuthHttpClient` via `SimpleHttpClient`
3. [ ] Create `wasm_bindgen/` directory with `mod.rs` and `oauth.rs`
4. [ ] Implement `OAuthManager` in `wasm_bindgen/oauth.rs` using `web_sys` fetch
5. [ ] Add `wasm-oauth` feature flag with wasm-bindgen, js-sys, web-sys deps
6. [ ] Update `lib.rs` to conditionally export `wasm_bindgen::oauth::OAuthManager`
7. [ ] Verify `cargo build --target wasm32-unknown-unknown --features wasm,wasm-oauth`

## Verification

```bash
# Wasm build with OAuth support
cargo build -p foundation_auth --target wasm32-unknown-unknown \
  --features wasm,wasm-oauth

# Native still works
cargo build -p foundation_auth
```

## Notes

- No `tokio` or async runtime dependencies — use `wasm_bindgen_futures::JsFuture` for the fetch promise
- The `TokenResponse` struct is identical between native and wasm — consider moving to shared if both implementations share the exact same deserialization logic
- Cloudflare Workers use the service worker global scope, not the browser window — the `js_fetch` detection from reqwest handles both

---

_Created: 2026-05-17_
