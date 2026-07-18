# Spec 57 Progress: foundation-social-and-keychain

## Status: In Progress

Phase 0 (social login): F001-F007 complete
Phase 1 (keychain): 008 (staged vault+backends) — Stage 1 in progress; 009 (SSH) pending

**2026-07-18 — F007 completed + cross-platform HTTP made real.** `UpstreamOidcClient`
is now cross-platform (built on `default_http_client()` / `Arc<dyn HttpClient>`, runs on
native + wasm/Workers): provider models moved to `shared/`, `server` gate removed, and
`exchange_code` + `fetch_userinfo` added. Fixed a systemic body-read bug (all shared
clients read `get_body_ref()` on a lazy `SendSafeBody::Stream` = empty; now
`try_collect_bytes(take_body())`) and the chain of latent wasm-compilation breakage that
had kept `foundation_auth` from ever compiling on wasm32 (netio dep `default-features`,
web-sys `SubtleCrypto`/`CryptoKey`, `password_hash` WebCrypto bugs, `network_client` ssl
target-gating). 5 new `#[valtron_test]` integration tests exercise the broker end-to-end
over a real server. Native + wasm32 compile clean.

> Note: the pre-existing `e2e`/`idp_server` integration tests fail on a
> `#[tracing_test::traced_test]` + `#[valtron_test]` global-subscriber conflict (fails at
> test setup, unrelated to F007) — a separate test-harness issue to fix.

**Spec quality:** All 11 decisions have `Status:` fields. All 12 features have
`feature.md` + `start.md` with frontmatter. `foundation_cronjobs` crate
compiles (3 tests). `foundation_deployment_cloudflare::workers::websocket`
verified. Argon2id support confirmed in `foundation_auth::shared::password_hash`.

## Implementation Order

**Phase 0 — foundation_auth social login:**
1. ✅ 001 provider model + ✅ 002 migrations + ✅ 003 upstream client
2. ✅ 004 auth session store + ✅ 005 provisioning service + ✅ 006 provider admin API

**Phase 1 — foundation_keychain (008 consolidated + staged, 009 add-on):**
3. 🟡 008 keychain vault + backends — Stage 1 (portable core/api) in progress
   - ✅ core types/models/auth/notifications (26 tests)
   - ✅ Stage-1 architecture + **folders vertical**: `KeychainContext` + `core/store/` +
     `core/api/folders` + `migrations/0001_initial.sql`; 6 integration tests; native + wasm32 compile
   - ✅ **accounts vertical**: `core/crypto` (cross-platform PBKDF2 wrapper) + `core/store/users`
     (`vault_accounts` table) + `core/api/accounts` (prelogin/register/profile/verify); 7 tests
   - ✅ **identity/login vertical**: `connect/token` password + refresh grants → `LoginResponse`;
     JWT mint/verify via `foundation_auth::JwtSigningKey` (context gains signing key + verifier);
     `core/store/devices` (refresh-token rotation); 6 tests. **Fixed `foundation_auth::sign_claims`
     to carry custom claims + honor `exp` (was dropping everything but sub/iss/aud).**
   - ✅ **ciphers vertical**: `core/store/ciphers` + `core/api/ciphers` (CRUD + soft-delete/restore,
     content JSON in `data` column); 6 tests
   - ✅ **sync vertical**: `core/api/sync` assembles profile + folders + ciphers; 1 test
   - ✅ **sends vertical**: `core/store/sends` + `core/api/sends` (CRUD + anonymous access controls:
     password/max-count/disabled/expiry); 4 tests
   - ✅ **two_factor** (TOTP setup/enable/verify/disable via foundation_auth TOTPSecret; base32); 1 test
   - ✅ **icons** (SSRF-guarded favicon proxy over default_http_client); 2 tests
   - ✅ **events** (audit log collect/list); 1 test · ✅ **emergency** (faithful placeholder, empty)
   - ✅ **orgs** (create+owner+default collection, get/list/delete, collections CRUD, member
     invite/confirm/list; advanced ACLs/policies/groups/cipher-share deferred); 4 tests
   - ✅ **sync** now includes org collections + sends
   - **✅ STAGE 1 COMPLETE — portable vault API: all core/api entities, native + wasm32, 64 tests green**
   - 🔲 Stage 2 native backend (foundation_http transport) · 🔲 Stage 3 cloudflare/workers · 🔲 Stage 4 integration+docker
   - 🔲 Stage 2 native backend · 🔲 Stage 3 cloudflare/workers · 🔲 Stage 4 integration+docker
4. 🔲 009 SSH key provisioning (former 012)

> **2026-07-18 restructure:** former F008–F011 (core, cloudflare, native, integration)
> consolidated into **008-keychain-vault-and-backends** as 4 sequential stages (each
> unlocks the next); former F012 renumbered to **009**. Reconciled with decision 01
> (crypto via `foundation_auth`, no keychain crypto) and decision 03 (target gates, not
> `backend-*` features).

## Feature Progress

| Feature | Phase | Status | Tests |
|---------|-------|--------|-------|
| 001: Provider model + CRUD | 0 | ✅ Complete | 18 (provider_service) |
| 002: Provider migrations | 0 | ✅ Complete | — |
| 003: Upstream OIDC/OAuth2 client | 0 | ✅ Complete | 5 (upstream_client) |
| 004: Auth session store | 0 | ✅ Complete | 4 (auth_session) |
| 005: User provisioning | 0 | ✅ Complete | 5 (provisioning_service) |
| 006: Provider discovery + admin API | 0 | ✅ Complete | 10 (provider_admin) |
| 007: Cross-platform upstream client | 0 | ✅ Complete | 10 (5 unit + 5 integration) |
| 008: Keychain vault + backends (staged) | 1 | 🟡 Stage 1 partial | 26 (core) — S1 core/api next |
| 009: SSH key provisioning | 1 | 🔲 Not started | — |

## Test Results (2026-07-18)

- **foundation_auth**: 32 passed (provider_service: 18, upstream_client: 5, auth_session: 4, provisioning_service: 5, provider_admin: 10)
- **foundation_keychain**: 26 passed (core_tests: 22 integration + 4 unit)
- **Total: 58 passed, 0 failed**

## Keychain Crate Structure

```
foundation_keychain/              (✅ created, compiles on native + wasm)
├── src/core/
│   ├── error.rs                  # AppError, AppResult, ValidationError
│   ├── util.rs                   # SSRF icon guard, helpers
│   ├── models/
│   │   ├── cipher.rs             # Cipher, Login, Card, Identity, SecureNote
│   │   ├── folder.rs             # Folder, create/update requests
│   │   ├── org.rs                # Organization, membership types
│   │   ├── send.rs               # Send, time-limited sharing
│   │   ├── user.rs               # Prelogin, Register, Profile, KDF
│   │   └── sync.rs               # SyncData, EquivalentDomains
│   ├── auth/mod.rs               # BitwardenClaims, security stamp verify
│   ├── notifications/mod.rs      # SignalR MessagePack framing, UpdateType
│   └── api/                      # Handler stubs (accounts, ciphers, folders, orgs, sends, 2FA, sync, events, emergency, icons)
├── src/server/
│   ├── native.rs                 # 🔲 Stage 2 (native: foundation_http)
│   └── cloudflare.rs             # 🔲 Stage 3 (cloudflare/workers router)
└── tests/core_tests.rs           # 22 integration tests
```

## Foundation Reuse (no custom traits)

- `foundation_db`: QueryStore, AsyncQueryStore, KeyValueStore, BlobStore, RateLimiterStore, StorageProvider
- `foundation_auth`: JwtSigningKey, TOTPSecret, require_auth, IdpServer, ProviderService, ProvisioningService, UpstreamOidcClient, UpstreamAuthSession
- `foundation_http`, `foundation_netio`: native HTTP + WebSocket (Stage 2)
