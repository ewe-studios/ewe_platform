# Foundation Synthesis: `foundation_platform`

## Purpose

This document synthesizes the preceding foundation research into a coherent target
architecture for specification 52. It answers the question:

> Given `foundation_wasm_ui`, Hotwire Native's lessons, and Tauri's host model,
> what should `foundation_platform` actually be?

This is still an architectural foundation, not a final implementation decision.
Final decisions should be written after the open questions are resolved.

## One-sentence architecture

`foundation_platform` is the Tauri-based platform host that packages and serves
`foundation_wasm_ui`, coordinates route/navigation policy, exposes native
capabilities safely, and owns offline/cache/sync/resource lanes around the
WebView.

## Core synthesis

The three explored foundations fit together as follows:

```text
Basecamp / Hotwire Native teaches:
  URL/route-driven native presentation, bridge components, server-driven screens,
  active page identity, and path configuration.

Tauri provides:
  native shell, WebView hosting, Rust backend, commands, events, custom protocols,
  plugins, asset packaging, and desktop/mobile build integration.

foundation_wasm_ui provides:
  Rust-first UI runtime, signals, templates, DOM operations, web components,
  morphing, event runtime, server-driven protocols, and WASM/JS DOM integration.
```

The target shape is:

```text
foundation_platform
        │
        ├── hosts and packages foundation_wasm_ui
        ├── provides Tauri shell/bootstrap/runtime policy
        ├── maps app routes to presentation/cache/security policy
        ├── mediates native capabilities through typed contracts
        ├── serves bundled/cached/generated/remote resources
        ├── coordinates offline cache and sync behavior
        └── exposes explicit transport lanes
```

## Layered target architecture

```text
┌────────────────────────────────────────────────────────────────────┐
│ Application                                                        │
│ - routes, business logic, screens, auth, domain workflows          │
└──────────────────────────────┬─────────────────────────────────────┘
                               │
┌──────────────────────────────▼─────────────────────────────────────┐
│ foundation_platform                                                │
│ - Tauri app bootstrap                                              │
│ - route/presentation policy                                        │
│ - WebView profiles                                                 │
│ - resource/custom protocol serving                                 │
│ - native capability registry                                       │
│ - offline/cache/sync orchestration                                 │
│ - command/event wrappers                                           │
│ - packaging/dev/build integration                                  │
└───────────────┬──────────────────────────────────────┬─────────────┘
                │                                      │
┌───────────────▼────────────────┐      ┌──────────────▼─────────────┐
│ Tauri / native host             │      │ Rust platform services      │
│ - windows/webviews              │      │ - network                   │
│ - IPC/events                    │      │ - filesystem                │
│ - custom protocols              │      │ - database/cache            │
│ - plugins/permissions           │      │ - sync/mutation queue       │
│ - mobile/desktop lifecycle      │      │ - Arrow/data projections    │
└───────────────┬────────────────┘      └──────────────┬─────────────┘
                │                                      │
┌───────────────▼──────────────────────────────────────▼─────────────┐
│ WebView                                                             │
│ - foundation-wasm.js                                                │
│ - foundation-wasm-ui.js                                             │
│ - application WASM                                                  │
│ - DOM rendering                                                     │
│ - events / web components / morphing / streamed updates             │
└────────────────────────────────────────────────────────────────────┘
```

## What `foundation_platform` is not

It is not:

- a replacement for `foundation_wasm_ui`;
- a new UI component framework;
- a Hotwire Native clone;
- a server-only app shell;
- a blanket Arrow-over-everything transport;
- a native SwiftUI/Compose renderer by default;
- a generic Tauri wrapper with no opinion about routes, cache, or security.

## Application modes to support

The platform should support multiple modes. The architecture should not be biased
toward only one.

### 1. Bundled local WASM app

```text
Tauri package -> local HTML/JS/WASM -> foundation_wasm_ui -> DOM
```

Use for:

- offline-capable tools;
- local-first screens;
- apps with mostly local state;
- predictable packaged UI.

### 2. Server-rendered app hosted in Tauri

```text
Remote server -> HTML document/fragments -> WebView -> morph/hydrate/islands
```

Use for:

- SaaS/content/product flows;
- server-owned forms;
- flows that benefit from no app-store release for UI changes.

### 3. Server-driven DOM operation stream

```text
Server/local service -> DomOps/protocol stream -> JS runtime -> DOM
```

Use for:

- fine-grained live updates;
- Rust server-driven screens;
- collaborative/live data surfaces.

### 4. Hybrid local/server app

```text
Local app shell + cached data + remote patches + offline mutation queue
```

Use for:

- mobile productivity apps;
- intermittently connected workflows;
- apps where server state matters but local continuity is required.

### 5. Cached/offline replay mode

```text
Route request -> cache policy -> cached HTML/DomOps/data projection -> UI
```

Use for:

- offline reads;
- restoring last known state;
- low-connectivity mobile use;
- graceful degraded behavior.

## Route policy as the central abstraction

The strongest synthesis is that `foundation_platform` should be route-policy
centric.

A route policy should answer:

- What is the logical route/app location?
- Is it local, remote, cached, generated, or external?
- Should it open in the same WebView, a modal, a new window, or the system
  browser?
- Should navigation be push, replace, refresh, root reset, or no-op?
- Which render mode is preferred: bundled WASM, full HTML, fragment+morph,
  DomOps stream, island hydration, or data projection?
- What offline behavior applies: cache-first, network-first, local-only,
  online-only, stale-while-revalidate?
- Which native capabilities are allowed?
- Which WebView profile/security level applies?
- Which auth/session policy applies?
- Which transport/protocols are allowed?

A possible conceptual shape:

```text
RoutePolicy {
  route_match,
  source_policy,
  presentation_policy,
  render_policy,
  offline_policy,
  capability_policy,
  security_profile,
  cache_policy,
  transport_policy,
}
```

This adapts Hotwire's path configuration idea without limiting the platform to
server-rendered HTML.

## Communication lanes

The platform should make communication lanes explicit.

### Control lane

Tauri command IPC. Small typed request/response operations.

Examples:

- request native file dialog;
- get app/device status;
- begin sync;
- request permission;
- trigger native share sheet;
- ask for cached route metadata.

### Notification lane

Tauri events. One-way or broadcast notifications.

Examples:

- sync progress;
- network changed;
- cache invalidated;
- mutation queue drained;
- auth session changed;
- native capability completed.

### Rendering lane

`foundation_wasm_ui` protocols and HTML/morph mechanisms.

Examples:

- local signal updates;
- server DomOps;
- HTML fragments;
- web component hydration;
- morph patches.

### Resource lane

Custom protocol/fetch-backed resources.

Examples:

- runtime JS/WASM;
- cached HTML;
- route config;
- local generated projections;
- authenticated resources mediated by Rust.

### Storage/data lane

Rust-owned database/cache/data services.

Examples:

- SQLite/foundation_db;
- file cache;
- Arrow IPC blobs;
- mmap or append-only logs where appropriate;
- mutation queue;
- index/search/vector stores.

### Optional native acceleration lane

Future lane for direct platform/native integration where Tauri abstractions are
not enough.

Examples:

- platform-specific background services;
- high-performance native media/data APIs;
- UniFFI-style wrappers around Rust core for non-WebView native surfaces.

This lane should remain optional until justified by concrete requirements.

## Capability bridge model

Hotwire Bridge Components suggest a good shape, but `foundation_platform` can
make it Rust/Tauri-native.

A platform capability should be:

- named;
- typed;
- permissioned;
- route/session scoped;
- auditable;
- testable;
- revocable;
- able to return success, failure, cancellation, or progress.

Example categories:

- filesystem/document picker;
- camera/media capture;
- biometrics;
- notifications;
- clipboard;
- share sheet;
- deep links;
- secure storage;
- background sync triggers;
- app/window controls;
- printing;
- platform metadata.

A conceptual message shape:

```text
CapabilityRequest {
  id,
  route_id,
  session_id,
  capability,
  action,
  payload,
  required_permissions,
}

CapabilityResponse {
  id,
  route_id,
  session_id,
  status,
  payload_or_error,
}
```

The page/session identity is essential. A result for an old page should not update
a newly navigated page by accident.

## Offline-first model

The hybrid architecture should treat offline behavior as a first-class platform
concern rather than a screen-level afterthought.

Important pieces:

- route cache policy;
- resource cache;
- rendered UI cache where useful;
- local data cache;
- mutation queue;
- conflict/reconciliation strategy;
- background/foreground sync lifecycle;
- stale data indicators;
- cache invalidation and versioning;
- server capability negotiation;
- authentication expiration handling.

A realistic flow:

```text
Route requested
        │
        ├── if online and policy allows: fetch remote/latest
        │       ├── update cache
        │       └── render fresh result
        │
        ├── if offline or fetch fails and cache allowed:
        │       ├── load cached HTML/DomOps/data projection
        │       └── render stale/offline state
        │
        └── if mutation submitted offline:
                ├── validate locally
                ├── enqueue mutation
                ├── optimistic UI update where safe
                └── replay when online/app active
```

## Data model and Arrow positioning

Arrow should be positioned as a data-layer and high-throughput protocol tool, not
as a magic universal bridge.

Recommended stance:

- use `foundation_wasm_ui`'s existing DomOp/columnar/custom-binary paths for UI
  operation batches;
- use real Arrow IPC for structured data payloads where it is valuable;
- keep large datasets in Rust-owned cache/storage;
- expose renderable slices, summaries, or projections to the WebView;
- use custom protocol/fetch or runtime-specific binary paths for WebView delivery;
- measure before optimizing Tauri command IPC for binary payloads.

## Adopt / reject / defer

### Adopt now as architectural direction

- Tauri as the host shell.
- `foundation_wasm_ui` as the UI/runtime foundation.
- Route-policy-centric platform model.
- Hotwire-inspired bridge component/capability pattern.
- Custom protocol/resource lane.
- Offline/cache policy as first-class.
- Explicit communication lanes.
- Page/session identity on bridge messages.

### Reject as default assumptions

- Server-only state.
- SPA-only frontend model.
- HTML-only server-driven UI.
- Native SwiftUI/Compose rendering as the default target.
- Large data through JSON IPC.
- Unscoped global native bridge calls.
- Claims of direct native pointer sharing into WebView JS.

### Defer until concrete need

- UniFFI/direct native wrapper lane.
- Native-rendered alternative to DOM applicator.
- Multiple concurrent WebViews per navigation stack.
- App-store update bypass policy for remote executable UI.
- Background services requiring platform-specific APIs.
- CRDT/event-sourcing conflict model.

## Documentation implication

Before implementation, decisions should be written around these seams:

1. route policy model;
2. WebView profile/security model;
3. runtime asset packaging;
4. custom protocol/resource model;
5. command/event bridge model;
6. capability registry model;
7. offline/cache/sync model;
8. render mode selection;
9. mobile lifecycle constraints;
10. testing strategy.

## Summary

The solid architecture is a layered hybrid:

```text
Tauri gives us the secure platform shell.
foundation_wasm_ui gives us the UI/runtime protocol system.
Hotwire Native gives us navigation and bridge design lessons.
foundation_platform binds these into route-aware, offline-capable,
security-conscious desktop/mobile applications.
```

This should be the basis for future decision documents and implementation work.
