# Feature 07: WASM Upstream Client

**Status:** Partial

**Depends on:** `03-oauth-upstream-client`

## Summary

Provides a wasm32-compatible OAuth client (`WasmOAuth`) that uses browser fetch APIs instead of the native HTTP stack, for Tauri/desktop embed scenarios where the IdP runs in a WebView. The `WasmOAuth` struct and its `exchange_code` / `exchange_code_async` methods exist but are **not yet wired** into the `UpstreamOidcClient` for automatic platform dispatch.

## What was built

### Files created

- `backends/foundation_auth/src/wasm_bindgen/oauth.rs` -- `WasmOAuth`, `exchange_code`, `exchange_code_async`

### File modified

- `backends/foundation_auth/src/wasm_bindgen/mod.rs` -- `mod oauth` registered

### Key types / methods

| Type | Purpose |
|------|---------|
| `WasmOAuth` | WASM-native OAuth client using `web_sys` / browser fetch |
| `exchange_code()` | Synchronous JS binding wrapper around `exchange_code_async` |
| `exchange_code_async(config, code, code_verifier)` | Async implementation using browser fetch to POST to token endpoint |

### What is missing (partial status)

The `UpstreamOidcClient` in `shared/upstream_client.rs` documents that `exchange_code()` is "platform-gated" with native using `NativeOAuth` and wasm using `WasmOAuth`, but the actual dispatch is **not implemented**:

- `UpstreamOidcClient` has no `exchange_code` method -- it only provides `authorize_url` and `userinfo_endpoint`
- Callers must manually choose `NativeOAuth` or `WasmOAuth` based on `cfg(target_arch = "wasm32")`
- No `#[cfg]`-gated `impl UpstreamOidcClient { fn exchange_code(...) }` that dispatches automatically

### Shared types

The types used by the wasm client (`UpstreamTokenResponse`, `UpstreamProfile`, `UpstreamClientError`) are in `shared/upstream_client.rs` and compile for both native and wasm32 targets -- no duplication needed.

## Tests

No dedicated tests for `WasmOAuth` yet. The shared types are covered by F003's 5 unit tests.

## Related decisions

- `decisions/00-identity-broker-pattern.md` -- upstream authentication patterns
- `decisions/03-feature-gate-strategy.md` -- platform gating strategy

## Implementation notes

- **`WasmOAuth` exists and compiles for wasm32** -- the struct, `exchange_code`, and `exchange_code_async` are all present in `wasm_bindgen/oauth.rs`
- **Not wired to `UpstreamOidcClient`** -- the platform dispatch is documented but not implemented. Callers must use `cfg(target_arch = "wasm32")` to select the right backend.
- The wasm upstream client is useful when the IdP is embedded in a WebView (Tauri) and needs to make upstream calls through the browser's network stack rather than Rust's native HTTP.
- `foundation_auth` already has a working `wasm_bindgen/oauth.rs` module so the browser fetch plumbing is proven -- what remains is the `UpstreamOidcClient` integration.
