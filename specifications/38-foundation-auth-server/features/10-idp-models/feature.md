# Feature 10: IdP Models

## Description

Data model entities for the IdP server: User, OAuth Client, Authorization Code, Device Code, Refresh Token. These map to foundation_db migrations and are used by services for database operations.

## Modules

`backends/foundation_auth/src/server/models/` — directory containing model files

## API Surface

### User Model (`models/user.rs`)

```rust
/// User entity — maps to migration 002_create_users.
pub struct User {
    pub id: String,
    pub email: String,
    pub username: Option<String>,
    pub password_hash: Option<String>,  // Argon2id hash
    pub email_verified: bool,
    pub email_verified_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub metadata: Option<serde_json::Value>,
    pub failed_login_attempts: u32,
    pub locked_until: Option<i64>,
    pub deleted_at: Option<i64>,
}

impl User {
    /// Check if the account is currently locked.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked_until.is_some_and(|until| {
            let now = chrono::Utc::now().timestamp_millis();
            now < until
        })
    }

    /// Record a failed login attempt.
    pub fn record_failed_attempt(&mut self, max_attempts: u32, lockout_duration: Duration);
}
```

### Client Model (`models/client.rs`)

```rust
/// OAuth client entity — maps to migration 016_create_oauth_clients.
pub struct OAuthClient {
    pub id: String,
    pub name: String,
    pub client_secret_hash: String,     // SHA256 hash of the secret
    pub redirect_uris: Vec<String>,     // JSON array in DB
    pub grant_types: Vec<String>,       // JSON array in DB
    pub scopes: Vec<String>,            // JSON array in DB
    pub is_public: bool,
    pub created_at: i64,
}

impl OAuthClient {
    /// Verify the client secret.
    #[must_use]
    pub fn verify_secret(&self, secret: &str) -> bool {
        let hash = sha256(secret.as_bytes());
        constant_time_eq(&hash, self.client_secret_hash.as_bytes())
    }

    /// Check if a redirect URI is allowed.
    #[must_use]
    pub fn allows_redirect(&self, uri: &str) -> bool {
        self.redirect_uris.iter().any(|r| r == uri)
    }

    /// Check if a grant type is allowed.
    #[must_use]
    pub fn allows_grant(&self, grant: &str) -> bool {
        self.grant_types.iter().any(|g| g == grant)
    }
}
```

### Authorization Code Model (`models/code.rs`)

```rust
/// Single-use authorization code — maps to migration 017_create_authorization_codes.
pub struct AuthorizationCode {
    pub code: String,           // Random code value
    pub user_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: Option<String>,  // PKCE challenge (S256)
    pub scope: String,
    pub nonce: Option<String>,
    pub expires_at: i64,
    pub created_at: i64,
}

impl AuthorizationCode {
    /// Generate a new authorization code.
    #[must_use]
    pub fn new(
        user_id: String,
        client_id: String,
        redirect_uri: String,
        code_challenge: Option<String>,
        scope: String,
        nonce: Option<String>,
        ttl: Duration,
    ) -> Self;

    /// Check if the code is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool;

    /// Verify PKCE code challenge against verifier.
    #[must_use]
    pub fn verify_pkce(&self, code_verifier: &str) -> bool;
}

/// Device code for device authorization grant — maps to migration 019_create_device_codes.
pub struct DeviceCode {
    pub device_code: String,    // Long code for polling
    pub user_code: String,      // Short code for user entry (e.g., "WXYZ-1234")
    pub client_id: String,
    pub scope: String,
    pub expires_at: i64,
    pub interval: u32,          // Polling interval in seconds
    pub user_id: Option<String>, // Set when user approves
    pub created_at: i64,
}
```

### Refresh Token Model (`models/token.rs`)

```rust
/// Refresh token entity — maps to migration 018_create_refresh_tokens.
pub struct RefreshToken {
    pub id: String,
    pub user_id: String,
    pub client_id: String,
    pub token_hash: String,     // SHA256 hash (never store plaintext)
    pub expires_at: i64,
    pub rotated_at: Option<i64>, // When this token was rotated (null if current)
    pub created_at: i64,
}

impl RefreshToken {
    /// Verify the refresh token against its hash.
    #[must_use]
    pub fn verify(&self, token: &str) -> bool {
        let hash = sha256(token.as_bytes());
        constant_time_eq(&hash, self.token_hash.as_bytes())
    }

    /// Check if the token is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool;
}
```

## Implementation Details

### Password hashing
- Uses existing `argon2` crate (already in Cargo.toml)
- Argon2id parameters: memory 64MB, iterations 3, parallelism 4
- Stored as string: `$argon2id$v=19$m=65536,t=3,p=4$<salt>$<hash>`

### Constant-time comparison
- Already exists in `two_factor.rs` (`constant_time_eq`)
- Can be moved to a shared utility or kept inline

### Database operations
- Models are pure data types
- Database queries are in services (Feature 11)
- SQL queries use `foundation_db::QueryStore` with `DataValue` params

## Dependencies

- Existing: `argon2`, `sha2`, `chrono`, `serde`, `serde_json`
- Existing: `foundation_db` (DataValue, QueryStore for DB ops)

## Testing

- `User::is_locked()` with future locked_until → true
- `User::is_locked()` with past locked_until → false
- `OAuthClient::verify_secret()` with correct secret → true
- `OAuthClient::verify_secret()` with wrong secret → false
- `OAuthClient::allows_redirect()` exact match → true
- `AuthorizationCode::verify_pkce()` correct verifier → true
- `AuthorizationCode::verify_pkce()` wrong verifier → false
- `RefreshToken::verify()` correct token → true
- `RefreshToken::verify()` wrong token → false
