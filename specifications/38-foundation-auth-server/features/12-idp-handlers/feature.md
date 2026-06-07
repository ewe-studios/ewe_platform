---
feature: "IdP Handlers"
description: "OIDC endpoints: authorize, token, userinfo, jwks, discovery, introspect, device_authorize"
status: "pending"
priority: "high"
depends_on: ["09-idp-server", "10-idp-models", "11-idp-services"]
estimated_effort: "large"
created: 2026-06-05
last_updated: 2026-06-07
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 1
  total: 1
  completion_percentage: 0%
---

# Feature 12: IdP Handlers

## Description

HTTP handler implementations for all OIDC and auth endpoints.

Each endpoint has one **async core method** in `IdpHandlerCore`. `IdpHandlerCore`
implements `ServeCf` and `ServeWeb` directly (both are async). Only `Serve` (native,
sync) needs a separate adapter struct with valtron bridging.

```
                    ┌─────────────────────────────────┐
                    │     IdpHandlerCore (async)      │
                    │                                 │
                    │ async fn authorize(...) -> ...  │  ← implements ServeCf
                    │ async fn token(...) -> ...      │  ← implements ServeWeb
                    │ async fn userinfo(...) -> ...   │
                    │ ... all endpoints ...           │
                    └────────────┬────────────────────┘
                                 │
                    ┌────────────┘
                    ▼
          ┌─────────────────┐
          │  ServeAdapter    │  ← only adapter struct needed
          │  (native sync)   │     valtron: from_future + execute + collect_one
          │  wraps Arc<Core> │
          │  valtron bridge  │
          └─────────────────┘
```

- **Serve** (native) — sync trait, bridges to async core via valtron
  (`from_future` + `execute` + `collect_one`). Only trait that needs an adapter.
- **ServeCf** (Cloudflare Workers) — async trait, `impl ServeCf for IdpHandlerCore`
- **ServeWeb** (wasm/browser) — async trait, `impl ServeWeb for IdpHandlerCore`

No separate adapter structs for ServeCf or ServeWeb — they call the async core
directly. Only `ServeAdapter` exists for the native sync bridge.

## Modules

`backends/foundation_auth/src/server/handlers/` — directory containing handler files

### Core (`handlers/core.rs`)

```rust
/// Shared async business logic for all IdP endpoints.
pub struct IdpHandlerCore {
    config: Arc<IdpConfig>,
    db: StorageProvider,
    token_service: Arc<TokenService>,
    user_service: Arc<UserService>,
    client_service: Arc<ClientService>,
    session_service: Arc<SessionService>,
}

impl IdpHandlerCore {
    pub fn new(config: Arc<IdpConfig>, db: StorageProvider) -> Self;

    // All endpoints are async — one method per endpoint
    pub async fn discovery(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn authorize(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn login(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn mfa(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn token(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn userinfo(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn jwks(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn introspect(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
    pub async fn device_authorize(&self, bag: &ContextBag, req: &Request) -> Result<Response, IdpError>;
}
```

### Native Adapter (`handlers/serve_adapter.rs`)

The only adapter struct. Wraps `Arc<IdpHandlerCore>` and bridges async core to sync trait.

```rust
pub struct ServeAdapter { core: Arc<IdpHandlerCore> }

impl Serve for ServeAdapter {
    fn serve(&self, bag: &ContextBag, req: Request, conn: &mut Connection) -> ConnectionResult {
        let core = Arc::clone(&self.core);
        let bag = bag.clone();
        let req = req.clone();
        let task = from_future(async move {
            core.dispatch(&bag, &req).await  // dispatch() routes to the right endpoint
        });
        let stream = execute(task, None)?;
        collect_one(stream)
            .ok_or_else(|| IdpError::NoResult)?
            .to_connection_response(conn)
    }
}
```

### ServeCf + ServeWeb — Direct Implementation on IdpHandlerCore

Both are async traits — no adapter structs needed. `IdpHandlerCore` implements them directly:

```rust
impl ServeCf for IdpHandlerCore {
    async fn serve_cf(
        &self, bag: &ContextBag, req: Request, conn: &mut CfConnection,
    ) -> CfConnectionResult {
        self.dispatch(bag, &req).await.to_cf_connection_response(conn)
    }
}

impl ServeWeb for IdpHandlerCore {
    async fn serve_web(
        &self, bag: &ContextBag, req: Request, conn: &mut WebConnection,
    ) -> WebConnectionResult {
        self.dispatch(bag, &req).await.to_web_connection_response(conn)
    }
}
```

## Endpoint Details

### Discovery (`async fn discovery`)

**GET /.well-known/openid-configuration**

- Reads `IdpConfig` from ContextBag
- Returns full OIDC discovery JSON document
- No authentication required
- CORS enabled for this endpoint

### Authorize (`async fn authorize`)

**GET /oidc/authorize**

Query parameters: `client_id`, `redirect_uri`, `response_type`, `scope`, `state`, `code_challenge`, `code_challenge_method`, `nonce`

Flow:
1. Validate required params: `client_id`, `redirect_uri`, `response_type=code`, `scope`, `state`
2. Look up client by `client_id` → 400 if not found
3. Validate `redirect_uri` against client's registered URIs → 400 if not allowed
4. Validate `response_type` is `code` → 400 if not
5. If PKCE required and no `code_challenge` → 400
6. Check session cookie for authenticated session
   - If authenticated:
     - Generate authorization code (random, 64 bytes)
     - Store code in DB with user_id, client_id, redirect_uri, code_challenge, scope, nonce, expires_at (10 min)
     - Return JSON: `{"status": "authorized", "code": "...", "state": "...", "redirect_uri": "..."}`
   - If not authenticated:
     - Return JSON: `{"status": "login_required", "login_url": "/auth/v1/login", "return_to": "<original_url>"}`

### Login (`async fn login`)

**POST /auth/v1/login**

Body: `{"email": "...", "password": "..."}`

Flow:
1. Parse JSON body → 400 if invalid
2. Look up user by email → 401 if not found
3. Check if account locked → 423 if locked
4. Verify password via Argon2id → 401 if wrong
   - On wrong password: increment failed attempts, lock if threshold exceeded
5. On correct password: reset failed attempts
6. Check if MFA enabled for user
   - If MFA: create TOTP challenge, return `{"status": "mfa_required", "challenge_id": "..."}`
   - If no MFA: create session, return `{"status": "authenticated", "session_id": "...", "cookie": {...}}`

### MFA (`async fn mfa`)

**POST /auth/v1/mfa**

Body: `{"challenge_id": "...", "code": "123456"}`

Flow:
1. Look up challenge → 400 if not found or expired
2. Verify TOTP code against stored secret → 401 if wrong
3. On success: create session, return `{"status": "authenticated", "session_id": "...", "cookie": {...}}`
4. Include cookie also in response for clients that prefer to pick it up from there as well.

### Token (`async fn token`)

**POST /oidc/token**

Body (form-urlencoded): varies by grant type

**Authorization code grant:**
1. Validate `grant_type=authorization_code`, `code`, `redirect_uri`, `client_id`, `client_secret`
2. Look up client → 401 if not found or secret wrong
3. Look up auth code → 400 if not found or expired
4. Verify code is for this client and redirect_uri → 400 if mismatch
5. Verify PKCE if present → 400 if wrong
6. Delete auth code (single use)
7. Look up user from code
8. Generate tokens via TokenService
9. Store refresh token hash
10. Return: `{"access_token": "...", "token_type": "Bearer", "expires_in": 900, "refresh_token": "...", "id_token": "..."}`

**Refresh token grant:**
1. Validate `grant_type=refresh_token`, `refresh_token`, `client_id`, `client_secret`
2. Look up client → 401 if not found
3. Look up refresh token hash → 400 if not found
4. Verify refresh token → 401 if wrong
5. Check if token was already rotated (replay attack) → 401
6. Generate new tokens with new refresh token
7. Mark old refresh token as rotated
8. Store new refresh token hash
9. Return new token pair

**Client credentials grant:**
1. Validate `grant_type=client_credentials`, `client_id`, `client_secret`
2. Look up and validate client
3. Generate access token (no ID token, no refresh token for client credentials)
4. Return: `{"access_token": "...", "token_type": "Bearer", "expires_in": 900, "scope": "..."}`

### UserInfo (`async fn userinfo`)

**GET /oidc/userinfo**

1. Extract `Authorization: Bearer <token>` header → 401 if missing
2. Verify JWT signature using `JwtVerifier` → 401 if invalid
3. Check token has `openid` scope → 403 if not
4. Look up user by `sub` claim
5. Return user claims JSON: `{"sub": "...", "email": "...", "name": "..."}`

### JWKS (`async fn jwks`)

**GET /oidc/jwks**

1. Get public key from `IdpConfig.signing_key`
2. Return JWKS JSON: `{"keys": [{"kty": "OKP", "crv": "Ed25519", "x": "...", "kid": "...", "use": "sig", "alg": "EdDSA"}]}`
3. CORS enabled for this endpoint

### Introspect (`async fn introspect`)

**POST /oidc/introspect**

Body: `token=...&client_id=...&client_secret=...`

1. Validate resource server credentials → 401 if wrong
2. Try to verify token as JWT:
   - If valid JWT: return `{"active": true, "sub": "...", "scope": "..."}`
   - If expired JWT: return `{"active": false}`
3. Try to look up as refresh token:
   - If valid: return `{"active": true, ...}`
4. Return `{"active": false}`

### Device Authorize (`async fn device_authorize`)

**POST /oidc/device_authorization**

1. Validate `client_id`, `scope`
2. Generate device_code (64 bytes random, base64url)
3. Generate user_code (8 chars, format `XXXX-XXXX`)
4. Store device code in DB with expires_at (10 min), interval (5s)
5. Return: `{"device_code": "...", "user_code": "WXYZ-1234", "verification_uri": "/device", "verification_uri_complete": "/device?code=WXYZ-1234", "expires_in": 600, "interval": 5}`

## Response Helpers

```rust
/// Return a JSON error response.
fn error_response(status: u16, error: &str, description: &str) -> Response {
    let body = serde_json::json!({"error": error, "error_description": description});
    Response::json(status, body)
}

/// Return a JSON success response.
fn success_response(status: u16, body: &impl Serialize) -> Response {
    Response::json(status, body)
}
```

## Dependencies

- Existing: `foundation_http` (Serve, ServeCf, ServeWeb traits, ContextBag)
- Existing: all server models and services

## Testing

- Discovery: GET /.well-known → 200, correct JSON structure
- Authorize: missing client_id → 400
- Authorize: valid params, not authenticated → 200 login_required
- Authorize: valid params, authenticated → 200 authorized with code
- Login: wrong password → 401
- Login: correct password → 200 authenticated
- Token: valid auth code + PKCE → 200 with tokens
- Token: wrong PKCE verifier → 400
- Token: expired auth code → 400
- Token: refresh token replay → 401
- UserInfo: no bearer token → 401
- UserInfo: valid token → 200 with claims
- JWKS: GET /oidc/jwks → 200 with Ed25519 key
- Introspect: active token → `{"active": true}`
- Introspect: expired token → `{"active": false}`
- Device authorize: valid client → 200 with device_code + user_code
