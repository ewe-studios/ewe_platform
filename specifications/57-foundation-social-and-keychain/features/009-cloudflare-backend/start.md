---
feature: "009-cloudflare-backend"
spec: "57-foundation-social-and-keychain"
depends: "008-core-types-and-domain"
status: "pending"
---

# Feature 009: Cloudflare Backend

## Current state
Not started.

## Remaining work
- Cloudflare crypto (Web Crypto via `web_sys::SubtleCrypto`): PBKDF2, HMAC, random bytes
- Server bootstrap (`#[event(fetch)]` entry point via `foundation_deployment_cloudflare::workers`)
- Cloudflare notifications (Durable Object `UserNotifier` with WebSocket + SignalR)
- Integration with `foundation_auth`: JWT signing, D1CredentialStore, TOTP, rate limiting
- WASM compilation target (`wasm32-unknown-unknown`), binary under 5MB compressed
- OrangeVault integration tests against Miniflare
