//! Tests for PlatformSession — the central coordination bus.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use foundation_platform::*;

fn session() -> Arc<PlatformSession> {
    PlatformSession::new(std::path::PathBuf::from("."))
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
            if i.url.contains("/app/") { Some(webview_app()) } else { None }
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
        if i.url.contains("/app/") { Some(webview_app()) } else { None }
    }));
    s.register_handler(FnRouteHandler::new(|i, _| {
        if i.url.contains("/app/") { Some(remote_fetch()) } else { None }
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
            Some(webview_app())
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
        if i.url.contains("/auth/") { Some(remote_fetch().with_profile(Profile::Auth)) } else { None }
    }));
    s.register_handler(FnRouteHandler::new(|i, _| {
        if i.url.contains("/app/") { Some(webview_app()) } else { None }
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
    assert!(s.set_online(false));  // was true -> returns true
    assert!(!s.is_online());
    assert!(!s.set_online(true)); // was false -> returns false
    assert!(s.is_online());
    assert!(s.set_online(false));  // was true -> returns true
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

    assert!(s.update_online(false));  // online -> offline: fired
    assert!(!s.update_online(false)); // offline -> offline: no fire
    assert!(s.update_online(true));   // offline -> online: fired

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

    s.on_suspend();   // fires -> count = 1, listener removed
    s.on_resume();    // fires -> no listener, count stays 1

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
