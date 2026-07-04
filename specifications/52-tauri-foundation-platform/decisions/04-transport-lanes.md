# 04 — Transport lanes: protocols stay in wasm_ui, transports added by platform

**Date:** 2026-07-04
**Status:** Resolved

### Decision

`foundation_wasm_ui` owns the protocol layer (wire formats). `foundation_platform`
owns the transport layer (how bytes move). Platform transports carry UI
protocols — they don't invent new ones.

The shell ships with every app regardless of deployment model. It owns all
communication between WebView, native APIs, and remote servers.

### Protocol vs transport

**Protocols** (defined by `foundation_wasm_ui`, NOT re-litigated by the platform):
- Custom binary batch instructions (DomOps)
- Columnar v1 (wasm-loop, no-std, TypedArray-friendly DOM operation batches)
- JSON DOM operation representation
- Real Apache Arrow IPC (for structured data payloads, not UI ops)
- HTML fragments (for server-rendered markup)
- Event payloads and function-call ABI frames

**Transports** (added by `foundation_platform`):
- Tauri command IPC — control lane for small typed request/response (auth, file picker,
  sync triggers, app metadata)
- Tauri events — notification lane for lifecycle, progress, invalidation (sync status,
  online/offline, cache invalidated, mutation queue drained)
- Tauri custom protocol — resource lane for bundled/cached/generated/remote assets
  served through the session backbone
- Native shell IPC — zero-copy data lane for same-process Rust↔native communication
  (Arrow IPC delivered as shared-memory `ArrayBuffer`)
- Local embedded server — WebSocket/HTTP lane when the Rust backend runs as a
  standalone process on the device
- SSE/WebSocket to remote servers — standard web streaming for server-driven updates
- Browser fetch — standard web resource loading

### All lanes are v1

No lane is deferred. The shell provides all of them. The user chooses which to
use per route/component. The platform ensures each one works correctly and
aligns with Tauri's primitives.

### What the shell owns

Regardless of deployment model (shell + compiled app as static lib, shell +
WASM, or shell that loads a remote app), the shell owns:

- Communication between WebView and native APIs via Tauri's primitives
- Communication between the app and remote servers via the resource lane
- Communication between IPC peers via the native shell IPC lane
- Protocol selection per payload (the Rust side decides; the shell carries)

### Where protocol decisions live

Protocol selection is a backend concern. Component authors don't pick formats —
the WASM, server, or native backend already decides the protocol per payload.
`mount-data` / `mount-stream` bindings can optionally override, but the
default is the efficient Arrow/custom binary format already built into the
platform.
