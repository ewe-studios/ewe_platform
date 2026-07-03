---
feature: "Auth: AuthInfo contract, async AuthFunc, authenticators, Cedar authz (D09)"
description: "HTTP-level async authentication + seam-level authorization on foundation_auth"
status: "pending"
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
