---
description: "foundation_auth social login / upstream identity provider federation. Adds OIDC/OAuth2 upstream provider configuration, redirect-based login flows (Google, Facebook, GitHub, Apple, etc.), automatic user provisioning, account linking, and admin-facing provider management to the IdP server."
status: "in-progress"
priority: "high"
created: 2026-07-17
updated: 2026-07-17
author: "claude"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - authentication
    - oauth2
    - oidc
    - social-login
    - federation
    - identity-broker
    - server
has_features: true
has_fundamentals: true
builds_on:
  - "specifications/completed/38-foundation-auth-server"
related_specs:
  - "specifications/completed/38-foundation-auth-server"
  - "specifications/57-foundation-keychain"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# foundation_auth — Social Login / Upstream IdP Federation

## Overview

The IdP server (spec-38) is a standards-compliant OIDC provider that manages its own users and issues JWTs. It supports multiple registered client applications. What it lacks is the ability to **accept identity from upstream providers** — Google, Facebook, GitHub, Apple, Microsoft, or any OIDC/OAuth2 IdP.

This spec adds **identity brokering** to the existing IdP server:

1. **Admin-configurable upstream providers** — store provider credentials (client_id, client_secret, discovery URL, scopes) in the database
2. **Redirect-based login flows** — when a user clicks "Login with Google", the IdP redirects to Google's authorization endpoint, receives the callback, exchanges the code, fetches the user profile, and mints its own JWT
3. **Automatic user provisioning** — first-time social login creates a local user record linked to the upstream identity
4. **Account linking** — users can connect multiple upstream identities to one local account
5. **Provider discovery document** — the IdP's own discovery document advertises which upstream providers are available

### What This Is NOT

This does not turn the IdP into a SAML IdP, a SCIM provisioning target, or a general-purpose OAuth proxy. Each upstream provider is an **authentication source** for the IdP's login flow. The IdP remains the single token-issuing authority; downstream apps always talk to the IdP, never directly to Google/Facebook/etc.

## Existing Codebase — What We Already Have

### Migrations 006/007 (orphaned, client-side)

- **006_create_oauth_credentials** — stores `client_id`, `client_secret_encrypted`, `authorization_url`, `token_url`, `scopes`, `redirect_uri`. Designed for *outbound* OAuth but never wired into the IdP server.
- **007_create_oauth_states** — stores `state_param`, `code_verifier` for PKCE/CSRF protection in outbound flows. Never wired.

These tables have the right shape for *provider credentials* but need a `provider` column and re-purpose for IdP upstream configuration.

### Migrations 016-019 (IdP server tables)

- 016: `oauth_clients` — apps registered with the IdP (our downstream clients)
- 017: `authorization_codes` — auth codes the IdP issues
- 018: `refresh_tokens` — refresh tokens the IdP issues
- 019: `device_codes` — device codes the IdP issues

These are the IdP server's own tables and remain unchanged.

### IdpServer endpoints (spec-38 features 09/12)

All OIDC standard endpoints are implemented: discovery, JWKS, authorize, token, userinfo, introspect, device_authorize. Plus auth (login, MFA, logout), registration, password reset, WebAuthn, PoW, ToS, account management.

### What's Missing

| Capability | Status |
|---|---|
| Store upstream provider config | ❌ migrations 006/007 exist but orphaned |
| Redirect user to upstream auth URL | ❌ |
| Handle upstream callback | ❌ |
| Exchange upstream code for tokens | ❌ |
| Fetch upstream user profile | ❌ |
| Create local user from upstream profile | ❌ |
| Link multiple providers to one user | ❌ |
| Provider availability in discovery | ❌ |
| Admin API to manage providers | ❌ |

## Architecture

### Identity Broker Pattern

```
┌──────────┐     ┌─────────────┐     ┌─────────────┐
│  App     │     │   IdP       │     │  Upstream   │
│  (client)│     │   (us)      │     │  (Google)   │
└────┬─────┘     └──────┬──────┘     └──────┬──────┘
     │                  │                    │
     │  GET /authorize  │                    │
     │  ?provider=google│                    │
     ├─────────────────►│                    │
     │                  │  302 → Google auth │
     │  ◄───────────────┤                    │
     │  (user browser)  │                    │
     │                  ├────── 302 ────────►│
     │                  │                    │
     │                  │  User logs in      │
     │                  │                    │
     │                  │◄──── callback ─────┤
     │                  │  ?code=...&state=  │
     │                  │                    │
     │                  │  POST /token       │
     │                  ├────── code ───────►│
     │                  │◄──── tokens ───────┤
     │                  │                    │
     │                  │  GET /userinfo     │
     │                  ├──── bearer ───────►│
     │                  │◄──── profile ──────┤
     │                  │                    │
     │                  │  (create/link user)│
     │                  │  (mint our JWT)    │
     │  ◄───────────────┤                    │
     │  redirect_uri    │                    │
     │  ?code=OUR_CODE  │                    │
     │                  │                    │
     │  POST /token     │                    │
     ├─────────────────►│                    │
     │◄── our JWT ──────┤                    │
```

### Key Design Points

1. **The IdP is always the token issuer.** Apps receive JWTs signed by the IdP, never by Google. Google is just a login method.
2. **Provider selection happens at /authorize.** The app can request a specific provider (`?provider=google`) or let the user choose (login page shows buttons).
3. **Upstream state is managed by the IdP.** The IdP generates its own `state` for the app and a separate `state` for the upstream provider. These are correlated server-side.
4. **Callback goes to the IdP, not the app.** The upstream provider redirects to `https://idp.example.com/auth/v1/providers/{id}/callback`, which then redirects to the app with the IdP's own auth code.

## Sync/Async Pattern

Same as spec-38: async-first with valtron sync wrappers. All upstream HTTP I/O is async.

## Storage

Uses `foundation_db` (QueryStore + KeyValueStore). Provider credentials stored in the enhanced migrations 006/007 (add `provider` column, rename for IdP-facing use). User-provider links in a new migration.

## Feature Index

| Feature | Description | Phase | Status |
|---------|-------------|-------|--------|
| [01-provider-model](features/01-provider-model/) | UpstreamProvider entity, provider secret storage, CRUD service | 1 | ⬜ |
| [02-provider-migrations](features/02-provider-migrations/) | Enhance 006/007 for IdP-facing use, new migration for user_provider_links | 1 | ⬜ |
| [03-oauth-upstream-client](features/03-oauth-upstream-client/) | Generic upstream OIDC/OAuth2 client: discover, authorize URL, exchange code, fetch userinfo | 2 | ⬜ |
| [04-social-login-flow](features/04-social-login-flow/) | Redirect handler + callback handler, double-state correlation, code minting | 2 | ⬜ |
| [05-user-provisioning](features/05-user-provisioning/) | Auto-create local user from upstream profile, account linking, email merge | 2 | ⬜ |
| [06-provider-discovery](features/06-provider-discovery/) | Extend IdP discovery to list available providers, admin provider management API | 3 | ⬜ |
| [07-wasm-upstream-client](features/07-wasm-upstream-client/) | wasm32 upstream client using browser fetch (for Tauri/desktop embed scenarios) | 3 | ⬜ |

## Server Persistence Strategy

Same as spec-38: `QueryStore` for SQL-backed persistence, `KeyValueStore` for caching (provider configs cached with TTL, OAuth state stored with expiry).

## Module References

- `backends/foundation_auth/` — target crate (server/ module, shared/ module)
- `backends/foundation_db/` — persistence
- `backends/foundation_http/` — HTTP server
- `backends/foundation_netio/` — HTTP client for upstream calls
- `backends/foundation_keychain/` — optional: encrypt provider secrets (spec-57)

## Feature Flags

```toml
# Existing features unchanged
server = ["foundation_http"]       # IdP server module
cedar = ["foundation_cedar"]       # Cedar policy authorization

# New feature
social = []                         # Upstream provider / social login (pulls in server)
```

## Language Stack

- **Rust** — all implementation

## Success Criteria

- [ ] UpstreamProvider entity with OIDC discovery, client_id, encrypted secret, scopes, mapping config
- [ ] Provider CRUD service with encryption for secrets
- [ ] Generic upstream OIDC client handles discovery, authorize URL, code exchange, userinfo fetch
- [ ] Google OAuth2 works end-to-end (authorize → callback → exchange → profile → mint)
- [ ] GitHub OAuth2 works (no OIDC userinfo, use /user API)
- [ ] Automatic user creation on first social login
- [ ] Account linking: connect a second provider to existing user (by email match or explicit link)
- [ ] App can request `?provider=google` at /authorize to skip the login page
- [ ] IdP discovery document lists available providers
- [ ] Admin API: list/add/update/delete providers
- [ ] Callback URL is `POST /auth/v1/providers/{id}/callback` (handles GET redirect from upstream)
- [ ] Double-state: IdP's state for the app, separate state for upstream, correlated server-side
- [ ] Provider errors handled gracefully (user sees "login failed", not a panic)
- [ ] Migrations apply cleanly
- [ ] `social` feature flag gates all new code

---

_Created: 2026-07-17_
