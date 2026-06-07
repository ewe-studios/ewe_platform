# Feature 03: OIDC Discovery Client

## Description

Fetch and parse the OIDC discovery document from `/.well-known/openid-configuration`.
Auto-configure `OAuthConfig` from the discovery response instead of manual endpoint setup.
Works on both native and wasm.

### Discovery Document — Cache, Don't Persist

The `/.well-known/openid-configuration` path is a **standard HTTP endpoint**, not storage.
It is defined by the OIDC specification (RFC 8414) and lives at:

```
{issuer}/.well-known/openid-configuration
```

For example:
- `https://accounts.google.com/.well-known/openid-configuration`
- `https://login.microsoftonline.com/common/.well-known/openid-configuration`

The document returns a small JSON blob of provider metadata (endpoints, supported algorithms,
scopes, etc.) that **rarely changes** — only when a provider adds or removes capabilities.

**Caching strategy:**
- Fetch once, cache in memory for the lifetime of the application.
- No TTL needed — unlike JWKS (Feature 02) which rotates keys frequently, discovery documents
  are essentially static configuration.
- No KVStore, no foundation_db, no disk persistence. Just an in-memory `Option<OidcDiscovery>`.
- Provide `refresh()` for explicit cache invalidation (e.g. provider URL changed at runtime).
- On process restart, re-fetch naturally.

**What does NOT use foundation_db:**
- The discovery document is NOT a credential, session, or policy. It does not need
  `KeyValueStore` or `QueryStore`. It is HTTP response metadata, cached in a struct field.

**What DOES use foundation_db in this spec:**
- User records, OAuth clients, authorization codes, refresh tokens, device codes →
  `QueryStore` (Feature 10 IdP models, Feature 13 migrations).
- Credential storage, session data → `KeyValueStore` (existing auth).
- Policy files from git/disk/S3 → `foundation_nativeapis` (Feature 14 Cedar only).

## Module

`backends/foundation_auth/src/shared/discovery.rs` — shared types and logic (native + wasm share the same HTTP client pattern via existing split)

## API Surface

```rust
/// OIDC discovery document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
    pub registration_endpoint: Option<String>,
    pub scopes_supported: Option<Vec<String>>,
    pub response_types_supported: Vec<String>,
    pub response_modes_supported: Option<Vec<String>>,
    pub grant_types_supported: Option<Vec<String>>,
    pub acr_values_supported: Option<Vec<String>>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub id_token_encryption_alg_values_supported: Option<Vec<String>>,
    pub userinfo_signing_alg_values_supported: Option<Vec<String>>,
    pub token_endpoint_auth_methods_supported: Option<Vec<String>>,
    pub token_endpoint_auth_signing_alg_values_supported: Option<Vec<String>>,
    pub display_values_supported: Option<Vec<String>>,
    pub claim_types_supported: Option<Vec<String>>,
    pub claims_supported: Option<Vec<String>>,
    pub service_documentation: Option<String>,
    pub claims_locales_supported: Option<Vec<String>>,
    pub ui_locales_supported: Option<Vec<String>>,
    pub introspection_endpoint: Option<String>,
    pub revocation_endpoint: Option<String>,
    pub device_authorization_endpoint: Option<String>,
    pub code_challenge_methods_supported: Option<Vec<String>>,
}

/// Discovery client — fetches and caches OIDC discovery documents.
///
/// The discovery document is configuration metadata that rarely changes.
/// It is cached in memory for the lifetime of the client — no disk or DB persistence.
pub struct DiscoveryClient {
    cached: Option<(String, OidcDiscovery)>,  // (issuer_url, discovery)
}

impl DiscoveryClient {
    /// Create a new discovery client with an empty cache.
    #[must_use]
    pub fn new() -> Self;

    /// Get the cached discovery document, fetching if not cached.
    pub async fn get(&mut self, issuer_url: &str) -> Result<&OidcDiscovery, DiscoveryError>;

    /// Fetch discovery document from the issuer URL, replacing cache.
    pub async fn fetch(&mut self, issuer_url: &str) -> Result<&OidcDiscovery, DiscoveryError>;

    /// Fetch discovery document from an explicit URL (bypasses cache, no caching).
    pub async fn fetch_from_url(&self, url: &str) -> Result<OidcDiscovery, DiscoveryError>;

    /// Clear the cached discovery document.
    pub fn clear_cache(&mut self);

    /// Returns true if a discovery document is cached for this issuer.
    #[must_use]
    pub fn is_cached(&self, issuer_url: &str) -> bool;
}

impl OidcDiscovery {
    /// Build an OAuthConfig from this discovery document.
    pub fn to_oauth_config(&self, client_id: &str, redirect_uri: &str) -> OAuthConfig;

    /// Get the JWKS URL, preferring jwks_uri over issuer + "/jwks".
    #[must_use]
    pub fn jwks_url(&self) -> String;
}
```

## Implementation Details

### Discovery URL construction
- If input is an issuer URL (no path or path ends with `/`): append `.well-known/openid-configuration`
- If input is already a full URL: use as-is
- Normalize trailing slashes: `https://auth.example.com` → `https://auth.example.com/.well-known/openid-configuration`

### OAuthConfig mapping
- `authorization_url` ← `authorization_endpoint`
- `token_url` ← `token_endpoint`
- `scopes` ← `scopes_supported` (filtered to `openid`, `profile`, `email`, `groups` by default)
- `pkce_enabled` ← `code_challenge_methods_supported` contains `"S256"`
- `response_type` ← `response_types_supported[0]` (usually `"code"`)
- `client_id` and `redirect_uri` passed in by caller

### HTTP fetch (shared — same pattern for native and wasm)
- The `DiscoveryClient` uses a platform-agnostic internal HTTP fetcher
- Native: delegates through `foundation_netio::SimpleHttpClient`
- Wasm: uses browser fetch API via `web-sys`
- Both return JSON body → `serde_json::from_str` → `OidcDiscovery`

### Caching (in-memory, no persistence)
- `cached: Option<(String, OidcDiscovery)>` stores `(issuer_url, discovery_document)`
- `get()` checks cache first — returns cached if issuer matches, otherwise fetches
- `fetch()` always fetches and replaces cache
- `clear_cache()` drops the cached entry
- No TTL, no expiration, no background refresh — discovery documents are static config

### Error type
```rust
pub enum DiscoveryError {
    InvalidIssuerUrl(String),
    FetchFailed(String),
    ParseError(String),
    MissingRequiredField(String),
}
```

## Dependencies

- Existing: `foundation_netio`, `serde`, `serde_json`
- Existing: `OAuthConfig` from `shared/oauth.rs`

## Testing

- Parse full discovery JSON → all fields populated
- Parse minimal discovery JSON → optional fields are None
- `to_oauth_config` → authorization_url, token_url, scopes set correctly
- PKCE enabled when S256 in code_challenge_methods_supported
- URL normalization: `https://auth.example.com` → correct well-known URL
- Error: invalid JSON → `ParseError`
- Error: missing issuer → `MissingRequiredField`

## Sync/Async Notes

The `async fn` methods shown are the primary implementation. For sync callers,
use valtron bridging (no tokio):

```rust
let task = from_future(async move { client.fetch(issuer_url).await });
let stream = execute(task, None)?;
collect_one(stream).ok_or_else(|| DiscoveryError::FetchFailed("no result".into()))?
```

For callers who want to hold onto `&mut DiscoveryClient` across async calls and need
sync access to the cached result, the `&self` methods (like `is_cached`, `clear_cache`,
`jwks_url`) are pure sync state accessors.

See the valtron skill and requirements.md "Valtron Bridging" section for full patterns.
