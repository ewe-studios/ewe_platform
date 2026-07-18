---
feature: "03-oauth-upstream-client"
spec: "57-foundation-social-and-keychain"
depends: "01-provider-model, 02-provider-migrations"
status: "complete"
---

# Feature 03: Generic Upstream OIDC/OAuth2 Client

## Goal

Build a reusable client that can authenticate with any OIDC/OAuth2 upstream provider: discover endpoints, build authorize URLs, exchange codes, fetch user profiles, and validate ID tokens.

## WHAT

### UpstreamOidcClient

The generic client that handles the standard OIDC flow:

```rust
pub struct UpstreamOidcClient {
    http: Arc<dyn HttpClient>,
}

impl UpstreamOidcClient {
    /// Fetch OIDC discovery document and populate provider config
    pub async fn discover(&self, discovery_url: &str) -> Result<OidcDiscovery, Error>;

    /// Build the authorization URL for the upstream provider
    pub fn authorize_url(
        &self, provider: &UpstreamProvider, state: &str, nonce: &str,
        code_challenge: &str, redirect_uri: &str,
    ) -> Result<Url, Error>;

    /// Exchange authorization code for tokens
    pub async fn exchange_code(
        &self, provider: &UpstreamProvider, code: &str, redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<UpstreamTokenResponse, Error>;

    /// Fetch user profile from userinfo endpoint
    pub async fn fetch_userinfo(
        &self, provider: &UpstreamProvider, access_token: &str,
    ) -> Result<serde_json::Value, Error>;

    /// Validate upstream ID token (if present)
    pub async fn validate_id_token(
        &self, provider: &UpstreamProvider, id_token: &str, nonce: &str,
    ) -> Result<IdTokenClaims, Error>;

    /// Fetch JWKS from upstream provider (for ID token validation)
    pub async fn fetch_jwks(&self, jwks_url: &str) -> Result<Jwks, Error>;
}
```

### ProviderBackend Trait

For providers that deviate from standard OIDC (D07):

```rust
pub trait ProviderBackend: Send + Sync + 'static {
    /// Override authorize URL (for providers without OIDC discovery)
    fn authorize_url(&self, _provider: &UpstreamProvider, _state: &str, _nonce: &str,
                     _code_challenge: &str, _redirect_uri: &str) -> Result<Option<Url>, Error> { Ok(None) }

    /// Override token exchange (for providers with non-standard responses)
    fn exchange_code(&self, _http: &dyn HttpClient, _provider: &UpstreamProvider,
                     _code: &str, _redirect_uri: &str, _code_verifier: &str)
                     -> Result<Option<UpstreamTokenResponse>, Error> { Ok(None) }

    /// Override user profile fetch (for providers without OIDC userinfo)
    fn fetch_profile(&self, _http: &dyn HttpClient, _provider: &UpstreamProvider,
                     _access_token: &str) -> Result<Option<ProviderProfile>, Error> { Ok(None) }

    /// Map upstream claims to normalized profile
    fn map_claims(&self, _raw: &serde_json::Value) -> Result<Option<ProviderProfile>, Error> { Ok(None) }
}
```

### Shipped Backends

- **GoogleBackend:** All default (standard OIDC)
- **GitHubBackend:** Overrides `fetch_profile` (GET `https://api.github.com/user` + `https://api.github.com/user/emails`), `map_claims` (GitHub uses `login`, `id`, no `email_verified`), `authorize_url` (no discovery, hardcoded URLs)
- **FacebookBackend:** Overrides `authorize_url` (no discovery), `fetch_profile` (custom endpoint with `fields=` param), `map_claims`

### UpstreamTokenResponse

```rust
pub struct UpstreamTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub id_token: Option<String>,    // OIDC only
    pub refresh_token: Option<String>,
    pub scope: String,
}
```

### ProviderProfile

```rust
pub struct ProviderProfile {
    pub subject: String,       // unique ID from provider
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: Option<String>,
    pub username: Option<String>,
    pub picture: Option<String>,
    pub raw: serde_json::Value, // original response for debugging
}
```

## HOW

### Files to create

- `backends/foundation_auth/src/shared/upstream/mod.rs` — module root
- `backends/foundation_auth/src/shared/upstream/client.rs` — UpstreamOidcClient
- `backends/foundation_auth/src/shared/upstream/traits.rs` — ProviderBackend trait
- `backends/foundation_auth/src/shared/upstream/backends/mod.rs` — provider backends
- `backends/foundation_auth/src/shared/upstream/backends/google.rs` — GoogleBackend
- `backends/foundation_auth/src/shared/upstream/backends/github.rs` — GitHubBackend
- `backends/foundation_auth/src/shared/upstream/backends/facebook.rs` — FacebookBackend
- `backends/foundation_auth/src/shared/upstream/types.rs` — UpstreamTokenResponse, ProviderProfile, OidcDiscovery

### Module placement

`shared/` not `server/` — the upstream client is a cross-platform component. The wasm32 variant (feature 07) reuses the same types.

### Dependencies

- `url` crate (URL building)
- `urlencoding` (already in Cargo.toml)
- `jsonwebtoken` or reuse `jwt-simple` (already a dependency) for ID token validation
- `http` crate (already in dependency tree)

### Key Design Details

**PKCE is always used.** Even for providers that don't require it, we send `code_challenge` and `code_verifier` (S256). This is consistent with the IdP's own PKCE requirement (D08).

**Nonce is always generated.** For OIDC providers, the nonce is included in the authorize URL and validated in the ID token. For OAuth2-only providers, a nonce is still generated but ignored (no ID token to validate).

**Timeout on upstream calls.** All upstream HTTP calls have a 10-second timeout. If Google doesn't respond, we fail fast and redirect the user back to the app with an error.

---

_Created: 2026-07-17_
