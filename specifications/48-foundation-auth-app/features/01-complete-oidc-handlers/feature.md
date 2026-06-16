---
feature: "Complete OIDC Handlers"
description: "Wire TokenService/UserService/ClientService to IdpHandlerCore stubs (authorize, token, userinfo, introspect, device_authorize)"
status: "pending"
priority: "high"
depends_on: ["spec-38 F10-F13"]
estimated_effort: "medium"
created: 2026-06-16
---

# Feature 01: Complete OIDC Handlers

## Description

Spec 38 Features 10–13 delivered **complete and tested** services and models.
The `IdpHandlerCore` in `handlers/core.rs` has stubs that return errors. This
feature wires the real implementations.

## Architecture change: IdpHandlerCore needs service fields + typed responses

**Current state (broken for extension):**
```rust
// handlers/core.rs
pub struct IdpHandlerCore {
    config: Arc<IdpConfig>,
}
// Returns serde_json::Value — no HTTP status codes
pub async fn authorize(...) -> Result<serde_json::Value, IdpError>;
```

**New state (supports all F01–F09):**
```rust
pub struct IdpHandlerCore {
    config: Arc<IdpConfig>,
    db: StorageProvider,
    token_service: Arc<TokenService>,
    user_service: Arc<UserService>,
    client_service: Arc<ClientService>,
    session_service: Arc<SessionService>,
}

/// Typed response that carries HTTP status code.
pub struct HandlerResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub headers: Vec<(String, String)>,  // for Set-Cookie, etc.
}

pub async fn authorize(...) -> Result<HandlerResponse, IdpError>;
```

The `ServeAdapter` is updated to extract status/headers from `HandlerResponse`
instead of hardcoding 200. This enables 202 (accepted), 204 (no content),
206 (partial/ToS required), 409 (conflict), 423 (locked), 429 (rate limited).

## IdpError expansion

The existing `IdpError` has only `Internal`, `BadRequest`, `Unauthorized`.
Extended to cover all cases used by F01–F09:

```rust
pub enum IdpError {
    Internal(String),           // 500
    BadRequest(String),         // 400
    Unauthorized(String),       // 401
    Forbidden(String),          // 403
    NotFound(String),           // 404
    Conflict(String),           // 409
    Locked(String),             // 423
    TooManyRequests(i64),       // 429, retry_not_before timestamp
}
```

Each variant maps to a specific HTTP status in `ServeAdapter`.

## serve_adapter.rs changes

```rust
impl Serve for ServeAdapter {
    fn serve(&self, bag: &ContextBag, req: Request, conn: &mut Connection) -> ConnectionResult {
        let response = core.dispatch(&bag, &req).await?;
        // response.status, response.body, response.headers
        respond::json_with_status(&mut conn, response.status, &response.body, &response.headers)
    }
}
```

## Current state (`handlers/core.rs`)

```rust
// discovery() — works (returns JSON from IdpConfig)
// jwks() — works (returns public key from IdpConfig)
// authorize() — Err("requires user session and storage backend")
// token() — Err("requires storage backend")
// userinfo() — Err("Bearer token required")
// introspect() — Ok({"active": false})
// device_authorize() — Err("requires storage backend")
```

Services available: `TokenService`, `UserService`, `SessionService`,
`ClientService` — all in `server/services/`. Models available: `User`,
`OAuthClient`, `AuthorizationCode`, `DeviceCode`, `RefreshToken`.

## authorize() — real implementation

```rust
impl IdpHandlerCore {
    pub async fn authorize(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // 1. Extract query params: client_id, redirect_uri, response_type, scope, state,
        //    code_challenge, code_challenge_method, nonce
        // 2. Validate required params → 400
        // 3. Look up client via ClientService → 400 if not found
        // 4. Validate redirect_uri → 400
        // 5. Validate response_type=code → 400
        // 6. Check PKCE requirement → 400 if missing code_challenge
        // 7. Check session cookie (via SessionService):
        //    - If authenticated:
        //      - Generate AuthorizationCode (uses existing model constructor)
        //      - Store in DB via QueryStore
        //      - Return: {"status": "authorized", "code": "...", "state": "...", "redirect_uri": "..."}
        //    - If not authenticated:
        //      - Return: {"status": "login_required", "login_url": "/auth/v1/login", "return_to": "...", "client_id": "..."}
    }
}
```

## token() — real implementation

```rust
impl IdpHandlerCore {
    pub async fn token(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // Parse form-urlencoded body → TokenRequest (existing type)
        // Match grant_type:
        //   "authorization_code" → validate code + PKCE → generate tokens
        //   "refresh_token" → validate + check rotation → generate new tokens
        //   "client_credentials" → generate access token only
        //   _ → 400 unsupported_grant_type
    }
}
```

## userinfo() — real implementation

```rust
impl IdpHandlerCore {
    pub async fn userinfo(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // 1. Extract Authorization: Bearer header
        // 2. Validate JWT via JwtVerifier
        // 3. Check openid scope → 403
        // 4. Look up user by sub claim
        // 5. Return claims JSON: {sub, email, name, ...}
    }
}
```

## introspect() — real implementation

```rust
impl IdpHandlerCore {
    pub async fn introspect(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // 1. Parse form: token, client_id, client_secret
        // 2. Validate client credentials
        // 3. Try JWT validation → active/inactive
        // 4. Try refresh token lookup → active/inactive
    }
}
```

## device_authorize() — real implementation

```rust
impl IdpHandlerCore {
    pub async fn device_authorize(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // 1. Parse form: client_id, scope
        // 2. Validate client
        // 3. Generate DeviceCode (existing model constructor)
        // 4. Store in DB
        // 5. Return device_code, user_code, verification_uri, expires_in, interval
    }
}
```

## Module changes

- `backends/foundation_auth/src/server/handlers/core.rs` — replace 5 stub methods,
  add `HandlerResponse` type, expand `IdpError`, add service fields to `IdpHandlerCore`
- `backends/foundation_auth/src/server/handlers/serve_adapter.rs` — extract status/headers
  from `HandlerResponse` instead of hardcoding 200
- `backends/foundation_auth/src/server/idp_server.rs` — wire services into `IdpHandlerCore`
  construction, fix discovery document paths to match served paths

## Routing fix (critical)

The current `IdpHandlerCore::dispatch()` uses `path.ends_with()` matching which
doesn't scale and can't extract path parameters. Replaced with a prefix-based
dispatch table:

```rust
impl IdpHandlerCore {
    pub async fn dispatch(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<HandlerResponse, IdpError> {
        let path = req.request_url.url.as_str();
        let path = path.split('?').next().unwrap_or(path);
        let method = req.method;

        match (method, path) {
            (GET, p) if p.ends_with("/.well-known/openid-configuration") => self.discovery(bag, req).await,
            (GET, p) if p.ends_with("/.well-known/jwks.json") => self.jwks(bag, req).await,
            (GET, p) if p.ends_with("/authorize") && !p.contains("/device/") => self.authorize(bag, req).await,
            (POST, p) if p.ends_with("/token") && !p.contains("/introspect") => self.token(bag, req).await,
            (GET, p) if p.ends_with("/userinfo") => self.userinfo(bag, req).await,
            (POST, p) if p.ends_with("/introspect") => self.introspect(bag, req).await,
            (POST, p) if p.ends_with("/device/authorize") => self.device_authorize(bag, req).await,
            // F02–F09 routes added by their respective features
            _ => Err(IdpError::NotFound(format!("Unknown endpoint: {path}"))),
        }
    }
}
```

For path-parameter endpoints (F03–F09), the dispatch uses pattern matching
on path segments:

```rust
// /auth/v1/users/{id}/reset — extract user_id from path
if let Some(id) = path.strip_prefix("/auth/v1/users/").and_then(|rest| {
    rest.split('/').next()
}) {
    if rest.ends_with("/reset") {
        return self.reset_password(bag, req, id).await;
    }
}
```

## Discovery document path fix

The current `OidcDiscoveryDocument::from_config()` builds paths like
`/introspect` and `/device/authorize` (relative to issuer). These must match
the actual served paths under the configured prefix. Fixed by making
discovery document paths relative to the `IdpConfig`'s route prefix:

```rust
impl OidcDiscoveryDocument {
    pub fn from_config(config: &IdpConfig, prefix: &str) -> Self {
        let base = config.issuer_url.trim_end_matches('/');
        // All endpoint paths use the same prefix as register_routes()
        Self {
            authorization_endpoint: format!("{base}{prefix}/authorize"),
            token_endpoint: format!("{base}{prefix}/token"),
            // ...
        }
    }
}
```

## Testing

- Authorize: valid params + session → auth code in DB, 200 response
- Authorize: no session → login_required response
- Token: valid auth code + PKCE → 200 with tokens
- Token: wrong PKCE → 400
- Token: replayed refresh token → 401
- UserInfo: valid bearer token → claims
- UserInfo: no token → 401
- Introspect: active token → {"active": true}
- Introspect: expired token → {"active": false}
- Device authorize: valid client → device_code + user_code
- Discovery document URLs match actual served paths
- ServeAdapter returns correct HTTP status codes (not just 200)
