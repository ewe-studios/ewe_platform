# Start: Wasm OAuth Manager

## Goal

Create a wasm-compatible `OAuthManager` for `foundation_auth` that uses `web_sys` fetch API instead of `SimpleHttpClient`. The shared OAuth types already exist — only the HTTP transport layer differs.

## Current State

- `shared/oauth.rs` contains `OAuthConfig`, `OAuthConfigBuilder`, `PkceChallenge`, `OAuthError`
- `shared/oauth_token.rs` contains `OAuthToken`
- `native/oauth.rs` contains `OAuthManager` with `SimpleHttpClient` calls
- Both native and wasm compile cleanly for foundation_auth

## Plan

1. Add `OAuthHttpClient` trait to `shared/oauth.rs`
2. Refactor `native/oauth.rs` to use the trait
3. Create `wasm_bindgen/` directory with `mod.rs` and `oauth.rs`
4. Implement `OAuthManager` using `web_sys` fetch with service worker detection
5. Add `wasm-oauth` feature flag

## Key Reference

The `js_fetch` function in reqwest's wasm client (`/home/darkvoid/Boxxed/@formulas/src.rust/src.wasm/reqwest/src/wasm/client.rs`) detects `ServiceWorkerGlobalScope` vs browser and routes fetch accordingly — this is the pattern we'll follow for Cloudflare Workers compatibility.
