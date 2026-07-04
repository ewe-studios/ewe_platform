# 17 — Crate boundaries: shared session types in `foundation_ui_traits`

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Session backbone types that are needed by both `foundation_wasm_ui` (web-side
session) and `foundation_platform` (native-side session) live in
`foundation_ui_traits`. This crate is already the shared boundary layer — it
owns `DomOp`, `ProtocolEncoder`, `Envelope`, and `IntoHtml`, serving as the
dependency-free middle that both `foundation_signals` and `foundation_wasm_ui`
depend on. Adding platform session types here extends its role naturally:
"shared types that multiple foundation crates need, with no runtime
dependencies."

### Why `foundation_ui_traits` and not `foundation_wasm_ui`

Putting session types in `foundation_wasm_ui` would force the dependency
graph: `foundation_platform → foundation_wasm_ui`. That works (platform
depends on UI; platform ties everything together). But `foundation_wasm_ui`
is a UI runtime crate — it owns signals, templates, DOM operations, and
rendering. Session backbone types (route decisions, capability contracts,
page identity) are coordination concepts, not UI concepts. They don't belong
in the UI crate.

`foundation_ui_traits` already solves exactly this pattern: types that are
too foundational for any one consumer crate, too specific for
`foundation_core`, and need to be available everywhere without pulling in
heavy dependencies.

### Why `foundation_ui_traits` and not a new crate

Spec 39 already established this pattern (decision 012). `foundation_ui_traits`
is the "shared types between crates" layer. Creating a new
`foundation_platform_traits` crate would fragment the shared-type surface
unnecessarily. The session backbone types are a natural extension of the
existing crate's purpose.

### What moves to `foundation_ui_traits`

These types are pure data — no Tauri dependency, no WASM dependency, no I/O:

| Type | What it carries |
|---|---|
| `SessionId` | Opaque session identifier. Generated at app launch. Used for route scoping, page identity checks, stale-message guards. |
| `PageIdentity` | `{ session_id, route, visit_id }`. Identifies a specific page load. Capability requests and responses carry this so results are delivered to the right page. |
| `RouteDecision` | `{ source, presentation, render_mode, protocol, cache_policy, capabilities, profile }`. The output of a route handler. Pure data — the platform executes it. |
| `NavigationIntent` | `{ url, method, source (link-click/form-submit/server-push/native-gesture), referrer }`. The input to a route handler. Pure data — the session creates it from intercepted events. |
| `CapabilityRequest` | `{ id, page_identity, capability_name, action, payload, permissions }`. A request from the web side for a native capability. |
| `CapabilityResponse` | `{ id, page_identity, status, payload_or_error }`. The result. Delivered scoped to the requesting page. |
| `Protocol` enum | `Columnar \| ArrowIpc \| Json \| Html \| CustomBinary`. Already partially in `foundation_ui_traits` via `ProtocolEncoder`. Extended with all protocol variants. |
| `Profile` enum | `App \| TrustedRemote \| UntrustedRemote \| Auth \| Devtools`. The WebView profile taxonomy. Needed by both the session (gating access) and the UI runtime (CSP, origin policy). |
| `CachePolicy` enum | `CacheFirst \| NetworkFirst \| OnlineOnly \| LocalOnly \| StaleWhileRevalidate`. Pure enum, no cache implementation. |
| `Presentation` enum | `Morph \| Push \| Modal \| Replace \| External \| Root`. How navigation is presented. The session decides; the platform executes. |
| `RenderMode` enum | `WasmApp \| HtmlDocument \| DomOpsStream \| FragmentMorph \| DataProjection`. How content is rendered. The session decides; the UI runtime executes. |

### What stays in `foundation_platform`

These types are Tauri-specific or platform-implementation-specific:

| Type | Why it stays in `foundation_platform` |
|---|---|
| `PlatformSession` struct | Wraps `AppHandle<R>`, holds the route handler chain, capability registry, cache handle, transport lane references. Tauri-dependent. |
| `RouteHandler` trait | References `PlatformSession` and `NavigationIntent`. Lives with the platform's session implementation. |
| `PatternRouter` | Impl of `RouteHandler`. Convenience, not core type. Lives with the platform. |
| `CapabilityRegistry` | Wraps Tauri's plugin system and command registration. Tauri-dependent. |
| `CacheManager` | Wraps `foundation_db` (SQLite) and Tauri's filesystem scope. Platform implementation. |
| `UriSchemeProtocol` adapter | Tauri-specific transport layer. |
| `PlatformBundleGenerator` | Build tool that targets Tauri's packaging pipeline. |
| `#[platform_bin]` / `#[platform_worker]` / `#[platform_service]` macros | Proc macros that generate platform-specific entrypoints. Live in `foundation_macros` or `foundation_platform`'s build tools. |
| Tauri command wrappers, event adapters, plugin integrations | All platform implementation. |

### Dependency graph after the change

```
                    foundation_ui_traits
                    ├── DomOp, ProtocolEncoder, Envelope (existing)
                    ├── IntoHtml, Html, Part descriptors (existing)
                    ├── SessionId, PageIdentity (new — session scoping)
                    ├── RouteDecision, NavigationIntent (new — route policy)
                    ├── CapabilityRequest, CapabilityResponse (new — capability contract)
                    ├── Profile, CachePolicy, Presentation, RenderMode (new — enums)
                    └── Protocol (extended)
                     ↑            ↑            ↑
                     |            |            |
           foundation_signals  foundation_wasm_ui  foundation_platform
           (uses IntoHtml)   (uses DomOp,        (uses SessionId,
                              ProtocolEncoder,     PageIdentity,
                              Envelope)            RouteDecision, etc.
                                                   + depends on Tauri
                                                   + depends on foundation_db
                                                   + depends on foundation_auth
                                                   + depends on foundation_nativeapis)
```

`foundation_wasm_ui` imports the session types it needs for the web-side
session (page identity tracking, protocol selection, profile-aware rendering).
No Tauri dependency.

`foundation_platform` imports the same types for the native-side session
(route policy execution, capability dispatch, cache coordination). Heavy
Tauri dependency.

`foundation_ui_traits` remains dependency-free — pure types, no I/O, no
Tauri, no WASM.

### What `foundation_platform` ties together

`foundation_platform` is the integration crate. Its `Cargo.toml`:

```toml
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
# ... etc
```

It imports the shared types from `foundation_ui_traits`, the UI runtime from
`foundation_wasm_ui`, and every platform service from their respective crates.
It wraps them all in the session backbone and exposes them through Tauri's
primitives. This is the whole point of the crate: deeply integrated with
Tauri, tying the entire foundation stack together for cross-platform apps.
