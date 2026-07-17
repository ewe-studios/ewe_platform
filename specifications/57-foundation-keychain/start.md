# 57: foundation_keychain

Portable Bitwarden-compatible keychain + credential provisioning service.

## Summary

Extracts OrangeVault's Bitwarden-compatible vault API into a portable crate. Adds SSH key provisioning + app registry for credential auto-provisioning. Zero custom storage traits — uses `foundation_db` directly. Uses `foundation_auth` for JWT/TOTP/PBKDF2/Argon2id/middleware/credential-stores. Workers runtime via `foundation_deployment_cloudflare::workers`. Cron via `foundation_cronjobs`.

## Progress

| Feature | Status | Notes |
|---------|--------|-------|
| F00: Core types + portable domain logic | Not started | |
| F01: Cloudflare backend | Not started | |
| F02: Native backend | Not started | |
| F03: Integration tests + Docker | Not started | |
| F04: SSH key provisioning + app registry | Not started | |

## Decisions

| # | Decision | Status |
|---|----------|--------|
| 01 | Crypto — foundation_auth (JwtSigningKey, TOTP, PBKDF2, Argon2id) | Resolved |
| 02 | Storage — foundation_db traits only (no custom traits) | Resolved |
| 03 | No backend features — target gates (`cfg(target_family = "wasm")`) | Resolved |
| 04 | Notifications — foundation_netio WebSocket (native), DO (Cloudflare) | Resolved |
| 05 | Jobs — foundation_cronjobs (valtron + foundation_db persistence) | Resolved |
| 06 | Auth — foundation_auth for JWT/TOTP/middleware; keychain adapts | Resolved |
| 07 | Blob — foundation_db BlobStore (R2Wasm, filesystem) | Resolved |
| 08 | age encryption (scrypt passphrase) — SSH key at-rest, native+WASM | Resolved |
| 09 | Workers runtime — foundation_deployment_cloudflare::workers | Resolved |
| 10 | SSH key provisioning — app registry + credential API | Resolved |

## Source

OrangeVault: `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/orangevault`

## Related

- [[project_spec41_f47_h2_server]] — foundation_http h2 server (complete)
- [[project_spec41_f51_websocket_unified]] — foundation_netio WebSocket (complete)
- [[project_spec55_foundation_wireguard]] — workspace crypto/boring pinning
- [[project_spec54_docker_deployment]] — Docker deployment patterns
