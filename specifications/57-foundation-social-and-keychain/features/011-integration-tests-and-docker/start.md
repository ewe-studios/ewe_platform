---
feature: "011-integration-tests-and-docker"
spec: "57-foundation-social-and-keychain"
depends: "008-core-types-and-domain, 009-cloudflare-backend, 010-native-backend"
status: "pending"
---

# Feature 011: Integration Tests + Docker Packaging

## Current state
Not started.

## Remaining work
- Auth flow integration tests: prelogin, register, token (password/refresh grants), profile, password change, account deletion
- Vault CRUD integration tests: sync, cipher CRUD (login/note/card/identity), attachments, folders
- Sends integration tests: text send, file send, anonymous access, JWT-gated download
- Organization integration tests: org CRUD, member invite/accept/confirm, collections, cipher sharing
- 2FA integration tests: TOTP setup, login with TOTP, recovery codes
- Cron integration tests: expired send purge, trashed cipher purge
- Crypto compatibility tests: PBKDF2, HMAC, JWT, TOTP bit-for-bit across both backends
- Dockerfile (multi-stage) and docker-compose.yml
- Bitwarden CLI end-to-end test
