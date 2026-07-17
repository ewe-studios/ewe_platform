# Spec 57 Progress: foundation_keychain

## Status: Spec Written, Awaiting Implementation

Spec rewritten 2026-07-18. All decisions resolved. Features scoped.

**Foundation reuse (no custom traits):**
- `foundation_db`: `QueryStore`, `AsyncQueryStore`, `KeyValueStore`, `AsyncKeyValueStore`, `BlobStore`, `AsyncBlobStore`, `RateLimiterStore`, `StorageProvider`, `DataValue`, `SqlRow`, `StorageItemStream`, `AsyncStorageItemStream`, `StorageBackend`, `SchemaMigration`, `AuthStore`, `AsyncAuthStore`
- `foundation_auth`: `JwtVerifier`, `JwtSigningKey`, `JwtManager`, `VerifiedClaims`, `TOTPSecret`, `BackupCodeSet`, `TwoFactorChallenge`, `require_auth`, `extract_bearer_token`, `AuthContext`, `AsyncCredentialStore`, `D1CredentialStore`, `UserService`, `TokenService`, `SessionService`, `IdpServer`, `pbkdf2::{pbkdf2_derive, pbkdf2_verify, SERVER_PASSWORD_ITERATIONS}`
- `foundation_cronjobs`: `CronScheduler`, `CronJob`, `JobConfig`, `JobState` (new crate, valtron-based with foundation_db persistence)
- `foundation_deployment_cloudflare::workers`: Durable Objects, WebSocket, Env bindings, RequestContext (new module, feature-gated)
- `foundation_http`: native HTTP server (F47-complete)
- `foundation_netio`: native WebSocket server (F51-complete)

## Feature Progress

| Feature | Status | Notes |
|---------|--------|-------|
| F00: Core types + portable domain logic | Not started | Error, util, models, SignalR, auth adapters, DB models, query functions, API handlers |
| F01: Cloudflare backend | Not started | foundation_deployment_cloudflare::workers + foundation_db WASM backends + foundation_auth |
| F02: Native backend | Not started | foundation_http + foundation_netio WS + foundation_cronjobs + foundation_auth |
| F03: Integration tests + Docker | Not started | Shared test suite, crypto compat, Dockerfile, docker-compose, bw CLI e2e |

## Key Design Decisions

1. **No custom storage traits** — foundation_db's traits used directly
2. **No reinvented auth** — foundation_auth for JWT/TOTP/PBKDF2/middleware/credential-stores
3. **No tokio** — valtron handles all async execution and cron scheduling
4. **Target gates, not feature gates** — `cfg(target_family = "wasm")` decides backend
5. **Workers runtime in foundation_deployment_cloudflare** — DO + WS + Env as reusable module
6. **Cron in foundation_cronjobs** — valtron + foundation_db persistence, shared across crates
