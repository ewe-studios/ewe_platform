---
feature: "WebAuthn/FIDO2"
description: "WebAuthn/FIDO2 passkey registration and authentication — phishing-resistant MFA"
status: "pending"
priority: "high"
depends_on: ["02-login-mfa-handlers"]
estimated_effort: "large"
created: 2026-06-16
---

# Feature 06: WebAuthn/FIDO2

## Description

Passkey support for phishing-resistant MFA. Follows rauthy's design:
non-discoverable credentials by default, User Verification required.

**Native-only.** `webauthn-rs-core` depends on `openssl` + `openssl-sys` which
does not compile to `wasm32-unknown-unknown`. The `server-native` feature flag
gates this. CF Workers deployments relay `/auth/v1/webauthn/*` and
`/auth/v1/passkey/*` to a native instance, or disable passkey support.

The rest of the server (F01–F05, F07–F09) works on both native and CF Workers
via the `server` feature (no `ring` dependency).

Two distinct flows:
1. **WebAuthn as MFA** — `/auth/v1/webauthn/*` — passkey as a second factor
2. **Passkey-only login** — `/auth/v1/passkey/login/*` — no password, passkey is the sole auth factor

## New dependency

```toml
webauthn-rs = { version = "0.5", optional = true }
```

Gated behind `server` feature — client library doesn't pull this in.

## Endpoints

### WebAuthn as MFA

| Method | Endpoint | Purpose |
|---|---|---|
| POST | `/auth/v1/webauthn/register/start` | Generate registration options |
| POST | `/auth/v1/webauthn/register/finish` | Verify attestation, store credential |
| POST | `/auth/v1/webauthn/login/start` | Generate authentication challenge (for existing session) |
| POST | `/auth/v1/webauthn/login/finish` | Verify assertion, complete login |
| DELETE | `/auth/v1/webauthn/{id}` | Remove passkey |
| PUT | `/auth/v1/webauthn/{id}` | Rename passkey |

### Passkey-only login (no password)

| Method | Endpoint | Purpose |
|---|---|---|
| POST | `/auth/v1/passkey/login/start` | Enter email, get WebAuthn get options |
| POST | `/auth/v1/passkey/login/finish` | Verify assertion, authenticate |

The passkey-only flow:
1. User enters email → `POST /auth/v1/passkey/login/start` → returns WebAuthn get options
2. Browser calls `navigator.credentials.get()` → `POST /auth/v1/passkey/login/finish`
3. Server verifies assertion → creates session → returns 202 with redirect

## New model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passkey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub credential_id: Vec<u8>,
    pub credential_public_key: Vec<u8>,
    pub counter: u32,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}
```

## New migration

```sql
-- migration_020_create_passkeys.sql
CREATE TABLE passkeys (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    credential_id BLOB NOT NULL UNIQUE,
    credential_public_key BLOB NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMP
);
```

## Service

```rust
pub struct WebAuthnService {
    config: Arc<IdpConfig>,
    db: StorageProvider,
    webauthn: WebAuthn,  // from webauthn-rs
}
```

## Handler integration

WebAuthn login finish → creates session cookie (same as password login).
Used by LoginPage as an alternative to password entry.

Passkey-only login finish → also creates session cookie. Used when user
chooses passkey-only authentication (no password flow).

## Module changes

- `backends/foundation_auth/src/server/models/mod.rs` — add passkey module (always compiled, `server` feature)
- `backends/foundation_auth/src/server/services/webauthn_service.rs` — NEW (`server-native` only)
- `backends/foundation_auth/src/server/handlers/core.rs` — add webauthn + passkey-login endpoints (`server-native` only)
- `backends/foundation_auth/src/server/idp_server.rs` — register routes (`server-native` only)
- `backends/foundation_auth/Cargo.toml` — add `server-native` feature with `dep:webauthn-rs`

```toml
[dependencies]
webauthn-rs = { version = "0.5", optional = true }

[features]
server-native = ["server", "dep:webauthn-rs"]
```

In `server/mod.rs`:
```rust
#[cfg(feature = "server-native")]
pub mod webauthn_service;

#[cfg(feature = "server-native")]
pub use webauthn_service::WebAuthnService;
```

The Passkey model itself is always compiled under `server` — it's needed by
account management (F09) for listing/deleting passkeys regardless of whether
WebAuthn crypto is available.

## Testing

- Registration generates valid WebAuthn options
- Authentication verifies assertion and returns user
- Counter increments (replay detection)
- Delete passkey removes from database
