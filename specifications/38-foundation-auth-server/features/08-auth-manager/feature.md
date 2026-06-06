# Feature 08: Auth Manager

## Description

Central lifecycle manager that coordinates `JwtManager`, `SessionManager`, `AuthStateMachine`, and `CredentialStore` into a single coherent interface. Handles authentication, token refresh, persistence, and logout in one place.

CredentialStorage is built on `foundation_db` capabilities (for DB-backed stores).
`foundation_nativeapis` is ONLY relevant if file-based credential caching is needed
(e.g., persisting tokens to disk for CLI tools). The primary path is DB-backed.

## Sync/Async Design

AuthManager is a coordinator. Its method design reflects the nature of each operation:

### Sync methods (no I/O, pure state/logic):
- `new()` — constructor
- `init_from_store()` — loads from credential store (may use valtron-bridged sync store)
- `get_valid_token()` — checks state machine, returns token from memory
- `is_authenticated()` — state check
- `state()` — state accessor
- `reset()` — state reset

### Async methods (involve I/O):
- `authenticate()` — calls async login/password-auth services
- `refresh()` — calls async token refresh endpoint
- `logout()` — calls async session revocation

### CredentialStorage

Built on foundation_db capabilities:
- `KeyValueStore` (sync) + `AsyncKeyValueStore` (async) for credential persistence
- For callers who want a fully-async interface, an `AsyncAuthManager` variant wraps
  all operations in async, using the stream-to-future bridge where valtron streams
  are involved.

## Module

`backends/foundation_auth/src/shared/auth_manager.rs` — shared, cross-platform

## API Surface

```rust
/// Central authentication lifecycle manager.
pub struct AuthManager {
    store: CredentialStorage,
    session_mgr: SessionManager<CredentialStorage>,
    jwt_manager: JwtManager,
    state_machine: AuthStateMachine,
    jwt_verifier: Option<JwtVerifier>,
    jwks_manager: Option<JwksManager>,
    config: AuthManagerConfig,
}

pub struct AuthManagerConfig {
    pub signing_key: Vec<u8>,          // For session token signing (min 32 bytes)
    pub session_config: SessionConfig, // Session TTL, cookie names, etc.
    pub token_storage_key: String,     // Key for persisting JWT tokens
    pub session_storage_key: String,   // Key prefix for session storage
}

impl AuthManager {
    /// Create a new auth manager.
    pub fn new(store: CredentialStorage, config: AuthManagerConfig) -> Result<Self, AuthManagerError>;

    /// On application startup — load persisted credential, validate, init state.
    /// Returns the loaded credential if available and valid.
    pub fn init_from_store(&mut self) -> Result<Option<Authenticated>, AuthManagerError>;

    /// Authenticate — login, persist credential, transition to Authenticated.
    /// The credential parameter determines the auth method (OAuth, password, etc.).
    pub async fn authenticate(
        &mut self,
        credential: AuthCredential,
    ) -> Result<Authenticated, AuthManagerError>;

    /// Get valid token — check state machine, queue if refreshing, return token.
    /// If token is expired/near-expiry, automatically refreshes.
    pub fn get_valid_token(&mut self) -> Result<AuthToken, AuthManagerError>;

    /// Refresh — refresh JWT, persist new refresh token, handle rotation.
    pub async fn refresh(&mut self) -> Result<(), AuthManagerError>;

    /// Logout — revoke all sessions, clear tokens, reset state machine.
    pub async fn logout(&mut self) -> Result<(), AuthManagerError>;

    /// Check if currently authenticated.
    #[must_use]
    pub fn is_authenticated(&self) -> bool;

    /// Get the current auth state.
    #[must_use]
    pub fn state(&self) -> AuthState;

    /// Reset the auth state (for retry after failure).
    pub fn reset(&mut self);
}
```

## Implementation Details

### `init_from_store` flow
1. Load JWT token from `token_storage_key` in credential store
2. If found: parse token, set in `JwtManager`
3. Verify token is not expired (or use `JwtVerifier` if configured)
4. If valid: set state machine to `Authenticated`
5. If expired: attempt refresh with stored refresh token
6. If refresh fails: set state to `Unauthenticated`, clear stored tokens

### `authenticate` flow
1. Transition state machine to `Authenticating`
2. Perform the actual authentication (depends on `AuthCredential` type):
   - `OAuth` credential: already has token, just persist
   - `UsernameAndPassword`: call `PasswordAuthClient` (caller provides endpoint)
   - `ClientSecret`: use client credentials flow
3. On success:
   - Store credential in credential store
   - Create session via `SessionManager` (if applicable)
   - Set token in `JwtManager`
   - Transition state to `Authenticated`
4. On failure:
   - Transition state to `Failed`
   - Return error

### `get_valid_token` flow
1. Check state machine: if `Refreshing`, enqueue request and return `Pending`
2. If `TokenExpired`: transition to `Refreshing`, call `refresh()`
3. If `Authenticated`: check if token expires within buffer
4. If near-expiry: call `refresh()`
5. Return current access token

### `refresh` flow
1. Get refresh token from `JwtManager`
2. Call refresh endpoint (caller provides the refresh function)
3. On success:
   - Update `JwtManager` with new token
   - Persist new refresh token to store
   - Transition state to `Authenticated`
4. On failure:
   - Clear tokens from memory and store
   - Transition state to `Failed`

### `logout` flow
1. Revoke all sessions via `SessionManager.revoke_all_sessions()`
2. Clear JWT from `JwtManager`
3. Clear stored tokens from credential store
4. Reset state machine to `Unauthenticated`

### Error type
```rust
pub enum AuthManagerError {
    StoreError(CredentialStoreError),
    SessionError(SessionError),
    JwtError(JwtError),
    AuthStateError(AuthStateError),
    AuthenticationFailed(AuthenticationErrors),
    NoRefreshToken,
    TokenExpired,
}
```

## Dependencies

- Existing: all components within foundation_auth
- No new external dependencies

## Testing

- `init_from_store` with valid persisted token → `Authenticated`
- `init_from_store` with expired token, valid refresh → `Authenticated` with new token
- `init_from_store` with expired token, invalid refresh → `Unauthenticated`
- `authenticate` with OAuth credential → token persisted, state `Authenticated`
- `authenticate` with wrong credentials → state `Failed`
- `get_valid_token` with valid token → returns token
- `get_valid_token` with expired token → auto-refreshes, returns new token
- `logout` → sessions revoked, tokens cleared, state `Unauthenticated`
- Concurrent `get_valid_token` during refresh → queued, served after refresh completes
