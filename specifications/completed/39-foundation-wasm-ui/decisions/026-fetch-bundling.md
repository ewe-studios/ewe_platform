# 026 — Fetch wrapper & request bundling

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**Bundling is always on for direct communication.** For HTTP transport, requests go through `/primal/messages` as a bundled batch.

### Communication modes

| Mode | Bundling | How |
|------|----------|-----|
| **WASM instance (direct)** | Always bundled | Shared memory batch |
| **WebWorker** | Always bundled | postMessage batch |
| **HTTP Server / ServiceWorker** | Bundled | `POST /primal/messages` with Arrow/JSON |
| **External HTTP (no primal server)** | No bundling | Individual fetches, user-configured |

### Message structure

Every message carries:

| Field | Purpose |
|-------|---------|
| `request_id` | Identifies which component made the request — response routes back to it |
| `order_id` | scru128 for ordering — messages arriving out of order are applied in sequence |
| `dataprotocol` | Protocol the payload uses: `arrow`, `json`, etc. |
| `version` | Protocol version |

### WASM / WebWorker: always bundled

Direct communication (shared memory or postMessage) is inherently batched. All pending requests in the same microtask cycle are encoded into one Arrow message and shipped:

```rust
// Rust side: InstructionReceiver flushes all pending ops, protocol encodes once
receiver.flush();  // protocol.send(ops, memory) — encodes + FFI
```

### HTTP Server / ServiceWorker: bundled endpoint

For primal servers (HTTP server or ServiceWorker intercepting HTTP calls), all bundled requests go to:

```
POST /primal/messages
Content-Type: application/primal-arrow   (or application/primal-json)
```

The HTTP server or ServiceWorker reads `Content-Type` to decode the bundle, processes each request, and streams responses back via SSE with `request_id`, `order_id`, `dataprotocol`.

### External HTTP (no primal server)

If the server doesn't support the primal protocol:

- Each `<mount-data>` / `<mount-stream>` makes individual fetches
- Responses are handled based on `Content-Type`
- User configures manually via `primal.configure({ ... })`

### Request bundling lifecycle

```
1. Component A calls primal.mountData("/api/update", data) → queued
2. Component B calls primal.mountData("/api/settings", data) → queued
3. Browser fires queueMicrotask → all bundled into one message
4. Message sent (shared memory / postMessage / POST /primal/messages)
5. Server/WASM processes bundle, streams responses with request_id, order_id
6. Client routes each response to the correct component
```
