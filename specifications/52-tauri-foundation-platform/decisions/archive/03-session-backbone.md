# 03 — Central session backbone spanning both crates

**Date:** 2026-07-04
**Status:** Resolved

### Decision

A central platform session (inspired by Hotwire Native's `Session` pattern,
verified against iOS `Session.swift` and Android `Session.kt`) spans both
sides — `foundation_wasm_ui` (web side) and `foundation_platform` (native
side). Everything plugs into it as a peer. No subsystem talks directly to
another without the session knowing.

### What it coordinates

- **Navigation intents** — link clicks, form submits, server pushes, native
  gestures. The session intercepts them, runs the route handler chain, and
  executes the resulting `RouteDecision`.
- **Bridge messages** — capability requests, native API calls, component
  registration. The session routes them to the appropriate handler and
  delivers responses scoped to the correct route/page.
- **Cache lookups** — a cache hit tells the session "I have content for this
  route"; the session routes it through the rendering lane. The cache doesn't
  inject into the DOM directly.
- **Native capability results** — return to the session, not to JS directly.
  The session delivers them scoped to the correct route/page/session.

### How it spans both crates

**Web side (`foundation_wasm_ui`).** The existing JS runtime already has a
dispatch loop, protocol framing, event routing, and DOM operation application.
The session extends this — it becomes the central bus for navigation intents,
bridge messages, and rendering updates, making existing capabilities (DomOps,
morphing, signals, templates, event runtime) available through a single
coordination surface.

**Native side (`foundation_platform`).** The platform crate provides route
policy execution, capability registry, custom protocol serving, cache
integration, native stack navigation, and the bridge to Swift/Kotlin. It
plugs into Tauri's primitives (`AppManager`, `StateManager`, `Listeners`,
events) but wraps them in the session coordination model that Tauri does not
provide.

**Both sides are peers.** The web-side session and native-side session are
the same coordination graph. A user action on the web side flows into the
session; the session decides (local WASM, cached replay, IPC, remote fetch,
native stack push); the result flows back through the rendering lane.

### What Tauri already provides

Verified against source (`manager/mod.rs`, `state.rs`, `app.rs`):

- `AppManager` — container: owns `WindowManager`, `WebviewManager`,
  `PluginStore`, `StateManager`, `Listeners`, `ResourceTable`, `Config`.
- `StateManager` — type-indexed dependency injection (`TypeId → Pin<Box<dyn
  Any>>`). No lifecycle, no route scoping, no message routing.
- `Listeners` — typed pub-sub with target scoping. Raw event bus.

What Tauri does NOT provide: route/navigation policy, session lifecycle
management, capability registry, bridge component routing, cache/offline
policy, page/screen identity tracking. `foundation_platform` builds these
on top of Tauri's raw primitives.

### Subsystem peer model

Rendering, networking, caching, native capabilities, and custom protocols
don't talk directly to each other. They register with the session backbone
and communicate through it:

```
Cache subsystem ──→ "I have cached content for /route"
                    Session ──→ routes through rendering lane → DOM

Native capability ──→ "biometric auth result"
                    Session ──→ scoped delivery to correct route/page → WebView

WebView action ──→ "user clicked link"
                  Session → route handler chain → decide → execute
```

### Design is our own

Hotwire Native's Session/Navigator split is a reference point, not a
blueprint. Our design is a Rust-and-WASM-native backbone, not a Swift/Kotlin
shell around a Turbo web app. The web side runs in `foundation_wasm_ui`.
The native side runs in `foundation_platform`. The session is the seam
between them.
