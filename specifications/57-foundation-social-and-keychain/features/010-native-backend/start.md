---
feature: "010-native-backend"
spec: "57-foundation-social-and-keychain"
depends: "008-core-types-and-domain"
status: "pending"
---

# Feature 010: Native Backend

## Current state
Not started.

## Remaining work
- Native server bootstrap (`foundation_http` + `foundation_db` + `foundation_auth` JWT signing)
- Native HTTP: wire `foundation_http` to ~150 API handlers, CORS + security headers
- Native notifications: WebSocket server via `foundation_netio`, SignalR handshake, per-user registry
- Schema migration via `foundation_db::SchemaMigration`
- Cron scheduler via `foundation_cronjobs` (purge expired sends, trash cleanup)
- Compile on `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`
- Bitwarden CLI end-to-end: register, login, sync, cipher CRUD
