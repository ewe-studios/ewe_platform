# 57: foundation-social-and-keychain

Identity federation + portable Bitwarden-compatible keychain + credential provisioning, all on `foundation_auth` and `foundation_db`.

## Scope

Merges two closely-coupled bodies of work that touch the same crates:

- **Phase 0 (001–007): foundation_auth social login** — upstream IdP federation (Google/GitHub/Facebook), user provisioning, account linking, admin provider API. Lands **first** — the keychain builds on `foundation_auth`.
- **Phase 1 (008–012): foundation_keychain** — Bitwarden vault + SSH key provisioning, built on the completed auth surface.

See `social-login-requirements.md` for phase 0 detail, `requirements.md` for phase 1.

## Progress

### Phase 0 — foundation_auth social login

| Feature | Status | Notes |
|---------|--------|-------|
| 001: Provider model + CRUD service | Not started | UpstreamProvider entity, ProviderService, ProviderCrypto |
| 002: Provider migrations | Not started | 024 upstream_providers, 025 user_provider_links |
| 003: Upstream OIDC/OAuth2 client | Not started | Generic client + ProviderBackend trait (Google/GitHub/Facebook) |
| 004: Social login flow | Not started | Redirect + callback, double-state correlation |
| 005: User provisioning | Not started | Auto-create/link by verified email |
| 006: Provider discovery + admin API | Not started | Discovery lists providers, admin CRUD |
| 007: WASM upstream client | Not started | Browser fetch for Tauri/desktop embed |

### Phase 1 — foundation_keychain

| Feature | Status | Notes |
|---------|--------|-------|
| 008: Core types + portable domain logic | Not started | Error, util, models, SignalR, auth adapters, queries |
| 009: Cloudflare backend | Not started | foundation_deployment_cloudflare::workers + foundation_db WASM |
| 010: Native backend | Not started | foundation_http + foundation_netio + foundation_cronjobs |
| 011: Integration tests + Docker | Not started | Shared suite, crypto compat, Docker, bw CLI e2e |
| 012: SSH key provisioning + app registry | Not started | App registration, Ed25519/RSA keys, age at-rest encryption |

## Decisions

| # | Decision | Status |
|---|----------|--------|
| 00 | Identity broker pattern — IdP always mints its own JWTs (D01–D10) | Resolved |
| 01 | Crypto — foundation_auth (JwtSigningKey, TOTP, PBKDF2, Argon2id) | Resolved |
| 02 | Storage — foundation_db traits only (no custom traits) | Resolved |
| 03 | No backend features — target gates (`cfg(target_family = "wasm")`) | Resolved |
| 04 | Notifications — foundation_netio WebSocket (native), DO (Cloudflare) | Resolved |
| 05 | Jobs — foundation_cronjobs (valtron + foundation_db persistence) | Resolved |
| 06 | Auth — foundation_auth for JWT/TOTP/middleware; keychain adapts | Resolved |
| 07 | Blob — foundation_db BlobStore (R2Wasm, filesystem) | Resolved |
| 08 | age encryption (scrypt passphrase) — SSH key at-rest, native+WASM | Resolved |
| 09 | Workers WebSocket — reuse foundation_netio framing, transport only | Resolved |
| 10 | SSH key provisioning — app registry + credential API | Resolved |

## Source

- Social login: designed from OIDC/OAuth2 specs (see `social-login-requirements.md`)
- Keychain: OrangeVault `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/orangevault`

## Related

- [[project_spec41_f47_h2_server]] — foundation_http h2 server (complete)
- [[project_spec41_f51_websocket_unified]] — foundation_netio WebSocket (complete)
- [[project_spec55_foundation_wireguard]] — workspace crypto/boring pinning
- [[project_spec54_docker_deployment]] — Docker deployment patterns
- spec-38 foundation-auth-server — IdP server base (social login builds on it)
