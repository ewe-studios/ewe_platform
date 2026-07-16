# 01 — Platform architecture and crate boundaries

**Date:** 2026-07-04
**Status:** Resolved

## Decision

We create a new crate `foundation_platform` to own the Tauri integration and
act as the bridge between the Rust backend and the frontend (WASM UI). We adopt
a server-rendered approach inspired by Basecamp's Hotwire Native where
appropriate.

Shared types that multiple foundation crates need live in
`foundation_ui_traits` — the existing dependency-free boundary layer. Tauri-
specific implementation lives in `foundation_platform`. This split keeps the
dependency graph clean and prevents Tauri from leaking into WASM crates.

## Table of Contents

1. [Platform architecture](#platform-architecture)
2. [The Rust shell](#the-rust-shell)
3. [Full dependency graph](#full-dependency-graph)
4. [What lives in `foundation_ui_traits`](#what-lives-in-foundation_ui_traits)
5. [What lives in `foundation_platform`](#what-lives-in-foundation_platform)
6. [What lives in other crates](#what-lives-in-other-crates)
7. [Why `foundation_ui_traits`](#why-foundation_ui_traits)
8. [What `foundation_platform` ties together](#what-foundation_platform-ties-together)

---

## Platform architecture

`foundation_platform` is a Tauri crate. It wraps Tauri's builder, lifecycle
hooks, and primitives in a cross-platform coordination model. It does not
replace or circumvent Tauri — it extends Tauri's lifecycle with:

- **Session backbone** — central coordination bus for navigation, capabilities,
  cache, and rendering ([decision 03](03-session-backbone-transport.md)).
- **Route policy** — user-defined Rust handlers that decide what happens on
  every navigation ([decision 02](02-route-policy-model.md)).
- **Transport lanes** — 7 pre-wired transports carrying `foundation_wasm_ui`'s
  wire protocols ([decision 03](03-session-backbone-transport.md)).
- **Deployment surfaces** — 5 ways to ship application logic, from bundled
  WASM to remote server ([decision 04](04-deployment-surfaces.md)).
- **WebView profiles** — runtime trust boundaries gating platform service
  access per route ([decision 06](06-webview-profiles.md)).
- **Capability registry** — native capabilities (camera, biometrics, file
  picker) registered and permissioned per route
  ([decision 07](07-native-capability-contract.md)).
- **WebView stack manager** — multi-WebView native-stack simulation with
  screenshot swap and background preload
  ([decision 10](10-multi-webview-stack.md)).
- **Offline and sync** — cache tiers, mutation queue, background sync
  ([decision 05](05-offline-and-sync.md)).

Tauri provides the raw primitives (window management, WebView creation, IPC
bridge, event bus, plugin system). `foundation_platform` builds the
coordination layer on top. User code talks to the platform session; the
session talks to Tauri.

---

## The Rust shell

The Rust shell is the universal wrapper that ships with EVERY app. It is a
compiled native binary (`.a`/`.so`/executable) that sits between Tauri and the
user's application code. The shell knows what deployment context it's in and
adapts accordingly.

### What the shell always provides

- Session backbone — route policy, capability dispatch, cache lookups
- Transport lanes — all 7 lanes pre-wired and ready
- WebView bootstrap — `foundation-wasm-ui.js` loaded, runtime initialized
- Capability registry — native capabilities registered and permissioned
- Cache database — SQLite, indexed by route, profile-gated
- Lifecycle management — app startup, shutdown, background/foreground events
- Update management — WASM module and frontend asset hot-swap

### How the shell wraps Tauri

- Wraps Tauri's builder at the API level: `PlatformBuilder::new()` internally
  creates `tauri::Builder::default()`.
- Runs INSIDE Tauri's lifecycle at runtime (registered via `builder.setup()`).
- Tauri owns the event loop, windows, WebViews, and platform integration.
- The shell extends Tauri's lifecycle — it doesn't replace or contain it.

### Boundary principle

The platform never bypasses Tauri. It wraps Tauri's primitives in the session
coordination model. User code talks to the session. The session talks to
Tauri. Tauri's primitives remain the authoritative source for
window/webview/plugin/event state — the session just coordinates them.

---

## Full dependency graph

```
                        foundation_ui_traits
                        ├── DomOp, ProtocolEncoder, Envelope (existing — spec 39)
                        ├── IntoHtml, Html, Part descriptors (existing — spec 39)
                        ├── SessionId, PageIdentity (session scoping)
                        ├── RouteDecision, NavigationIntent (route policy)
                        ├── CapabilityRequest, CapabilityResponse (capability contract)
                        ├── Profile, CachePolicy, Presentation, ViewKind (enums)
                        └── Protocol (extended)
                         ↑            ↑            ↑
                         |            |            |
               foundation_signals  foundation_wasm_ui  foundation_platform
               (uses IntoHtml)   (uses DomOp,        (uses SessionId,
                                  ProtocolEncoder,     PageIdentity,
                                  Envelope,             RouteDecision, etc.
                                  session types for     + depends on Tauri
                                  web-side session)     + depends on foundation_db
                                                        + depends on foundation_auth
                                                        + depends on foundation_nativeapis
                                                        + depends on foundation_arrow
                                                        + depends on foundation_signals
                                                        + depends on foundation_macros)

foundation_platform's Cargo.toml:
  [dependencies]
  tauri = { version = "2", features = [...] }
  foundation_ui_traits = { path = "../foundation_ui_traits" }
  foundation_wasm_ui = { path = "../foundation_wasm_ui" }
  foundation_db = { path = "../foundation_db" }
  foundation_auth = { path = "../foundation_auth" }
  foundation_nativeapis = { path = "../foundation_nativeapis" }
  foundation_arrow = { path = "../foundation_arrow" }
  foundation_signals = { path = "../foundation_signals" }
  foundation_macros = { path = "../foundation_macros" }
```

`foundation_ui_traits` remains dependency-free — pure types, no I/O, no Tauri,
no WASM.

`foundation_wasm_ui` imports the session types it needs for the web-side
session (page identity tracking, protocol selection, profile-aware rendering).
No Tauri dependency.

`foundation_platform` imports the same types for the native-side session
(route policy execution, capability dispatch, cache coordination). Heavy
Tauri dependency.

`foundation_platform` is the integration crate. It imports the shared types
from `foundation_ui_traits`, the UI runtime from `foundation_wasm_ui`, and
every platform service from their respective crates. It wraps them all in the
session backbone and exposes them through Tauri's primitives. This is the
whole point of the crate: deeply integrated with Tauri, tying the entire
foundation stack together for cross-platform apps.

---

## What lives in `foundation_ui_traits`

These types are pure data — no Tauri dependency, no WASM dependency, no I/O.
They are needed by both the web side (`foundation_wasm_ui`) and the native side
(`foundation_platform`).

| Type | What it carries |
|---|---|
| `SessionId` | Opaque session identifier. Generated at app launch. Used for route scoping, page identity checks, stale-message guards. |
| `PageIdentity` | `{ session_id, route, visit_id }`. Identifies a specific page load. Capability requests and responses carry this so results are delivered to the right page. |
| `RouteDecision` | `{ source, presentation, view_kind, protocol, cache_policy, profile, native_view_id, capabilities }`. The output of a route handler. Pure data — the platform executes it. |
| `NavigationIntent` | `{ url, method, source, referrer }`. The input to a route handler. Pure data — the session creates it from intercepted events. |
| `CapabilityRequest` | `{ id, page_identity, capability_name, action, payload, permissions }`. A request from the web side for a native capability. |
| `CapabilityResponse` | `{ id, page_identity, status, payload_or_error }`. The result. Delivered scoped to the requesting page. |
| `Protocol` enum | `Columnar \| Arrow \| ArrowIpc \| Json \| Html \| CustomBinary`. `Arrow` = raw RecordBatch binary (single batch, no streaming metadata). `ArrowIpc` = Arrow IPC streaming format (multi-batch, Schema + RecordBatch messages + EOS, interoperable with pyarrow/Flight). Already partially in `foundation_ui_traits` via `ProtocolEncoder`. Extended with all protocol variants. |
| `Profile` enum | `App \| TrustedRemote \| UntrustedRemote \| Auth \| Devtools`. The WebView profile taxonomy. Needed by both the session (gating access) and the UI runtime (CSP, origin policy). |
| `CachePolicy` enum | `CacheFirst \| NetworkFirst \| OnlineOnly \| LocalOnly \| StaleWhileRevalidate`. Pure enum, no cache implementation. |
| `Presentation` enum | `Morph \| Push \| Modal \| Replace \| External \| Root`. How navigation is presented. The session decides; the platform executes. |
| `ViewKind` enum | `WebView \| Native`. What kind of view renders this route. `WebView` = platform delivers bytes, `foundation_wasm_ui` handles rendering via Content-Type dispatch. `Native` = platform instantiates a registered native OS view component (SwiftUI, Jetpack Compose, etc.). The session decides; the platform creates the view container. Content format (DomOps, HTML, Arrow, WASM module, etc.) is `foundation_wasm_ui`'s concern — the platform doesn't branch on it. |

---

## What lives in `foundation_platform`

These types are Tauri-specific or platform-implementation-specific:

| Type | Why it stays in `foundation_platform` |
|---|---|
| `PlatformSession` struct | Wraps `AppHandle<R>`, holds the route handler chain, capability registry, native view registry, cache handle, transport lane references. Tauri-dependent. |
| `RouteHandler` trait | References `PlatformSession` and `NavigationIntent`. Lives with the platform's session implementation. |
| `PatternRouter` | Impl of `RouteHandler`. Convenience, not core type. Lives with the platform. |
| `FnRouteHandler` | Impl of `RouteHandler` for closures. Convenience. |
| `CapabilityRegistry` | Wraps Tauri's plugin system and command registration. Tauri-dependent. |
| `NativeViewRegistry` | Maps view IDs to platform-native view factories. Uses Tauri's `run_on_main_thread`/`run_on_android_context` hooks to instantiate SwiftUI / Jetpack Compose views. Tauri only provides WebViews — `foundation_platform` builds the native view abstraction on top of Tauri's escape hatches. |
| `CacheManager` | Wraps `foundation_db` (SQLite) and Tauri's filesystem scope. Platform implementation. |
| `WebViewStack` | Manages child WebViews and native view `ScreenSlot`s, screenshot capture, pool. Tauri `Webview<R>` dependent. |
| `UriSchemeProtocol` adapter | Tauri-specific `ewe://` transport layer. |
| `PlatformBuilder` | Wraps `tauri::Builder`, registers hooks, protocols, commands. |
| `PlatformBundleGenerator` | Build tool that targets Tauri's packaging pipeline. |
| Proc macros | `#[platform_bin]`, `#[platform_worker]`, `#[platform_service]`, `#[platform_capability]`. Generate platform-specific entrypoints. |
| Tauri command wrappers, event adapters, plugin integrations | All platform implementation. |

---

## What lives in other crates

### `foundation_wasm_ui` (existing, spec 39)

The UI runtime: signals, templates, DOM operations, morph engine, columnar
encoder/decoder, event runtime, custom protocols (DomOps, columnar v1, HTML
fragments). The web-side session extends this — the session becomes the
central bus for navigation intents, bridge messages, and rendering updates.

### `foundation_signals` (existing, spec 39)

Reactive signal graph. Templates bind to signals; when signals change, the DOM
updates. `foundation_wasm_ui`'s `html!` macro and template system depend on
this.

### `foundation_db` (existing)

Database abstraction: SQLite (local), Turso (edge), D1 (Cloudflare), R2
(object storage), KV (key-value). The platform's cache uses SQLite through
`foundation_db`.

### `foundation_auth` (existing)

Authentication: `AuthManager`, credential store, token management. The
platform's session backbone uses this for auth token attachment on remote
requests and profile-gated auth API access.

### `foundation_nativeapis` (existing)

Native API abstraction: platform watchers, IPC channels, OS-level APIs. The
platform's capability registry wraps this for per-route, profile-gated access.

### `foundation_arrow` (existing)

Arrow RecordBatch binary encoding/decoding. Used by the platform for structured data
payloads over the native shell IPC lane and the custom protocol lane.

### `foundation_http` (existing)

HTTP client and server abstractions. Used by the platform's transport lanes
for remote fetch, SSE, and WebSocket connections.

### `foundation_macros` (existing)

All proc macros and derive macros. Per project convention, macros go here, not
in companion `*_macros` crates. The platform's entrypoint macros
(`#[platform_bin]`, etc.) live here.

---

## Why `foundation_ui_traits`

### Why not `foundation_wasm_ui`

Putting session types in `foundation_wasm_ui` would force the dependency
graph: `foundation_platform → foundation_wasm_ui`. That works (platform
depends on UI; platform ties everything together). But `foundation_wasm_ui`
is a UI runtime crate — it owns signals, templates, DOM operations, and
rendering. Session backbone types (route decisions, capability contracts,
page identity) are coordination concepts, not UI concepts. They don't belong
in the UI crate.

### Why not a new crate

Spec 39 already established this pattern (decision 012). `foundation_ui_traits`
is the "shared types between crates" layer. Creating a new
`foundation_platform_traits` crate would fragment the shared-type surface
unnecessarily. The session backbone types are a natural extension of the
existing crate's purpose: "types that are too foundational for any one
consumer crate, too specific for `foundation_core`, and need to be available
everywhere without pulling in heavy dependencies."

### What was already there

`foundation_ui_traits` already owned (from spec 39): `DomOp`, `ProtocolEncoder`,
`Envelope`, `IntoHtml`, `Html`, `Part` descriptors. Adding session types here
extends its role naturally: "shared types that multiple foundation crates
need, with no runtime dependencies."

---

## What `foundation_platform` ties together

`foundation_platform` is the integration crate. It imports the shared types
from `foundation_ui_traits`, the UI runtime from `foundation_wasm_ui`, and
every platform service from their respective crates. It wraps them all in the
session backbone and exposes them through Tauri's primitives.

This is the whole point of the crate: deeply integrated with Tauri, tying the
entire foundation stack together for cross-platform apps. Every other
foundation crate does one thing well. `foundation_platform` is the crate that
makes them all work together on desktop and mobile.
