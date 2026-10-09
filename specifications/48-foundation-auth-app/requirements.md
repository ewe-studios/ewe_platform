---
description: "foundation_auth_app — Complete the spec-38 IdP server (stubs → real handlers), add missing endpoints (PoW, registration, password reset, WebAuthn, ToS, template/config API, account management), then deliver a WASM UI package and a combined app via /apps/foundation_auth_app."
status: "proposed"
priority: "high"
created: 2026-06-16
updated: 2026-06-16
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - authentication
    - identity-provider
    - webassembly
    - ui
    - end-to-end
    - proof-of-work
    - webauthn
    - rauthy
has_features: true
has_fundamentals: true
builds_on:
  - "specifications/completed/38-foundation-auth-server"
  - "specifications/42-ui-component"
  - "specifications/39-reactive-html"
  - "specifications/completed/21-http-framework"
related_specs:
  - "specifications/completed/38-foundation-auth-server"
  - "specifications/42-ui-component"
  - "specifications/completed/02-build-http-client"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# foundation_auth_app — Complete IdP + WASM UI + Combined App

## Overview

Spec 38 delivered the **foundation_auth** crate with:
- Client library (JWT, JWKS, discovery, introspection, AuthManager, sessions, TOTP)
- IdP server module behind `server` feature flag (models, services, handler stubs)

This spec closes three categories of gaps:

### 1. Handler stubs → real implementations (F01–F02)

Spec 38 Features 10–13 delivered models and services that are **complete and tested**:
- `TokenService` — JWT signing, token pairs, client credentials, refresh token hashing
- `UserService` — Argon2id password hashing, validation
- `SessionService` — wraps existing `SessionManager`
- Models — `User`, `OAuthClient`, `AuthorizationCode`, `DeviceCode`, `RefreshToken`

But the handlers in `IdpHandlerCore` return errors for everything except `discovery()` and `jwks()`. This spec wires the services to the handlers.

### 2. Missing endpoint categories (F03–F09)

rauthy provides these endpoint categories that spec 38 never addressed:

| Category | Endpoints | New Feature |
|---|---|---|
| User registration | `POST /users/register`, `POST /dev/register` | F03 |
| Password reset | `POST /users/request_reset`, `PUT /users/{id}/reset` | F04 |
| Proof of Work | `GET /auth/v1/pow`, `POST /auth/v1/pow` | F05 |
| WebAuthn/FIDO2 | `/auth/v1/webauthn/*` (register + login) | F06 |
| Terms of Service | `/auth/v1/tos/*` | F07 |
| Template/config API | `/auth/v1/templates/*` | F08 |
| Account management | `/auth/v1/users/{id}`, session listing, account deletion | F09 |

### 3. UI + App (F10–F11)

Per spec 38 principle #1 ("UI is a separate specification"):
- **`foundation_auth_ui`** — WASM UI package (moved from spec 42 feature 06)
- **`/apps/foundation_auth_app/`** — Combined deployable: server + UI + E2E tests

## Core design principles (inherited from spec 38)

1. **API-first JSON** — All server endpoints return JSON. No HTML rendering.
2. **Feature-gated server** — `server` feature flag inside `foundation_auth`.
   New server-only dependencies (webauthn-rs) are feature-gated behind `server`.
3. **foundation_db traits** — QueryStore (SQL) + KeyValueStore (cache). No custom store trait.
4. **Async first, sync via valtron** — Business logic async; sync bridges via valtron.
5. **Dual native + CF** — All services work on native Rust and wasm32 CF Workers.
6. **rauthy-compatible API** — Endpoint paths and response shapes match rauthy so the UI package targets either implementation.
7. **HTTP status codes matter** — Handlers return typed responses with correct HTTP status (202, 204, 206, 409, 423, 429), not just 200.

## Crate layout

```
backends/
├── foundation_auth/                    # Existing — extend server/ module
│   └── src/
│       ├── shared/                     # Client types (unchanged)
│       ├── native/                     # Client HTTP (unchanged)
│       ├── wasm_bindgen/              # Client wasm (unchanged)
│       └── server/                     # EXTEND — F01–F09
│           ├── config.rs               # IdpConfig, PasswordPolicy (complete)
│           ├── idp_server.rs           # IdpServer builder (extend routes)
│           ├── models/                 # Existing models (extend for F06/F07)
│           │   ├── mod.rs
│           │   ├── user.rs
│           │   ├── client.rs
│           │   ├── code.rs
│           │   └── token.rs
│           ├── services/               # Existing services (extend for F03–F09)
│           │   ├── mod.rs
│           │   ├── token_service.rs    # Complete
│           │   ├── user_service.rs     # Extend: create_user, reset_password
│           │   ├── session_service.rs  # Extend: cookie creation
│           │   ├── client_service.rs   # Complete
│           │   ├── pow_service.rs      # NEW: F05
│           │   ├── webauthn_service.rs # NEW: F06
│           │   ├── tos_service.rs      # NEW: F07
│           │   └── template_service.rs # NEW: F08
│           └── handlers/               # COMPLETE: F01–F09
│               ├── mod.rs
│               ├── core.rs             # IdpHandlerCore (replace stubs, add new)
│               └── serve_adapter.rs    # Existing valtron bridge
│
├── foundation_auth_ui/                 # NEW — WASM UI (F10, moved from spec 42 F06)
│   └── src/
│       ├── lib.rs                      # Re-exports
│       ├── layout.rs                   # AuthLayout, AuthHome, ErrorPage
│       ├── login.rs                    # LoginPage
│       ├── register.rs                 # RegisterPage
│       ├── password.rs                 # PasswordResetRequest + PasswordSetPage
│       ├── logout.rs                   # LogoutPage
│       ├── device.rs                   # DeviceAuthPage
│       ├── account.rs                  # AccountPage
│       ├── callback.rs                 # ProviderCallback
│       ├── api.rs                      # HTTP client functions
│       ├── types.rs                    # Request/response types
│       ├── pow.rs                      # PoW solver (client-side)
│       ├── webauthn.rs                 # WebAuthn JS FFI shim
│       ├── session.rs                  # Session helpers
│       └── tos.rs                      # TosAccept component
│
apps/
└── foundation_auth_app/                # NEW — Combined app (F11)
    ├── Cargo.toml
    └── src/
        ├── lib.rs                      # App factory
        ├── main.rs                     # Native binary
        └── cf_worker.rs                # CF Worker entrypoint
```

## Endpoint map (complete)

### OIDC standard endpoints (spec 38 — complete handlers in F01)

| Method | Endpoint | Handler | Status |
|---|---|---|---|
| GET | `/.well-known/openid-configuration` | `discovery()` | ✅ Works |
| GET | `/oidc/jwks` | `jwks()` | ✅ Works |
| GET | `/oidc/authorize` | `authorize()` | F01 — stub → real |
| POST | `/oidc/token` | `token()` | F01 — stub → real |
| GET | `/oidc/userinfo` | `userinfo()` | F01 — stub → real |
| POST | `/oidc/introspect` | `introspect()` | F01 — stub → real |
| POST | `/oidc/device/authorize` | `device_authorize()` | F01 — stub → real |

### Auth endpoints (F02 — aligned with rauthy paths)

| Method | Endpoint | Handler | Status |
|---|---|---|---|
| POST | `/auth/v1/oidc/authorize` | `login()` unified multi-step | F02 — NEW (replaces `/auth/v1/login`) |
| POST | `/auth/v1/mfa` | `mfa()` | F02 — NEW |
| POST | `/auth/v1/oidc/logout` | `logout()` | F02 — NEW |
| POST | `/auth/v1/passkey/login/start` | `passkey_login_start()` | F06 — passkey-only flow |
| POST | `/auth/v1/passkey/login/finish` | `passkey_login_finish()` | F06 — passkey-only flow |

### User management (F03–F04)

| Method | Endpoint | Feature |
|---|---|---|
| POST | `/auth/v1/users/register` | F03 |
| POST | `/auth/v1/dev/register` | F03 (dev mode) |
| POST | `/auth/v1/users/request_reset` | F04 |
| PUT | `/auth/v1/users/{id}/reset` | F04 |
| GET | `/auth/v1/users/{id}` | F09 |
| PUT | `/auth/v1/users/{id}` | F09 |
| POST | `/auth/v1/users/{id}/revoke` | F09 |

### WebAuthn (F06)

| Method | Endpoint | Feature |
|---|---|---|
| POST | `/auth/v1/webauthn/register/start` | F06 |
| POST | `/auth/v1/webauthn/register/finish` | F06 |
| POST | `/auth/v1/webauthn/login/start` | F06 |
| POST | `/auth/v1/webauthn/login/finish` | F06 |
| DELETE | `/auth/v1/webauthn/{id}` | F06 |
| PUT | `/auth/v1/webauthn/{id}` | F06 |

### Proof of Work (F05)

| Method | Endpoint | Feature |
|---|---|---|
| GET | `/auth/v1/pow` | F05 |
| POST | `/auth/v1/pow` | F05 |

### ToS (F07)

| Method | Endpoint | Feature |
|---|---|---|
| GET | `/auth/v1/tos/latest` | F07 |
| POST | `/auth/v1/tos/accept` | F07 |

### Template/config API (F08)

| Method | Endpoint | Feature |
|---|---|---|
| GET | `/auth/v1/templates/config` | F08 |
| GET | `/auth/v1/templates/user_values` | F08 |
| GET | `/auth/v1/templates/password_policy` | F08 |
| GET | `/auth/v1/templates/providers` | F08 |
| GET | `/auth/v1/templates/theme` | F08 |

### Dev mode endpoints

| Method | Endpoint | Feature |
|---|---|---|
| POST | `/auth/v1/dev/browser_id` | F02 |

## Feature Index

| Feature | Description | Depends on |
|---|---|---|
| [01-complete-oidc-handlers](features/01-complete-oidc-handlers/) | Wire services to IdpHandlerCore stubs; HandlerResponse type; IdpError expansion; routing fix | spec-38 F10–F13 |
| [02-login-mfa-handlers](features/02-login-mfa-handlers/) | Add login + MFA + logout routes/handlers, session cookie creation, CSRF | F01 |
| [03-user-registration](features/03-user-registration/) | User registration endpoint, dev mode registration | F05 |
| [04-password-reset](features/04-password-reset/) | Password reset request + set via magic link | F05 |
| [05-proof-of-work](features/05-proof-of-work/) | PoW challenge/solve endpoint, anti-bot rate limiting | — |
| [06-webauthn-fido2](features/06-webauthn-fido2/) | WebAuthn/FIDO2 + passkey-only login flow | F02 |
| [07-terms-of-service](features/07-terms-of-service/) | ToS versioning, display, acceptance tracking; integrates into login flow | F02 |
| [08-template-config-api](features/08-template-config-api/) | Template/config API for UI dynamic configuration | — |
| [09-account-management](features/09-account-management/) | User info CRUD, session listing, account deletion; passkey mgmt (optional F06) | F02 |
| [10-auth-ui-package](features/10-auth-ui-package/) | WASM UI components — all auth pages (moved from spec 42 feature 06) | F01–F09, spec-42 F00–F05 |
| [11-auth-app](features/11-auth-app/) | Combined app in /apps/ — mounts server + UI, foundation_testbed E2E | F01–F10 |

## New dependencies

| Crate | Feature | Why |
|---|---|---|
| `webauthn-rs = "0.5"` (optional) | F06 (WebAuthn) | Gated behind `server-native`. `webauthn-rs-core` depends on `openssl` + `openssl-sys` — does not compile to `wasm32-unknown-unknown`. CF Workers relay `/auth/v1/webauthn/*` to native or disable passkeys. |
| (no new deps) | F05 (PoW) | Uses existing `sha2` + `rand` |
| (no new deps) | F07 (ToS) | Uses existing `serde` + `foundation_db` |

## Feature Flags (extended)

```toml
# foundation_auth — existing features unchanged:
#   default = ["turso"]
#   turso = ["foundation_db/turso"]
#   server = ["dep:foundation_http"]                     # core server, native + CF
#   server-test = ["server", "foundation_core/multi", "foundation_http/multi"]
#   wasm = ["uuid/js", "chrono/wasmbind", "dep:getrandom", "getrandom/js"]
#   wasm-bindgen-oauth = [...]
#   wasm-bindgen-session = [...]

# foundation_auth — server-native adds WebAuthn (webauthn-rs-core depends on
# openssl + openssl-sys — not wasm compatible). The core server (server feature)
# works on native + CF. WebAuthn endpoints are native-only. CF Workers
# deployments relay /auth/v1/webauthn/* to native or disable passkey support.
#   server-native = ["server", "dep:webauthn-rs"]

# foundation_auth_ui (new)
[features]
styled = []  # pulls rauthy CSS theme

# foundation_auth_app (new)
[features]
native-server = ["foundation_auth/server-native"]
cf-worker = ["foundation_auth/server"]
full = ["native-server", "cf-worker"]
```

## Success Criteria

- [ ] `authorize()` handler creates auth codes, stores in DB, returns JSON
- [ ] `token()` handler validates auth codes + PKCE, issues JWT tokens
- [ ] `userinfo()` validates bearer token, returns claims
- [ ] `login()` handler creates session cookies on valid credentials
- [ ] `mfa()` handler validates TOTP, creates session
- [ ] `logout()` handler revokes session, clears cookie
- [ ] User registration creates accounts with Argon2id hashed passwords
- [ ] Password reset flow: request → magic link → set new password
- [ ] PoW endpoint generates challenges and validates solutions
- [ ] WebAuthn registration and authentication work
- [ ] ToS can be versioned, fetched, and accepted
- [ ] Template API returns correct config for UI
- [ ] Account page API supports user info update, session listing, account deletion
- [ ] All UI pages render and drive the API correctly
- [ ] foundation_testbed E2E tests pass
- [ ] Native binary starts and serves both API and UI
- [ ] CF Worker dispatches correctly

---

_Created: 2026-06-16_
