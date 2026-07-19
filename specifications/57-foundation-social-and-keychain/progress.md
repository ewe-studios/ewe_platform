# Spec 57 Progress: foundation-social-and-keychain

## Status: In Progress

Phase 0 (social login): F001-F007 complete
Phase 1 (keychain): 008 (staged vault+backends) Stages 1-4 done (native verified); 009 (SSH) done; 010 (DO+WS) done

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

> Note: the `e2e`/`idp_server` integration tests (17 + 15 tests) require the
> `--features server-test` flag. No `tracing_test` is used — the old subscriber-conflict
> note is stale. These tests compile and pass with the correct feature flag.

**Spec quality:** All 11 decisions have `Status:` fields. All 10 features (001-010) have
`feature.md` + `start.md` with frontmatter. `foundation_cronjobs` crate
compiles (3 tests). `foundation_deployment_cloudflare::workers::websocket`
verified. Argon2id support confirmed in `foundation_auth::shared::password_hash`.

## Implementation Order

**Phase 0 — foundation_auth social login:**
1. ✅ 001 provider model + ✅ 002 migrations + ✅ 003 upstream client
2. ✅ 004 auth session store + ✅ 005 provisioning service + ✅ 006 provider admin API

**Phase 1 — foundation_keychain (008 consolidated + staged, 009 add-on):**
3. ✅ 008 keychain vault + backends — ALL 4 STAGES done (native verified end-to-end)
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
   - **✅ STAGE 2 COMPLETE — native backend transport** (`server/native.rs`): `KeychainServer` +
     `ServeAdapter` over `foundation_http`, routes the Bitwarden API to the handlers, Bearer auth,
     form/JSON parsing, single-poll async drive (verified: async handlers complete in 1 poll over
     local Turso). **e2e test** boots real TCP server, drives register→login→sync→cipher/folder create.
     Fixes: type-code enums (`KdfType`/`CipherType`/`SendType`/etc.) now use `serde_repr` (integer wire
     format, Bitwarden-correct) not variant-name strings; request body drained via `try_collect_bytes`.
   - **✅ STAGE 3 — Workers backend transport compiles on wasm32** (`server/cloudflare.rs`):
     `#[event(fetch)]` builds a D1-backed `KeychainContext` (foundation_db `workers-rs` interop) and
     `.await`s the SHARED `core/router::route` (extracted so native single-polls it, Workers awaits it).
     ✅ All deferred items delivered 2026-07-19: signing-key KV persistence, Miniflare runtime tests (16 tests).
     ✅ **F010 complete (2026-07-19):** workers module + SignalR DO implemented and verified.
   - **✅ STAGE 4 (native) — server binary `src/bin/keychain.rs` VERIFIED**: boots valtron pool, opens
     Turso, applies schema, serves; smoke-tested with real `curl` (register → connect/token login return
     correct Bitwarden JSON). `Dockerfile` (multi-stage) + `docker-compose.yml` written.
   - Remaining (needs external tooling): Bitwarden-CLI e2e (needs `bw` + running server).
4. ✅ 009 SSH key provisioning (former 012) — app registry (Argon2id secrets) + Ed25519/RSA keygen
   (`ssh-key`) + age-encrypted private keys at rest; native-gated (`core/provisioning/`); 3 tests;
   wired into the native server (`/api/apps/*`, `/api/credentials/ssh-keys/*`, X-App-Secret/Bearer auth).

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
| 008: Keychain vault + backends (staged) | 1 | ✅ S1-4 (native verified) | 64 core + 1 e2e; wasm compiles |
| 009: SSH key provisioning | 1 | ✅ Complete | 3 (provisioning) |
| 010: Workers DO + WebSocket machinery | 1 | ✅ Complete | — (wasm32 + native compile; worker-build green; bindings verified) |

## Test Results (2026-07-19)

- **foundation_auth**: 37 lib + integration (provider/upstream/auth_session/provisioning/admin)
- **foundation_keychain**: ~72 tests — 22 core + 42 vertical (folders/accounts/identity/ciphers/sends/2fa/icons/events/orgs/sync) + 1 native-e2e + 3 provisioning; native binary curl-verified; native + wasm32 compile
- **spec-57 phase 1 (keychain): features 008 (all 4 stages) + 009 + 010 COMPLETE**

## Keychain Crate Structure

```
foundation_keychain/              (✅ complete, native + wasm32)
├── src/core/
│   ├── error.rs / util.rs        # AppError, SSRF icon guard
│   ├── models/                   # Cipher, Folder, Org, Send, User, Sync models
│   ├── auth/mod.rs               # Bitwarden JWT claims, security stamp
│   ├── notifications/mod.rs      # SignalR MessagePack framing
│   ├── router.rs                 # Portable method+path dispatch
│   ├── context.rs                # KeychainContext (D1 + JWT key)
│   ├── crypto.rs                 # Cross-platform PBKDF2 wrapper
│   ├── store/                    # SQL queries (accounts, ciphers, folders, etc.)
│   ├── api/                      # All handlers implemented (11 modules)
│   └── provisioning/             # SSH key generation (native-gated)
├── src/server/
│   ├── native.rs                 # ✅ Stage 2: foundation_http backend
│   ├── cloudflare.rs             # ✅ Stage 3: Workers fetch + KV signing key persistence
│   └── signalr_do.rs             # ✅ F010: SignalR Durable Object hub
├── tests/                        # 16 test files, 68+ tests
├── wrangler.toml                 # Workers config (D1 + KV + DO)
├── package.json                  # worker-build + Miniflare test scripts
└── test.mjs                      # 16 Miniflare integration tests
```

## Foundation Reuse (no custom traits)

- `foundation_db`: QueryStore, AsyncQueryStore, KeyValueStore, BlobStore, RateLimiterStore, StorageProvider, D1WasmStorage
- `foundation_auth`: JwtSigningKey (with KV persistence), TOTPSecret, PBKDF2
- `foundation_deployment_cloudflare::workers`: DO machinery (durable_object, websocket, env, context)
- `foundation_http`, `foundation_netio`: native HTTP + WebSocket (native backend)
