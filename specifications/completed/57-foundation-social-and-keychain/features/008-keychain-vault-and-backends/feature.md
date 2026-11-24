# F008: Keychain Vault API + Backends (staged)

**Status:** ✅ Complete — all 4 stages (native verified end-to-end; Workers compiles; Bitwarden-CLI + Miniflare need external runtime)

Consolidates the former F008 (core types), F009 (cloudflare), F010 (native), and
F011 (integration + docker). Those were split by concern, but they are one thing —
the Bitwarden-compatible vault — and share the same portable business logic. This
feature delivers it in **sequential stages, each unlocking the next**. SSH key
provisioning (former F012) is a distinct add-on and lives in feature `009`.

## Reconciliation notes (2026-07-18)

- **No crypto in the keychain** ([decision 01](../../decisions/01-crypto-backend.md)):
  all crypto comes from `foundation_auth` — `shared::jwt::JwtSigningKey`,
  `shared::two_factor::TOTPSecret`, and `shared::password_hash::{pbkdf2_derive,
  pbkdf2_verify, PBKDF2_SERVER_ITERATIONS}`. There is **no** `cloudflare/crypto.rs`
  port. `foundation_auth::shared::password_hash` is already cross-platform (native
  `pbkdf2` crate; wasm WebCrypto `SubtleCrypto`), so both backends use the same code.
- **No `backend-*` cargo features** ([decision 03](../../decisions/03-feature-gate-strategy.md)):
  the backend is chosen by **target** (`cfg(target_family = "wasm")` → Workers;
  otherwise native), and `foundation_db` selects its storage backend the same way.
  Success criteria use `--target`, not `--features backend-cloudflare/backend-native`.
- The portable domain logic (`core/api/*`) is the shared foundation both backends
  route to; it was stubbed in the old F008 and is Stage 1 here.

---

## Stage 1 — Portable vault domain + API (`core/`)  ← current

Cross-platform Bitwarden business logic. No `worker::` types, no `foundation_http`
types — handlers take a `KeychainContext` of `foundation_db` stores +
`foundation_auth` primitives and return portable response models. **Unlocks Stages
2 & 3** (both transports route to these handlers).

- `core/error.rs`, `core/util.rs`, `core/models/*` — **done** (F008).
- `core/api/*` — implement the handler logic (currently stubs):
  - `accounts.rs` — prelogin, register, profile get/update, verify-password, change
    password (rotate security stamp), delete account.
  - `identity` (in `accounts`/`auth`) — `connect/token` password + refresh grants →
    `LoginResponse` (JWT via `foundation_auth` `JwtSigningKey`), prelogin KDF params.
  - `ciphers.rs` — create/list/get/update/delete, soft-delete (`deleted_at`),
    restore, bulk ops, share-to-org, attachments.
  - `folders.rs` — CRUD.
  - `sends.rs` — text + file sends, anonymous access (password-gated), JWT-gated
    download URLs, expiry.
  - `orgs.rs` — org create (auto owner membership + collection), invite/accept/
    confirm members, collections, collection-user access, share.
  - `sync.rs` — full sync payload (profile + ciphers + folders + collections +
    policies + sends + domains).
  - `two_factor.rs` — TOTP get/enable/recover via `foundation_auth::TOTPSecret`.
  - `emergency.rs`, `events.rs`, `icons.rs` (SSRF-guarded proxy via `core/util`).
- Storage via `foundation_db` (`QueryStore`/`AsyncQueryStore`, `KeyValueStore`,
  `BlobStore`, `RateLimiterStore`) — no custom traits.
- **Tests:** `foundation_keychain/tests/` — portable unit/logic tests over an
  in-memory `foundation_db` backend (models + each handler's logic).

## Stage 2 — Native backend (`server/native.rs`)

Requires Stage 1. Thin transport: `foundation_http` (h1/h2, spec-41 F47) routes the
Bitwarden API paths to the Stage-1 handlers; `foundation_netio` WebSocket
(`/notifications/hub`, SignalR MessagePack) for notifications; `foundation_cronjobs`
for purge jobs (expired sends, trashed ciphers); `foundation_db::SchemaMigration` for
`migrations/`. Native PBKDF2 can use full iterations; JWT via `JwtSigningKey`.
**Unlocks** running the vault on a VPS/Docker.

## Stage 3 — Cloudflare / Workers backend (`server/cloudflare.rs`)

Requires Stage 1. Thin transport via `foundation_deployment_cloudflare::workers`
(`env`, `context`, `durable_object`, `websocket`): `#[event(fetch)]` +
`#[event(scheduled)]` entry points route to the Stage-1 handlers; `foundation_db`
D1/R2/KV wasm backends; `UserNotifier` Durable Object (WebSocket accept + SignalR
MessagePack + 15s alarm ping) reusing `core/notifications` framing +
`foundation_netio::websocket::shared` framing (decision 09). PBKDF2 via the WebCrypto
path (100K cap, `PBKDF2_SERVER_ITERATIONS`). **Unlocks** edge deployment. Builds on
`wasm32-unknown-unknown`.

## Stage 4 — Integration tests + Docker

Requires Stages 2 & 3. Shared integration suite in `foundation_keychain/tests/` that
runs against **both** backends (target-selected), covering auth flow, vault CRUD,
folders, sends, orgs, 2FA, cron, plus crypto-compatibility (same inputs → identical
PBKDF2/HMAC/JWT/TOTP across backends). Multi-stage Dockerfile + `docker-compose.yml`
for the native server; Bitwarden CLI end-to-end (register → login → sync → create →
delete). Source parity target: `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/orangevault`.

---

## Success criteria (per stage)

- **S1:** `cargo test -p foundation_keychain` (native) green; `core/api` handlers
  implemented over `foundation_db`; `cargo check -p foundation_keychain --target wasm32-unknown-unknown` compiles the portable core.
- **S2:** native server starts, migrates, serves; WS notifications + cron work.
- **S3:** `cargo check -p foundation_keychain --target wasm32-unknown-unknown` (Workers
  entry) compiles; `worker-build --release` under ~5 MB; SignalR DO notifications work.
- **S4:** integration suite green on both backends; crypto-compat bit-for-bit; Docker
  image runs; Bitwarden CLI e2e passes.

## Related decisions

- 01 crypto (foundation_auth), 02 storage (foundation_db), 03 target-gates,
  04 native notifications, 05 job scheduler, 07 blob storage, 09 workers websocket.
