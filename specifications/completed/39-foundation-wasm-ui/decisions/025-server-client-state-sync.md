# 025 — Server-client state synchronization: client = presentation + actions, server = state

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**The client never tells the server its state.** The server owns all signals/state and communicates what the next state should be. The client is purely presentation + action triggers.

### Client → Server: Actions only

The client sends **what it wants to do**, not what it has:

| Action | Example |
|--------|---------|
| **Mutate** | "Increment the counter" |
| **Toggle** | "Enable this setting" |
| **Execute** | "Run this server action" |
| **Fetch** | "Give me the render for /api/users" |
| **Subscribe** | "Stream state changes for /api/live-feed" |

### Server → Client: State updates

The server computes the next state and pushes updates to the client:

- **Signal patches** — which signals changed, new values
- **DOM updates** — Arrow batch or HTML to apply
- **Both can be mixed** in a single response

### Why this design

- **No state sync problem** — client doesn't own state, so there's no conflict
- **Clear boundaries** — client = presentation + actions, server = state + logic
- **Server is authoritative** — always knows the truth
- **Simplifies everything** — no diffs, no conflict resolution, no state serialization from client
- **Works across all execution modes** — WASM main thread, web worker, service worker, HTTP server all follow the same model

### No initial state hydration needed

Since the server renders self-contained HTML, there's nothing to bootstrap on the client. The rendered page is a standard web app — it works without WASM or signals loaded. When the runtime initializes, it scans for `primal:*` attributes and `<island>` tags, wiring up event handlers and scoped content. No signal values need to be synced or embedded.

If the client needs to update something, it sends an action (via `primal:onclick`, `<mount-data>`, etc.) — the server computes the next state and sends back HTML or Arrow DOM ops. The client never holds server state; it only holds what the server rendered.

**Client action request:**
```
{
  action: "increment" | "toggle" | "fetch" | "subscribe" | ...,
  target: "/api/users",
  payload: { ... },           // action-specific data
  component_id: "abc123",     // which component triggered it
}
```

**Server response:**
```
{
  request_id: "abc123",       // matches the component
  signal_patches: {           // which signals changed
    "counter": { old: 0, new: 1 },
  },
  dom_ops: [...],             // Arrow or HTML to apply
}
```
