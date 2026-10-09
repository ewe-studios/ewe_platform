# Fundamentals 01 — Authentication flows and session management

Zero-to-expert on how auth works in the platform.

---

## 1. The auth flow

```
Login → OAuth/Password → Verify → Create Session → JWT → Access
                                                     ↓
Request → Auth middleware → Verify JWT → Extract user → Handler
```

## 2. JWT structure

```
Header: {"alg": "RS256", "typ": "JWT"}
Payload: {
    "sub": "user-123",
    "email": "user@example.com",
    "roles": ["admin"],
    "exp": 1719446400,
    "iat": 1719360000,
    "session_id": "sess-abc"
}
Signature: RSASHA256(base64(header) + "." + base64(payload), private_key)
```

## 3. JWKS (JSON Web Key Set)

Public keys published at `/.well-known/jwks.json`. The `JwksManager`
auto-refreshes keys from the issuer's endpoint. Supports RSA (RS256), EC
(ES256), and HMAC (HS256).

## 4. OAuth2 with PKCE

Authorization code flow with Proof Key for Code Exchange:

```
Client → Generate code_verifier + code_challenge (SHA256)
       → Redirect to authorization URL with challenge
       → User authenticates
       → Callback with authorization code
       → Exchange code + verifier for access + refresh tokens
```

PKCE prevents authorization code interception attacks.

## 5. Two-Factor Auth (TOTP)

Time-based One-Time Password (RFC 6238):
- 30-second time steps
- HMAC-SHA1 with shared secret
- 6-digit codes

Setup: generate secret → show QR code → user scans → verify first code.
Login: password correct → request TOTP code → verify → grant access.

## 6. Session management

Sessions have configurable TTL and sliding window:
- **TTL** — maximum session lifetime (e.g., 24 hours)
- **Sliding window** — extend TTL on each activity (e.g., 30 min idle)

## 7. Credential storage

- Passwords: Argon2id (memory-hard, configurable parallelism/memory/iterations)
- OAuth tokens: AES-256-GCM encrypted at rest
- TOTP secrets: encrypted in `CredentialStore`

## 8. Integration with foundation_ai

`SessionAccessProvider` is the gatekeeper:

```rust
fn can_use_model(&self, user: &UserId, model: &str) -> Result<bool, AuthError>;
fn can_use_tool(&self, user: &UserId, tool: &str) -> Result<bool, AuthError>;
fn can_spend(&self, user: &UserId, tokens: u64) -> Result<bool, AuthError>;
fn token_budget(&self, user: &UserId) -> Result<TokenBudget, AuthError>;
```
