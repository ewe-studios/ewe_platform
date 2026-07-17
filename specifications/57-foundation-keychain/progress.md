# Spec 57 Progress: foundation_keychain

## Status: Spec Written, Awaiting Implementation

Spec rewritten 2026-07-17. All decisions resolved. Features scoped. Uses `foundation_db` traits directly (no custom traits) and `foundation_auth` surfaces directly (no reinvented auth).

## Feature Progress

| Feature | Status | Notes |
|---------|--------|-------|
| F00: Core types + portable domain logic | Not started | Error, util, models, SignalR framing, auth adapters, DB models, query functions, API handlers |
| F01: Cloudflare backend | Not started | D1Wasm/R2Wasm/KVWasm, Web Crypto, Durable Object notifications, workers-rs Router |
| F02: Native backend | Not started | Turso/Libsql, foundation_http, foundation_netio WebSocket, JwtSigningKey crypto |
| F03: Integration tests + Docker | Not started | Shared test suite, crypto compatibility, Dockerfile, docker-compose, bw CLI e2e |

## Foundation Reuse

**foundation_db** (no changes needed): `QueryStore`, `AsyncQueryStore`, `KeyValueStore`, `AsyncKeyValueStore`, `BlobStore`, `AsyncBlobStore`, `RateLimiterStore`, `StorageProvider`, `DataValue`, `SqlRow`, `StorageItemStream`, `AsyncStorageItemStream`, `StorageBackend`, `SchemaMigration`, `AuthStore`, `AsyncAuthStore`, `PasskeyStore`, `TosStore`.

**foundation_auth** (no changes needed): `JwtVerifier`, `JwtSigningKey`, `JwtManager`, `VerifiedClaims`, `TOTPSecret`, `BackupCodeSet`, `TwoFactorChallenge`, `require_auth`, `extract_bearer_token`, `AuthContext`, `AsyncCredentialStore`, `D1CredentialStore`, `UserService`, `TokenService`, `SessionService`, `IdpServer`, `PasswordAuthClient`, `WasmSessionManager`.

## Key Design Decisions

1. **No custom storage traits** — uses `foundation_db`'s `QueryStore`, `KeyValueStore`, `BlobStore`, `RateLimiterStore` directly
2. **No reinvented auth** — uses `foundation_auth` for JWT/TOTP/middleware/credential-stores
3. **Bitwarden-specific adapters only** — `BitwardenClaims` (maps onto `VerifiedClaims::custom`), `check_security_stamp` (after JWT verify), `build_login_response` (TokenPair → Bitwarden shape)
4. **Feature-gated backends** — `backend-cloudflare` (wasm) vs `backend-native` (native), mutually exclusive
