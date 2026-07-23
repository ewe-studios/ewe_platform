//! `PlatformSession` — the central coordination bus.
//!
//! Everything plugs into the session as a peer. No subsystem talks directly
//! to another without the session knowing. The session is NOT generic over
//! Tauri's Runtime — event dispatch is done via simple listener callbacks
//! so the session is self-contained and fully testable.
//!
//! `Arc<PlatformSession>` is the shareable handle — all subsystems hold a
//! clone. Internally, `RwLock` + `Atomic` fields keep things lock-free
//! where possible.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::route::RouteDecisionExt;
use crate::route_handler::RouteResponder;
use crate::backend::http::HttpBackend;
use foundation_ui_traits::{SessionId, PageIdentity, NavigationIntent, RouteDecision, Presentation, Profile, CachePolicy};

// ── Session event types ───────────────────────────────────────────────

/// Events emitted by the session backbone.
/// Subsystems listen for these to react to lifecycle transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// App entered background (mobile: didEnterBackground).
    Background,
    /// App entered foreground (mobile: willEnterForeground).
    Foreground,
    /// App is shutting down (`RunEvent::Exit`).
    Shutdown,
    /// Device connectivity changed.
    OnlineChanged(bool),
}

/// A listener for session events. Returns `true` to stay registered,
/// `false` to unsubscribe after this event.
type EventListener = Box<dyn Fn(&SessionEvent) -> bool + Send + Sync>;

// ── Session struct ────────────────────────────────────────────────────

/// Central coordination bus. Subsystems hold `Arc<PlatformSession>`.
/// Non-generic — Tauri's `R: Runtime` is internal, not on the API surface.
pub struct PlatformSession {
    /// Route handler chain — iterated in registration order on every
    /// navigation. First `Some(decision)` wins, `None` falls through.
    route_handlers: RwLock<Vec<Box<dyn super::route_handler::RouteHandler>>>,

    /// Platform IPC registry (Ipc + 5-layer defense, was CapabilityRegistry).
    capability_registry: crate::capability::PlatformIpcRegistry,

    /// Cache manager.
    cache: crate::cache::CacheManager,

    /// Mutation queue — offline mutations replayed on connectivity restore.
    mutation_queue: crate::mutation::MutationQueue,

    /// Session identity — generated once at app launch, never changes.
    session_id: SessionId,

    /// Monotonically incrementing visit counter.
    visit_counter: AtomicU64,

    /// The currently active page. `None` until the first navigation.
    active_page: RwLock<Option<PageIdentity>>,

    /// Connectivity state.
    online: AtomicBool,

    /// Event listeners. Called on every lifecycle/connectivity transition.
    /// Listeners return `true` to stay subscribed, `false` to unsubscribe.
    event_listeners: RwLock<Vec<EventListener>>,

    /// Per-route responder registry. Maps `handler_id` → responder.
    /// Each `webview_app()` / `ipc_shell()` registers its own responder.
    /// `execute_decision()` looks up the handler and calls `respond()`.
    handler_registry: RwLock<HashMap<String, Box<dyn super::route_handler::RouteResponder>>>,

    /// Resolved resource directory for MobileDirectory-backed assets.
    /// Set by `PlatformBuilder` at startup. On Android this is the Tauri-extracted
    /// resource path; on desktop it's the app bundle resource dir.
    pub resource_root: PathBuf,

    /// Script injector (F24) — resolves platform runtime scripts with
    /// disk→embedded fallback chains. Evaluated on every webview at startup.
    script_injector: crate::injector::ScriptInjector,

    /// IPC registry (F25) — central backend communication hub.
    /// All IPCs (query, emit, page) are registered here and invoked
    /// from JS via `__ewe_ipc` or programmatically from route handlers.
    ipc_registry: crate::ipc::IpcRegistry,

    /// Streaming IPC registry (F26) — separate from `IpcRegistry` because
    /// streaming handlers need `stream()` and `accept_stream()` methods
    /// that the standard `Ipc` trait doesn't provide.
    stream_registry: crate::ipc::streaming::PlatformStreamRegistry,

    /// Shared HTTP backend for remote content fetching (F29 Stage 2).
    /// All route handlers share this client — auth tokens and connection
    /// pooling are managed centrally.
    http_backend: HttpBackend,

    /// WebView stack manager (F06 + F29 Stage 3). Tracks navigation history,
    /// screenshots, and the multi-WebView pool. Wrapped in `RwLock` so
    /// route handlers can push/pop without `&mut self`.
    webview_stack: RwLock<crate::stack::WebViewStack>,

    /// Per-app isolation registry (F21). Maps route prefixes to WebView labels,
    /// profiles, capability allowlists. Resolved during route dispatch.
    app_isolation: crate::multi_app::AppIsolation,

    /// Worker registry (F34). Named typed channels for background workers.
    pub(crate) workers_: crate::worker::WorkerRegistry,

    /// Window manager (F35). Bridges Tauri WebView lifecycle with the pool.
    /// `None` in tests; injected by `PlatformBuilder` in production.
    window_manager: crate::window::WindowManager,

    /// Wasmtime shell registry (F33). Named WASM modules that run in-process
    /// via wasmtime (Surface 3). Registered at setup time, built on first request.
    pub(crate) wasmtime_shell_: crate::wasmtime_responder::WasmtimeShell,

    /// Bundle asset manager (F40). Installed by `PlatformBuilder` at setup.
    ///
    /// `None` in tests and in any embedding that constructs a session
    /// directly — callers fall back to `resource_root` and plain `std::fs`,
    /// which is correct everywhere except Android, where APK-bundled assets
    /// are not on disk and only the manager's VFS can reach them.
    asset_manager: RwLock<Option<Arc<crate::assets::PlatformAssetManager>>>,
}

// ── Construction ──────────────────────────────────────────────────────

impl PlatformSession {
    /// Initialize the session with the resolved resource root directory and
    /// a pre-configured [`ScriptInjector`] (F24).
    /// Called once at app launch.
    /// Returns `Arc<Self>` — subsystems clone the Arc to share ownership.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(resource_root: PathBuf, script_injector: crate::injector::ScriptInjector) -> Arc<Self> {
        let session_id = SessionId(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        Arc::new(Self {
            route_handlers: RwLock::new(Vec::new()),
            capability_registry: crate::capability::PlatformIpcRegistry::new(),
            cache: crate::cache::CacheManager::in_memory(),
            mutation_queue: crate::mutation::MutationQueue::in_memory(),
            session_id,
            visit_counter: AtomicU64::new(0),
            active_page: RwLock::new(None),
            online: AtomicBool::new(true),
            event_listeners: RwLock::new(Vec::new()),
            handler_registry: RwLock::new(HashMap::new()),
            resource_root,
            script_injector,
            ipc_registry: crate::ipc::IpcRegistry::new(),
            stream_registry: crate::ipc::streaming::PlatformStreamRegistry::new(),
            http_backend: HttpBackend::new(),
            webview_stack: RwLock::new(crate::stack::WebViewStack::new(
                crate::stack::StackConfig::default(),
            )),
            app_isolation: crate::multi_app::AppIsolation::new(),
            workers_: crate::worker::WorkerRegistry::new(),
            window_manager: crate::window::WindowManager::new(),
            wasmtime_shell_: crate::wasmtime_responder::WasmtimeShell::new(),
            asset_manager: RwLock::new(None),
        })
    }
}

impl PlatformSession {
    /// TEST ONLY: create a session with an empty ScriptInjector.
    #[doc(hidden)]
    #[must_use]
    pub fn new_test(resource_root: PathBuf) -> Arc<Self> {
        Self::new(resource_root, crate::injector::ScriptInjector::new(PathBuf::from(".")))
    }
}

// ── Route handler registration ────────────────────────────────────────

impl PlatformSession {
    /// Register a route handler. Handlers are checked in registration order
    /// on every navigation. First `Some(decision)` wins.
    /// Register a route handler.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register_handler(&self, handler: impl super::route_handler::RouteHandler) {
        self.route_handlers.write().unwrap().push(Box::new(handler));
    }

    /// Register a pattern + decision + responder at setup time (F22).
    /// Like `PlatformBuilder::route_with` but usable from `.setup()` callbacks
    /// where `self.resource_root` is already resolved.
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register_route_with(
        &self,
        pattern: &str,
        mut decision: foundation_ui_traits::RouteDecision,
        responder: impl super::route_handler::RouteResponder,
    ) {
        let handler_id = format!("__route_{pattern}");
        self.register_responder(&handler_id, Box::new(responder));
        decision.handler_id = Some(handler_id);
        self.route(pattern, decision.clone());

        if pattern.ends_with("/*") {
            let prefix = pattern.trim_end_matches('*').trim_end_matches('/');
            self.route(prefix, decision);
        }
    }

    /// Resolve a navigation intent through the handler chain.
    /// Returns the first `Some(decision)` or the platform default.
    ///
    /// This is step 2 of the 9-step execution contract from decision 02.
    /// # Panics
    ///
    /// Panics if the route handler `RwLock` is poisoned.
    pub fn resolve_route(&self, intent: &NavigationIntent) -> RouteDecision {
        let handlers = self.route_handlers.read().unwrap();
        for handler in handlers.iter() {
            if let Some(decision) = handler.resolve(intent, self) {
                return decision;
            }
        }
        Self::default_decision_for(intent)
    }

    /// Register a pattern-based route handler.
    /// Convenience wrapper: creates a single-pattern `PatternRouter`,
    /// registers it as a handler in the chain.
    ///
    /// Pattern syntax: `*` matches one path segment, `**` matches any depth.
    /// First-registered first-matched.
    ///
    /// # Panics
    /// Panics if the pattern string is invalid.
    /// # Panics
    ///
    /// Panics if the pattern string is invalid or the internal `RwLock`
    /// is poisoned.
    pub fn route(&self, pattern: &str, decision: RouteDecision) {
        let mut router = crate::pattern::PatternRouter::new();
        router.route(pattern, decision);
        self.register_handler(router);
    }

    /// Register a closure-based route handler.
    /// Convenience wrapper around `FnRouteHandler` + `register_handler`.
    /// Register a closure-based route handler.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn on_navigate(
        &self,
        f: impl Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision> + Send + Sync + 'static,
    ) {
        self.register_handler(super::route_handler::FnRouteHandler::new(f));
    }

    fn default_decision_for(intent: &NavigationIntent) -> RouteDecision {
        // External URLs → system browser
        if intent.url.starts_with("http://") || intent.url.starts_with("https://") {
            return super::route::webview_app().with_presentation(Presentation::External);
        }

        // Same-origin ewe:// with no handler → UntrustedRemote sandbox
        super::route::remote_fetch()
            .with_profile(Profile::UntrustedRemote)
            .with_cache_policy(CachePolicy::OnlineOnly)
    }

    /// Execute a `RouteDecision` through the full 9-step contract.
    ///
    /// Steps 2-9 of the execution contract:
    /// 2. Handler chain — already resolved, decision passed in
    /// 3. Cache check
    /// 4. Presentation — update stack based on presentation mode (F29 Stage 3)
    /// 5. Handler dispatch — calls registered `RouteResponder::respond()`
    ///    or falls back to `SessionTransport` if no handler registered
    /// 6-7. Protocol selection + encoding
    /// 8-9. Post-render — page identity tracking
    ///
    /// Returns the `tauri::http::Response` directly when a route handler
    /// is registered. The caller passes it straight to the `WebView`.
    ///
    /// # Panics
    ///
    /// Panics if the response builder fails (e.g. invalid header values)
    /// or if internal locks are poisoned.
    pub fn execute_decision(
        &self,
        decision: &RouteDecision,
        intent: &NavigationIntent,
    ) -> tauri::http::Response<Vec<u8>> {
        // Step 3: Cache check
        let route = crate::pattern::extract_path(&intent.url);
        if self.cache.should_serve_cached(decision, &route) {
            if let Some(entry) = self.cache.get(decision.profile, &route) {
                // Even for cached responses, record the navigation mode.
                self.record_presentation(&route, &decision.presentation, decision.target.as_deref());
                return tauri::http::Response::builder()
                    .status(200)
                    .header("Content-Type", entry.content_type.as_str())
                    .body(entry.body)
                    .unwrap();
            }
        }

        // Offline fallback
        if !self.is_online() && self.cache.can_serve_offline(decision, &route) {
            if let Some(entry) = self.cache.get(decision.profile, &route) {
                self.record_presentation(&route, &decision.presentation, decision.target.as_deref());
                return tauri::http::Response::builder()
                    .status(200)
                    .header("Content-Type", entry.content_type.as_str())
                    .body(entry.body)
                    .unwrap();
            }
        }

        // Step 4: Presentation — update the WebView stack before rendering.
        // This records which WebView label the navigation targets and the
        // transition style (morph, replace, new).
        self.record_presentation(&route, &decision.presentation, decision.target.as_deref());

        // Step 4b: ViewKind routing — if the decision targets a labeled WebView,
        // ensure it exists in the pool. The actual WebView creation happens in
        // the Tauri layer; here we just track the label.
        if let Some(ref target_label) = decision.target {
            if let Ok(mut stack) = self.webview_stack.write() {
                if let Some(pool) = stack.pool_mut() {
                    pool.get_or_create(target_label);
                }
            }
        }

        // Step 5: Handler dispatch
        let response = if let Some(handler_id) = decision.handler_id.as_ref() { if let Some(handler) = self.get_responder(handler_id) { handler.respond(intent, decision, self) } else {
            let body = format!("No handler for '{handler_id}'").into_bytes();
            tauri::http::Response::builder()
                .status(500)
                .header("Content-Type", "text/plain")
                .body(body).unwrap()
        } } else {
            let body = format!("No handler_id on decision for route: {route}").into_bytes();
            tauri::http::Response::builder()
                .status(500)
                .header("Content-Type", "text/plain")
                .body(body).unwrap()
        };

        // Steps 8-9: Record navigation + page identity
        self.record_navigation(&route);

        response
    }

    /// Record a navigation with its presentation mode in the WebView stack.
    ///
    /// Implements the F06 Hotwire Native model (revised 2026-07-22):
    ///
    /// - Morph: in-place navigate, same WebView, no stack change.
    /// - Replace: swap current slot's route, screenshot for back transition.
    /// - Push: create new FullScreen WebView, push slot, old alive behind.
    ///   Back pops the top WebView and reveals the previous one.
    /// - Modal: create new Modal WebView, push slot tagged Modal. Back
    ///   dismisses only the modal — the slot underneath is untouched.
    /// - Root: destroy all WebViews, clear stack, create one new root.
    ///   Back from root exits the app.
    /// - External: open in OS system browser, no stack change.
    fn record_presentation(
        &self,
        route: &str,
        presentation: &Presentation,
        _target: Option<&str>,
    ) {
        // A WebViewOps that does nothing: the handler response from
        // `execute_decision` IS the navigation for single-WebView mode.
        // Multi-WebView ops go through the WindowManager in each branch.
        struct ResponderWebview;
        impl crate::WebViewOps for ResponderWebview {
            fn navigate(&self, _url: &str) {}
            fn screenshot(&self) -> Vec<u8> { Vec::new() }
            fn eval(&self, _script: &str) {}
            fn reload(&self) {}
        }

        let win_mgr = self.window_manager();
        let wv = ResponderWebview;

        if let Ok(mut stack) = self.webview_stack.write() {
            if stack.depth() == 0 {
                stack.init(route);
                if let Some(mut pool) = stack.pool_mut() {
                    win_mgr.ensure(&mut pool, "main", route);
                    pool.set_state("main", crate::stack::WebViewState::Active);
                }
                return;
            }

            // Capture depth BEFORE any mutable borrows through pool_mut.
            let depth = stack.depth();
            match presentation {
                Presentation::Morph => {
                    stack.morph(route, &wv);
                }
                Presentation::Replace => {
                    stack.replace(route, &wv);
                }
                Presentation::Push => {
                    if let Some(mut pool) = stack.pool_mut() {
                        let new_label = format!("wv_{depth}");
                        win_mgr.ensure(&mut pool, &new_label, route);
                        pool.set_state(&new_label, crate::stack::WebViewState::Active);
                        pool.set_route(&new_label, route);
                    }
                    stack.push(route, &wv);
                }
                Presentation::Modal => {
                    if let Some(mut pool) = stack.pool_mut() {
                        let modal_label = format!("modal_{depth}");
                        win_mgr.ensure(&mut pool, &modal_label, route);
                        pool.set_state(&modal_label, crate::stack::WebViewState::Active);
                        pool.set_route(&modal_label, route);
                    }
                    stack.push_modal(route, &wv);
                }
                Presentation::External => {
                    if let Some(mut pool) = stack.pool_mut() {
                        win_mgr.open_external(&mut pool, route);
                    }
                }
                Presentation::Root => {
                    if let Some(mut pool) = stack.pool_mut() {
                        let labels: Vec<String> = pool.labels();
                        for old_label in labels {
                            win_mgr.destroy(&mut pool, &old_label);
                        }
                        win_mgr.ensure(&mut pool, "wv_0", route);
                        pool.set_state("wv_0", crate::stack::WebViewState::Active);
                        pool.set_route("wv_0", route);
                    }
                    stack.set_root(route, &wv);
                }
            }
        }
    }
}

// ── Page identity tracking ────────────────────────────────────────────

impl PlatformSession {
    /// Increment the visit counter and update the active page.
    /// Called after every navigation. Returns the new `PageIdentity`.
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
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

    /// Check if a request is from the currently active page.
    /// Stale-page guard: requests from navigated-away pages are dropped.
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn is_active_page(&self, page: &PageIdentity) -> bool {
        self.active_page
            .read()
            .unwrap()
            .as_ref()
            .is_some_and(|active| active == page)
    }

    /// Return the currently active page identity, if any.
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn active_page_identity(&self) -> Option<PageIdentity> {
        self.active_page.read().unwrap().clone()
    }
}

// ── Connectivity ──────────────────────────────────────────────────────

impl PlatformSession {
    /// Returns `true` if the device is currently online.
    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }

    /// Atomically set connectivity state. Returns the previous state.
    pub fn set_online(&self, online: bool) -> bool {
        self.online.swap(online, Ordering::SeqCst)
    }

    /// Update connectivity and fire an `OnlineChanged` event if the
    /// state actually changed. Returns whether an event was emitted.
    pub fn update_online(&self, online: bool) -> bool {
        let changed = self.set_online(online) != online;
        if changed {
            self.emit(&SessionEvent::OnlineChanged(online));
        }
        changed
    }
}

// ── Session identity ─────────────────────────────────────────────────

impl PlatformSession {
    /// The session identifier. Constant for the lifetime of the session.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// The current visit counter value (before increment).
    pub fn visit_count(&self) -> u64 {
        self.visit_counter.load(Ordering::SeqCst)
    }

    /// Access the cache manager for storing/retrieving cached responses.
    pub fn cache(&self) -> &crate::cache::CacheManager {
        &self.cache
    }

    /// Access the F41 platform IPC registry for invoking native capabilities.
    /// Access the F41 platform IPC registry.
    pub fn capabilities(&self) -> &crate::capability::PlatformIpcRegistry {
        &self.capability_registry
    }

    pub fn register_capability<C: crate::capability::PlatformIpc + 'static>(&self, cap: C) {
        self.capability_registry.register(cap);
    }

    pub fn get_capability(&self, name: &str) -> Option<&dyn crate::capability::PlatformIpc> {
        self.capability_registry.get(name)
    }

    pub fn script_injector(&self) -> &crate::injector::ScriptInjector {
        &self.script_injector
    }

    pub fn ipc_registry(&self) -> &crate::ipc::IpcRegistry {
        &self.ipc_registry
    }

    pub fn register_ipc<I: crate::ipc::PlatformIpc + 'static>(&self, ipc: I) {
        self.ipc_registry.register(ipc);
    }

    pub fn get_ipc(&self, name: &str) -> Option<&dyn crate::ipc::PlatformIpc> {
        self.ipc_registry.get(name)
    }

    pub fn stream_registry(&self) -> &crate::ipc::streaming::PlatformStreamRegistry {
        &self.stream_registry
    }

    /// Access the shared HTTP backend for remote content fetching (F29 Stage 2).
    pub fn http_backend(&self) -> &HttpBackend {
        &self.http_backend
    }

    /// The resolved bundle resource root (F22).
    ///
    /// In dev mode (`{resource_root}/public/` exists), returns that path so
    /// WASM artifacts from `cargo build` land at the right spot. On bundled
    /// builds (Android `apk` / `*.app`), Tauri extracts `bundle.resources`
    /// directly into `resource_root` so this returns `resource_root` as-is.
    ///
    /// Callers (codegen, app setups) use this to feed `MobileDirectory`
    /// responders — they never need to know the dev vs. prod layout.
    #[must_use]
    pub fn bundle_root(&self) -> PathBuf {
        // The asset manager already resolved the dev-vs-bundled question when
        // it picked its mount point; re-deriving it here would second-guess it.
        if let Some(manager) = self.asset_manager() {
            return manager.base_root().to_path_buf();
        }
        let dev = self.resource_root.join("public");
        if dev.exists() { dev } else { self.resource_root.clone() }
    }

    /// The bundle asset manager (F40), if one was installed.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn asset_manager(&self) -> Option<Arc<crate::assets::PlatformAssetManager>> {
        self.asset_manager.read().unwrap().clone()
    }

    /// Install the bundle asset manager. Called once by `PlatformBuilder`.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn set_asset_manager(&self, manager: Arc<crate::assets::PlatformAssetManager>) {
        *self.asset_manager.write().unwrap() = Some(manager);
    }

    /// The directory an app's assets are served from (F40).
    ///
    /// Delegates to the asset manager when one is installed, so per-app
    /// version directories are resolved. Without a manager this is just
    /// `bundle_root()/{app_id}`, which is what every pre-F40 caller assumed.
    #[must_use]
    pub fn app_root(&self, app_id: &str) -> PathBuf {
        match self.asset_manager() {
            Some(manager) => manager.app_root(app_id),
            None => self.bundle_root().join(app_id),
        }
    }

    /// Access the WebView stack for navigation tracking (F29 Stage 3).
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn webview_stack(&self) -> std::sync::RwLockReadGuard<'_, crate::stack::WebViewStack> {
        self.webview_stack.read().unwrap()
    }

    /// Mutable access to the WebView stack (F29 Stage 3).
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn webview_stack_mut(&self) -> std::sync::RwLockWriteGuard<'_, crate::stack::WebViewStack> {
        self.webview_stack.write().unwrap()
    }

    /// Access the per-app isolation registry (F21).
    pub fn app_isolation(&self) -> &crate::multi_app::AppIsolation {
        &self.app_isolation
    }

    /// Access the window manager (F35). Used to create/show/hide Tauri windows.
    pub fn window_manager(&self) -> &crate::window::WindowManager {
        &self.window_manager
    }

    pub fn register_streaming_ipc<S: crate::ipc::streaming::StreamingIpc + 'static>(&self, ipc: S) {
        self.stream_registry.register(ipc);
    }

    /// Access the mutation queue for enqueuing/replaying offline mutations.
    pub fn mutation_queue(&self) -> &crate::mutation::MutationQueue {
        &self.mutation_queue
    }

    /// Register a per-route responder. Called once at startup (or in tests).
    /// When a `RouteDecision` with `handler_id` is resolved, this responder's
    /// `respond()` is called instead of the central `BackendTransport`.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register_responder(&self, handler_id: &str, responder: Box<dyn RouteResponder>) {
        self.handler_registry
            .write()
            .unwrap()
            .insert(handler_id.to_string(), responder);
    }

    /// Look up a responder by `handler_id`. Returns `None` if not found.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn get_responder(&self, handler_id: &str) -> Option<&dyn RouteResponder> {
        // SAFETY: Extending the lifetime of the Box<dyn> inside the RwLock.
        // The registry lives as long as the session, so this is safe.
        let guard = self.handler_registry.read().unwrap();
        guard
            .get(handler_id)
            .map(|b| unsafe { &*std::ptr::from_ref::<dyn RouteResponder>(b.as_ref()) })
    }
}

// ── Lifecycle hooks ──────────────────────────────────────────────────

impl PlatformSession {
    /// Called when the app enters background.
    /// Emits `Background` event to all listeners.
    pub fn on_suspend(&self) {
        self.emit(&SessionEvent::Background);
    }

    /// Called when the app enters foreground.
    /// Emits `Foreground` event to all listeners.
    pub fn on_resume(&self) {
        self.emit(&SessionEvent::Foreground);
    }

    /// Called on app shutdown.
    /// Emits `Shutdown` event to all listeners.
    pub fn on_shutdown(&self) {
        self.emit(&SessionEvent::Shutdown);
    }
}

// ── Event system ─────────────────────────────────────────────────────

impl PlatformSession {
    /// Register an event listener. The callback receives every session
    /// event. Return `true` to stay subscribed, `false` to unsubscribe
    /// after this event (one-shot listeners).
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn on_event(&self, f: impl Fn(&SessionEvent) -> bool + Send + Sync + 'static) {
        self.event_listeners.write().unwrap().push(Box::new(f));
    }

    /// Emit an event to all registered listeners.
    /// Dead listeners (those that returned `false` on a previous call)
    /// are cleaned up during emission.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    fn emit(&self, event: &SessionEvent) {
        let mut listeners = self.event_listeners.write().unwrap();
        let mut i = 0;
        while i < listeners.len() {
            let keep = (listeners[i])(event);
            if keep {
                i += 1;
            } else {
                let _ = listeners.remove(i);
            }
        }
    }

    /// Return the number of active event listeners.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn listener_count(&self) -> usize {
        self.event_listeners.read().unwrap().len()
    }
}

// Tests moved to tests/session_suite.rs
