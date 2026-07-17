# Spec 57 Progress: foundation-social-and-keychain

## Status: Spec Written, Awaiting Implementation

Merged spec-58 (foundation_auth social login) into spec-57 on 2026-07-18 — both
touch `foundation_auth` + `foundation_db`. Social login is phase 0 (features
001–007), keychain is phase 1 (features 008–012). Phase 0 lands first so the
auth surface is complete before the vault builds on it.

## Implementation Order

**Phase 0 — foundation_auth social login (do first):**
1. 001 provider model + CRUD → 002 migrations → 003 upstream client
2. 004 social login flow → 005 user provisioning
3. 006 provider discovery + admin API → 007 wasm client

**Phase 1 — foundation_keychain (builds on phase 0):**
4. 008 core types + domain → 009 Cloudflare / 010 native (parallel) → 011 tests+Docker
5. 012 SSH key provisioning + app registry

## Feature Progress

| Feature | Phase | Status |
|---------|-------|--------|
| 001: Provider model + CRUD | 0 | Not started |
| 002: Provider migrations | 0 | Not started |
| 003: Upstream OIDC/OAuth2 client | 0 | Not started |
| 004: Social login flow | 0 | Not started |
| 005: User provisioning | 0 | Not started |
| 006: Provider discovery + admin API | 0 | Not started |
| 007: WASM upstream client | 0 | Not started |
| 008: Core types + portable domain logic | 1 | Not started |
| 009: Cloudflare backend | 1 | Not started |
| 010: Native backend | 1 | Not started |
| 011: Integration tests + Docker | 1 | Not started |
| 012: SSH key provisioning + app registry | 1 | Not started |

## Foundation Reuse (no custom traits)

- `foundation_db`: `QueryStore`, `AsyncQueryStore`, `KeyValueStore`, `BlobStore`, `RateLimiterStore`, `StorageProvider`, `SchemaMigration`, `AuthStore`
- `foundation_auth`: `JwtSigningKey`, `TOTPSecret`, `require_auth`, `AsyncCredentialStore`, `IdpServer`, `UserService`, `TokenService`, `password_hash::{pbkdf2, argon2id}`, + new social-login: `ProviderService`, `UpstreamOidcClient`, `ProviderBackend`, `ProvisioningService`
- `foundation_cronjobs`: `CronScheduler` (new crate)
- `foundation_deployment_cloudflare::workers`: DO + WebSocket transport + Env (new module)
- `foundation_http`, `foundation_netio`: native HTTP + WebSocket

## Key Design Decisions

1. Social login lands first (phase 0) — completes the auth surface
2. Identity broker — IdP always mints its own JWTs, upstream providers are auth sources only
3. No custom storage traits — foundation_db directly
4. No reinvented auth — foundation_auth for JWT/TOTP/PBKDF2/Argon2id/middleware
5. No tokio — valtron for all async + cron
6. Target gates, not feature gates
7. age scrypt passphrase for SSH key at-rest (native+WASM)
