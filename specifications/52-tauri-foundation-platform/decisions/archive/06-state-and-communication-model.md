# 06 — State ownership and communication model

**Date:** 2026-07-04
**Status:** Resolved

### Decision

The backend owns all state. The platform provides transports and caching.
The user chooses their communication model (RPC, WebSocket, HTTP, IPC, SSE, etc.).
The platform does not dictate either.

### State ownership

"Server" means wherever the application logic runs: WASM in the WebView, Rust
in the native shell, an IPC process on the device, a local embedded server, or
a remote HTTP server. Whatever it is, it owns state. The platform does not
decide where state lives — the application does:

- `foundation_wasm_ui` components do NOT own state by default. They receive
  state from the Rust side and render it. Only `mount-data` and `mount-stream`
  bindings explicitly declare "I need to send state to get state-backed
  updates." This means the majority of the UI is stateless and cache-friendly.
- Users define what state matters when building the app. Components can cache
  state for performance, but it's deliberate, not forced.
- The platform's job is transports and caching. It does not own state machines,
  conflict resolution, or sync protocols. Those are the backend's domain.

Users can bring whatever they want — state machines, sync protocols, CRDTs,
event sourcing — as additional tools layered on top of the platform. A native
integrated system for state and sync may come later; for now the focus is
getting the core platform correct.

### Communication model

`foundation_wasm_ui` only dictates how changes are **streamed to the frontend
for rendering** (DomOps, HTML fragments, Arrow/JSON payloads, morph patches).
Users are free to decide how and what they wish to communicate for everything
else:

- **RPC** — if users want request-response semantics, the platform provides
  tooling for that over the appropriate transport.
- **WebSocket** — bidirectional streaming; the platform makes it
  straightforward.
- **HTTP** — standard fetch/request; the platform's custom protocol and
  resource lanes handle this.
- **IPC** — for same-device communication to a local Rust process; the
  platform builds a resilient and efficient IPC process.
- **SSE** — unidirectional server-pushed streaming; already supported by
  `foundation_wasm_ui`'s server module.

The platform does not decide the communication model. It provides the tooling,
constructs, and capabilities to make each option pain-free.

### Why this separation

`foundation_wasm_ui`'s existing model is clean: the Rust side executes, the
UI renders. Adding state ownership or communication model decisions to the
platform layer would constrain users unnecessarily. The platform is a host,
not a framework — it provides the lanes, the user drives on them however
they want.
