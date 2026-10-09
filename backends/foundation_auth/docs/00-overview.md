# foundation_auth — Authentication and authorization

## What it is
Auth primitives: session management, JWT verification, OAuth2 flows, two-factor
auth, and credential storage.

## Key modules
- **`jwt/`** — JWT signing and verification (RS256, ES256, HS256).
- **`jwks/`** — JSON Web Key Set management with automatic refresh.
- **`oauth/`** — OAuth2 authorization code flow with PKCE.
- **`session/`** — Session management with configurable TTL, sliding windows.
- **`two_factor/`** — TOTP-based two-factor authentication.
- **`credential_store/`** — Secure credential storage with encryption.

## Integration
- **foundation_ai**: Auth providers control model access, tool permissions,
  and token budgets.
- **foundation_http**: Auth middleware protects HTTP endpoints.
- **foundation_db**: Credential storage uses the KV/SQL backends.

## Feature flags
- `wasm` — Wasm-compatible session and JWT handling
- `turso` — Turso/libSQL-backed credential storage
