---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F01-session-backbone"
this_file: "specifications/52-tauri-foundation-platform/features/F01-session-backbone/feature.md"

status: pending
priority: critical
created: 2026-07-17

depends_on:
  - "F00-crate-skeleton"

tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# F01 — Session backbone

## Overview

Implement the `PlatformSession` — the central coordination bus that spans both
`foundation_wasm_ui` (web side) and `foundation_platform` (native side).
Subsystems register with the session; the session routes navigation intents,
capability requests, and cache lookups through the handler chain.

[Decision 03](../decisions/03-session-backbone-transport.md) defines the
session backbone architecture.

## Dependencies

Depends on:
- `F00-crate-skeleton` — Uses `NavigationIntent`, `RouteDecision`, `RouteHandler`, `PlatformBuilder`

Required by:
- `F02-route-handler` — Handlers register with the session
- `F03-ewe-protocol` — Custom protocol handler routes through session
- Every subsequent feature

## Requirements

### 1. `PlatformSession` struct

```rust
// foundation_platform/src/session.rs

pub struct PlatformSession<R: Runtime> {
    /// Tauri app handle for window/webview/plugin access
    app_handle: AppHandle<R>,

    /// Route handler chain — iterated in registration order, first match wins
    route_handlers: Vec<Box<dyn RouteHandler>>,

    /// Capability registry — capability name → handler
    capability_registry: HashMap<String, Box<dyn Capability>>,

    /// Cache handle (SQLite, indexed by route)
    cache: CacheManager,

    /// Session identity
    session_id: SessionId,

    /// Active page tracking
    active_page: Mutex<Option<PageIdentity>>,

    /// Connectivity state
    online: AtomicBool,
}
```

### 2. Handler chain registration

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Register a route handler. Handlers are checked in registration order.
    pub fn register_handler(&self, handler: impl RouteHandler) { ... }

    /// Resolve a navigation intent through the handler chain.
    /// Returns the first Some(decision) or the platform default.
    pub fn resolve_route(&self, intent: &NavigationIntent) -> RouteDecision { ... }
}
```

### 3. Navigation interception

The session hooks into Tauri's `on_navigation` callback:

```rust
// In PlatformBuilder setup:
builder.on_navigation(|url| {
    let intent = NavigationIntent {
        url: url.clone(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: session.active_page().map(|p| p.route),
    };
    let decision = session.resolve_route(&intent);
    // false = allow, true = handled by session
    session.execute_decision(&decision)
});
```

### 4. Session lifecycle

From [decision 03](../decisions/03-session-backbone-transport.md) section "Session lifecycle":

```
App launch → Tauri boots
  → setup() hook fires
    → PlatformSession::initialize()
      → Transport lanes pre-wired
      → Capability registry populated
      → Cache database opened
      → Route handlers registered
      → WebView created, foundation-wasm-ui.js injected
      → User's main() called with session handle
        → Rendering loop begins
```

```rust
impl<R: Runtime> PlatformSession<R> {
    pub fn initialize(app_handle: AppHandle<R>) -> Self { ... }
    pub fn on_suspend(&self) { ... }   // emit "background" event
    pub fn on_resume(&self) { ... }    // emit "foreground" event
    pub fn on_shutdown(&self) { ... }  // flush cache, close transports
}
```

### 5. Subsystem peer model

Each subsystem gets a reference to the session. They communicate through it,
not directly:

```
Cache subsystem → session.cache().get(route)  → session routes through rendering lane
Capability     → session.invoke("camera", ...) → session routes to handler → response
Navigation     → session.on_navigate(url)       → session runs handler chain → decides
```

### 6. Session events

```rust
// Events the session emits:
session.emit("navigated", &NavigatedEvent { ... });
session.emit("online", &());
session.emit("offline", &());
session.emit("background", &());
session.emit("foreground", &());
session.emit("shutdown", &());
```

## Architecture

```
PlatformBuilder::new()
  → tauri::Builder::default()
  → .setup(|app| {
        session = PlatformSession::initialize(app.handle());
        // user's #[platform_bin] main(session) called here
    })
```

```
Navigation flow:
  WebView link click
    → on_navigation(url)
    → NavigationIntent constructed
    → session.resolve_route(&intent)
      → iterate route_handlers[]
        → handler.resolve(&intent, &session)
        → Some(decision) → return
    → session.execute_decision(&decision)
      → cache check → presentation → backend query → render
```

## Tasks

### PlatformSession
- [ ] Create `src/session.rs` with `PlatformSession<R>` struct
- [ ] Store `AppHandle<R>`, handler chain vec, capability registry map
- [ ] Initialize cache manager with SQLite
- [ ] Generate `SessionId` at initialization

### Handler chain registration
- [ ] Implement `register_handler()` — push to handlers vec
- [ ] Implement `resolve_route()` — iterate handlers, return first match
- [ ] Implement platform default for unmatched routes (external browser)
- [ ] Implement `execute_decision()` stub — dispatches based on RouteSource
- [ ] Test: register 3 handlers, verify first-match-wins ordering

### Navigation interception
- [ ] Wire `on_navigation` callback in PlatformBuilder setup
- [ ] Construct `NavigationIntent` from intercepted URL
- [ ] Route through session.resolve_route()
- [ ] Return false to allow navigation, or handle via execute_decision

### Session lifecycle
- [ ] Implement `initialize()` — create all subsystems
- [ ] Implement `on_suspend()` / `on_resume()` — event emission
- [ ] Implement `on_shutdown()` — flush cache, close connections
- [ ] Test: full lifecycle (init → navigate → shutdown)

### Session events
- [ ] Define standard session event types
- [ ] Implement `emit()` wrapping Tauri's event system
- [ ] Add page-identity scoping for targeted delivery

### Connectivity tracking
- [ ] Track online/offline state via Tauri or OS APIs
- [ ] Emit `online`/`offline` events on transitions
- [ ] Test: toggle connectivity, verify events fire

## Verification Commands

```bash
cargo check --package foundation_platform
cargo test --package foundation_platform -- session
```
