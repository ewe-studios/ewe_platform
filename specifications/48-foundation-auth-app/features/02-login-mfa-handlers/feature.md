---
feature: "Login + MFA + Logout Handlers"
description: "Add login + MFA + logout routes/handlers, session cookie creation, CSRF protection"
status: "pending"
priority: "high"
depends_on: ["01-complete-oidc-handlers"]
estimated_effort: "medium"
created: 2026-06-16
---

# Feature 02: Login + MFA + Logout Handlers

## Description

Spec 38 defined `POST /auth/v1/login` and `POST /auth/v1/mfa` in the endpoint
spec but never registered routes or implemented handlers. This feature adds them.

## Route registration (extend `idp_server.rs`)

```rust
impl IdpServer {
    pub fn register_routes(app: &mut HttpApp<Arc<dyn Serve>>, prefix: &str) {
        let p = prefix.trim_end_matches('/');
        // Existing OIDC routes
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/.well-known/openid-configuration"));
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/.well-known/jwks.json"));
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/authorize"));
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("{p}/token"));
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/userinfo"));
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("{p}/introspect"));
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("{p}/device/authorize"));
        // NEW — auth endpoints (using /auth/v1 prefix, separate from /oidc)
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("/auth/v1/oidc/authorize"));  // unified login
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("/auth/v1/mfa"));
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("/auth/v1/oidc/logout"));
    }
}
```

## login() handler — unified multi-step flow

**POST /auth/v1/oidc/authorize**

This endpoint handles the rauthy multi-step login flow:

**Step 1: Email only** — client sends `{email, client_id, redirect_uri, ...}`
- If user has no password set → returns "need password" status, UI reveals password input
- If user exists → checks MFA, creates session or returns MFA challenge

**Step 2: Email + password** — client sends `{email, password, client_id, ...}`
- Verifies password, creates session or returns MFA challenge

```rust
impl IdpHandlerCore {
    pub async fn login(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<HandlerResponse, IdpError> {
        // 1. Parse JSON body: {email, password?, client_id, redirect_uri, ...}
        // 2. Look up user by email via UserService/QueryStore → 401 if not found
        // 3. Check account locked (User::is_locked) → 423 with x-retry-not-before
        // 4. If password provided:
        //    - Verify via verify_password() (Argon2id) → 401 if wrong
        //    - On wrong: user.record_failed_attempt() → persist
        //    - On correct: user.reset_failed_attempts() → persist
        // 5. If no password provided and user has no password → return "password_required"
        // 6. Check MFA:
        //    - If user has TOTP: create challenge, return 200 with mfa_required
        // 7. Check ToS (F07): if ToS needs re-acceptance → return 206 with tos_await_code
        // 8. Create session via SessionService → (Session, Vec<Cookie>)
        // 9. Return 202 with Location header for redirect
    }
}
```

Response status codes (matching spec 42 UI expectations):

| Status | Meaning | Body |
|---|---|---|
| 202 | Authenticated | `{"status": "authenticated"}` + Location header |
| 200 | MFA/WebAuthn required | `{"status": "mfa_required", "challenge_id": "...", "type": "totp"}` |
| 205 | Needs profile update | `{"status": "profile_update_required"}` |
| 206 | ToS update needed | `{"status": "tos_required", "tos_await_code": "..."}` |
| 400 | Bad request | `{"error": "..."}` |
| 401 | Invalid credentials | `{"status": "invalid_credentials", "attempts_remaining": N}` |
| 403 | Forbidden (password expired) | `{"error": "password_expired"}` |
| 406 | Client forces MFA, user has none | `{"error": "mfa_required_by_client"}` |
| 423 | Account locked | `{"error": "account_locked"}` |
| 429 | Too many requests | `{"error": "too_many_requests"}` + x-retry-not-before |

## mfa() handler

**POST /auth/v1/mfa**

```rust
impl IdpHandlerCore {
    pub async fn mfa(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<HandlerResponse, IdpError> {
        // 1. Parse JSON body: {challenge_id, code}
        // 2. Look up challenge → 400 if not found/expired
        // 3. Verify TOTP code via TOTPSecret → 401 if wrong
        // 4. Create session → return 202 with cookie
    }
}
```

## logout() handler

**POST /auth/v1/oidc/logout**

```rust
impl IdpHandlerCore {
    pub async fn logout(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<HandlerResponse, IdpError> {
        // 1. Extract session cookie
        // 2. Revoke session via SessionService
        // 3. Return 200 {"status": "logged_out"} with Set-Cookie header to clear
    }
}
```

## Dev mode browser_id

**POST /auth/v1/dev/browser_id**

Generates a browser ID for PoW correlation: `{"browser_id": "uuid"}`.

## Module changes

- `backends/foundation_auth/src/server/handlers/core.rs` — add login(), mfa(), logout() (return HandlerResponse)
- `backends/foundation_auth/src/server/idp_server.rs` — register new routes under `/auth/v1`
- `backends/foundation_auth/src/server/services/session_service.rs` — add create_cookie()

## Testing

- Login with valid credentials → 202 + Location header
- Login email only, no password set → 200 with "password_required"
- Login with wrong password → 401 invalid_credentials + attempts_remaining
- Login for locked account → 423
- MFA with correct TOTP → 202 + cookie
- MFA with wrong code → 401
- Logout → session revoked, cookie cleared
- Account lockout after 5 consecutive failures → 423
