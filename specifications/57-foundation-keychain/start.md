# 57: foundation_keychain

Portable Bitwarden-compatible keychain service built on `foundation_db` and `foundation_auth`.

## Summary

Extracts OrangeVault's Bitwarden-compatible vault API into a portable crate. Zero custom storage traits — uses `foundation_db`'s `QueryStore`, `KeyValueStore`, `BlobStore`, `RateLimiterStore`, `StorageProvider` directly. Uses `foundation_auth` for JWT/TOTP/middleware/credential-stores. The only new code is the Bitwarden domain logic (API handlers, SignalR protocol, org lifecycle).

## Progress

| Feature | Status | Notes |
|---------|--------|-------|
| F00: Core types + portable domain logic | Not started | |
| F01: Cloudflare backend | Not started | |
| F02: Native backend | Not started | |
| F03: Integration tests + Docker | Not started | |

## Decisions

| # | Decision | Status |
|---|----------|--------|
| 01 | Crypto — Web Crypto (wasm) + foundation_auth JwtSigningKey (native) | Resolved |
| 02 | Storage — foundation_db traits only (no custom traits) | Resolved |
| 03 | Feature gates — mutually exclusive (backend-cloudflare / backend-native) | Resolved |
| 04 | Notifications — foundation_netio WebSocket (native), DO (Cloudflare) | Resolved |
| 05 | Jobs — valtron-based cron scheduler (native), cron triggers (Cloudflare) | Resolved |
| 06 | Auth — foundation_auth for JWT/TOTP/middleware; keychain adapts | Resolved |
| 07 | Blob — foundation_db BlobStore (R2Wasm native, filesystem native) | Resolved |

## Source

OrangeVault: `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/orangevault`
- 45 Rust source files, 14 API modules, 16 SQL tables, 13 Vitest integration tests
- Bitwarden-compatible: official clients connect without patches
- ~150 route handlers across accounts, identity, ciphers, folders, orgs, sends, 2FA, events, icons, notifications

## Related

- [[project_spec41_f47_h2_server]] — foundation_http h2 server (complete), native HTTP
- [[project_spec41_f51_websocket_unified]] — foundation_netio WebSocket (complete), native notifications
- [[project_spec41_connectrpc_shared_native]] — shared/native split pattern
- [[project_spec55_foundation_wireguard]] — workspace crypto/boring pinning
- [[project_spec54_docker_deployment]] — Docker deployment patterns
