---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
this_file: "specifications/07-foundation-ai/features/00a-auth-infrastructure/start.md"
created: 2026-03-20
author: "Main Agent"
---

# Start: Authentication Infrastructure Extension

## Feature Overview

This feature extends `foundation_auth` with comprehensive authentication infrastructure including JWT management, OAuth 2.0 flows, secure credential storage, authentication state machine, and two-factor authentication support.

## Required Reading (Before Implementation)

1. **Read `.agents/skills/rust-valtron-usage/skill.md`** — Valtron execution model, stream-returning patterns, sync boundary helpers. MANDATORY before writing any I/O code. Most auth operations are CPU-bound (sync), but DB storage and HTTP OAuth calls use Valtron streams.
2. **Read `feature.md`** — Full requirements, Iron Laws, task list.
3. **Read `../../LEARNINGS.md`** — Spec-level learnings including `from_future` patterns.

## Prerequisites

Before starting, ensure you understand:
1. Existing `foundation_auth` types (`AuthCredential`, `AuthenticationErrors`, `ConfidentialText`)
2. OAuth 2.0 authorization code flow and PKCE
3. JWT token structure and refresh flows
4. TOTP algorithm for two-factor authentication
5. Secure memory handling with `Zeroizing`

## Agent Workflow

### Phase 1: Setup and Planning

1. **Read existing foundation_auth**
   - Read: `backends/foundation_auth/src/lib.rs`
   - Read: `backends/foundation_auth/Cargo.toml`
   - Understand existing types and their usage
   - Note: `ConfidentialText`, `OAuthCredential`, `JwtCredential`, `SessionCredential`

2. **Plan dependency additions**
   - Review required crates in feature.md
   - Update `Cargo.toml` with: `sha2`, `hmac`, `time`, `serde`, `serde_json`, `url`

### Phase 2: JWT Management

3. **Create JWT module**
   - Create: `backends/foundation_auth/src/jwt.rs`
   - Implement: `JWTToken` struct with fields:
     - `access_token: ConfidentialText`
     - `refresh_token: Option<ConfidentialText>`
     - `expires_at: f64`
     - `scope: Option<String>`
   - Implement: `JWTToken::from_parts()`, `is_expired()`, `expires_in()`
   - Implement: `serde::Serialize` and `serde::Deserialize`

4. **Create JWTManager**
   - Implement: `JWTManager` struct with internal `Option<JWTToken>`
   - Implement: `set_token()`, `get_token()`, `clear_token()`
   - Implement: `get_valid_token(refresh_buffer_secs)` with auto-refresh
   - Implement: `refresh_if_needed()` helper
   - Add: JWT payload parsing for `exp` claim extraction
   - Test: Expiration detection and refresh triggering

### Phase 3: OAuth 2.0 Flows

5. **Create OAuth module**
   - Create: `backends/foundation_auth/src/oauth.rs`
   - Implement: `OAuthConfig` struct with:
     - `client_id: ConfidentialText`
     - `client_secret: Option<ConfidentialText>`
     - `redirect_uri: String`
     - `scopes: Vec<String>`
     - `authorization_url: String`
     - `token_url: String`
     - `use_pkce: bool`
   - Implement: Builder pattern with defaults

6. **Implement PKCE**
   - Create: `PKCEChallenge` struct
   - Implement: `PKCEChallenge::generate()` using `sha2::Sha256`
   - Generate: `code_verifier` (random 32-96 bytes, base64url)
   - Generate: `code_challenge` = SHA256(code_verifier), base64url
   - Store: `challenge_method: "S256"`

7. **Implement OAuthManager**
   - Implement: `OAuthManager` struct
   - Implement: `generate_state()` - random URL-safe string
   - Implement: `validate_state(generated, returned)` - constant-time comparison
   - Implement: `get_authorization_url(config, state, pkce)` - build full OAuth URL
   - Implement: `exchange_code(config, code, state, code_verifier)` - POST to token endpoint
   - Implement: `client_credentials(config)` - service-to-service auth
   - Implement: `refresh_token(config, refresh_token)` - token refresh
   - Test: URL generation, state validation, code exchange

### Phase 4: Credential Storage

8. **Create CredentialStore**
   - Create: `backends/foundation_auth/src/credential_store.rs`
   - Define: `CredentialStore` trait with methods:
     - `get<K>(&self, key: K) -> Option<ConfidentialText>`
     - `set<K>(&mut self, key: K, value: ConfidentialText)`
     - `delete<K>(&mut self, key: K)`
     - `exists<K>(&self, key: K) -> bool`
   - Implement: `InMemoryCredentialStore` using `HashMap`
   - Wrap: All values in `Zeroizing<String>`
   - Implement: `Drop` trait to zeroize on drop
   - Test: Storage, retrieval, secure deletion

### Phase 5: Auth State Machine

9. **Create AuthState**
   - Create: `backends/foundation_auth/src/auth_state.rs`
   - Define: `AuthState` enum:
     - `Unauthenticated`
     - `Authenticating`
     - `Authenticated`
     - `TokenExpired`
     - `Refreshing`
     - `Failed(String)`
   - Implement: `can_make_request()`, `is_terminal()`

10. **Implement AuthStateMachine**
    - Implement: `AuthStateMachine` struct
    - Implement: `transition_to(new_state)` with validation
    - Implement: `handle_event(event)` for state transitions
    - Implement: Request queue for concurrent refresh handling
    - Implement: `enqueue_request()`, `process_queue()`
    - Test: State transitions, concurrent request handling

### Phase 6: Two-Factor Authentication

11. **Create TwoFactor module**
    - Create: `backends/foundation_auth/src/two_factor.rs`
    - Implement: `TOTPSecret` struct
    - Implement: `TOTPSecret::generate()` - random 20-byte secret
    - Implement: `TOTPSecret::now()` - current 6-digit code
    - Implement: `TOTPSecret::verify(code, window)` - verify with time tolerance
    - Implement: HMAC-SHA1 per RFC 6238
    - Implement: `BackupCode` generation and validation
    - Test: TOTP generation and verification

### Phase 7: Type Extensions

12. **Create AuthToken**
    - Create: `backends/foundation_auth/src/auth_token.rs`
    - Define: `AuthToken` enum:
      - `JWT(JWTToken)`
      - `OAuth(OAuthToken)`
      - `Session(SessionToken)`
      - `Bearer(ConfidentialText)`
    - Implement: Unified interface for token operations

13. **Extend existing types**
    - Update: `lib.rs` with new error variants:
      - `TokenExpired`
      - `RefreshFailed`
      - `OAuthError { error, description }`
      - `InvalidState`
      - `PKCEFailed`
    - Update: `OnAuthData` with `OAuthAuthorizationRequired { url, state }`
    - Update: `AuthCredential` if new variants needed

### Phase 8: Integration

14. **Update lib.rs**
    - Declare all new modules: `jwt`, `oauth`, `credential_store`, `auth_state`, `two_factor`, `auth_token`
    - Re-export all public types
    - Ensure clean compilation

15. **Update Cargo.toml**
    - Add: `sha2 = "0.10"`
    - Add: `hmac = "0.12"`
    - Add: `time = "0.3"`
    - Add: `serde = { version = "1.0", features = ["derive"] }`
    - Add: `serde_json = "1.0"`
    - Add: `url = "2.5"`

16. **Run verification**
    ```bash
    cargo check --package foundation_auth
    cargo clippy --package foundation_auth -- -D warnings
    cargo test --package foundation_auth
    cargo fmt --package foundation_auth -- --check
    ```

### Phase 9: Documentation

17. **Update LEARNINGS.md**
    - Document JWT refresh design decisions
    - Document OAuth flow implementation details
    - Document PKCE implementation approach
    - Document TOTP algorithm choices
    - Add security considerations

## File Checklist

- [ ] `src/jwt.rs` - JWT token and manager
- [ ] `src/oauth.rs` - OAuth flows and PKCE
- [ ] `src/credential_store.rs` - Credential storage trait and impl
- [ ] `src/auth_state.rs` - Auth state machine
- [ ] `src/two_factor.rs` - TOTP and 2FA
- [ ] `src/auth_token.rs` - Unified token type
- [ ] `src/lib.rs` - Updated exports
- [ ] `Cargo.toml` - Updated dependencies

## Key Implementation Details

### PKCE Generation
```rust
use sha2::{Sha256, Digest};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

let code_verifier = generate_random_bytes(32);
let code_challenge = Sha256::digest(&code_verifier);

// Both encoded as base64url without padding
let verifier_b64 = URL_SAFE_NO_PAD.encode(code_verifier);
let challenge_b64 = URL_SAFE_NO_PAD.encode(code_challenge);
```

### TOTP Algorithm
```rust
use hmac::{Hmac, Mac};
use sha1::Sha1;
use time::OffsetDateTime;

type HmacSha1 = Hmac<Sha1>;

// Counter = (current_unix_time - epoch) / 30
let counter = (now.unix_timestamp() - 0) / 30;
let counter_bytes = counter.to_be_bytes();

// HMAC-SHA1(secret, counter)
let mut mac = HmacSha1::new_from_slice(secret.as_ref()).unwrap();
mac.update(&counter_bytes);
let result = mac.finalize();

// Dynamic truncation to 6 digits
let offset = (result.into_bytes()[19] & 0x0f) as usize;
let code = ((result[offset] & 0x7f) as u32) << 24
    | result[offset + 1] as u32 << 16
    | result[offset + 2] as u32 << 8
    | result[offset + 3] as u32;
let code_6digit = code % 1_000_000;
```

### State Parameter
```rust
use rand::RngCore;

fn generate_state() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
```

## Success Criteria

- [ ] All modules compile without errors
- [ ] All types exported from `lib.rs`
- [ ] All unit tests pass
- [ ] `cargo clippy -- -D warnings` passes
- [ ] All secrets use `Zeroizing`
- [ ] Debug impls redact sensitive data
- [ ] LEARNINGS.md updated

## Dependencies

Ensure `Cargo.toml` includes:

```toml
[dependencies]
foundation_core = { workspace = true }

derive_more = { version = "2.0", features = ["from", "debug", "error"] }
base64 = "0.22"
sha1 = "0.10"
sha2 = "0.10"
bytes = "1.5"
zeroize = { version = "1" }
hmac = "0.12"
time = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
url = "2.5"
```

---

_Created: 2026-03-20_
