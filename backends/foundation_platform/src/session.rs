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
            session_id,
            visit_counter: AtomicU64::new(0),
            active_page: RwLock::new(None),
            online: AtomicBool::new(true),
            event_listeners: RwLock::new(Vec::new()),
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

    fn default_decision_for(&self, _intent: &NavigationIntent) -> RouteDecision {
        super::route::remote_fetch()
            .with_profile(Profile::UntrustedRemote)
            .with_cache_policy(CachePolicy::OnlineOnly)
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

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route_handler::{FnRouteHandler, RouteHandler};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn session() -> Arc<PlatformSession> {
        PlatformSession::new()
    }

    fn intent(url: &str) -> NavigationIntent {
        NavigationIntent {
            url: url.to_string(),
            method: Method::Get,
            source: IntentSource::LinkClick,
            referrer: None,
        }
    }

    // ── Initialization ──────────────────────────────────────────────

    #[test]
    fn starts_with_valid_state() {
        let s = session();
        assert_eq!(s.visit_count(), 0);
        assert!(s.is_online());
        assert!(s.active_page_identity().is_none());
    }

    // ── Handler chain ───────────────────────────────────────────────

    #[test]
    fn first_match_wins() {
        let s = session();

        struct A;
        impl RouteHandler for A {
            fn resolve(&self, i: &NavigationIntent, _: &PlatformSession) -> Option<RouteDecision> {
                if i.url.contains("/app/") { Some(crate::route::webview_app()) } else { None }
            }
        }

        struct B;
        impl RouteHandler for B {
            fn resolve(&self, _: &NavigationIntent, _: &PlatformSession) -> Option<RouteDecision> {
                panic!("B should never be called");
            }
        }

        s.register_handler(A);
        s.register_handler(B);
        let d = s.resolve_route(&intent("/app/items"));
        assert_eq!(d.source, RouteSource::WebviewApp);
    }

    #[test]
    fn falls_through_to_default() {
        let s = session();
        let d = s.resolve_route(&intent("/remote/dashboard"));
        assert_eq!(d.profile, Profile::UntrustedRemote);
    }

    #[test]
    fn respects_registration_order() {
        let s = session();
        s.register_handler(FnRouteHandler::new(|i, _| {
            if i.url.contains("/app/") { Some(crate::route::webview_app()) } else { None }
        }));
        s.register_handler(FnRouteHandler::new(|i, _| {
            if i.url.contains("/app/") { Some(crate::route::remote_fetch()) } else { None }
        }));
        let d = s.resolve_route(&intent("/app/items"));
        assert_eq!(d.source, RouteSource::WebviewApp);
    }

    #[test]
    fn closure_captures_state() {
        let s = session();
        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        s.register_handler(FnRouteHandler::new(move |i, _| {
            if i.url.contains("/auth/") && f.load(Ordering::SeqCst) {
                Some(crate::route::webview_app())
            } else { None }
        }));
        assert_eq!(s.resolve_route(&intent("/auth/login")).profile, Profile::UntrustedRemote);
        flag.store(true, Ordering::SeqCst);
        assert_eq!(s.resolve_route(&intent("/auth/login")).source, RouteSource::WebviewApp);
    }

    #[test]
    fn none_falls_through() {
        let s = session();
        s.register_handler(FnRouteHandler::new(|_, _| None));
        assert_eq!(s.resolve_route(&intent("/app/items")).profile, Profile::UntrustedRemote);
    }

    #[test]
    fn multi_handler_fallthrough() {
        let s = session();
        s.register_handler(FnRouteHandler::new(|i, _| {
            if i.url.contains("/auth/") { Some(crate::route::remote_fetch().with_profile(Profile::Auth)) } else { None }
        }));
        s.register_handler(FnRouteHandler::new(|i, _| {
            if i.url.contains("/app/") { Some(crate::route::webview_app()) } else { None }
        }));
        assert_eq!(s.resolve_route(&intent("/app/items")).source, RouteSource::WebviewApp);
        assert_eq!(s.resolve_route(&intent("/auth/login")).profile, Profile::Auth);
        assert_eq!(s.resolve_route(&intent("/other/path")).profile, Profile::UntrustedRemote);
    }

    // ── Page identity ───────────────────────────────────────────────

    #[test]
    fn visit_counter_increments() {
        let s = session();
        let p1 = s.record_navigation("/app/home");
        let p2 = s.record_navigation("/app/items");
        assert_eq!(p1.visit_id, 0);
        assert_eq!(p2.visit_id, 1);
    }

    #[test]
    fn active_page_tracks_current() {
        let s = session();
        let p1 = s.record_navigation("/app/home");
        assert_eq!(s.active_page_identity(), Some(p1));
    }

    #[test]
    fn stale_page_guard_rejects_old() {
        let s = session();
        let p1 = s.record_navigation("/app/home");
        s.record_navigation("/app/items");
        assert!(!s.is_active_page(&p1));
    }

    // ── Connectivity ────────────────────────────────────────────────

    #[test]
    fn starts_online() { assert!(session().is_online()); }

    #[test]
    fn toggle_connectivity() {
        let s = session();
        // set_online returns the PREVIOUS state, not whether it changed
        assert!(s.set_online(false));  // was true → returns true
        assert!(!s.is_online());
        assert!(!s.set_online(true)); // was false → returns false
        assert!(s.is_online());
        assert!(s.set_online(false));  // was true → returns true
    }

    #[test]
    fn update_online_emits_event_only_on_change() {
        let s = session();
        let events = Arc::new(RwLock::new(Vec::new()));
        let e = events.clone();
        s.on_event(move |ev| {
            e.write().unwrap().push(ev.clone());
            true
        });

        assert!(s.update_online(false));  // online → offline: fired
        assert!(!s.update_online(false)); // offline → offline: no fire
        assert!(s.update_online(true));   // offline → online: fired

        let fired = events.read().unwrap();
        assert_eq!(fired.len(), 2);
        assert_eq!(fired[0], SessionEvent::OnlineChanged(false));
        assert_eq!(fired[1], SessionEvent::OnlineChanged(true));
    }

    // ── Lifecycle events ────────────────────────────────────────────

    #[test]
    fn lifecycle_events_fire_to_listeners() {
        let s = session();
        let events = Arc::new(RwLock::new(Vec::new()));
        let e = events.clone();
        s.on_event(move |ev| {
            e.write().unwrap().push(ev.clone());
            true
        });

        s.on_suspend();
        s.on_resume();
        s.on_shutdown();

        let fired = events.read().unwrap();
        assert_eq!(fired.len(), 3);
        assert_eq!(fired[0], SessionEvent::Background);
        assert_eq!(fired[1], SessionEvent::Foreground);
        assert_eq!(fired[2], SessionEvent::Shutdown);
    }

    #[test]
    fn listener_can_unsubscribe() {
        let s = session();
        let count = Arc::new(AtomicU64::new(0));
        let c = count.clone();
        s.on_event(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            false // unsubscribe after first event
        });

        s.on_suspend();   // fires → count = 1, listener removed
        s.on_resume();    // fires → no listener, count stays 1

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn multiple_listeners_fire_in_order() {
        let s = session();
        let order = Arc::new(RwLock::new(Vec::new()));
        let o1 = order.clone();
        let o2 = order.clone();
        s.on_event(move |_| { o1.write().unwrap().push(1); true });
        s.on_event(move |_| { o2.write().unwrap().push(2); true });

        s.on_suspend();
        assert_eq!(*order.read().unwrap(), vec![1, 2]);
    }

    #[test]
    fn empty_listeners_does_not_panic() {
        let s = session();
        s.on_suspend(); // no listeners, should not panic
        s.on_resume();
        s.on_shutdown();
    }
}
