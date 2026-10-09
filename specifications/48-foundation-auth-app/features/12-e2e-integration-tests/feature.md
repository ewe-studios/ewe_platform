---
feature: "E2E Integration Tests"
description: "Full end-to-end user story validation — every auth flow exercised against a real IdP server with real Turso storage"
status: "proposed"
priority: "critical"
depends_on: ["01-complete-oidc-handlers", "02-login-mfa-handlers", "03-user-registration", "04-password-reset", "05-proof-of-work", "06-webauthn-fido2", "07-terms-of-service", "08-template-config-api", "09-account-management"]
estimated_effort: "large"
created: 2026-06-17
---

# Feature 12: E2E Integration Tests

## Description

Each test is a **complete user story** — not just "endpoint returns 200" but
"here's what the user does, here's what the server responds, here's what state
changed, here's what the next step sees." Tests spin up a real IdP with real
Turso storage. No server mocks.

## Story Format

Each scenario follows **Why → What → How → Then → Verify**:

| Field | Purpose |
|---|---|
| **Why** | The user's goal / motivation |
| **What** | The user action(s) taken |
| **How** | The HTTP requests/responses exchanged |
| **Then** | The state change on the server |
| **Verify** | The assertions that prove it worked |

## User Stories

---

### Story 1: OIDC Discovery — "I'm a new developer, I need to configure my app"

**Why:** Developer integrates their app with the IdP. They need the OIDC
configuration document to know endpoints, supported algorithms, and capabilities.

**What:** Developer fetches `/.well-known/openid-configuration` from the IdP.

**How:**
1. `GET /idp/.well-known/openid-configuration`
2. → `200 OK` with `OidcDiscoveryDocument` JSON

**Then:** Discovery document is served with correct issuer URL and all endpoint
paths match the server's configured prefix.

**Verify:**
- `issuer` = configured issuer URL (trailing slash stripped)
- `authorization_endpoint` = `{issuer}/idp/authorize`
- `token_endpoint` = `{issuer}/idp/token`
- `userinfo_endpoint` = `{issuer}/idp/userinfo`
- `jwks_uri` = `{issuer}/idp/.well-known/jwks.json`
- `introspection_endpoint` = `{issuer}/idp/introspect`
- `device_authorization_endpoint` = `{issuer}/idp/device/authorize`
- `response_types_supported` contains `"code"`
- `grant_types_supported` contains `"authorization_code"`, `"refresh_token"`, `"client_credentials"`, `"urn:ietf:params:oauth:grant-type:device_code"`
- `scopes_supported` contains `"openid"`
- `id_token_signing_alg_values_supported` contains `"EdDSA"`
- `code_challenge_methods_supported` contains `"S256"`

---

### Story 2: JWKS — "I need the public key to verify JWT signatures"

**Why:** Resource server receives JWTs from the IdP and must verify their
signatures. It fetches the JWKS to get the signing public key.

**What:** Developer fetches `/.well-known/jwks.json` from the IdP.

**How:**
1. `GET /idp/.well-known/jwks.json`
2. → `200 OK` with `{"keys": [{"kty":"OKP","crv":"Ed25519","x":"...","kid":"default","use":"sig","alg":"EdDSA"}]}`

**Then:** JWKS contains exactly one Ed25519 key with `use=sig` and `alg=EdDSA`.

**Verify:**
- `keys` array has exactly 1 element
- `kty` = `"OKP"`, `crv` = `"Ed25519"`, `use` = `"sig"`, `alg` = `"EdDSA"`
- `kid` = `"default"`
- `x` field is present (public key material)
- Same key returned on repeated requests (key stability)

---

### Story 3: New User Registration — "I want to create an account"

**Why:** User discovered the service and wants to sign up.

**What:** User fills in email + password on registration form, submits.

**How:**
1. User requests PoW challenge: `GET /auth/v1/pow` → `{challenge, difficulty}`
2. User computes PoW nonce locally
3. User submits: `POST /auth/v1/users/register` with `{email, password, pow_challenge, pow_nonce}`
4. → `200 OK` with `{user_id, email}`

**Then:**
- User record created in `users` table with Argon2id-hashed password
- `email_verified` = `false`
- `created_at` = now
- Password policy enforced (reject weak passwords)

**Verify:**
- Registration with strong password → 200, returns user_id
- Registration with weak password → 400, lists policy violations
- Registration without PoW → 400 or PoW required error
- Registration with duplicate email → 409 conflict
- User exists in DB: `find_user_by_email(email)` returns the user
- Password hash starts with `$argon2id$`

---

### Story 4: Login with Password — "I want to access my account"

**Why:** User has an account and wants to authenticate.

**What:** User enters email + password on login form, submits.

**How:**
1. User requests PoW challenge: `GET /auth/v1/pow` → solves it
2. User submits: `POST /auth/v1/oidc/authorize` with `{email, password, pow_challenge, pow_nonce}`
3. Server verifies PoW, looks up user by email, verifies Argon2id hash
4. → `200 OK` with `{session_token}` + `Set-Cookie` header

**Then:**
- Session created in KV store with user_id
- Session cookie set with HttpOnly, Secure flags
- Auth state transitions to `Authenticated`

**Verify:**
- Valid credentials → 200, Set-Cookie header present
- Response contains session_token
- Cookie has HttpOnly flag
- Subsequent request with cookie → user is authenticated

---

### Story 5: Login with Wrong Password — "I forgot my password"

**Why:** User mistypes password or genuinely forgot it.

**What:** User enters email + wrong password.

**How:**
1. `POST /auth/v1/oidc/authorize` with `{email, password: "wrong"}`
2. → `401 Unauthorized` with `{error: "invalid_credentials"}`

**Then:**
- Failed login attempt counter incremented on user record
- If attempts reach max (5), account is locked
- No session created

**Verify:**
- Wrong password → 401, error message
- After 5 failures: `find_user_by_email` shows `locked_until` in the future
- Login attempt on locked account → 423 Locked
- Error response does NOT reveal whether email exists (security)

---

### Story 6: Password Reset — "I need to reset my forgotten password"

**Why:** User forgot password, needs to set a new one without knowing the old one.

**What:** User requests reset → receives magic link → clicks link → sets new password.

**How:**
1. `POST /auth/v1/users/request_reset` with `{email}`
2. → `200 OK` (always, even if email doesn't exist — security)
3. Server creates reset token, stores it, "sends" magic link email
4. User receives link with token: `/auth/v1/users/reset?token=xyz`
5. `PUT /auth/v1/users/{user_id}/reset` with `{token, new_password}`
6. → `200 OK` with `{status: "password_updated"}`

**Then:**
- Reset token consumed (one-time use)
- Password hash updated in DB
- Old password no longer works, new password works

**Verify:**
- Request reset → 200 (even for nonexistent email)
- Reset with valid token + new password → 200
- Login with new password → succeeds
- Login with old password → fails
- Reuse same reset token → 400, token expired/consumed

---

### Story 7: Logout — "I want to sign out"

**Why:** User is done and wants to end their session.

**What:** User clicks logout.

**How:**
1. `POST /auth/v1/oidc/logout` with session cookie
2. → `200 OK` with `Set-Cookie: session=; Max-Age=0; Path=/`

**Then:**
- Session revoked in KV store
- Cookie cleared (expired)
- Auth state transitions to `Unauthenticated`

**Verify:**
- Logout → 200, Set-Cookie clears session
- Subsequent request with old cookie → 401 Unauthorized
- Session removed from KV store

---

### Story 8: Session Persistence — "I navigate between pages, stay logged in"

**Why:** User expects their login to persist across page loads.

**What:** User logs in, makes multiple requests with the same cookie.

**How:**
1. Login → receive session cookie
2. `GET /idp/userinfo` with cookie → `200 OK` with user claims
3. `GET /auth/v1/users/{id}` with cookie → `200 OK` with profile
4. Wait until session expires → next request → `401`

**Then:**
- Session is valid across multiple requests
- Session eventually expires based on configured TTL

**Verify:**
- Cookie-bearing requests → authenticated responses
- After expiry: `401 Unauthorized`
- Session TTL matches config (default 1 hour)

---

### Story 9: Concurrent Sessions — "I'm logged in on phone and laptop"

**Why:** User uses multiple devices simultaneously.

**What:** User logs in from two different clients.

**How:**
1. Client A: login → session_A
2. Client B: login → session_B (same user)
3. Both sessions active simultaneously
4. Revoke session_A → session_B still works

**Then:**
- Two independent sessions in KV store
- Revoking one doesn't affect the other
- "Revoke all sessions" kills both

**Verify:**
- Two sessions exist for same user
- Revoke session_A → session_A returns 401, session_B returns 200
- Revoke all → both return 401

---

### Story 10: OIDC Authorization Code Flow with PKCE — "My app wants to authenticate users"

**Why:** Third-party app uses OIDC to authenticate its users against this IdP.

**What:** App initiates PKCE flow: generate verifier/challenge → authorize → get code → exchange for tokens.

**How:**
1. App generates `code_verifier` (random string), computes `code_challenge = SHA256(verifier)`
2. App redirects user to: `GET /idp/authorize?client_id=app1&response_type=code&redirect_uri=https://app.com/cb&code_challenge=XYZ&code_challenge_method=S256&state=abc&scope=openid`
3. User authenticates → server creates authorization code
4. → `302 redirect` to `https://app.com/cb?code=AUTH_CODE&state=abc`
5. App POSTs to `/idp/token` with `grant_type=authorization_code&code=AUTH_CODE&redirect_uri=https://app.com/cb&client_id=app1&client_secret=SECRET&code_verifier=ORIGINAL_VERIFIER`
6. → `200 OK` with `{access_token, id_token, refresh_token, token_type: "Bearer", expires_in}`

**Then:**
- Authorization code stored in DB with PKCE challenge
- Code consumed on first use (one-time)
- Token set contains access_token, id_token, refresh_token

**Verify:**
- Auth code created → redirect to callback with code + matching state
- Token exchange with correct verifier → 200 with tokens
- access_token is non-empty JWT string
- id_token is non-empty JWT string with `sub` claim = user_id
- refresh_token is non-empty
- Auth code reused → 400, "invalid_grant"
- Wrong code_verifier → 400, "invalid_grant"

---

### Story 11: PKCE Enforcement — "Public clients must use PKCE"

**Why:** Public clients (SPAs, mobile apps) can't store secrets securely. PKCE
prevents authorization code interception attacks.

**What:** App tries to authorize without PKCE on a public client.

**How:**
1. `GET /idp/authorize?client_id=public_app&response_type=code&...` (no code_challenge)
2. → `400 Bad Request` with `{error: "bad_request", error_description: "PKCE required"}`

**Then:** Authorization denied for public client without PKCE.

**Verify:**
- Public client without code_challenge → 400
- Confidential client without code_challenge → allowed (if configured)
- With code_challenge → allowed for both

---

### Story 12: Refresh Token Rotation — "My access token expired, get me a new one"

**Why:** Access token expired, app uses refresh token to get new tokens without
user re-authentication.

**What:** App POSTs to token endpoint with refresh_token grant.

**How:**
1. App has refresh_token from initial token response
2. `POST /idp/token` with `grant_type=refresh_token&refresh_token=RT&client_id=app1&client_secret=SECRET`
3. → `200 OK` with `{access_token, id_token, refresh_token: NEW_RT}`
4. Old refresh_token is invalidated (rotation)

**Then:**
- New token set issued
- Old refresh_token marked as rotated (cannot be reused)

**Verify:**
- Valid refresh_token → 200, new tokens
- New refresh_token differs from old one (rotation)
- Old refresh_token reused → 401, "invalid_grant"

---

### Story 13: Client Credentials Grant — "My service needs an access token"

**Why:** Service-to-service authentication, no user involved.

**What:** Service authenticates with client_id + client_secret.

**How:**
1. `POST /idp/token` with `grant_type=client_credentials&client_id=service1&client_secret=SECRET&scope=openid`
2. → `200 OK` with `{access_token, token_type: "Bearer", expires_in}`
3. No id_token, no refresh_token (service-to-service doesn't need them)

**Then:** Access token issued for the client (no user context).

**Verify:**
- Valid client credentials → 200, access_token present
- id_token is empty/null
- refresh_token is empty/null
- Wrong client_secret → 401

---

### Story 14: Token Introspection — "Is this access token valid?"

**Why:** Resource server received a Bearer token and needs to check if it's still
valid before serving the request.

**What:** Resource server POSTs the token to the introspection endpoint.

**How:**
1. `POST /idp/introspect` with `token=ACCESS_TOKEN&client_id=resource_server&client_secret=SECRET`
2. → `200 OK` with `{active: true, sub: "user-123", scope: "openid", exp: ..., iat: ...}`
3. For expired/unknown token: `{active: false}`

**Then:** Token validity returned without exposing token contents.

**Verify:**
- Valid token → `active: true` with claims
- Expired token → `active: false`
- Unknown token → `active: false`
- Introspection without client auth → 401

---

### Story 15: Device Code Flow — "I want to authorize on my TV"

**Why:** User wants to sign in on a device with limited input (TV, CLI, IoT).

**What:** Client requests device code → user approves on another device → client polls for token.

**How:**
1. Client: `POST /idp/device/authorize` with `client_id=tv_app`
2. → `200 OK` with `{device_code, user_code: "ABCD-EFGH", verification_uri: "https://auth.example.com/device", expires_in, interval: 5}`
3. Client polls: `POST /idp/token` with `grant_type=urn:ietf:params:oauth:grant-type:device_code&device_code=XYZ&client_id=tv_app`
4. → `400` with `{error: "authorization_pending"}` (repeated every `interval` seconds)
5. User visits verification_uri, enters user_code, approves
6. Next poll → `200 OK` with `{access_token, refresh_token, id_token}`

**Then:**
- Device code stored with user_code mapping
- Polling returns `authorization_pending` until user approves
- After approval: token set issued

**Verify:**
- Device authorize → 200, user_code is 8 chars with dash
- Poll before approval → 400, error = "authorization_pending"
- After approval → 200, tokens present
- Expired device code → 400, error = "expired_token"

---

### Story 16: Proof of Work — "Prove you're not a bot"

**Why:** Server needs to prevent automated abuse of auth endpoints.

**What:** Client solves a computational puzzle before submitting auth request.

**How:**
1. `GET /auth/v1/pow` → `{challenge: "abc123", difficulty: 22, expires_in: 300}`
2. Client hashes `challenge + nonce` until hash has ≥22 leading zero bits
3. `POST /auth/v1/pow` with `{challenge: "abc123", solution: "12345"}`
4. → `200 OK` with `{valid: true}`

**Then:** Challenge marked as solved, cannot be reused.

**Verify:**
- Get challenge → returns challenge string, difficulty, expiry
- Valid solution → `{valid: true}`
- Invalid solution → `{valid: false}`
- Expired challenge → `{valid: false}`
- Reused solution → `{valid: false}` (solved flag prevents replay)

---

### Story 17: WebAuthn Passkey Registration — "I want to use my fingerprint"

**Why:** User wants phishing-resistant authentication via biometric passkey.

**What:** User initiates passkey registration, authenticator creates key pair.

**How:**
1. `POST /auth/v1/webauthn/register/start` with `{user_id, email}`
2. → `200 OK` with `{publicKey: {...}, session: "uuid"}`
3. Browser calls `navigator.credentials.create()` with challenge
4. Authenticator generates key pair, signs challenge
5. `POST /auth/v1/webauthn/register/finish` with `{session, response: {id, rawId, type: "public-key", response: {clientDataJSON, attestationObject}}}`
6. → `200 OK` with `{id, name: "Passkey"}`

**Then:**
- Passkey stored in DB (credential_id + CBOR-serialized public key)
- Registration session consumed (one-time)

**Verify:**
- Start → 200, publicKey contains challenge, rp, user, pubKeyCredParams
- Finish with valid response → 200, passkey in DB
- Finish with same session twice → 400, session consumed
- Passkey has credential_id and credential_public_key in storage

---

### Story 18: WebAuthn Passkey Login — "Authenticate with my passkey"

**Why:** User wants to log in without typing a password.

**What:** User selects passkey, authenticator signs challenge.

**How:**
1. `POST /auth/v1/webauthn/login/start` with `{user_id}`
2. → `200 OK` with `{publicKey: {...}, session: "uuid"}`
3. Browser calls `navigator.credentials.get()` with challenge
4. Authenticator signs with private key
5. `POST /auth/v1/webauthn/login/finish` with `{session, response: {id, rawId, type: "public-key", response: {clientDataJSON, authenticatorData, signature}}}`
6. → `202 Accepted` with `{status: "authenticated", user_id}`

**Then:**
- Authenticator signature verified against stored public key
- Counter updated if needed

**Verify:**
- Start → 200, challenge with allowCredentials list
- Finish with valid signature → 202, user_id returned
- Finish with tampered signature → 400, verification failed
- Unknown credential → 400, not found

---

### Story 19: Terms of Service — "Accept the terms before logging in"

**Why:** Legal requirement — user must accept current ToS version before
accessing the service.

**What:** User logs in, server checks ToS version, prompts acceptance if updated.

**How:**
1. `GET /auth/v1/tos/latest` → `{version: "2.0", content: "..."}`
2. User accepts: `POST /auth/v1/tos/accept` with `{user_id, version: "2.0"}`
3. → `200 OK` with `{accepted: true}`
4. User logs in with outdated ToS → `206 Partial Content` with `{tos_await_code: true, latest_version: "3.0"}`

**Then:**
- Acceptance recorded with user_id + version + timestamp
- Login flow checks last acceptance against latest ToS version

**Verify:**
- Fetch latest ToS → version + content
- Accept → acceptance in DB
- Re-accept same version → still accepted (idempotent)
- Login with outdated ToS → 206, prompts re-acceptance

---

### Story 20: Account Management — "Update my profile and change password"

**Why:** User wants to manage their account: see profile, update username,
change password, view sessions, delete account.

**What:** User accesses account page, makes changes.

**How:**
1. `GET /auth/v1/users/{id}` → `200 OK` with `{id, email, username, created_at}`
2. `PUT /auth/v1/users/{id}` with `{username: "new_name"}` → `200 OK`
3. `POST /auth/v1/users/{id}/change_password` with `{current_password, new_password}` → `200 OK`
4. `GET /auth/v1/users/{id}/sessions` → `200 OK` with `{sessions: [...]}`
5. `POST /auth/v1/users/{id}/revoke` → `200 OK` with `{status: "account_revoked"}`

**Then:**
- Profile updated in DB
- Password hash changed, old password rejected
- Account soft-deleted (deleted_at set)

**Verify:**
- Get user → returns email, username, created_at
- Update username → subsequent get shows new username
- Change password with correct current → login with new works, old fails
- Change password with wrong current → 400
- Revoke account → login fails (deleted_at set)

---

### Story 21: Template Config API — "UI needs to know how to render pages"

**Why:** Frontend UI dynamically configures forms, validation, and branding
based on server settings.

**What:** UI fetches template config on page load.

**How:**
1. `GET /auth/v1/templates/config` → `{password_policy: {min_length: 12, ...}, issuer: "..."}`
2. `GET /auth/v1/templates/password_policy` → `{min_length: 12, require_uppercase: true, ...}`

**Then:** Config loaded from IdpConfig at startup, no DB needed.

**Verify:**
- Config returns password policy with all fields
- Password policy matches IdpConfig defaults (min_length=12, etc.)
- Issuer URL matches configured value

---

## Test Infrastructure

### TestIdpServer
- Creates temp Turso SQLite file
- Builds IdpServer with real config + HandlerStorage
- Binds to random port, returns (addr, shutdown_handle)
- Cleanup on drop (temp dir deleted, server shut down)

### Shared Valtron Pool
- Initialized once per test with `initialize_pool(42, Some(5))`
- `#[serial(idp_e2e)]` prevents parallel test interference

### Assertions
- Every test verifies HTTP status code, response body structure, and side effects
- Side effects verified via direct storage queries (find_user_by_email, etc.)

## Execution
- `cargo test -p foundation_auth --features server-test --test e2e_integration_tests`
- Each test: `#[ntest::timeout(120000)]` (2 min max)
- Each test: `#[serial(idp_e2e)]` (one at a time)
