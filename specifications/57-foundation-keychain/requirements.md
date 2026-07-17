# Spec 57: foundation_keychain

## Problem

Users need a central, self-hosted keychain service that:

1. **Stores encrypted secrets** with zero-knowledge encryption
2. **Serves Bitwarden-compatible clients** (web, extension, desktop, mobile, CLI) without patches
3. **Runs anywhere**: Cloudflare Workers (WASM), Docker, VPS — same code, different backends
4. **Uses our foundations**: `foundation_db` (storage), `foundation_auth` (JWT/TOTP/middleware/IdpServer), `foundation_http` (native HTTP), `foundation_netio` (WebSocket)

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/orangevault` — 45 Rust files, 14 API modules, 16 SQL tables, 13 Vitest integration tests, ~150 route handlers.

## Why OrangeVault Isn't Enough

OrangeVault works but is tightly coupled to Cloudflare Workers:

- **Direct `worker::` bindings**: `D1Database`, `KvStore`, `Bucket`, `Request`, `Response` — not portable
- **Web Crypto only**: `web_sys::SubtleCrypto` — doesn't compile outside WASM
- **workers-rs Router**: Not our `foundation_http`
- **No trait abstraction**: Everything is runtime-specific
- **Reinvents auth**: JWT, TOTP, password verification — all in `foundation_auth` already

## Goal

Extract and generalize OrangeVault into `foundation_keychain`:

1. **Uses `foundation_db`'s existing traits** — `QueryStore`, `AsyncQueryStore`, `KeyValueStore`, `AsyncKeyValueStore`, `BlobStore`, `AsyncBlobStore`, `RateLimiterStore`, `StorageProvider` — **no new storage traits**
2. **Uses `foundation_auth`** — JWT (signing/verification), TOTP, auth middleware, credential stores, `IdpServer` — **no reinvented auth**
3. **Implements Cloudflare backends** via `foundation_db::StorageBackend::{D1Wasm, R2Wasm, KVWasm}` + Web Crypto
4. **Implements native backends** via `foundation_db::StorageBackend::{Turso, Libsql}` + filesystem + `foundation_http`
5. **Compiles to WASM** (`wasm32-unknown-unknown`) and **native** (`x86_64`/`aarch64`)
6. **Maintains Bitwarden API compatibility**

## What `foundation_db` Already Provides (No New Traits Needed)

| foundation_db Trait | Purpose | OrangeVault Equivalent |
|---|---|---|
| `QueryStore` / `AsyncQueryStore` | SQL: `query()`, `execute()`, `execute_batch()` | Direct D1/SQLite queries |
| `KeyValueStore` / `AsyncKeyValueStore` | KV: `get`, `set`, `delete`, `exists`, `list_keys` | Cloudflare KV |
| `BlobStore` / `AsyncBlobStore` | Blob: `put_blob`, `get_blob`, `delete_blob` | R2 / filesystem |
| `RateLimiterStore` / `AsyncRateLimiterStore` | Rate limiting: `check`, `record`, `reset` | None (needed for login throttle) |
| `StorageProvider` | Unified enum over all backends | N/A (new) |
| `DataValue` | SQL param type (Null/Integer/Real/Text/Blob) | `worker::d1::D1Type` |
| `SqlRow` | Row with `get<T>`, `get_by_name<T>` | `serde::Deserialize` from D1 |
| `StorageItemStream<T>` / `AsyncStorageItemStream<T>` | Lazy streaming results | N/A (new) |
| `StorageBackend` enum | `Turso`, `Libsql`, `D1Wasm`, `R2Wasm`, `KVWasm`, `JsonFile`, `Memory` | N/A (new) |
| `SchemaMigration` | Migration runner | Wrangler D1 migrations |
| `AuthStore` / `AsyncAuthStore` | `find_user_by_email` | `queries::find_user_by_email` |
| `PasskeyStore` / `AsyncPasskeyStore` | Passkey CRUD | N/A (not used by vault) |
| `TosStore` / `AsyncTosStore` | ToS acceptance | N/A (not used by vault) |

**Zero custom storage traits.** `foundation_keychain` uses `foundation_db`'s traits directly.

## What `foundation_auth` Already Provides

| foundation_auth Surface | Purpose | OrangeVault Equivalent |
|---|---|---|
| `shared::jwt::JwtVerifier` (RS256/ES256/EdDSA) | JWT verification | `auth/jwt.rs` `verify_and_decode_jwt` |
| `shared::jwt::JwtSigningKey` | JWT signing | `auth/jwt.rs` `sign_jwt` + `crypto/rsa.rs` |
| `shared::jwt::JwtManager` | Token lifecycle (refresh) | `auth/jwt.rs` `create_access_token` / `create_refresh_token` |
| `shared::jwt::VerifiedClaims` | Standard claims (sub, iss, aud, exp, custom) | `auth/claims.rs` `LoginClaims`, `RefreshClaims` |
| `shared::two_factor::TOTPSecret` | TOTP generation/verification (RFC 6238) | `crypto/totp.rs` `generate_totp`, `validate_totp` |
| `shared::two_factor::BackupCodeSet` | Recovery codes | `crypto/totp.rs` `generate_recovery_code` |
| `shared::two_factor::TwoFactorChallenge` | 2FA attempt tracking | Inline in `identity.rs` |
| `shared::middleware::require_auth` | Auth guard | `auth/guards.rs` `auth_from_request` |
| `shared::middleware::extract_bearer_token` | Bearer extraction | `auth/guards.rs` `extract_bearer_token` |
| `shared::middleware::AuthContext` | Auth context (token, path, IP, sub) | `auth/guards.rs` `AuthenticatedUser` |
| `shared::credential_store::AsyncCredentialStore` | Credential lookup | `db/queries.rs` `find_user_by_email` |
| `shared::credential_store::D1CredentialStore` | D1 credential store (wasm) | None (already exists) |
| `shared::pbkdf2` | PBKDF2-HMAC-SHA256 derive/verify | `crypto/pbkdf2.rs` + `auth/guards.rs` verify_master_password |
| `server::services::UserService` | User CRUD, password hashing | `api/identity.rs` register |
| `server::services::TokenService` | OAuth token issuance | `api/identity.rs` `connect_token` |
| `server::services::SessionService` | Session management | Device tracking in `identity.rs` |
| `server::IdpServer` | Full OIDC server | Partial (only password + refresh grants) |
| `server::storage::HandlerStorage` | Storage over `QueryStore` | N/A — keychain uses `foundation_db` directly |
| `native::password_auth::PasswordAuthClient` | Native password auth | Form parsing in `identity.rs` |
| `wasm_bindgen::D1CredentialStore` | WASM D1 credential store | None (already exists) |

## What `foundation_keychain` Adds (Bitwarden Domain Logic Only)

| Surface | Description |
|---------|-------------|
| **Bitwarden HTTP API** | ~150 route handlers: accounts, identity, ciphers, folders, orgs, sends, 2FA, events, icons |
| **Bitwarden JWT claims** | `LoginClaims` with `sstamp`, `orgowner`, `orgadmin`, etc. — mapped onto `VerifiedClaims::custom` |
| **Security stamp middleware** | Checks `sstamp` against DB `security_stamp` after `foundation_auth` JWT verify |
| **Bitwarden token response** | Adapter: `foundation_auth::TokenPair` → Bitwarden `LoginResponse` (with `Key`, `PrivateKey`, `Kdf`, `UserDecryptionOptions`, `AccountKeys`) |
| **Cipher/folder/org/send CRUD** | Domain logic with permission checks (personal vs org, collection-based, read-only) |
| **Organization membership lifecycle** | Invite → accept → confirm state machine |
| **SignalR MessagePack notifications** | Portable framing + backend-specific transport (DO for Cloudflare, WebSocket for native) |
| **Equivalent domains** | Global + custom domain groups for autofill |
| **Send anonymous access** | Password-gated, expiration, access count limits |
| **Icon proxy with SSRF guard** | `validate_icon_domain()` |
| **Cron purge jobs** | Expired sends, trashed ciphers |
| **Bitwarden error shapes** | `AppError` with OAuth + `ErrorModel` variants |

## Crate Structure

```
foundation_keychain/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Feature-gated re-exports
│   ├── core/                     # Bitwarden domain logic (portable, no runtime deps)
│   │   ├── mod.rs
│   │   ├── error.rs              # AppError (portable from orangevault)
│   │   ├── util.rs               # Helpers (portable from orangevault)
│   │   ├── models/               # API request/response types (portable serde)
│   │   │   ├── cipher.rs
│   │   │   ├── folder.rs
│   │   │   ├── user.rs
│   │   │   ├── organization.rs
│   │   │   ├── send.rs
│   │   │   └── sync.rs
│   │   ├── notifications/        # SignalR MessagePack framing (portable)
│   │   │   ├── mod.rs
│   │   │   └── types.rs          # UpdateType enum, serialize_msgpack, create_notification
│   │   ├── auth/                 # Bitwarden-specific adapters over foundation_auth
│   │   │   ├── mod.rs
│   │   │   ├── bitwarden_claims.rs   # LoginClaims → VerifiedClaims mapping
│   │   │   ├── stamp_middleware.rs   # Security stamp check after JWT verify
│   │   │   └── token_response.rs     # TokenPair → LoginResponse adapter
│   │   └── api/                  # Route handlers (use foundation_db traits + foundation_auth)
│   │       ├── mod.rs
│   │       ├── accounts.rs
│   │       ├── ciphers.rs
│   │       ├── emergency.rs
│   │       ├── events.rs
│   │       ├── folders.rs
│   │       ├── icons.rs
│   │       ├── identity.rs
│   │       ├── notifications.rs
│   │       ├── organizations.rs
│   │       ├── sends.rs
│   │       ├── sync.rs
│   │       ├── two_factor.rs
│   │       └── web.rs
│   ├── server/                   # Target-gated: wasm → Workers, native → foundation_http
│   │   ├── cloudflare.rs         # #[cfg(target_family = "wasm")] #[event(fetch)] + workers-rs Router
│   │   └── native.rs             # #[cfg(not(target_family = "wasm"))] foundation_http + valtron
│   └── notifications/            # Target-gated transport
│       ├── cloudflare.rs         # #[cfg(target_family = "wasm")] Durable Object
│       └── native.rs             # #[cfg(not(target_family = "wasm"))] foundation_netio WebSocket
├── tests/
│   └── integration/
│       ├── auth.test.rs
│       ├── vault.test.rs
│       ├── sends.test.rs
│       └── organizations.test.rs
└── migrations/
    └── 0001_initial.sql          # Same as orangevault (16 tables, standard SQLite)
```

## Cargo.toml

No `backend-*` features. Target gates handle everything:

```toml
[package]
name = "foundation_keychain"
version = "0.1.0"
edition = "2024"

[dependencies]
# Core (always)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_repr = "0.1"
serde_urlencoded = "0.7"
base64 = "0.22"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", default-features = false, features = ["clock"] }
rmpv = "1"
tracing = "0.1"
async-trait = "0.1"
futures-lite = "2"

# foundation_db for ALL storage (QueryStore, KeyValueStore, BlobStore, RateLimiterStore)
foundation_db = { path = "../foundation_db" }

# foundation_auth for JWT, TOTP, PBKDF2, middleware, credential stores
foundation_auth = { path = "../foundation_auth" }

# WASM deps (Cloudflare Workers)
[target.'cfg(target_family = "wasm")'.dependencies]
foundation_deployment_cloudflare = { path = "../foundation_deployment_cloudflare", features = ["workers"] }
foundation_db = { path = "../foundation_db", features = ["wasm-bindgen-storage"] }
foundation_auth = { path = "../foundation_auth", features = ["wasm-bindgen-session", "wasm-pbkdf2"] }

# Native deps (Docker/VPS)
[target.'cfg(not(target_family = "wasm"))'.dependencies]
foundation_http = { path = "../foundation_http" }
foundation_netio = { path = "../foundation_netio" }
foundation_cronjobs = { path = "../foundation_cronjobs" }
foundation_auth = { path = "../foundation_auth", features = ["server"] }
```

Build for the target, get the right backend. No feature flags needed:
- `cargo build --target wasm32-unknown-unknown` → Workers
- `cargo build` → native

## Backend Selection

| Backend | `foundation_db` Storage | Blob | KV | Crypto | HTTP | Notifier |
|---------|------------------------|------|-----|--------|------|----------|
| **Cloudflare Workers** | `StorageBackend::D1Wasm` | `StorageBackend::R2Wasm` | `StorageBackend::KVWasm` | Web Crypto + `foundation_auth::JwtSigningKey` | `worker::Router` | Durable Objects |
| **Docker/VPS (native)** | `StorageBackend::Turso` or `Libsql` | `StorageBackend::JsonFile` or filesystem | `StorageBackend::Turso` (KV table) | `foundation_auth::JwtSigningKey` | `foundation_http` | `foundation_netio` WebSocket |

No custom traits for any of these. `StorageProvider::new(StorageBackend::...)` gives you `QueryStore` + `KeyValueStore` + `BlobStore` + `RateLimiterStore` in one object.

## Schema

Same as `orangevault/migrations/0001_initial.sql` — 16 tables, standard SQLite. Uses `foundation_db::SchemaMigration` for portable migration running.

## Deployment Targets

| Target | `foundation_db` Backend | `foundation_auth` Features |
|--------|------------------------|---------------------------|
| Cloudflare Workers | `D1Wasm` + `R2Wasm` + `KVWasm` | `wasm-bindgen-session`, `D1CredentialStore` |
| Docker (Linux) | `Turso` or `Libsql` | `server` (IdpServer, UserService, TokenService) |
| VPS (bare-metal) | `Turso` or `Libsql` | `server` |
| WASM (browser, future) | `D1Wasm` | `wasm-bindgen-session` |

## What Goes Where

### foundation_db (no changes needed)
Already has everything: `QueryStore`, `AsyncQueryStore`, `KeyValueStore`, `AsyncKeyValueStore`, `BlobStore`, `AsyncBlobStore`, `RateLimiterStore`, `StorageProvider`, `DataValue`, `SqlRow`, `StorageItemStream`, `AsyncStorageItemStream`, `StorageBackend`, `SchemaMigration`, `AuthStore`, `AsyncAuthStore`, `PasskeyStore`, `TosStore`.

### foundation_auth (add if needed)
| Surface | Status | Action |
|---------|--------|--------|
| JWT signing/verification | ✅ Exists | Use as-is |
| TOTP | ✅ Exists | Use as-is |
| Auth middleware | ✅ Exists | Use as-is |
| CredentialStore / D1CredentialStore | ✅ Exists | Use as-is |
| IdpServer / TokenService / UserService | ✅ Exists | Use as-is |
| **Bitwarden-specific claims** | ❌ Missing | Add to `foundation_auth`? Or keep in keychain as adapter? → **Keep in keychain** (Bitwarden-specific, not general auth) |
| **Security stamp check** | ❌ Missing | Add to `foundation_auth`? → **Keep in keychain** (Bitwarden-specific invalidation model) |
| **Bitwarden LoginResponse** | ❌ Missing | Add to `foundation_auth`? → **Keep in keychain** (Bitwarden-specific response shape) |
| Password verification (hash-of-hash) | ⚠️ Partial | `foundation_auth` does standard password hashing; Bitwarden's model is "client derives hash, server re-hashes with 1 iteration" → **Add helper to foundation_auth** since it's a password hashing variant, not Bitwarden-specific |

### foundation_keychain (new crate)
Everything Bitwarden-specific: API handlers, domain models, SignalR protocol, icon proxy, cron jobs. All backed by `foundation_db` traits and `foundation_auth` surfaces.

## Key Risks

1. **foundation_db row deserialization**: OrangeVault uses `serde::Deserialize` on D1 rows. `foundation_db` uses `SqlRow` + `get<T>`/`get_by_name<T>`. Keychain query functions must adapt (like `foundation_auth::server::storage.rs` already does with `parse_user_row`).

2. **foundation_auth JWT claims mapping**: Bitwarden `LoginClaims` has `sstamp`, `orgowner`, `orgadmin`, `orguser`, `orgmanager`. `VerifiedClaims` puts non-standard claims in `custom: Map<String, Value>`. The adapter must serialize/deserialize correctly.

3. **WASM binary size**: Must stay under ~5MB compressed. OrangeVault achieves `opt-level = "s"` + `lto = true`.

4. **SignalR MessagePack (native)**: Durable Object → standard WebSocket server. `foundation_netio` has WebSocket support (F51-complete).

5. **Web Crypto ↔ native crypto compatibility**: PBKDF2/RSA output must match bit-for-bit for cross-backend token/portability.
