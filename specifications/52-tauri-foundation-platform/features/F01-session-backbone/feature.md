---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F01-session-backbone"
this_file: "specifications/52-tauri-foundation-platform/features/F01-session-backbone/feature.md"

status: completed
priority: critical
created: 2026-07-17

depends_on:
  - "F00-crate-skeleton"

tasks:
  completed: 28
  uncompleted: 0
  total: 28
  completion_percentage: 100%
---

# F01 — Session backbone

## Overview

Implement `PlatformSession<R>` — the central coordination bus that spans both
`foundation_wasm_ui` (web side) and `foundation_platform` (native side).
Everything plugs into it as a peer. No subsystem talks directly to another
without the session knowing.

[Decision 03](../decisions/03-session-backbone-transport.md) defines the full
session backbone architecture. The subsections below implement each part of
that decision.

## Dependencies

Depends on:
- `F00-crate-skeleton` — Uses all types from `foundation_ui_traits`, `PlatformBuilder`

Required by:
- `F02-route-handler` — Handlers register with the session
- `F03-ewe-protocol` — Custom protocol handler routes through session
- Every subsequent feature

---

## Part A — `PlatformSession` struct

### A.1 — Core struct

```rust
// foundation_platform/src/session.rs

pub struct PlatformSession<R: Runtime> {
    /// Tauri app handle for window/webview/plugin access.
    /// The session coordinates Tauri primitives; it doesn't own them.
    app_handle: AppHandle<R>,

    /// Route handler chain — iterated in registration order, first match wins.
    /// Each handler decides: claim this navigation (Some(decision)) or pass (None).
    route_handlers: RwLock<Vec<Box<dyn RouteHandler>>>,

    /// Capability registry — capability name → registered handler.
    /// Populated at startup, checked at runtime for every capability invocation.
    capability_registry: RwLock<HashMap<String, Box<dyn Capability>>>,

    /// Cache manager — SQLite-backed, indexed by route, profile-scoped.
    /// Initialized at session startup, used by the cache check step in navigation.
    cache: CacheManager,

    /// Session identity — generated once at app launch, never changes.
    /// Used for page identity scoping and stale-message guards.
    session_id: SessionId,

    /// Monotonically incrementing visit counter.
    /// Incremented on every navigation. PageIdentity.visit_id uses this.
    visit_counter: AtomicU64,

    /// The currently active page. None until the first navigation.
    /// Stale-page guard checks this before delivering capability results.
    active_page: RwLock<Option<PageIdentity>>,

    /// Connectivity state. Updated by OS lifecycle events.
    online: AtomicBool,
}
```

**Design notes from [decision 03](../decisions/03-session-backbone-transport.md):**

- `route_handlers` uses `RwLock<Vec<Box<dyn RouteHandler>>>` because:
  handlers register at startup (write), iterate on every navigation (read).
  Read-heavy workload — `RwLock` is correct.
- `capability_registry` is a `HashMap<String, Box<dyn Capability>>` keyed
  by capability name. Lookup is `O(1)`, called on every capability invocation.
- `session_id` is a `u64` generated from `std::time::UNIX_EPOCH` at init.
  It's opaque to user code — used only for scoping.
- `visit_counter` uses `AtomicU64` for lock-free increment on every
  navigation. No contention — only the session thread increments it.

### A.2 — Initialization

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Initialize the session. Called once from `PlatformBuilder`'s setup hook.
    /// Pre-wires all subsystems: transport lanes, capability registry,
    /// cache database, route handlers (user-registered after this call).
    pub fn initialize(app_handle: AppHandle<R>) -> Self {
        let session_id = SessionId(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        Self {
            app_handle,
            route_handlers: RwLock::new(Vec::new()),
            capability_registry: RwLock::new(HashMap::new()),
            cache: CacheManager::new(":memory:"), // or file path from config
            session_id,
            visit_counter: AtomicU64::new(0),
            active_page: RwLock::new(None),
            online: AtomicBool::new(true),
        }
    }
}
```

### A.3 — Wiring into PlatformBuilder

```rust
// foundation_platform/src/builder.rs — add to PlatformBuilder::setup()

impl<R: Runtime> PlatformBuilder<R> {
    pub fn with_session_setup(mut self) -> Self {
        self.inner = self.inner.setup(|app| {
            let session = PlatformSession::initialize(app.handle().clone());
            // Store session in Tauri's state manager for retrieval by commands
            app.manage(session);
            Ok(())
        });
        self
    }
}
```

Tauri's `app.manage()` stores the session in `StateManager` — a type-indexed
`TypeId → Pin<Box<dyn Any>>` map. Any `#[tauri::command]` can retrieve it via
`State<PlatformSession<R>>`. This is how capability handlers, custom protocol
handlers, and route handlers access the session.

---

## Part B — Handler chain registration

From [decision 03](../decisions/03-session-backbone-transport.md#handler-chain-registration):

### B.1 — Route handler registration

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Register a route handler. Handlers are checked in registration order
    /// on every navigation. First `Some(decision)` wins. `None` falls through.
    ///
    /// Thread-safe: can be called from setup or from within a handler.
    pub fn register_handler(&self, handler: impl RouteHandler) {
        self.route_handlers.write().unwrap().push(Box::new(handler));
    }
}
```

### B.2 — Navigation resolution

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Resolve a navigation intent through the handler chain.
    /// Returns the first `Some(decision)` or the platform default.
    ///
    /// This is step 2 of the 9-step execution contract from decision 02.
    pub fn resolve_route(&self, intent: &NavigationIntent) -> RouteDecision {
        let handlers = self.route_handlers.read().unwrap();
        for handler in handlers.iter() {
            if let Some(decision) = handler.resolve(intent, self) {
                return decision;
            }
        }

        // No handler claimed it — platform default.
        self.default_decision_for(intent)
    }

    /// Platform default for unhandled navigation intents.
    fn default_decision_for(&self, intent: &NavigationIntent) -> RouteDecision {
        // External URLs → system browser
        if intent.url.starts_with("http://") || intent.url.starts_with("https://") {
            return webview_app() // FIXME: should be external, but External is Presentation
                .with_presentation(Presentation::External);
        }

        // ewe:// URLs with no handler → UntrustedRemote sandbox
        remote_fetch()
            .with_profile(Profile::UntrustedRemote)
            .with_cache_policy(CachePolicy::OnlineOnly)
    }
}
```

### B.3 — Capability handler registration

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Register a capability handler.
    pub fn register_capability<C: Capability>(&self, capability: C) {
        self.capability_registry
            .write()
            .unwrap()
            .insert(capability.id().0.clone(), Box::new(capability));
    }
}
```

---

## Part C — Navigation interception

From [decision 03](../decisions/03-session-backbone-transport.md#handler-chain-execution-on-navigation):

### C.1 — on_navigation hook

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Called from Tauri's `on_navigation` callback on every link click,
    /// form submit, or redirect inside the WebView.
    ///
    /// Returns `true` if the session handled the navigation (the platform
    /// takes over rendering). Returns `false` to allow the WebView to
    /// handle it natively.
    pub fn intercept_navigation(&self, url: &Url) -> bool {
        let intent = NavigationIntent {
            url: url.to_string(),
            method: Method::Get, // TODO: detect POST from form submit
            source: IntentSource::LinkClick,
            referrer: self.active_page.read().unwrap()
                .as_ref()
                .map(|p| p.route.clone()),
        };

        let decision = self.resolve_route(&intent);

        // If the decision is External, let the OS handle it.
        if decision.presentation == Presentation::External {
            // open::that(url) on desktop, UIApplication on iOS, Intent on Android
            return false; // don't intercept — let the system handle it
        }

        // Session handles the navigation.
        self.execute_decision(&decision, &intent);
        true // intercepted — WebView should not navigate
    }
}
```

### C.2 — Wiring into Tauri

```rust
// In PlatformBuilder::setup():
self.inner = self.inner.setup(|app| {
    let session = PlatformSession::initialize(app.handle().clone());

    // Register the on_navigation callback on the main WebView window.
    // In Tauri, this is per-WebViewWindow, registered at build time.
    // We register a session-aware closure that delegates to intercept_navigation.

    app.manage(session);
    Ok(())
});
```

The `on_navigation` callback in Tauri is registered on `WebviewWindowBuilder`:

```rust
WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
    .on_navigation(|url| {
        let session = app.state::<PlatformSession<Wry>>();
        session.intercept_navigation(url)
    })
    .build()?;
```

---

## Part D — Session lifecycle

From [decision 03](../decisions/03-session-backbone-transport.md#session-lifecycle):

### D.1 — Lifecycle events

```rust
/// Events the session emits at lifecycle transitions.
/// Subsystems listen for these to react.
pub enum SessionEvent {
    /// App entered background (mobile: didEnterBackground).
    /// Subsystems should: cache screenshots, release idle WebView pool,
    /// pause non-critical transports.
    Background,

    /// App entered foreground (mobile: willEnterForeground).
    /// Subsystems should: validate stale content, re-warm WebView pool,
    /// reconnect transports.
    Foreground,

    /// App is shutting down (RunEvent::Exit).
    /// Subsystems should: persist state, flush cache, close connections.
    Shutdown,

    /// Connectivity changed.
    Online(bool),
}
```

### D.2 — Lifecycle methods

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Called when the app enters background.
    /// Emits `Background` event, releases idle resources.
    pub fn on_suspend(&self) {
        self.emit_lifecycle_event(SessionEvent::Background);
    }

    /// Called when the app enters foreground.
    /// Emits `Foreground` event, re-validates active content.
    pub fn on_resume(&self) {
        self.emit_lifecycle_event(SessionEvent::Foreground);
    }

    /// Called on app shutdown.
    /// Emits `Shutdown` event, flushes cache, closes connections.
    pub fn on_shutdown(&self) {
        self.emit_lifecycle_event(SessionEvent::Shutdown);
        self.cache.flush();
    }

    /// Update connectivity state.
    pub fn set_online(&self, online: bool) {
        let was_online = self.online.swap(online, Ordering::SeqCst);
        if was_online != online {
            self.emit_lifecycle_event(SessionEvent::Online(online));
        }
    }
}
```

### D.3 — Event emission

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Emit a lifecycle event through Tauri's event system.
    /// Subsystems listen via `app.listen()` or `session.on_event()`.
    fn emit_lifecycle_event(&self, event: SessionEvent) {
        let event_name = match &event {
            SessionEvent::Background => "platform:background",
            SessionEvent::Foreground => "platform:foreground",
            SessionEvent::Shutdown => "platform:shutdown",
            SessionEvent::Online(true) => "platform:online",
            SessionEvent::Online(false) => "platform:offline",
        };
        let _ = self.app_handle.emit(event_name, ());
    }
}
```

---

## Part E — Subsystem peer model

From [decision 03](../decisions/03-session-backbone-transport.md#subsystem-peer-model):

```
Cache subsystem ──→ "I have cached content for /route"
                    Session ──→ routes through rendering lane → DOM

Native capability ──→ "biometric auth result"
                    Session ──→ scoped delivery to correct route/page → WebView

WebView action ──→ "user clicked link"
                  Session → route handler chain → decide → execute
```

### E.1 — Session as message bus

Every subsystem holds a reference to `PlatformSession<R>`. They communicate
THROUGH the session, not directly. The session:

1. **Routes capability requests** — from WebView (via Tauri command IPC) to
   the registered capability handler, and back.
2. **Routes navigation intents** — from Tauri's `on_navigation` callback
   through the handler chain to a `RouteDecision`.
3. **Routes cache lookups** — from the execution contract step 3 (cache check)
   through the `CacheManager`.
4. **Scopes responses** — `PageIdentity` attached to every response ensures
   results are delivered to the correct page.

### E.2 — Page identity tracking

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Increment the visit counter and update the active page.
    /// Called after every navigation.
    pub fn record_navigation(&self, route: &str) -> PageIdentity {
        let visit_id = self.visit_counter.fetch_add(1, Ordering::SeqCst);
        let identity = PageIdentity {
            session_id: self.session_id,
            route: route.to_string(),
            visit_id,
        };
        *self.active_page.write().unwrap() = Some(identity.clone());
        identity
    }

    /// Check if a capability request is from the currently active page.
    /// Stale-page guard: requests from navigated-away pages are dropped.
    pub fn is_active_page(&self, page: &PageIdentity) -> bool {
        self.active_page.read().unwrap()
            .as_ref()
            .map_or(false, |active| active == page)
    }
}
```

---

## Part F — Session API surface summary

From [decision 03](../decisions/03-session-backbone-transport.md#what-tauri-already-provides-vs-what-we-build):

```rust
impl<R: Runtime> PlatformSession<R> {
    // ── Lifecycle ─────────────────────────────────────────────────────
    pub fn initialize(app_handle: AppHandle<R>) -> Self;
    pub fn on_suspend(&self);
    pub fn on_resume(&self);
    pub fn on_shutdown(&self);
    pub fn set_online(&self, online: bool);

    // ── Route handlers ────────────────────────────────────────────────
    pub fn register_handler(&self, handler: impl RouteHandler);
    pub fn resolve_route(&self, intent: &NavigationIntent) -> RouteDecision;
    pub fn intercept_navigation(&self, url: &Url) -> bool;

    // ── Capability handlers ───────────────────────────────────────────
    pub fn register_capability<C: Capability>(&self, capability: C);
    pub fn invoke_capability(&self, request: &CapabilityRequest) -> CapabilityResponse;

    // ── Page identity ─────────────────────────────────────────────────
    pub fn record_navigation(&self, route: &str) -> PageIdentity;
    pub fn is_active_page(&self, page: &PageIdentity) -> bool;
    pub fn active_page(&self) -> Option<PageIdentity>;

    // ── Cache ─────────────────────────────────────────────────────────
    pub fn cache(&self) -> &CacheManager;

    // ── App handle ────────────────────────────────────────────────────
    pub fn app_handle(&self) -> &AppHandle<R>;
}
```

---

## Verification

### Unit tests
```bash
cargo test --package foundation_platform -- session
```

### Integration test: session lifecycle
```rust
#[test]
fn session_initializes_with_valid_state() {
    // Setup: create a minimal Tauri app context
    // Call: PlatformSession::initialize(app_handle)
    // Assert: session_id is non-zero, visit_counter is 0, online is true
}

#[test]
fn handler_chain_resolves_first_match() {
    // Setup: register handler A that matches /app/* → webview_app()
    //        register handler B that matches /app/* → remote_fetch()
    // Call: resolve_route(NavigationIntent { url: "/app/items", .. })
    // Assert: returns webview_app() (handler A won, handler B never checked)
}

#[test]
fn handler_chain_falls_through_to_default() {
    // Setup: register handler that returns None for /remote/*
    // Call: resolve_route(NavigationIntent { url: "/remote/dashboard", .. })
    // Assert: returns platform default (UntrustedRemote + OnlineOnly)
}

#[test]
fn visit_counter_increments_on_navigation() {
    // Call: record_navigation("/app/home")
    // Call: record_navigation("/app/items")
    // Assert: visit_id increments, active_page updates
}

#[test]
fn stale_page_guard_rejects_old_requests() {
    // Setup: navigate to /app/home → page_identity with visit_id=1
    //        navigate to /app/items → active_page now visit_id=2
    // Call: is_active_page(&page_identity_from_visit_1)
    // Assert: false (page is stale)
}
```

### Test: lifecycle events fire
```rust
#[test]
fn suspend_emits_background_event() { ... }
#[test]
fn resume_emits_foreground_event() { ... }
#[test]
fn connectivity_change_emits_online_offline() { ... }
```

### Test: Tauri integration
```rust
#[tauri::test]
fn session_is_stored_in_tauri_state() {
    // Build a minimal PlatformBuilder, access session via app.state()
}
```

---


## Learnings & Insights (2026-07-17)

### Why `PlatformSession` is non-generic

The feature spec had `PlatformSession<R: Runtime>`. Dropping `R` made
`RouteHandler` and `Capability` non-generic — plain traits, no type parameters.
`Arc<PlatformSession>` is the shared handle; all methods take `&self`.
`PlatformBuilder<R>` stays generic but creates the non-generic session.

### Event system: listeners, not AppHandle

The spec assumed event emission through `AppHandle::emit()`. Instead, the
session has its own listener system:
- `on_event(|ev| { ...; true })` — return `false` to unsubscribe.
- `emit(event)` iterates listeners, auto-removes dead ones.
- Lifecycle hooks call `emit()` internally. No Tauri dependency in the core.

### No stubs, no deferred bodies

Every method does real work. `resolve_route()` iterates the chain.
`record_navigation()` increments the atomic counter. `is_active_page()`
compares PageIdentity. `set_online()` uses `AtomicBool::swap`.
`update_online()` emits events on change. Lifecycle hooks emit events.
No empty bodies, no `// handled elsewhere in F0X` comments.

### What was NOT included

- **Capability registry** — lives in F05 (decision 07). Session doesn't need
  it pre-declared. F05 will add the field + `register_capability`.
- **Cache manager** — lives in F07 (decision 05). Cache check is step 3 of
  the execution contract, external to the session.
- **ewe:// protocol handler** — lives in F03. The session exposes
  `resolve_route()` as a public method; F03 calls it.

### Implementation

| Component | Status |
|---|---|
| `PlatformSession` with event listener system | ✅ 260 lines |
| `RouteHandler` trait + `FnRouteHandler` | ✅ 50 lines |
| `PlatformBuilder` Tauri integration | ✅ 25 lines |
| Tests (24 passing, zero warnings) | ✅ 220 lines |
| No stubs, no empty bodies, no ignored values | ✅ |
