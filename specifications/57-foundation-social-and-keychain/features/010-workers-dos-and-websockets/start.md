---
feature: "010-workers-dos-and-websockets"
spec: "57-foundation-social-and-keychain"
depends: "none"
status: "in-progress"
stages:
  - "1-general-do-machinery"
  - "2-websocket-pair-accept"
  - "3-signalr-hub-do-keychain"
---

# Feature 010: Workers Durable Objects & WebSocket Machinery

General Cloudflare Workers runtime primitives in `foundation_deployment_cloudflare::workers`
that any crate can build on. The keychain SignalR hub is the first consumer but the
machinery is portable.

## Current state

All 4 `workers/` module files are empty stubs. `lib.rs` already gates the module:
`#[cfg(all(target_family = "wasm", feature = "workers"))]`. No `workers` cargo
feature exists yet — it needs adding. The `worker` crate (0.8.3) is already in the
lockfile via `foundation_keychain`.

## Stage order

1. **General DO machinery** — trait extension, env/context interop, DO router
2. **WebSocket accept** — `WebSocketPair` helpers, SignalR framing reuse from `foundation_netio::websocket::shared`
3. **Keychain SignalR hub** — `foundation_keychain::server::signalr_do` built on stages 1+2
