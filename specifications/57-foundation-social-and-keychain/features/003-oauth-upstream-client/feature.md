# Feature 03: Generic Upstream OIDC/OAuth2 Client

**Status:** Complete

**Depends on:** `01-provider-model`, `02-provider-migrations`

## Summary

Builds a platform-agnostic upstream OIDC/OAuth2 client that can authenticate with any upstream provider. Handles OIDC discovery, authorize URL construction with PKCE, and user profile fetching. Token exchange is platform-gated (native uses `NativeOAuth`, wasm uses `WasmOAuth`).

## What was built

### Files created

- `backends/foundation_auth/src/shared/upstream_client.rs` -- `UpstreamOidcClient`, `UpstreamClientError`, `UpstreamProfile`

### Key types

| Type | Purpose |
|------|---------|
| `UpstreamOidcClient` | Main client: wraps `DiscoveryClient` for OIDC, `OAuthManager` for authorize URLs |
| `UpstreamClientError` | Error enum covering discovery, OAuth, HTTP, and token failures |
| `UpstreamProfile` | Normalized profile from userinfo (`sub`, `email`, `email_verified`, `name`, `username`) |

### Key methods

- `discover_and_configure(provider)` -- fetches OIDC discovery doc, builds `OAuthConfig`
- `authorize_url(state)` -> `(url, pkce_challenge)` -- ready for redirect
- `authorize_url_no_pkce(state)` -- for providers that don't support PKCE
- `userinfo_endpoint()` -- prefers discovery doc value, falls back to provider config
- `exchange_code()` -- **platform-gated** (native: `NativeOAuth`, wasm: `WasmOAuth`)

### Module placement

`shared/` -- not `server/` -- the upstream client is cross-platform. The wasm32 variant (F007) reuses the same types.

## Tests

- `backends/foundation_auth/src/shared/upstream_client.rs` (inline `#[cfg(test)] mod tests`) -- **5 unit tests**
  - Client construction with provider config
  - Authorize URL generation with PKCE
  - OIDC discovery + configure flow
  - Userinfo endpoint resolution
  - Error handling for missing endpoints

## Related decisions

- `decisions/00-identity-broker-pattern.md` -- upstream authentication patterns
- `decisions/03-feature-gate-strategy.md` -- platform gating approach

## Implementation notes

- **PKCE is always used.** Even for providers that don't require it, the client sends `code_challenge`/`code_verifier` (S256).
- **10-second timeout on upstream HTTP calls.** If the upstream provider doesn't respond, the client fails fast.
- **Nonce is always generated.** For OIDC providers it's validated in the ID token; for OAuth2-only providers it's generated but ignored.
- **exchange_code is platform-gated.** The `UpstreamOidcClient` uses `default_http_client()` (cross-platform via `Arc<dyn HttpClient>`). F007 completed the wasm path — both native and Workers work.
- Google, GitHub, and Facebook backends are documented in the spec but implemented as configuration profiles rather than separate `ProviderBackend` trait impls -- the normalized `UpstreamProfile` + `mapping_config` handles all standard OIDC/OAuth2 providers without per-provider code.
