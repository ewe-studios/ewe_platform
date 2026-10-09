# LEARNINGS — foundation_auth_app

## 2026-06-16: Spec Creation + Review

### Spec 38 Gap Analysis — Corrected

After reviewing the actual codebase (not just the spec):

**Services and models are COMPLETE:**
- `TokenService` — JWT signing, token pairs, client credentials, refresh token hashing ✅
- `UserService` — Argon2id hash, verify, validate_password ✅
- `SessionService` — wraps existing SessionManager ✅
- `ClientService` — create client record ✅
- Models — User (with lockout), OAuthClient, AuthorizationCode (with PKCE), DeviceCode, RefreshToken ✅

**Handlers are STUBS:**
- `discovery()` and `jwks()` work
- `authorize()`, `token()`, `userinfo()`, `introspect()`, `device_authorize()` return errors

**Routes NOT registered:**
- `/auth/v1/login` — defined in spec 38 endpoint spec, but never added to `register_routes()`
- `/auth/v1/mfa` — same

**Missing entirely:**
- PoW (rauthy's anti-bot mechanism)
- User registration endpoints
- Password reset endpoints
- WebAuthn/FIDO2
- ToS
- Template/config API
- Account management

### Critical Architecture Issues (from review)

1. **ServeAdapter returns only HTTP 200** — `respond::json(&mut conn, 200, &body)`
   can't return 202/204/206/409/423/429. Fixed by introducing `HandlerResponse` type
   with status + body + headers.

2. **Dispatch uses `path.ends_with()`** — doesn't scale, can't extract path parameters,
   no method-based routing. Fixed by prefix-based dispatch with explicit pattern matching.

3. **Discovery doc paths don't match served paths** — `/introspect` in discovery doc
   vs `/idp/introspect` in route registration. Fixed by passing prefix to
   `OidcDiscoveryDocument::from_config()`.

4. **Login endpoint path mismatch** — UI expects `/auth/v1/oidc/authorize` (spec 42),
   spec was going to create `/auth/v1/login`. Fixed: login at `/auth/v1/oidc/authorize`.

5. **Passkey-only login flow missing** — rauthy has `/auth/v1/passkey/login/*` separate
   from `/auth/v1/webauthn/*`. Added to F06.

6. **Dependency chains wrong** — F03 didn't need F02, F07 needed F02. Fixed.

7. **webauthn-rs not feature-gated** — Added `server-native` feature (separate from
   `server`). `webauthn-rs-core` depends on `openssl` + `openssl-sys` — does not
   compile to `wasm32-unknown-unknown`. Core server (F01–F05, F07–F09) works
   native + CF. WebAuthn is native-only. CF Workers relay webauthn endpoints to
   native or disable passkeys.

### Crate Split Decision

Spec 38 put the server behind `server` feature-gate inside `foundation_auth`.
This is correct and stays. No new `foundation_auth_server` crate needed.

New crates are ONLY:
- `foundation_auth_ui` (backends/) — WASM UI components
- `foundation_auth_app` (apps/) — combined deployable

### Out of Scope (noted for future specs)

- Group/role management — rauthy has groups, not covered here
- Event/audit log — rauthy logs auth events, not covered
- API keys — service-to-service auth keys, not covered
- Health check — `/health` endpoint, trivial, can be added to F11
- Config management runtime updates — read-only from IdpConfig, updates deferred
- Email sending — deferred (separate spec)
- Admin UI — explicitly out of scope
- Dynamic client registration — deferred
- FedCM — browser-native, pass-through, deferred

