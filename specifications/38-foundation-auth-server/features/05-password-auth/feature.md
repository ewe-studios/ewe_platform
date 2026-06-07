---
feature: "Password Authentication Flow"
description: "Username/password login against IdP, MFA challenge support, native + wasm"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-06-05
last_updated: 2026-06-07
author: "Main Agent"
tasks:
  completed: 1
  uncompleted: 0
  total: 1
  completion_percentage: 100%
---

# Feature 05: Password Authentication Flow

## Description

Implement actual username/password login against an IdP endpoint. The `AuthCredential::UsernameAndPassword` and `AuthCredential::EmailAuth` types exist but have no login implementation. Both native and wasm variants are needed.

## Modules

- `backends/foundation_auth/src/native/password_auth.rs` — native via `SimpleHttpClient`
- `backends/foundation_auth/src/wasm_bindgen/password_auth.rs` — wasm via browser fetch (gated behind `wasm-bindgen-oauth` feature)

## API Surface

```rust
/// Password authentication client.
pub struct PasswordAuthClient {
    base_url: String,
    http_client: HttpClient,  // platform-specific
}

/// Login request body.
pub struct LoginRequest {
    pub email: Option<String>,
    pub username: Option<String>,
    pub password: ConfidentialText,
}

/// Login response from IdP.
pub enum LoginResponse {
    /// Authentication succeeded, session established.
    Authenticated {
        session_id: String,
        cookie: Option<Cookie>,
        mfa_required: bool,
    },
    /// MFA challenge required — caller must submit TOTP code.
    MfaRequired {
        challenge_id: String,
        mfa_type: MfaType,  // Totp, WebAuthn
    },
    /// Credentials were invalid.
    InvalidCredentials {
        attempts_remaining: Option<u32>,
        locked: bool,
    },
}

impl PasswordAuthClient {
    /// Create a new password auth client.
    pub fn new(base_url: String) -> Self;

    /// Attempt login with username and password.
    /// Returns Authenticated on success, MfaRequired if 2FA enabled, or error.
    pub async fn login(&self, request: LoginRequest) -> Result<LoginResponse, PasswordAuthError>;

    /// Submit MFA code for an existing challenge.
    pub async fn submit_mfa(&self, challenge_id: &str, code: &str) -> Result<LoginResponse, PasswordAuthError>;
}

pub enum MfaType {
    Totp,
    WebAuthn,
}

pub enum PasswordAuthError {
    ConnectionFailed(String),
    ServerError { status: u16, message: String },
    ParseError(String),
    AccountLocked,
}
```

## Implementation Details

### Login flow (native)
1. `POST {base_url}/auth/v1/login` with JSON body `{"email": "...", "password": "..."}`
2. Response:
   - 200 + `{"status": "authenticated", ...}` → `LoginResponse::Authenticated`
   - 200 + `{"status": "mfa_required", "challenge_id": "...", "type": "totp"}` → `LoginResponse::MfaRequired`
   - 401 + `{"attempts_remaining": 2}` → `LoginResponse::InvalidCredentials`
   - 423 + `{"locked": true}` → `PasswordAuthError::AccountLocked`
3. On authenticated response, extract session cookie if present

### Login flow (wasm)
- Same request/response pattern using browser fetch API
- No cookie extraction (wasm runs in browser — cookies are handled by the browser)
- Returns session_id and auth state only

### MFA flow
1. `POST {base_url}/auth/v1/mfa` with `{"challenge_id": "...", "code": "123456"}`
2. Response: same as login (Authenticated or InvalidCredentials)

### HTTP body
- Content-Type: `application/json`
- Body: `{"email": "user@example.com", "password": "secret"}` or `{"username": "user", "password": "secret"}`

## Dependencies

- Native: `foundation_netio::SimpleHttpClient` (existing)
- Wasm: `web-sys`, `wasm-bindgen` (existing via `wasm-bindgen-oauth` feature)
- Existing: `ConfidentialText`, `Cookie`

## Testing

- Native: mock server returns 200 authenticated → `LoginResponse::Authenticated`
- Native: mock server returns 200 MFA required → `LoginResponse::MfaRequired`
- Native: mock server returns 401 → `LoginResponse::InvalidCredentials`
- Native: mock server returns 423 locked → `PasswordAuthError::AccountLocked`
- Wasm: parse mock JSON response (unit test, no HTTP)
- Password is ConfidentialText → zeroized on drop

## Sync/Async Notes

The `async fn` methods shown are the primary implementation. For sync callers,
use valtron bridging: `from_future` + `execute` + `collect_one`. See the valtron skill.
