# 05 — Offline model and cache tiers

**Date:** 2026-07-04
**Status:** Resolved

## Decision

Two-tier offline model. Tier 1: local execution (WASM in WebView or native
shell) — offline is the default, no network needed. Tier 2: rendered page
caching — remote content is cached locally so it's available offline. Cache
policies are per-route, assigned in the `RouteDecision`.

The mutation queue, background sync, and worker lifecycle are separate
decisions: [decision 11](11-background-workers.md) and
[decision 12](12-mutation-queue-and-conflict.md).

## Table of Contents

1. [Offline model: two tiers](#offline-model-two-tiers)
2. [Cache policies: per-route control](#cache-policies-per-route-control)
3. [Cache storage](#cache-storage)
4. [Connectivity lifecycle](#connectivity-lifecycle)
5. [What the platform provides vs what the user provides](#what-the-platform-provides-vs-what-the-user-provides)

---

## Offline model: two tiers

### Tier 1 — Local WASM execution

Because `foundation_wasm_ui` compiles to WASM, it can run on-device in
multiple configurations:

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
  server sends the latest page, the platform replaces the whole view. Best for
  content-driven screens where surgical patching adds no value.
- **Surgical update** — the client sends cached state to the backend ("user
  was on step 3 of this form with these values"), and the backend surgically
  patches only what changed. Best for preserving in-progress work.

---

## Cache policies: per-route control

The `CachePolicy` in `RouteDecision` (defined in [decision
02](02-route-policy-model.md)) controls cache behavior per route:

| Policy | Behavior | Offline behavior | Best for |
|---|---|---|---|
| `CacheFirst` | Serve from cache. Fetch on miss. | Works offline (cached content). | Static content, offline-first apps. |
| `NetworkFirst` | Try network. Fall back to cache. | Serves stale cache. | Dynamic content that should be fresh. |
| `OnlineOnly` | Never cache. Always network. | Fails with offline error. | Real-time data, auth-gated content. |
| `LocalOnly` | Always cache. Never network. | Always works (no network needed). | Bundled content. |
| `StaleWhileRevalidate` | Serve cache, refresh in background. | Serves stale cache. | Content that should feel instant. |

### Cache storage

The cache database stores responses as-encoded: if the server returned
`application/primal-columnar`, the cache stores the columnar bytes. On replay,
the cache returns the exact same bytes with the same `Content-Type`. The
runtime renders them identically — the cache is protocol-transparent.

### Backend owns state, platform owns transport

The platform provides the cache infrastructure (store, retrieve, invalidate).
The backend decides what to cache, when to invalidate, and what update
strategy to use. The platform's job is transports and caching — not state
machines, conflict resolution, or sync protocols.

---

## Cache storage

The cache database stores responses as-encoded: if the server returned
`application/primal-columnar`, the cache stores the columnar bytes. On replay,
the cache returns the exact same bytes with the same `Content-Type`. The
runtime renders them identically — the cache is protocol-transparent.

**Backend owns state, platform owns transport.** The platform provides the
cache infrastructure (store, retrieve, invalidate). The backend decides what
to cache, when to invalidate, and what update strategy to use. The platform's
job is transports and caching — not state machines, conflict resolution, or
sync protocols.

---

## Connectivity lifecycle

The platform manages connectivity state transitions for caching purposes:

```
Online (active)
  │
  ├── Cache serves fresh content from the network
  ├── Cached content is updated in the background
  │
  ▼ App goes to background
Background (suspended)
  │
  ├── Cache is preserved (no eviction)
  ├── Background fetch may refresh stale cache entries (OS-decided)
  │
  ▼ App goes offline
Offline
  │
  ├── Cache serves stale content (CacheFirst, NetworkFirst, StaleWhileRevalidate)
  ├── OnlineOnly routes show an offline error
  │
  ▼ Connectivity returns + app foregrounded
Online (reconnected)
  │
  ├── Cache invalidates stale entries (server decides which)
  ├── Server streams latest state (surgical update if configured)
  └── App reflects current state
```

Mutation queue replay and background sync are handled by
[decision 12](12-mutation-queue-and-conflict.md) and
[decision 11](11-background-workers.md) — they are separate concerns from the
cache tiers.

---

## What the platform provides vs what the user provides

| Layer | Platform provides | User provides |
|---|---|---|
| **Cache storage** | SQLite database, indexed by route, profile-gated. `session.cache().get(route)`, `session.cache().invalidate(routes)`. | What to cache, when to invalidate, which routes are stale. |
| **Cache policies** | `CachePolicy` enum — `CacheFirst`, `NetworkFirst`, `OnlineOnly`, `LocalOnly`, `StaleWhileRevalidate`. Applied per-route in `RouteDecision`. | Which policy per route. |
| **Connectivity** | Connectivity change events. App lifecycle events (foreground/background). Online/offline detection. | React to events — invalidate cache, refresh content. |
| **Offline rendering** | WASM execution on-device (WebView or native shell). No network needed for local content. Cache serves remote content when offline. | Application logic that runs locally. |
