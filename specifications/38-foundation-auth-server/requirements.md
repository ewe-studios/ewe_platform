---
description: "foundation_auth Rework v3 — OIDC client library hardening + IdP server module. Closes critical client-side security gaps (JWT signature verification, JWKS, OIDC discovery, UserInfo, password auth, token introspection, nonce). Adds IdP server module behind `server` feature flag using foundation_http, with full OIDC authorization code + PKCE, refresh token, client credentials, and device code grant support. Uses foundation_db QueryStore for SQL-backed persistence with migrations 016-019 for OIDC server tables."
status: "in-progress"
priority: "high"
created: 2026-06-05
updated: 2026-06-05
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - authentication
    - oauth2
    - oidc
    - jwt
    - server
    - identity-provider
    - wasm
    - rust
has_features: true
has_fundamentals: true
builds_on:
  - "specifications/21-http-framework"
related_specs:
  - "specifications/21-http-framework"
  - "specifications/37-overlay-vfs"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# foundation_auth Rework v3 — Client Hardening + IdP Server

## Overview

foundation_auth is an OAuth/OIDC client library with session management, TOTP/MFA, credential storage, and auth state machines. This rework accomplishes two goals:

1. **Close critical client-side security gaps** — JWT signature verification (currently tokens are decoded but signatures are never verified), JWKS fetching, OIDC discovery, password authentication flows, token introspection, nonce support.
2. **Add an IdP server module** behind the `server` feature flag — using `foundation_http` for HTTP serving and `foundation_db` for SQL-backed persistence, providing a standards-compliant OpenID Connect provider.

## Sync/Async Pattern — Async First, Sync via Valtron

**Core principle: async is the default implementation target.** All business logic
lives in async methods (`*_async`). Sync callers use valtron to bridge.

### Naming Convention

- Async trait methods end with `*_async` suffix: `fn find_by_email_async(&self, ...) -> Result<T, E>`
- Sync trait methods use the plain name: `fn find_by_email(&self, ...) -> Result<T, E>`
- Traits are separate: `AsyncXxxService` (primary) and `XxxService` (sync wrapper)

### Valtron Bridging (Both Native and WASM)

The sync trait wraps the async implementation using valtron. This works on **both** native
and wasm — valtron is the execution engine for this project, not tokio:

```rust
// Sync wrapper — works on both native and wasm via valtron
pub fn find_by_email(&self, email: &str) -> Result<Option<User>, Error> {
    let this = self.clone();
    let email = email.to_string();
    let task = from_future(async move {
        this.find_by_email_async(&email).await
    });
    let stream = execute(task, None)?;
    collect_one(stream).ok_or_else(|| Error::NoResult)
}
```

### Caveats and When Sync Cannot Wrap Async

Some patterns make valtron bridging impractical:
- **`&mut self` requirements** — valtron requires `Send + 'static`, mutable borrows complicate this
- **Heavy state mutation** — if an operation mutates complex internal state synchronously,
  duplicating for sync may be simpler
- **When duplication is acceptable** — if the valtron bridge would add more complexity than
  a separate sync implementation

In these cases, maintain separate sync implementations. This is rare but permitted.
Feature files should clearly articulate whether valtron bridging is feasible for each design.

### Foundation_db Stream Parity (See Feature 00)

`foundation_db` has a known mismatch: `QueryStore` (sync) returns `StorageItemStream`
(valtron StreamIterator) while `AsyncQueryStore` (async_trait) returns `Result<Vec<T>>`.
**Feature 00** (`features/00-query-store-stream-parity/`) resolves this by making
`AsyncQueryStore::query_async` return `AsyncQueryStream<SqlRow>` — a `futures_core::Stream`
that wraps valtron's StreamIterator. Both native and wasm backends are updated.
This parity fix is a prerequisite for the IdP service features (10-12).

### Storage Clarification

- **foundation_db** — Database-backed storage (Turso, libsql, D1, in-memory). Use for
  credential storage, OAuth tokens, sessions, policy data.
- **foundation_nativeapis** — Filesystem operations. Use only if we need to store data
  directly on the filesystem (e.g., local policy files, credential cache on disk).
  The IdP server primarily uses foundation_db. foundation_nativeapis is relevant for
  features like local-file policy stores in Cedar.

### CredentialStorage Design

Built on foundation_db capabilities:
- `KeyValueStore` (sync) + `AsyncKeyValueStore` (async) for credential persistence
- foundation_nativeapis is ONLY relevant if file-based credential caching is needed
  (e.g., persisting tokens to disk for CLI tools). The primary path is DB-backed.

## Research Sources

- **Rauthy** (explored: `/home/darkvoid/Boxxed/@dev/repo-expolorations/src.auth/src.rauthy/rauthy/markdown/`): Lightweight Rust IdP — OIDC/OAuth2, FIDO2/WebAuthn passkeys, PAM integration, Hiqlite/Postgres storage, HA mode via Raft, ~35-65MB memory, ed25519 token signing, S256 PKCE
- **foundation_db migrations** (`backends/foundation_db/src/core/schema/`): 15 existing migrations covering users, sessions, OAuth, JWT, 2FA, API keys, rate limits, audit logs. Migrations 016-019 will add OIDC server tables.

## Core Design Principles

1. **API-first JSON** — All server endpoints return JSON (hypermedia style). No HTML rendering in this spec. UI is a separate specification (web components).
2. **Feature-gated server** — `server` feature flag pulls in `foundation_http`. Client library remains lightweight when server is not needed.
3. **Shared/ native/ wasm_bindgen/ split** — Core types and logic in `shared/`. HTTP transport implementations in `native/` (SimpleHttpClient) and `wasm_bindgen/` (browser fetch).
4. **foundation_db traits for persistence** — Server uses `QueryStore` (SQL) for users/clients/codes, `KeyValueStore` for caches. No custom IdpStore trait — the existing trait hierarchy is sufficient.
5. **Central AuthManager** — Coordinates `JwtManager`, `SessionManager`, `AuthStateMachine`, and `CredentialStore` into a single lifecycle manager in `shared/auth_manager.rs`.
6. **Migrations centralized** — New OIDC server tables added as migrations 016-019 in `foundation_db/src/core/schema/`.
7. **Refresh token rotation handled** — Client already preserves old refresh token when server doesn't return a new one. No work needed.
8. **jwt-simple for JWT signing** — Already a dependency. Supports EdDSA, RS256, ES256. No new crypto dependencies for the server.

## Feature Index

| Feature | Description | Phase | Status |
|---------|-------------|-------|--------|
| [00-query-store-stream-parity](features/00-query-store-stream-parity/) | Fix AsyncQueryStore API parity — return streams not Vec for multi-row queries | 0 | ✅ complete |
| [01-jwt-verifier](features/01-jwt-verifier/) | Cryptographic JWT signature verification (EdDSA, RS256, ES256), claim validation, issuer/audience checking | 1 | ✅ complete |
| [02-jwks-manager](features/02-jwks-manager/) | JWKS fetcher, cache with TTL, key rotation, kid lookup, native + wasm | 1 | ✅ complete |
| [03-oidc-discovery](features/03-oidc-discovery/) | OIDC discovery client, auto-configure OAuthConfig from .well-known | 1 | ✅ complete |
| [04-userinfo-client](features/04-userinfo-client/) | Fetch user profile from /oidc/userinfo with bearer token | 1 | ✅ complete |
| [05-password-auth](features/05-password-auth/) | Username/password login flow against IdP, MFA challenge support, native + wasm | 1 | ✅ complete |
| [06-token-introspection](features/06-token-introspection/) | RFC 7662 token introspection client for resource servers | 2 | ✅ complete |
| [07-nonce-support](features/07-nonce-support/) | OIDC anti-replay nonce in auth requests, ID token nonce validation | 2 | ✅ complete |
| [08-auth-manager](features/08-auth-manager/) | Central lifecycle manager coordinating JWT, sessions, state machine, credential store | 2 | ✅ complete |
| [09-idp-server](features/09-idp-server/) | IdP HTTP server using foundation_http, router setup, CORS, rate limiting | 2 | ✅ complete |
| [10-idp-models](features/10-idp-models/) | User, client, authorization code, device code, refresh token entities | 2 | ✅ complete |
| [11-idp-services](features/11-idp-services/) | Token generation (jwt-simple), user management (Argon2id), client management, session service | 2 | ✅ complete |
| [12-idp-handlers](features/12-idp-handlers/) | OIDC endpoints: authorize, token, userinfo, jwks, discovery, introspect, device_authorize | 3 | ✅ complete |
| [13-oidc-migrations](features/13-oidc-migrations/) | foundation_db migrations 016-019: oauth_clients, authorization_codes, refresh_tokens, device_codes | 1 | ✅ complete |
| [14-cedar-policy-engine](features/14-cedar-policy-engine/) | Cedar policy engine with multi-source storage (R2, D1, local file, git), entity providers, HTTP middleware | 3 | ✅ complete |

## Architecture

```
foundation_auth/
├── src/
│   ├── lib.rs                      # Re-exports, cfg-gated server module
│   ├── shared/                     # Always compiled, cross-platform
│   │   ├── auth_manager.rs         # NEW — central lifecycle coordinator
│   │   ├── auth_state.rs           # Existing — state machine
│   │   ├── auth_token.rs           # Existing — unified token enum
│   │   ├── credential_store.rs     # Existing — KV store wrapper
│   │   ├── jwt.rs                  # EXTENDED — add JwtVerifier
│   │   ├── jwks.rs                 # NEW — JWKS fetcher + manager
│   │   ├── discovery.rs            # NEW — OIDC discovery client
│   │   ├── userinfo.rs             # NEW — UserInfo endpoint client
│   │   ├── introspection.rs        # NEW — RFC 7662 introspection
│   │   ├── middleware.rs           # Existing — guard functions
│   │   ├── oauth.rs                # EXTENDED — add nonce support
│   │   ├── oauth_token.rs          # Existing — token data
│   │   ├── session.rs              # Existing — session management
│   │   ├── types.rs                # Existing — credentials, errors
│   │   └── two_factor.rs           # Existing — TOTP, backup codes
│   ├── native/                     # Non-wasm32 only
│   │   ├── oauth.rs                # Existing — HTTP token exchange
│   │   └── password_auth.rs        # NEW — password login via SimpleHttpClient
│   ├── wasm_bindgen/               # wasm32 only, feature-gated
│   │   ├── oauth.rs                # Existing — browser fetch token exchange
│   │   └── password_auth.rs        # NEW — password login via browser fetch
│   └── server/                     # NEW, cfg-gated behind `server` feature
│       ├── mod.rs                  # Re-exports
│       ├── config.rs               # IdpConfig (issuer, signing key, TTLs, policy)
│       ├── idp_server.rs           # IdpServer builder → HttpApp → HttpServer
│       ├── models/
│       │   ├── mod.rs
│       │   ├── user.rs             # User entity (maps to migration 002)
│       │   ├── client.rs           # OAuth client entity
│       │   ├── code.rs             # Authorization codes, device codes
│       │   └── token.rs            # Refresh token entity
│       ├── services/
│       │   ├── mod.rs
│       │   ├── token_service.rs    # JWT signing (jwt-simple), ID/access/refresh tokens
│       │   ├── user_service.rs     # User CRUD, Argon2id password verify
│       │   ├── client_service.rs   # OAuth client lookup by ID
│       │   └── session_service.rs  # Wraps existing SessionManager
│       └── handlers/
│           ├── mod.rs
│           ├── authorize.rs        # GET  /oidc/authorize → JSON login_required or auth code
│           ├── token.rs            # POST /oidc/token → access_token + id_token + refresh_token
│           ├── userinfo.rs         # GET  /oidc/userinfo → user profile claims
│           ├── jwks.rs             # GET  /oidc/jwks → public key set
│           ├── discovery.rs        # GET  /.well-known/openid-configuration
│           ├── introspect.rs       # POST /oidc/introspect → RFC 7662 active/inactive
│           └── device_authorize.rs # POST /oidc/device_authorization → device_code + user_code

foundation_db/
└── src/core/schema/
    ├── sql/001_create_kv_store.sql          # Existing
    ├── sql/002_create_users.sql             # Existing — used by server
    ├── sql/003_create_sessions.sql          # Existing — used by server
    ├── ...                                  # Existing 004-015
    ├── sql/016_create_oauth_clients.sql     # NEW
    ├── sql/017_create_authorization_codes.sql  # NEW
    ├── sql/018_create_refresh_tokens.sql    # NEW
    └── sql/019_create_device_codes.sql      # NEW
```

## Client-Server Communication Flow

```
┌─────────────────────┐                    ┌─────────────────────────┐
│  Client Application  │                    │  foundation_auth Server  │
│  (uses client lib)   │                    │  (server feature)        │
│                      │                    │                          │
│  1. Discover:        │  GET /.well-known/  │                          │
│     auto-config      │  ────────────────►  │  Returns discovery JSON  │
│                      │                    │                          │
│  2. Authorize:       │  GET /oidc/         │                          │
│     request code     │  authorize          │  If not authenticated:   │
│                      │  ◄───────────────   │  {status:login_required, │
│  3. Login:           │  POST /auth/v1/     │   login_url}             │
│     submit creds     │  login              │                          │
│                      │  ────────────────►  │  Returns session/token   │
│                      │                    │                          │
│  4. Retry authorize: │  GET /oidc/         │                          │
│                      │  authorize          │  Returns auth_code +     │
│                      │  ◄───────────────   │  redirect_uri            │
│                      │                    │                          │
│  5. Exchange code:   │  POST /oidc/token   │                          │
│     for tokens       │  ────────────────►  │  Verifies PKCE, issues   │
│                      │  ◄───────────────   │  access + id + refresh   │
│                      │                    │                          │
│  6. Verify JWT sig:  │  (local verify)     │                          │
│     using JWKS       │  GET /oidc/jwks     │  Returns public keys     │
│                      │  ────────────────►  │  (Ed25519 OKP)           │
│                      │  ◄───────────────   │                          │
└─────────────────────┘                    └─────────────────────────┘
```

## Server Endpoint Specification (JSON API)

All endpoints return `Content-Type: application/json`.

### GET /.well-known/openid-configuration

Returns OIDC discovery document:
```json
{
  "issuer": "https://auth.example.com",
  "authorization_endpoint": "/oidc/authorize",
  "token_endpoint": "/oidc/token",
  "userinfo_endpoint": "/oidc/userinfo",
  "jwks_uri": "/oidc/jwks",
  "introspection_endpoint": "/oidc/introspect",
  "device_authorization_endpoint": "/oidc/device_authorization",
  "response_types_supported": ["code"],
  "grant_types_supported": ["authorization_code", "refresh_token", "client_credentials", "urn:ietf:params:oauth:grant-type:device_code"],
  "subject_types_supported": ["public"],
  "id_token_signing_alg_values_supported": ["EdDSA", "RS256", "ES256"],
  "scopes_supported": ["openid", "profile", "email", "groups"],
  "code_challenge_methods_supported": ["S256"]
}
```

### GET /oidc/authorize

Query params: `client_id`, `redirect_uri`, `response_type`, `scope`, `state`, `code_challenge`, `code_challenge_method`, `nonce`

If authenticated (valid session cookie):
- Validates client_id, redirect_uri, response_type=code, scope, PKCE
- Generates single-use authorization code
- Returns: `{"status": "authorized", "code": "...", "state": "...", "redirect_uri": "..."}`

If not authenticated:
- Returns: `{"status": "login_required", "login_url": "/auth/v1/login", "return_to": "/oidc/authorize?..."}`

### POST /auth/v1/login

Body: `{"email": "...", "password": "..."}`

On success: `{"status": "authenticated", "session_id": "...", "mfa_required": false, "cookie": {...}}`

If MFA enabled: `{"status": "mfa_required", "challenge_id": "...", "type": "totp"}`

### POST /auth/v1/mfa

Body: `{"challenge_id": "...", "code": "123456"}`

On success: `{"status": "authenticated", "session_id": "...", "cookie": {...}}`

### POST /oidc/token

Body (form-urlencoded): `grant_type=authorization_code&code=...&code_verifier=...&client_id=...&client_secret=...&redirect_uri=...`

Response:
```json
{
  "access_token": "eyJhbG...",
  "token_type": "Bearer",
  "expires_in": 900,
  "refresh_token": "dGhpcy...",
  "id_token": "eyJhbG...",
  "scope": "openid profile"
}
```

### GET /oidc/userinfo

Requires `Authorization: Bearer <access_token>` with `openid` scope.

Returns:
```json
{
  "sub": "user_123",
  "email": "alice@example.com",
  "email_verified": true,
  "name": "Alice",
  "groups": ["users", "admins"]
}
```

### GET /oidc/jwks

Returns public key set:
```json
{
  "keys": [
    {
      "kty": "OKP",
      "crv": "Ed25519",
      "x": "base64-public-key",
      "kid": "key-2025-01",
      "use": "sig",
      "alg": "EdDSA"
    }
  ]
}
```

### POST /oidc/introspect

Body: `token=...&client_id=...&client_secret=...`

Returns:
```json
{"active": true, "scope": "openid profile", "client_id": "myapp", "username": "alice@example.com", "exp": 1640995200}
```
or `{"active": false}` for invalid/expired tokens.

### POST /oidc/device_authorization

Returns:
```json
{
  "device_code": "abc123...",
  "user_code": "WXYZ-1234",
  "verification_uri": "https://auth.example.com/device",
  "verification_uri_complete": "https://auth.example.com/device?code=WXYZ-1234",
  "expires_in": 600,
  "interval": 5
}
```

## AuthManager Design

The `AuthManager` in `shared/auth_manager.rs` coordinates all existing auth components:

```rust
pub struct AuthManager {
    store: CredentialStorage,
    session_mgr: SessionManager<CredentialStorage>,
    jwt_manager: JwtManager,
    state_machine: AuthStateMachine,
    jwt_verifier: Option<JwtVerifier>,
    jwks_manager: Option<JwksManager>,
}

impl AuthManager {
    /// On application startup — load persisted credential, validate, init state.
    pub fn init_from_store(&mut self) -> Result<(), AuthError>;

    /// Authenticate — login, persist credential, transition to Authenticated.
    pub async fn authenticate(&mut self, credential: AuthCredential) -> Result<Authenticated, AuthError>;

    /// Get valid token — check state machine, queue if refreshing, return token.
    pub fn get_valid_token(&mut self) -> Result<AuthToken, AuthError>;

    /// Refresh — refresh JWT, persist new refresh token, handle rotation.
    pub async fn refresh(&mut self) -> Result<(), AuthError>;

    /// Logout — revoke all sessions, clear tokens, reset state machine.
    pub async fn logout(&mut self) -> Result<(), AuthError>;
}
```

## Server Persistence Strategy

The server uses `foundation_db` trait interfaces directly:

- **`QueryStore`** (sync, native) — SQL queries for users (by email), clients (by ID), auth codes (by code), refresh tokens (by hash). Implemented by Turso and libsql backends.
- **`AsyncQueryStore`** (async, wasm) — D1 SQLite queries via Promise-based JS APIs.
- **`KeyValueStore`** — JWKS cache, token cache, session cache. Available on all backends.
- **`AsyncKeyValueStore`** — Cloudflare KV for wasm.

The server accepts any `QueryStore + KeyValueStore` implementor, so it works with Turso (embedded SQLite), libsql (remote SQLite), D1 (edge SQLite), or in-memory (testing).

## Module References

- `backends/foundation_auth/` — target crate
- `backends/foundation_cedar/` — Cedar policy engine (feature 14 — new crate)
- `backends/foundation_http/` — HTTP server framework (router, middleware, ServeWriter, HttpApp)
- `backends/foundation_db/` — persistence (QueryStore, KeyValueStore, MigrationRunner)
- `backends/foundation_netio/` — HTTP client (SimpleHttpClient for native)
- `backends/foundation_core/` — valtron execution, Stream types
- `backends/foundation_errstacks/` — error traces
- `backends/foundation_logging/` — tracing

## Feature Flags

```toml
# New features in foundation_auth Cargo.toml
server = ["foundation_http"]       # Opt-in IdP server module
cedar = ["foundation_cedar"]       # Opt-in Cedar policy authorization (feature 14)

# New crate: backends/foundation_cedar/
# Feature flags defined there: native, wasm, full, partial-eval, http-middleware
```

Existing features unchanged: `turso`, `wasm`, `wasm-bindgen-oauth`, `wasm-bindgen-session`.

## Language Stack

- **Rust** — all implementation

## Success Criteria

- [ ] JwtVerifier rejects unsigned/forged tokens, accepts valid EdDSA/RS256/ES256 signed tokens
- [ ] JwksManager fetches and caches keys, handles rotation
- [ ] OidcDiscovery auto-configures OAuthConfig from discovery document
- [ ] UserInfo client fetches user profile with bearer token
- [ ] Password auth works on native (SimpleHttpClient) and wasm (browser fetch)
- [ ] Token introspection returns RFC 7662 compliant responses
- [ ] Nonce included in auth URL, validated in ID token
- [ ] AuthManager coordinates all components: persist on login, refresh & persist, init from store
- [ ] IdpServer starts and serves all OIDC endpoints as JSON
- [ ] Full authorization code + PKCE flow works end-to-end (client → server → client)
- [ ] Token refresh with rotation works
- [ ] Client credentials flow works
- [ ] Device authorization grant works
- [ ] Migrations 016-019 apply cleanly on Turso and D1
- [ ] JWT signatures use Ed25519 (default), with RS256/ES256 support
- [ ] Rate limiting active on /auth/v1/login and /oidc/token endpoints
- [ ] CORS configured for /.well-known and /oidc/jwks endpoints
- [ ] CedarEngine evaluates permit/forbid policies correctly with schema validation
- [ ] PolicyStore loads from R2, D1, local file, and git backends
- [ ] EntityProvider builds entities from JWT claims, database, or static config
- [ ] CedarLayer middleware gates IdP server endpoints
- [ ] foundation_cedar compiles to both native and wasm32 targets

---

_Created: 2026-06-05_
