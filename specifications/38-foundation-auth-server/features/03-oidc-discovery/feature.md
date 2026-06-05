# Feature 03: OIDC Discovery Client

## Description

Fetch and parse the OIDC discovery document from `/.well-known/openid-configuration`. Auto-configure `OAuthConfig` from the discovery response instead of manual endpoint setup. Works on both native and wasm.

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

/// Discovery client — fetches and parses OIDC discovery documents.
pub struct DiscoveryClient;

impl DiscoveryClient {
    /// Fetch discovery document from the issuer URL.
    /// Constructs the well-known URL: {issuer}/.well-known/openid-configuration
    pub async fn fetch(issuer_url: &str) -> Result<OidcDiscovery, DiscoveryError>;

    /// Fetch discovery document from an explicit URL.
    pub async fn fetch_from_url(url: &str) -> Result<OidcDiscovery, DiscoveryError>;
}

impl OidcDiscovery {
    /// Build an OAuthConfig from this discovery document.
    pub fn to_oauth_config(&self, client_id: &str, redirect_uri: &str) -> OAuthConfig;

    /// Get the JWKS URL, preferring jwks_uri over issuer + "/jwks".
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

### HTTP fetch
- Native: `SimpleHttpClient::get(url).send_async()`
- Wasm: browser fetch API (same pattern as `wasm_bindgen/oauth.rs`)
- Both return JSON body → `serde_json::from_str`

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
