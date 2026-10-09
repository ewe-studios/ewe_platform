# Feature 07: Cross-Platform Upstream Client (native + wasm/Workers)

**Status:** ✅ Complete

**Depends on:** `03-oauth-upstream-client`

## Summary

The upstream broker client (`UpstreamOidcClient`) is now **fully cross-platform**:
discovery, authorize-URL generation, token exchange, and userinfo all run over the
shared `foundation_netio` `HttpClient` (`default_http_client() -> Arc<dyn HttpClient>`),
which resolves to the native HTTP stack on native targets and browser/Worker `fetch`
on wasm32. The whole identity-broker flow therefore executes wherever an `HttpClient`
exists — **including a Cloudflare Worker** — with no platform-gated branching in the
client itself.

This supersedes the original F07 framing (a separate `WasmOAuth` that callers would
manually dispatch to via `cfg(target_arch = "wasm32")`). Per [decision 00](../decisions/00-identity-broker-pattern.md),
the IdP is a **server-side broker** that runs the OAuth dance itself, and platform
independence is achieved through the injected `HttpClient` (`&dyn HttpClient`), not
through cfg dispatch. `WasmOAuth` (`wasm_bindgen/oauth.rs`) remains as an optional
browser-direct OAuth binding but is no longer on the broker path.

## What was built

### `UpstreamOidcClient` is no longer `server`-gated

- The provider models (`UpstreamProvider`, `ProviderType`, `ProviderMapping`,
  `ProviderUpdate` — pure serde) moved from `server/models/provider.rs` to
  `shared/provider.rs` so they compile on wasm. `server::models::provider` re-exports
  them for back-compat.
- `UpstreamOidcClient`, `UpstreamProfile::from_userinfo`/`from_id_token`, and the crate
  re-exports dropped their `#[cfg(feature = "server")]` gates. The client compiles and
  runs on both native and wasm32.

### New methods on `UpstreamOidcClient`

| Method | Purpose |
|--------|---------|
| `async fn exchange_code(&self, code, code_verifier, client_secret)` | OAuth2 `authorization_code` grant against the provider's token endpoint → `OAuthToken`. Sends `client_secret` only when provided (public PKCE clients pass `None`); falls back to any secret on the configured `OAuthConfig`. |
| `async fn fetch_userinfo(&self, access_token)` | `GET` the userinfo endpoint with `Authorization: Bearer` → `UpstreamProfile` mapped through the provider's `mapping_config`. |

Both run over `default_http_client()` and mirror the existing shared clients
(`discovery.rs`, `userinfo.rs`).

### Cross-platform HTTP correctness fixes (shared clients)

`send`/`send_async` return the response body as a lazy `SendSafeBody::Stream`.
`discovery.rs`, `userinfo.rs`, `jwks.rs`, `introspection.rs`, and the new
`upstream_client.rs` helpers previously read the body via `get_body_ref()`, which
observes a `Stream` body as **empty** — so every real fetch silently returned an empty
string. All five now drain the body with
`foundation_netio::shared::client::body_reader::try_collect_bytes(resp.take_body())`.

### wasm-compilation fixes (foundation_auth had never compiled cleanly on wasm32)

- All five shared clients imported the native-only `foundation_netio::http::default_http_client`
  → repointed to the platform-agnostic crate-root `foundation_netio::default_http_client`.
- `Cargo.toml`: the wasm-target `foundation_netio` dependency was missing
  `default-features = false`; cargo unions default-features across declarations, so the
  wasm build was re-enabling `foundation_netio`'s `default` feature (→ native `ssl` +
  `multi` + `wire-native`) and failing to compile. Added `default-features = false`.
- `Cargo.toml`: `web-sys` was missing the `SubtleCrypto` + `CryptoKey` features needed
  by the WASM PBKDF2 (`password_hash.rs`) WebCrypto path.
- `password_hash.rs`: two real bugs in the never-compiled WebCrypto branch (`Reflect::set`
  returns `Result<bool,_>` not `Result<(),_>`; ambiguous `"deriveBits".into()`).
- `foundation_netio/network_client.rs` (dependency): the `tls_connector` field/method were
  gated on `ssl-*` features only, not the target, so `netcap::ssl` (native-only) was
  referenced on wasm when feature unification enabled `ssl`. Now `not(target_family = "wasm")`
  as well.

## Tests

`tests/integration/upstream_client/mod.rs` — 5 `#[valtron_test]` integration tests that
drive the async methods end-to-end against a real `TestHttpServer` over the same
`default_http_client()` wasm/Workers use:

- `exchange_code_posts_grant_and_parses_token` — asserts the posted form
  (`grant_type`/`code`/`client_id`/`client_secret`/`code_verifier`) and the parsed `OAuthToken`.
- `exchange_code_surfaces_endpoint_error` — a 400 from the token endpoint is an error.
- `fetch_userinfo_sends_bearer_and_maps_profile` — asserts `Authorization: Bearer` and the mapped `UpstreamProfile`.
- `fetch_userinfo_without_endpoint_errors` — missing userinfo endpoint errors, not panics.
- `discover_and_configure_reads_endpoints_from_discovery_doc` — end-to-end OIDC discovery
  over the shared client, proving the `discovery.rs` body-read fix.

Plus the 5 existing `upstream_client` unit tests. Native (`server`) and wasm32
(`wasm-pbkdf2`) both compile clean.

## Related decisions

- `decisions/00-identity-broker-pattern.md` — server-side broker; `&dyn HttpClient` injection
- `decisions/03-feature-gate-strategy.md` — target gates, not backend features
