---
feature: "008-keychain-vault-and-backends"
spec: "57-foundation-social-and-keychain"
depends: "none"
status: "complete"
stages:
  - "1-portable-vault-api"      # ← current
  - "2-native-backend"
  - "3-cloudflare-workers-backend"
  - "4-integration-tests-and-docker"
---

# Feature 008: Keychain Vault API + Backends (staged)

Consolidates former F008/F009/F010/F011. See `feature.md` for the stage plan and the
2026-07-18 reconciliation notes (crypto via `foundation_auth`; target gates, not
`backend-*` features).

## Current state (Stage 1 partial)

`foundation_keychain` compiles (native + wasm core) with error types, models
(cipher, folder, org, send, user, sync), auth adapters, notification framing. **26
tests pass.** All `core/api/*` handlers are implemented; native + Workers backends
shipped; KV-backed signing key persistence; SignalR DO wired.

## Stage order (each unlocks the next)

1. **Portable vault API** (`core/api/*`) + tests in `foundation_keychain/tests/`.
2. **Native backend** (`server/native.rs`) — `foundation_http` + `foundation_netio` WS + `foundation_cronjobs`.
3. **Cloudflare/Workers backend** (`server/cloudflare.rs`) — `foundation_deployment_cloudflare::workers` + D1/R2/KV + `UserNotifier` DO.
4. **Integration tests + Docker** — shared suite over both backends, crypto-compat, Dockerfile/compose, Bitwarden CLI e2e.
