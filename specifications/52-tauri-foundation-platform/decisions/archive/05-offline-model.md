# 05 — Offline model: local WASM execution + rendered page caching

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Two-tier offline model. Tier 1: local WASM execution (offline is the default).
Tier 2: rendered page caching (offline is a fallback for remote content).

### Tier 1 — Local WASM execution

Because `foundation_wasm_ui` compiles to WASM, it can run on-device in multiple
configurations:

- **WebView** — WASM bundled with the app, runs locally in the WebView.
- **Native shell** — WASM loaded by the native shell, runs in-process with
  zero-copy Arrow access.
- **IPC process** — WASM in a local Rust process on the device.
- **Mobile backend service** — WASM running as an on-device background process.

In all cases, the WASM is local. There is no network round-trip. The WASM
generates responses, renders pages, handles state, processes actions — entirely
on-device. Offline is not a fallback; it's the default. This covers the full
application without any additional effort beyond deploying the WASM to the
device.

### Tier 2 — Rendered page caching (remote backends)

For when the backend runs remotely (WASM or server elsewhere):

1. The platform caches rendered pages in a local SQLite database, indexed by
   route.
2. When offline, the platform serves the cached page instantly (fast perceived
   response).
3. Once connectivity returns, the backend does whatever it needs — full page
   replacement or incremental diffing/updating of changed elements.

The platform provides APIs to store and retrieve cached rendered content by
route. The backend owns the update strategy. The platform makes both paths
possible.

Two update strategies, selected per route:

- **Full replace** — the snapshot provides instant responsiveness; when the
  server sends the latest page, the platform replaces the whole view. Best
  for content-driven screens where surgical patching adds no value.
- **Surgical update** — the client sends cached state to the backend
  ("user was on step 3 of this form with these values"), and the backend
  surgically patches only what changed. Best for preserving in-progress work.

### Backend owns state, platform owns transport

The platform provides the cache infrastructure (store, retrieve, invalidate).
The backend decides what to cache, when to invalidate, and what update
strategy to use. The platform's job is transports and caching — not state
machines, conflict resolution, or sync protocols.
