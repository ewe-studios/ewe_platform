---
feature: "04-social-login-flow"
spec: "57-foundation-social-and-keychain"
depends: "01-provider-model, 02-provider-migrations, 03-oauth-upstream-client"
status: "complete"
---

# Feature 04: Social Login Flow (Redirect + Callback)

## Goal

Implement the redirect-based social login flow: user clicks a provider, gets redirected to the upstream auth URL, returns via callback, and the IdP mints its own auth code for the app.

## WHAT

### Flow Overview

```
App → IdP /authorize?provider=google
     → Store UpstreamAuthSession (KV, TTL)
     → 302 redirect → Google authorize URL

User authenticates with Google
     → Google redirects to /auth/v1/providers/google/callback?code=XXX&state=YYY

Callback handler:
     1. Look up UpstreamAuthSession by state
     2. Validate state matches
     3. Exchange code with Google (feature 03)
     4. Fetch user profile from Google
     5. Create/link user (feature 05)
     6. Create IdP authorization code (migration 017)
     7. Delete UpstreamAuthSession
     8. 302 redirect → app's redirect_uri?code=OUR_CODE&state=APP_STATE
```

### UpstreamAuthSession (KV Store)

```rust
pub struct UpstreamAuthSession {
    pub upstream_state: String,     // the state we sent to Google
    pub app_state: String,          // the state the app sent us
    pub app_client_id: String,      // which app initiated
    pub app_redirect_uri: String,   // where to redirect after
    pub app_pkce_challenge: Option<String>,  // app's PKCE challenge
    pub app_nonce: Option<String>,  // app's nonce
    pub app_scopes: String,         // scopes the app requested
    pub provider_id: String,        // which upstream provider
    pub our_nonce: String,          // nonce for upstream ID token validation
    pub our_code_verifier: String,  // PKCE verifier for upstream exchange
    pub created_at: i64,
    pub expires_at: i64,
}
```

Stored in `KeyValueStore` with key `upstream_auth:{upstream_state}` and TTL = 600s.

### Handlers Added to IdpHandlerCore

#### `social_authorize` — extends existing `authorize` handler

When `provider` query param is present:
1. Validate provider exists and is active
2. Generate `upstream_state`, `our_nonce`, `our_code_verifier`
3. Store `UpstreamAuthSession` in KV
4. Build upstream authorize URL (feature 03)
5. Return redirect response (302) to upstream URL

When `provider` is absent: existing behavior unchanged.

#### `provider_callback` — `GET /auth/v1/providers/{id}/callback`

1. Extract `code` and `state` from query params
2. Look up `UpstreamAuthSession` by state
3. If not found → redirect to app with `error=invalid_request&error_description=Unknown+session`
4. Exchange code with upstream provider
5. Fetch user profile
6. Provision user (feature 05)
7. Create IdP authorization code
8. Delete session from KV
9. Redirect to app's `redirect_uri?code=OUR_AUTH_CODE&state=APP_STATE`

#### Error path

Any failure → redirect to `app_redirect_uri` with:
```
?error=provider_error&error_description=Google+login+failed&state=APP_STATE
```

### Route Registration

In `IdpServer::register_routes`:
```rust
// Social login — extends /authorize with optional ?provider=
// (no new route, modify existing authorize handler)

// Provider callbacks
app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/providers/{id}/callback");
```

### Dispatch Addition

In `IdpHandlerCore::dispatch`:
```rust
else if path.contains("/auth/v1/providers/") && path.ends_with("/callback") {
    self.provider_callback(bag, req).await
}
```

## HOW

### Files to modify

- `backends/foundation_auth/src/server/handlers/core.rs` — add `social_authorize` logic to `authorize`, add `provider_callback`
- `backends/foundation_auth/src/server/mod.rs` — re-export new types

### Files to create

- `backends/foundation_auth/src/server/models/upstream_session.rs` — UpstreamAuthSession

### Key Design Details

**The authorize handler branches on `provider` param.** If present, it executes the social login flow. If absent, it executes the existing email/password login flow. This keeps the routing simple — one endpoint, two paths.

**PKCE for the upstream flow.** The IdP generates its own `code_verifier`/`code_challenge` for the upstream exchange, independent of the app's PKCE. This means the IdP validates PKCE twice: once with the upstream provider, once when the app exchanges the IdP's auth code.

**Session cookie on callback.** After successful social login, the IdP should also create a session cookie for the user's browser (same as password login). This enables subsequent logins without re-authenticating with the upstream provider. The `authorize` handler checks for this cookie before starting a new upstream flow.

---

_Created: 2026-07-17_
