---
feature: "IdP Services"
description: "Token generation (jwt-simple), user management (Argon2id), client management, session service"
status: "pending"
priority: "high"
depends_on: ["00-query-store-stream-parity", "10-idp-models"]
estimated_effort: "large"
created: 2026-06-05
last_updated: 2026-06-07
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 1
  total: 1
  completion_percentage: 0%
---

# Feature 11: IdP Services

## Description

Business logic services for the IdP server: token generation/signing, user management with Argon2id password hashing, OAuth client management, and session management. These services use the models (Feature 10) and foundation_db QueryStore for persistence.

## Sync/Async Design — Dual Traits

Each service has an **async trait** (primary implementation) and a **sync trait** (valtron wrapper).

### Pattern

```rust
// === ASYNC TRAIT (PRIMARY) ===
#[async_trait::async_trait]
pub trait AsyncTokenService: Send + Sync {
    async fn generate_tokens_async(...) -> Result<TokenPair, TokenServiceError>;
    async fn generate_client_credentials_tokens_async(...) -> Result<TokenPair, TokenServiceError>;
    async fn refresh_tokens_async(...) -> Result<TokenPair, TokenServiceError>;
    async fn store_refresh_token_async(...) -> Result<(), TokenServiceError>;
    async fn rotate_refresh_token_async(&self, token_id: &str) -> Result<(), TokenServiceError>;
}

// === SYNC TRAIT (WRAPPER — CALLS ASYNC VIA VALTRON) ===
pub trait TokenService: Send + Sync {
    fn generate_tokens(...) -> Result<TokenPair, TokenServiceError>;
    fn generate_client_credentials_tokens(...) -> Result<TokenPair, TokenServiceError>;
    fn refresh_tokens(...) -> Result<TokenPair, TokenServiceError>;
    fn store_refresh_token(...) -> Result<(), TokenServiceError>;
    fn rotate_refresh_token(&self, token_id: &str) -> Result<(), TokenServiceError>;
}

// === SYNC BRIDGE ===
pub struct SyncTokenServiceBridge<S: AsyncTokenService> {
    inner: Arc<S>,
}

impl<S: AsyncTokenService + 'static> TokenService for SyncTokenServiceBridge<S> {
    fn generate_tokens(...) -> Result<TokenPair, TokenServiceError> {
        // valtron from_future + execute + collect_one
        // See requirements.md "Valtron Bridging" section for the pattern
    }
}
```

This same pattern applies to: **UserService**, **ClientService**, **SessionService**.

### Caveat: Argon2 Password Hashing

Argon2 password hashing is CPU-intensive. The hash/verify functions from the `argon2` crate
are sync. For async contexts, the service method should NOT block — the CPU work happens
naturally within the async flow. The valtron thread pool handles execution scheduling.
No tokio involvement.

## Modules

`backends/foundation_auth/src/server/services/` — directory containing service files

## API Surface

### Token Service (`services/token_service.rs`)

```rust
/// Token generation and signing service.
pub struct TokenService {
    config: Arc<IdpConfig>,
    db: StorageProvider,
}

/// Token pair returned from token endpoint.
pub struct TokenPair {
    pub access_token: String,
    pub id_token: String,
    pub refresh_token: String,  // Plaintext — caller must persist hash
    pub expires_in: u64,
    pub scope: String,
}

impl TokenService {
    /// Generate tokens for an authorization code exchange.
    pub fn generate_tokens(
        &self,
        user: &User,
        client: &OAuthClient,
        scope: &str,
        nonce: Option<&str>,
    ) -> Result<TokenPair, TokenServiceError>;

    /// Generate tokens for client credentials flow (no user context).
    pub fn generate_client_credentials_tokens(
        &self,
        client: &OAuthClient,
        scope: &str,
    ) -> Result<TokenPair, TokenServiceError>;

    /// Refresh tokens using a refresh token.
    /// Returns new token pair with rotated refresh token.
    pub fn refresh_tokens(
        &self,
        old_refresh_token: &str,
    ) -> Result<TokenPair, TokenServiceError>;

    /// Store a hashed refresh token.
    pub fn store_refresh_token(
        &self,
        user_id: &str,
        client_id: &str,
        token_hash: &str,
        expires_at: i64,
    ) -> Result<(), TokenServiceError>;

    /// Mark a refresh token as rotated (for rotation detection).
    pub fn rotate_refresh_token(&self, token_id: &str) -> Result<(), TokenServiceError>;
}
```

### Token generation details

**Access Token (JWT):**
```json
{
  "iss": "https://auth.example.com",
  "sub": "user_123",
  "aud": "client_id",
  "exp": <now + 15min>,
  "iat": <now>,
  "jti": "unique-token-id",
  "scope": "openid profile email",
  "client_id": "client_id"
}
```
- Signed with `JwtSigningKey` (EdDSA/Ed25519 default)
- `kid` header set to current key ID

**ID Token (JWT):**
```json
{
  "iss": "https://auth.example.com",
  "sub": "user_123",
  "aud": "client_id",
  "exp": <now + 15min>,
  "iat": <now>,
  "nonce": "from-auth-request",
  "email": "user@example.com",
  "email_verified": true,
  "name": "User Name"
}
```
- Standard OIDC ID token claims
- `nonce` included if present in auth request

**Refresh Token:**
- Random 64-byte string (base64url encoded)
- SHA256 hash stored in database
- Never returned in plaintext except on initial issuance
- Rotated on each use (old hash marked as rotated, new hash stored)

### User Service (`services/user_service.rs`)

```rust
/// User management service.
pub struct UserService {
    db: StorageProvider,
}

impl UserService {
    /// Find a user by email.
    pub fn find_by_email(&self, email: &str) -> Result<Option<User>, UserServiceError>;

    /// Find a user by ID.
    pub fn find_by_id(&self, id: &str) -> Result<Option<User>, UserServiceError>;

    /// Create a new user with hashed password.
    pub fn create_user(
        &self,
        email: &str,
        password: &str,
        username: Option<&str>,
    ) -> Result<User, UserServiceError>;

    /// Verify a user's password.
    pub fn verify_password(&self, user: &User, password: &str) -> Result<bool, UserServiceError>;

    /// Update user password.
    pub fn update_password(
        &self,
        user_id: &str,
        new_password: &str,
    ) -> Result<(), UserServiceError>;

    /// Record a failed login attempt and lock if threshold exceeded.
    pub fn record_failed_login(&self, user_id: &str) -> Result<(), UserServiceError>;

    /// Reset failed login attempts (on successful login).
    pub fn reset_failed_logins(&self, user_id: &str) -> Result<(), UserServiceError>;
}
```

### Password hashing

```rust
fn hash_password(password: &str) -> Result<String, UserServiceError> {
    use argon2::{
        Argon2, PasswordHasher,
        password_hash::{SaltString, OsRng},
    };

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(65536, 3, 4, None)?, // 64MB, 3 iterations, 4 threads
    );

    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

fn verify_password(hash: &str, password: &str) -> Result<bool, UserServiceError> {
    use argon2::{Argon2, PasswordVerifier};
    let parsed_hash = PasswordHash::new(hash)?;
    let argon2 = Argon2::default();
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e.into()),
    }
}
```

### Client Service (`services/client_service.rs`)

```rust
/// OAuth client management service.
pub struct ClientService {
    db: StorageProvider,
}

impl ClientService {
    /// Find a client by ID.
    pub fn find_by_id(&self, client_id: &str) -> Result<Option<OAuthClient>, ClientServiceError>;

    /// Create a new OAuth client.
    pub fn create_client(
        &self,
        name: &str,
        redirect_uris: Vec<String>,
        grant_types: Vec<String>,
        scopes: Vec<String>,
        is_public: bool,
    ) -> Result<(OAuthClient, String), ClientServiceError>;  // Returns (client, plaintext_secret)

    /// Validate client credentials.
    pub fn validate_client(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<Option<OAuthClient>, ClientServiceError>;
}
```

### Session Service (`services/session_service.rs`)

```rust
/// Session management service — wraps existing SessionManager.
pub struct SessionService {
    session_mgr: Arc<SessionManager<CredentialStorage>>,
}

impl SessionService {
    /// Create a session for a user.
    pub fn create_session(
        &self,
        user_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(Session, Vec<Cookie>), SessionServiceError>;

    /// Validate a session token.
    pub fn get_session(&self, token: &str) -> Result<Option<Session>, SessionServiceError>;

    /// Revoke a session.
    pub fn revoke_session(&self, session_id: &str) -> Result<(), SessionServiceError>;

    /// Revoke all sessions for a user.
    pub fn revoke_all_sessions(&self, user_id: &str) -> Result<usize, SessionServiceError>;
}
```

## Implementation Details

### Database queries

All queries use `foundation_db::QueryStore` with `DataValue` params:

```rust
// Find user by email
let rows = store.query(
    "SELECT * FROM users WHERE email = ? AND deleted_at IS NULL",
    &[DataValue::Text(email.to_string())],
)?;

// Find client by ID
let rows = store.query(
    "SELECT * FROM oauth_clients WHERE id = ?",
    &[DataValue::Text(client_id.to_string())],
)?;

// Store auth code
store.execute(
    "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, code_challenge, scope, nonce, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    &[
        DataValue::Text(code.clone()),
        DataValue::Text(user_id.clone()),
        DataValue::Text(client_id.clone()),
        DataValue::Text(redirect_uri.clone()),
        DataValue::Text(code_challenge.unwrap_or_default()),
        DataValue::Text(scope.clone()),
        DataValue::Text(nonce.unwrap_or_default()),
        DataValue::Integer(expires_at),
    ],
)?;
```

### SQL row parsing

Helper function to parse `SqlRow` into model types:

```rust
fn parse_user(row: &SqlRow) -> Result<User, Error> {
    Ok(User {
        id: row.get_by_name("id")?,
        email: row.get_by_name("email")?,
        username: row.get_by_name("username")?,
        password_hash: row.get_by_name("password_hash")?,
        email_verified: row.get_by_name("email_verified")?,
        // ...
    })
}
```

### Error types

Each service has its own error enum, all implement `From<StorageError>`:
- `TokenServiceError` — signing failed, storage error
- `UserServiceError` — password hash error, storage error, user not found
- `ClientServiceError` — storage error, client not found
- `SessionServiceError` — wraps existing `SessionError`

## Dependencies

- Existing: `argon2`, `sha2`, `chrono`, `serde`, `serde_json`
- Existing: `foundation_db` (QueryStore, DataValue, SqlRow)
- Existing: all internal types (JwtSigningKey, SessionManager, etc.)

## Testing

- Token generation → valid JWT with correct claims
- ID token contains nonce when provided
- Refresh token rotation → old token invalidated, new token issued
- Password hash → verify same password → true
- Password hash → verify wrong password → false
- User creation → stored in DB, findable by email
- Client creation → secret returned, hash stored
- Client validation → correct secret → Some(client)
- Client validation → wrong secret → None
- Session creation → returns session + cookies
- Session validation → valid token → Some(session)
