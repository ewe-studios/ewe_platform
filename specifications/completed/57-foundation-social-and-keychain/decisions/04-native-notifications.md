# Decision 04: Native Notifications — foundation_netio WebSocket Server

**Status:** Resolved (2026-07-18) — multi-instance pub/sub deferred

## Problem

Cloudflare uses Durable Objects for per-user WebSocket notifications. The native backend needs an equivalent:

1. **foundation_netio WebSocket server** — the workspace already has WebSocket support (spec-41 F51)
2. **Separate WebSocket crate** (tokio-tungstenite, axum) — more control, but adds dependency
3. **SSE instead of WebSocket** — simpler, but Bitwarden clients expect SignalR over WebSocket

## Analysis

- **foundation_netio**: Already implements WebSocket client/server (F51 complete, cross-platform). Using it keeps the dependency graph tight and leverages existing code. The SignalR MessagePack framing is portable — only the transport (WebSocket accept/multiplex) differs.
- **Separate crate**: Unnecessary if foundation_netio covers it. Adds complexity.
- **SSE**: Bitwarden clients use SignalR over WebSocket. SSE is not a compatible replacement.

## Decision: foundation_netio WebSocket server

The native backend uses foundation_netio's WebSocket server to accept client connections. Each connected WebSocket is tracked in an in-memory registry keyed by user UUID. The `NotifierBackend` trait implementation broadcasts SignalR MessagePack frames to all connected sockets for a given user.

### Architecture

```
Client ──WebSocket──▶ foundation_netio WebSocket server
                              │
                              ├──▶ UserNotifierRegistry (HashMap<user_uuid, Vec<WsSender>>)
                              │         │
                              │         ├── user_abc: [ws1, ws2]
                              │         └── user_def: [ws3]
                              │
                    NotifierBackend::notify(user, type, ctx, payload)
                              │
                              └──▶ serialize_msgpack(frame) → send to all matching WsSender
```

### SignalR Protocol Compatibility

The SignalR MessagePack hub protocol is **identical** to the Cloudflare backend:
- Handshake: JSON text frame `{"protocol":"messagepack","version":1}\x1E`
- Accept: binary `{}`\x1E`
- Messages: VarInt length prefix + MessagePack payload (binary frame)
- Ping: every 15s, MessagePack `[6]`

The only difference is the transport layer: Durable Objects (Cloudflare) vs. in-process WebSocket registry (native).

## Consequences

- Need a WebSocket connection registry (in-memory, `Arc<RwLock<HashMap>>`)
- No hibernation on native — connections are lost on restart (acceptable, clients reconnect)
- Multi-instance deployments need a pub/sub layer (Redis, NATS) — deferred to future
- The portable `notifications.rs` module handles MessagePack framing; both backends use it
