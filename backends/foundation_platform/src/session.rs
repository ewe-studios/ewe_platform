//! PlatformSession — the central coordination bus.
//!
//! Everything plugs into the session as a peer. No subsystem talks directly
//! to another without the session knowing. The session is NOT generic over
//! Tauri's Runtime — event dispatch is done via simple listener callbacks
//! so the session is self-contained and fully testable.
//!
//! `Arc<PlatformSession>` is the shareable handle — all subsystems hold a
//! clone. Internally, `RwLock` + `Atomic` fields keep things lock-free
//! where possible.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::backend::{BackendTransport, DEFAULT_TRANSPORT};
use crate::route::RouteDecisionExt;
use foundation_ui_traits::*;

// ── Session event types ───────────────────────────────────────────────

/// Events emitted by the session backbone.
/// Subsystems listen for these to react to lifecycle transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// App entered background (mobile: didEnterBackground).
    Background,
    /// App entered foreground (mobile: willEnterForeground).
    Foreground,
    /// App is shutting down (RunEvent::Exit).
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

    /// Capability registry — registered at startup, invoked at runtime.
    capability_registry: crate::capability::CapabilityRegistry,

    /// Cache manager — protocol-transparent, profile-scoped, per-route policies.
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

    /// Pluggable backend transport. Injected at startup; falls back to
    /// [`DefaultTransport`] if not set. Tests inject their own transport.
    backend_transport: RwLock<Option<Box<dyn BackendTransport>>>,
}

// ── Construction ──────────────────────────────────────────────────────

impl PlatformSession {
    /// Initialize the session. Called once at app launch.
    /// Returns `Arc<Self>` — subsystems clone the Arc to share ownership.
    pub fn new() -> Arc<Self> {
        let session_id = SessionId(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        Arc::new(Self {
            route_handlers: RwLock::new(Vec::new()),
            capability_registry: crate::capability::CapabilityRegistry::new(),
            cache: crate::cache::CacheManager::in_memory(),
            mutation_queue: crate::mutation::MutationQueue::in_memory(),
            session_id,
            visit_counter: AtomicU64::new(0),
            active_page: RwLock::new(None),
            online: AtomicBool::new(true),
            event_listeners: RwLock::new(Vec::new()),
            backend_transport: RwLock::new(None),
        })
    }
}

impl Default for PlatformSession {
    fn default() -> Self {
        unreachable!("use PlatformSession::new() which returns Arc<PlatformSession>")
    }
}

// ── Route handler registration ────────────────────────────────────────

impl PlatformSession {
    /// Register a route handler. Handlers are checked in registration order
    /// on every navigation. First `Some(decision)` wins.
    pub fn register_handler(&self, handler: impl super::route_handler::RouteHandler) {
        self.route_handlers.write().unwrap().push(Box::new(handler));
    }

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
        self.default_decision_for(intent)
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
    pub fn route(&self, pattern: &str, decision: RouteDecision) {
        let mut router = crate::pattern::PatternRouter::new();
        router.route(pattern, decision);
        self.register_handler(router);
    }

    /// Register a closure-based route handler.
    /// Convenience wrapper around `FnRouteHandler` + `register_handler`.
    pub fn on_navigate(
        &self,
        f: impl Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision>
            + Send + Sync + 'static,
    ) {
        self.register_handler(super::route_handler::FnRouteHandler::new(f));
    }

    fn default_decision_for(&self, intent: &NavigationIntent) -> RouteDecision {
        // External URLs → system browser
        if intent.url.starts_with("http://") || intent.url.starts_with("https://") {
            return super::route::webview_app()
                .with_presentation(Presentation::External);
        }

        // Same-origin ewe:// with no handler → UntrustedRemote sandbox
        super::route::remote_fetch()
            .with_profile(Profile::UntrustedRemote)
            .with_cache_policy(CachePolicy::OnlineOnly)
    }

    /// Execute a RouteDecision through the full 9-step contract.
    ///
    /// Steps 2-9 of the execution contract (step 1 = interception, done by caller):
    /// 2. Handler chain — already resolved, decision passed in
    /// 3. Cache check
    /// 4. Presentation — record navigation for stack manager
    /// 5. Backend query
    /// 6. Protocol selection
    /// 7. Content encoding
    /// 8-9. Post-render — page identity tracking
    ///
    /// Returns the encoded response (body, content_type) ready for delivery.
    pub fn execute_decision(
        &self,
        decision: &RouteDecision,
        intent: &NavigationIntent,
    ) -> (Vec<u8>, String) {
        // Step 3: Cache check — serves cached content when policy allows.
        // StaleWhileRevalidate: serve cache immediately, caller spawns background
        // revalidation via cache.needs_revalidation() if needed.
        let route = crate::pattern::extract_path(&intent.url);
        if self.cache.should_serve_cached(decision, &route) {
            if let Some(entry) = self.cache.get(decision.profile, &route) {
                return (entry.body, entry.content_type);
            }
        }

        // Offline fallback: NetworkFirst/OnlineOnly skip cache normally, but when
        // offline and a stale entry exists, serve it instead of failing.
        if !self.is_online() && self.cache.can_serve_offline(decision, &route) {
            if let Some(entry) = self.cache.get(decision.profile, &route) {
                return (entry.body, entry.content_type);
            }
        }

        // Step 5: Backend query through the registered transport
        let content = crate::backend::query_backend(self.backend(), decision, &route);

        // Step 6: Protocol selection
        // Extract proto query param if present
        let proto_hint: Option<String> = intent.url.split('?').nth(1)
            .and_then(|q| q.split('&')
                .find(|p| p.starts_with("proto="))
                .map(|p| p[6..].to_string()));

        let protocol = super::ewe::select_protocol(
            &decision.protocol,
            proto_hint.as_ref(),
            &content,
        );

        // Step 7: Content encoding
        let (body, content_type) = super::ewe::encode_protocol(&protocol, &content);

        // Step 9: Post-render — record navigation
        self.record_navigation(&route);

        (body, content_type.to_string())
    }
}

// ── Page identity tracking ────────────────────────────────────────────

impl PlatformSession {
    /// Increment the visit counter and update the active page.
    /// Called after every navigation. Returns the new `PageIdentity`.
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
    pub fn is_active_page(&self, page: &PageIdentity) -> bool {
        self.active_page
            .read()
            .unwrap()
            .as_ref()
            .is_some_and(|active| active == page)
    }

    /// Return the currently active page identity, if any.
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
            self.emit(SessionEvent::OnlineChanged(online));
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

    /// Access the capability registry for invoking native capabilities.
    pub fn capabilities(&self) -> &crate::capability::CapabilityRegistry {
        &self.capability_registry
    }

    /// Access the mutation queue for enqueuing/replaying offline mutations.
    pub fn mutation_queue(&self) -> &crate::mutation::MutationQueue {
        &self.mutation_queue
    }

    /// Inject a backend transport. Called once at startup (or in tests).
    /// All subsequent [`execute_decision`] calls will use this transport.
    pub fn set_backend(&self, transport: Box<dyn BackendTransport>) {
        *self.backend_transport.write().unwrap() = Some(transport);
    }

    /// Return the active backend transport, or the default.
    fn backend(&self) -> &dyn BackendTransport {
        // Safety: we leak a reference to the DefaultTransport static.
        // This is fine — DefaultTransport is a unit struct living in a static.
        // We need a reference with a stable address; the RwLock guard would
        // be temporary. The &*Box<dyn> inside the option lives as long as the
        // option, but we can't return that from a method without GATs.
        // Instead, always return &DefaultTransport when no custom transport
        // is set, since DefaultTransport is stateless.
        if let Some(ref t) = *self.backend_transport.read().unwrap() {
            // SAFETY: The Box lives as long as self. We're returning a
            // reference with the same lifetime as &self.
            unsafe { &*(t.as_ref() as *const dyn BackendTransport) }
        } else {
            &DEFAULT_TRANSPORT
        }
    }
}

// ── Lifecycle hooks ──────────────────────────────────────────────────

impl PlatformSession {
    /// Called when the app enters background.
    /// Emits `Background` event to all listeners.
    pub fn on_suspend(&self) {
        self.emit(SessionEvent::Background);
    }

    /// Called when the app enters foreground.
    /// Emits `Foreground` event to all listeners.
    pub fn on_resume(&self) {
        self.emit(SessionEvent::Foreground);
    }

    /// Called on app shutdown.
    /// Emits `Shutdown` event to all listeners.
    pub fn on_shutdown(&self) {
        self.emit(SessionEvent::Shutdown);
    }
}

// ── Event system ─────────────────────────────────────────────────────

impl PlatformSession {
    /// Register an event listener. The callback receives every session
    /// event. Return `true` to stay subscribed, `false` to unsubscribe
    /// after this event (one-shot listeners).
    pub fn on_event(
        &self,
        f: impl Fn(&SessionEvent) -> bool + Send + Sync + 'static,
    ) {
        self.event_listeners.write().unwrap().push(Box::new(f));
    }

    /// Emit an event to all registered listeners.
    /// Dead listeners (those that returned `false` on a previous call)
    /// are cleaned up during emission.
    fn emit(&self, event: SessionEvent) {
        let mut listeners = self.event_listeners.write().unwrap();
        let mut i = 0;
        while i < listeners.len() {
            let keep = (listeners[i])(&event);
            if keep {
                i += 1;
            } else {
                let _ = listeners.remove(i);
            }
        }
    }

    /// Return the number of active event listeners.
    pub fn listener_count(&self) -> usize {
        self.event_listeners.read().unwrap().len()
    }
}

// Tests moved to tests/session_suite.rs
