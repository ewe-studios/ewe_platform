# Decision 09: Cloudflare Workers Runtime — foundation_deployment_cloudflare::workers

## Problem

Any crate targeting Cloudflare Workers needs:
- **Durable Objects** — singleton per-key compute with WebSocket accept, hibernation, alarm API, internal fetch
- **Workers WebSocket** — `WebSocketPair`, accept, binary/text send/recv (no native TCP)
- **Env bindings** — typed access to D1, KV, R2, DO stubs
- **RequestContext** — pattern for carrying bindings + auth through the request lifecycle

OrangeVault implemented all of this inline against `worker::` types.

## Analysis

These are Workers-specific runtime primitives. The next crate targeting Workers (keychain, or anything else) will need them too. Rather than each crate re-implementing DO lifecycle, WebSocket framing, and Env binding access, `foundation_deployment_cloudflare` should provide them.

## Decision: workers module in foundation_deployment_cloudflare

```
foundation_deployment_cloudflare/src/workers/
├── mod.rs              # Re-exports
├── durable_object.rs   # DO lifecycle wrapper (fetch, ws_message, alarm, hibernation, serialize/deserialize state)
├── websocket.rs        # WebSocketPair accept, binary/text send/recv, handshake
├── env.rs              # Typed Env binding access (D1, KV, R2, DO stubs)
└── context.rs          # RequestContext pattern (bindings + auth through request lifecycle)
```

Gated behind `workers` feature + `target_family = "wasm"`. Depends on `worker` 0.8.

## Consequences

- foundation_keychain imports `foundation_deployment_cloudflare::workers` instead of `worker::`
- DO lifecycle, WebSocket, and Env binding are reusable across all Workers-targeting crates
- The `worker::` dependency is isolated to one crate, not scattered
