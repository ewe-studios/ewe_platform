---
feature: "07-wasm-upstream-client"
spec: "57-foundation-social-and-keychain"
depends: "03-oauth-upstream-client"
status: "pending"
---

# Feature 07: WASM Upstream Client

## Goal

Provide a wasm32-compatible upstream OIDC client using browser fetch instead of the native HTTP client, for Tauri/desktop embed scenarios where the IdP runs in a WebView.

## WHAT

### WasmUpstreamClient

Uses `wasm_bindgen` fetch API instead of `SimpleHttpClient`:

```rust
// wasm_bindgen/upstream.rs
pub struct WasmUpstreamClient;

impl WasmUpstreamClient {
    pub async fn exchange_code(
        provider: &UpstreamProvider, code: &str, redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<UpstreamTokenResponse, Error> {
        // Uses web_sys::fetch or similar
    }

    pub async fn fetch_userinfo(
        provider: &UpstreamProvider, access_token: &str,
    ) -> Result<serde_json::Value, Error> {
        // Uses web_sys::fetch with Bearer auth header
    }
}
```

### Shared Types

The types (`UpstreamTokenResponse`, `ProviderProfile`, `ProviderBackend` trait) are in `shared/upstream/` and compile for both native and wasm32.

### Feature Gate

```toml
# In foundation_auth/Cargo.toml
[features]
social = ["foundation_http"]  # pulls in server + upstream client

# The upstream client is shared — no separate feature needed for wasm
```

## HOW

### Files to create

- `backends/foundation_auth/src/wasm_bindgen/upstream.rs` — WasmUpstreamClient
- `backends/foundation_auth/src/wasm_bindgen/mod.rs` — add `mod upstream`

### Notes

This feature is lower priority than the native path. The IdP server itself is native-only (`server` feature). The wasm upstream client is useful when the IdP is embedded in a WebView (Tauri) and needs to make upstream calls through the browser's network stack.

---

_Created: 2026-07-17_
