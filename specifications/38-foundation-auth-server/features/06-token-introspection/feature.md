# Feature 06: Token Introspection

## Description

RFC 7662 token introspection client for resource servers. Allows a resource server to validate an access token by asking the authorization server directly, rather than verifying the signature locally.

## Module

`backends/foundation_auth/src/shared/introspection.rs` — shared types

## API Surface

```rust
/// Token introspection result (RFC 7662).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionResult {
    /// Whether the token is currently active.
    pub active: bool,
    /// Space-separated list of scopes.
    pub scope: Option<String>,
    /// Client ID the token was issued to.
    pub client_id: Option<String>,
    /// Subject identifier (user ID).
    pub sub: Option<String>,
    /// Human-readable username.
    pub username: Option<String>,
    /// Token type (e.g., "Bearer").
    pub token_type: Option<String>,
    /// Expiration time (Unix timestamp).
    pub exp: Option<i64>,
    /// Issuance time (Unix timestamp).
    pub iat: Option<i64>,
    /// Not-before time (Unix timestamp).
    pub nbf: Option<i64>,
    /// Subject of the token.
    pub iss: Option<String>,
    /// Intended audience.
    pub aud: Option<String>,
    /// Token identifier.
    pub jti: Option<String>,
}

/// Token introspection client.
pub struct IntrospectionClient;

impl IntrospectionClient {
    /// Introspect a token at the given endpoint.
    /// The resource authenticates with client_id + client_secret.
    pub async fn introspect(
        introspection_url: &str,
        token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<IntrospectionResult, IntrospectionError>;
}
```

## Implementation Details

### HTTP request
- `POST {introspection_url}` with `Content-Type: application/x-www-form-urlencoded`
- Body: `token={token}`
- Auth: HTTP Basic Auth with `client_id:client_secret` (`Authorization: Basic base64(client_id:client_secret)`)

### Response
- 200: parse JSON into `IntrospectionResult`
- 401: `IntrospectionError::Unauthorized` (resource server credentials invalid)
- Other: `IntrospectionError::ServerError`

### Usage pattern
```rust
let result = IntrospectionClient::introspect(
    "https://auth.example.com/oidc/introspect",
    &access_token,
    "resource-server-id",
    "resource-server-secret",
).await?;

if result.active {
    // Token is valid — proceed
    let user_id = result.sub.as_ref().unwrap();
    let scopes: Vec<&str> = result.scope.as_deref().unwrap_or("").split_whitespace().collect();
} else {
    // Token is inactive — reject
}
```

### Error type
```rust
pub enum IntrospectionError {
    ConnectionFailed(String),
    Unauthorized(String),
    ServerError { status: u16, message: String },
    ParseError(String),
}
```

## Dependencies

- Existing: `foundation_netio`, `serde`, `serde_json`, `base64`

## Testing

- Parse active token response → all fields populated
- Parse inactive token response → `active: false`, other fields None
- Error: 401 → `Unauthorized`
- Error: invalid JSON → `ParseError`
