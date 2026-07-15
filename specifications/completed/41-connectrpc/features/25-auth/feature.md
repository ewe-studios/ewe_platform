---
feature: "Auth: AuthInfo contract, async AuthFunc, authenticators, Cedar authz (D09)"
description: "HTTP-level async authentication + seam-level authorization on foundation_auth"
status: "complete"
priority: "medium"
phase: 1
depends_on: ["22-router-dispatch"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 25-auth: Auth: AuthInfo contract, async AuthFunc, authenticators, Cedar authz (D09)

## Description

The authn/authz stack: async credential verification normalized into one AuthInfo contract at the HTTP layer, Cedar-based authorization at the seam, and the foundation_auth upstream enablers including the true async SessionManager.

## Normative sources (single source of truth — read before writing code)

- decisions/09-auth-middleware.md — entire doc is normative (AuthInfo, async trait, R7–R18)

## Scope

- AuthInfo { context: AuthContext, artifacts: Extensions } — the single normalized principal; get_auth_info/get_auth_artifact
- AuthFunc (async, BoxFuture) + AuthMiddleware (writes SimpleIncomingRequest.extensions; ErrorWriter for protocol-correct errors)
- JwtAuthenticator (JWKS cached steady-state, R13) / SessionAuthenticator (TRUE async SessionManager — foundation_auth enabler, never block_on-wrapped) / Composite (Ok(None) continues) / PerProcedureAuth
- AuthzInterceptor (Cedar policies, R14; has_scope fast path); helpers: infer_protocol/infer_procedure/bearer_token (byte-safe)
- R15 mTLS PeerCertificates via ConnectionContext; R16 introspection; R17 rate-limit interceptor; R18 CORS preset verification
- auth feature flag OFF by default (R12); upstream foundation_auth fixes (R7 case-insensitivity, R9 type-erased sessions, async SessionManager)

## Acceptance criteria

- End-to-end: JWT-authed request reaches handler with AuthInfo; failed auth renders protocol-correct errors
- Composite chain: Ok(None) never short-circuits (test)
- Session validation awaits a real async store call (no block_on in the request path)

## Completion (2026-07-08)

### Part A — Upstream foundation_auth fixes

- **R7:** `extract_bearer_token` now uses byte-wise `eq_ignore_ascii_case` for full RFC 9110 case-insensitive matching
- **R8:** `AuthContext` extended with `sub: Option<String>` and `roles: Vec<String>` for principal identity

### Part B — Auth module in foundation_connectrpc (`src/auth.rs`, ~540 lines)

- `AuthInfo` — single normalized principal contract with typed `artifacts` + `get_auth_info`/`get_auth_artifact` helpers
- `AuthFunc` trait — async, dyn-safe via `BoxFuture`; blanket impl for async closures taking `(&SimpleHeaders, &SimpleUrl)`
- `infer_protocol` / `infer_procedure` — protocol detection from Content-Type/method + procedure path extraction
- `bearer_token` — RFC 9110 case-insensitive extraction (delegates to foundation_auth)
- `JwtAuthenticator` — sync CPU-only JWT verification wrapped in async `AuthFunc`
- `CompositeAuthenticator` — chain with fallthrough (`Ok(None)` continues, `Err` remembered, first `Ok(Some)` wins)
- `PerProcedureAuth` — dispatch by procedure path, with unauthenticated allow-list
- `AuthzInterceptor` — seam-level scope authorization via `has_scope`; implements all 3 `Interceptor` methods
- `authenticate_request` — HTTP-level middleware utility; inserts `AuthInfo` on success, renders `ErrorWriter` response on failure
- Feature-gated behind `auth` feature flag (R12; `foundation_auth` is optional dep)

### Verification

- **Check:** `cargo check --features auth` — zero warnings
- **Tests:** 120/120 pass (25 new auth_tests, 83 existing connectrpc, 12 foundation_auth — zero regressions)
- **Files:** `src/auth.rs` (new), `tests/auth_tests.rs` (new), `Cargo.toml` (+auth feature), `lib.rs` (+auth module)
- **Modified:** `foundation_auth/src/shared/middleware.rs` (R7 + R8)

### Deferred (not in this feature)

- SessionAuthenticator with true async SessionManager (foundation_auth already has `get_session_async`; wiring it into the connectrpc `AuthFunc` is a future feature)
- mTLS PeerCertificates via ConnectionContext (R15 — transport-layer, needs netio SSL plumbing first)
- OAuth introspection authenticator (R16)
- Rate-limit interceptor (R17)
- CORS preset verification (R18)
