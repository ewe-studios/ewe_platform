# Spec 57 Progress: foundation-social-and-keychain

## Status: In Progress

Phase 0 (social login): F001-F006 complete, F007 partial
Phase 1 (keychain): F008 complete, F009-F012 pending

**Spec quality:** All 11 decisions have `Status:` fields. All 12 features have
`feature.md` + `start.md` with frontmatter. `foundation_cronjobs` crate
compiles (3 tests). `foundation_deployment_cloudflare::workers::websocket`
verified. Argon2id support confirmed in `foundation_auth::shared::password_hash`.

## Implementation Order

**Phase 0 — foundation_auth social login:**
1. ✅ 001 provider model + ✅ 002 migrations + ✅ 003 upstream client
2. ✅ 004 auth session store + ✅ 005 provisioning service + ✅ 006 provider admin API

**Phase 1 — foundation_keychain:**
3. ✅ 008 core types + models + auth + notifications
4. 🔲 009 Cloudflare backend / 🔲 010 native backend / 🔲 011 integration tests / 🔲 012 SSH provisioning

## Feature Progress

| Feature | Phase | Status | Tests |
|---------|-------|--------|-------|
| 001: Provider model + CRUD | 0 | ✅ Complete | 18 (provider_service) |
| 002: Provider migrations | 0 | ✅ Complete | — |
| 003: Upstream OIDC/OAuth2 client | 0 | ✅ Complete | 5 (upstream_client) |
| 004: Auth session store | 0 | ✅ Complete | 4 (auth_session) |
| 005: User provisioning | 0 | ✅ Complete | 5 (provisioning_service) |
| 006: Provider discovery + admin API | 0 | ✅ Complete | 10 (provider_admin) |
| 007: WASM upstream client | 0 | 🟡 Partial | WasmOAuth exists, needs adapter |
| 008: Keychain core types | 1 | ✅ Complete | 26 (core_tests) |
| 009: Cloudflare backend | 1 | 🔲 Not started | — |
| 010: Native backend | 1 | 🔲 Not started | — |
| 011: Integration tests + Docker | 1 | 🔲 Not started | — |
| 012: SSH key provisioning | 1 | 🔲 Not started | — |

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
│   ├── native.rs                 # 🔲 foundation_http server (F010)
│   └── cloudflare.rs             # 🔲 Workers router (F009)
└── tests/core_tests.rs           # 22 integration tests
```

## Foundation Reuse (no custom traits)

- `foundation_db`: QueryStore, AsyncQueryStore, KeyValueStore, BlobStore, RateLimiterStore, StorageProvider
- `foundation_auth`: JwtSigningKey, TOTPSecret, require_auth, IdpServer, ProviderService, ProvisioningService, UpstreamOidcClient, UpstreamAuthSession
- `foundation_http`, `foundation_netio`: native HTTP + WebSocket (F010)
