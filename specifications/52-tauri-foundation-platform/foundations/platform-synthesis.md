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

## Adopt / explore / defer

### Adopt now as architectural direction

- Tauri as the host shell.
- `foundation_wasm_ui` as the UI/runtime foundation.
- Route-policy-centric platform model.
- Hotwire-inspired bridge component/capability pattern.
- Custom protocol/resource lane.
- Offline/cache policy as first-class.
- Explicit communication lanes.
- Page/session identity on bridge messages.

### Explore and integrate as needed

These are areas where common defaults don't fit our context well. We are not
rejecting them — we are exploring alternatives and keeping all options open.
Nothing is off the table; we will add whatever proves useful.

- Server-only state — we also want local-first and hybrid state.
- SPA-only frontend model — we also want server-driven and multi-mode rendering.
- HTML-only server-driven UI — we also want DomOps, columnar protocol, custom
  binary batches, real Arrow IPC, and WASM signals as update paths.
- Native SwiftUI/Compose rendering as the default target — we may use native
  rendering for specific high-performance surfaces when it adds value.
- Large data through JSON IPC — we explore Arrow, binary protocols, and custom
  resource lanes for bulk data.
- Unscoped global native bridge calls — we scope by route/session/page identity.
- Claims of direct native pointer sharing into WebView JS — we evaluate actual
  zero-copy and copy-minimized mechanisms as they become available.

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

## Design principles for `foundation_platform`

These principles are about how the three foundations (`foundation_wasm_ui`,
Tauri, Hotwire Native lessons) bind together. They are platform design
decisions, not facts about what any single foundation already provides.

### Central backbone: the platform session as the coordination spine

Hotwire Native's Session pattern (verified against iOS `Session.swift` and
Android `Session.kt`) is an inspiration, not gospel. The key idea is a central
coordination backbone that spans both sides of the platform — the WebView/web
side and the native/Rust side — and everything plugs into it.

This backbone should be designed into both crates, not bolted on later:

**Tauri already provides the raw primitives, not the coordination model.**
Verified against the Tauri source (`manager/mod.rs`, `state.rs`, `app.rs`):

- `AppManager` — the central "god struct" that owns `WindowManager`,
  `WebviewManager`, `PluginStore`, `StateManager`, `Listeners`, `ResourceTable`,
  `Config`. It handles window/webview lifecycle, event emit/listen, asset
  serving + CSP, plugin init, state management, and IPC dispatch.
- `StateManager` — a type-indexed global container (`HashMap<TypeId, Pin<Box<dyn
  Any>>>`). Any `Send + Sync + 'static` type can be inserted once via
  `manage<T>()`. Commands access it as `State<T>`. It's a dependency injection
  container, not a session coordinator — no lifecycle, no route scoping, no
  message routing.
- `Listeners` / event system — typed pub-sub with target scoping (`EventTarget::Any`,
  `Window { label }`, `Webview { label }`). Global and per-window/webview
  listeners. The raw event bus, nothing more.

What Tauri does NOT provide: route/navigation policy, session lifecycle
management, capability registry with permission scoping, bridge component
message routing, cache/offline policy, page/screen identity tracking. These are
what `foundation_platform` builds on top of Tauri's raw primitives.

**Web side (`foundation_wasm_ui`).** The existing JS runtime already has a
dispatch loop, protocol framing, event routing, and DOM operation application.
The platform session extends this — it becomes the central bus that routes
navigation intents, bridge messages, capability requests, cache lookups, and
rendering updates. `foundation_wasm_ui` provides the web-side half of the
backbone, making its existing capabilities (DomOps, morphing, signals,
templates, event runtime) available through a single coordination surface.

**Native side (`foundation_platform`).** The platform crate provides the
native-side half — route policy, capability registry, custom protocol serving,
cache integration, native stack navigation, and the bridge to Swift/Kotlin
when native code is needed. It plugs into Tauri's `AppManager` (state, events,
IPC, custom protocols) but wraps them in the session coordination model that
Tauri itself does not provide.

**Both sides connect through the session.** The web-side session and
native-side session are peers in the same coordination graph. A user action
on the web side (link click, form submit, bridge component call) flows into
the session backbone. The session decides: is this a local WASM response, a
cached replay, an IPC call to the native shell, a remote server fetch, or a
native stack push? The decision flows back to the web side through the
rendering lane.

**Subsystems plug in as peers.** Rendering, networking, caching, native
capabilities, and custom protocols don't talk directly to each other. They
register with the session backbone and communicate through it. A cache hit
doesn't bypass the session to inject HTML into the DOM — it tells the session
"I have cached content for this route," and the session routes it through the
rendering lane. A native capability result doesn't call back into JS directly —
it returns to the session, which delivers it scoped to the correct route/page.

**Enhancing what's already there.** This doesn't replace any of
`foundation_wasm_ui`'s capabilities. It wraps them in a coordination model
that makes them composable with the platform services `foundation_platform`
provides. The existing protocol layer, rendering modes, morphing engine, event
runtime, and server-driven hooks remain exactly as they are — they just gain a
central bus that other things can plug into for their various needs.

Basecamp's Session/Navigator split is a useful reference point, but our design
is our own: a Rust-and-WASM-native backbone, not a Swift/Kotlin shell around
a Turbo web app. The web side runs in `foundation_wasm_ui`. The native side
runs in `foundation_platform`. The session is the seam between them.

### State ownership is simple and already decided

`foundation_wasm_ui` has a clean model that `foundation_platform` should
preserve, not complicate:

- **The server owns all state.** "Server" here means wherever the application
  logic runs: it could be WASM in the WebView, Rust in the native shell, an IPC
  process on the device, a local embedded server, or a remote HTTP server.
  Whatever it is, it owns state. The platform does not decide where state
  lives — the application does.
- **Communication model is the user's choice.** `foundation_wasm_ui` only
  dictates how changes are streamed to the frontend for rendering (DomOps,
  HTML fragments, Arrow/JSON payloads, morph patches). Users are free to
  decide how and what they wish to communicate: RPC, WebSocket, HTTP, IPC,
  SSE, or whatever fits their application. The platform provides the tooling,
  constructs, and capabilities to make each option pain-free — if users want
  IPC, the platform builds a resilient and efficient IPC process; if they
  want WebSocket, the platform makes that straightforward. The platform does
  not decide the communication model; it ensures each one works well.
- **The platform's job** is to provide the transports and caching. It does not
  own the state machine, conflict resolution, or sync protocol — those are the
  backend's domain. Nothing stops users from bringing whatever they want:
  state machines, sync protocols, CRDTs, event sourcing — they're additional
  tools layered on top of the platform. A native integrated system for state
  and sync may come later, but for now the focus is on getting the core
  platform correct. The backend (WASM, Rust, server) decides how state,
  storage, and sync work — whether via libsql/Turso, SQLite, mmap, or
  something else.

### Offline: local WASM execution is the foundation

Offline support is more than caching rendered pages. Because `foundation_wasm_ui`
compiles to WASM, it can run on-device in many places:

- **WebView** — WASM bundled with the app, runs locally in the WebView.
- **Native shell** — WASM loaded by the native shell, runs in-process with
  zero-copy Arrow access.
- **IPC process** — WASM in a local Rust process on the device.
- **Mobile backend service** — WASM running as an on-device background service.

In all of these, the WASM is local. There is no network round-trip. The WASM
generates responses, renders pages, handles state, processes actions — entirely
on-device. Offline is not a fallback; it's the default when WASM runs locally.
This covers the full application without any additional effort beyond deploying
the WASM to the device.

For remote backends (WASM or server running elsewhere), the platform adds
rendered page caching:

1. The platform caches rendered pages in a local SQLite database, indexed by
   route.
2. When offline, the platform serves the cached page instantly (fast perceived
   response).
3. Once connectivity returns, the backend does whatever it needs: full replace
   of the page, or incremental diffing/updating of elements.

The platform should provide simple, fast APIs to store and retrieve cached
rendered content indexed by route. The backend owns the update strategy. The
platform just makes both paths — full replace and surgical update — possible.

### Navigation: study then decide

Navigation in a hybrid web/native app is worth getting right. Rather than
leaving it as an open question or pre-deciding an abstraction, the approach is:

1. Study how Basecamp Hotwire Native maps URLs to native navigation stacks.
2. Survey Tauri-specific approaches (window management, WebView history, custom
   protocol routing, event-driven navigation).
3. Create a decision document with the options and trade-offs.
4. Decide.

Until then, the platform defaults to single-WebView browser-style navigation
and adds structured route metadata incrementally.

### Protocols over transports: already decided

The protocol layer (custom binary, columnar v1, JSON, Arrow IPC, HTML
fragments) is defined by `foundation_wasm_ui`. `foundation_platform` does not
re-litigate this. It adds the **transports** that matter for Tauri and mobile:

- Tauri command IPC (control lane);
- Tauri events (notification lane);
- Tauri custom protocol (resource lane);
- native shell IPC (zero-copy data lane, same process);
- local embedded server in the mobile app (WebSocket/HTTP lane);
- SSE/WebSocket to remote servers;
- browser fetch (standard web resource loading).

Protocol decisions stay in `foundation_wasm_ui`. Transport decisions are the
platform's job.

### Background workers: provide options, don't pick one

Native background workers can be implemented several ways:

- Pure Rust/Tauri async tasks for foreground and active-app work.
- Tauri plugins for wrapped platform APIs.
- UniFFI-style direct native wrappers for high-performance or platform-specific
  background services.

`foundation_platform` should provide APIs that make each option possible, show
how each works and what it's best for, and let application developers choose the
right one for their context. Nothing should be an either/or. Each has a
place — the platform documents the trade-offs and provides the integration
surface.

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
