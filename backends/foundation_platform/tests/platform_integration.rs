//! Full integration test — every subsystem composed together in a real process.
//!
//! Proves that PlatformSession, route handler chain, cache tiers, mutation queue,
//! capability registry, profile gates, WebView stack, and session events all
//! work together correctly without a Tauri runtime.
//!
//! Each test builds a real session, registers real handlers, and exercises
//! the full subsystem composition.

use std::sync::{Arc, Mutex};

use foundation_platform::*;

// ── Complete system bootstrap ────────────────────────────────────────

#[test]
fn full_system_bootstrap() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));

    // All subsystems available immediately
    assert_eq!(session.visit_count(), 0);
    assert!(session.is_online());
    assert!(session.session_id().0 > 0);

    // Cache, capabilities, mutations all initialized
    assert_eq!(session.cache().entry_count(), 0);
    assert_eq!(session.mutation_queue().pending_count(), 0);
}

// ── Route handler chain with multiple handlers ───────────────────────

#[test]
fn multi_handler_route_resolution() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));

    // Register routes for different surfaces
    session.route("/app/*", webview_app().with_profile(Profile::App));
    session.route("/remote/*", remote_fetch()
        .with_profile(Profile::TrustedRemote)
        .with_cache_policy(CachePolicy::NetworkFirst));
    session.route("/cached/*", remote_fetch()
        .with_cache_policy(CachePolicy::CacheFirst));
    session.route("/auth/*", remote_fetch()
        .with_profile(Profile::Auth));

    // Each URL resolves to the correct source and profile
    let d = session.resolve_route(&intent("/app/dashboard"));
    assert_eq!(d.source, RouteSource::WebviewApp);
    assert_eq!(d.profile, Profile::App);

    let d = session.resolve_route(&intent("/remote/dashboard"));
    assert_eq!(d.source, RouteSource::RemoteServer);
    assert_eq!(d.profile, Profile::TrustedRemote);
    assert_eq!(d.cache_policy, CachePolicy::NetworkFirst);

    let d = session.resolve_route(&intent("/auth/login"));
    assert_eq!(d.profile, Profile::Auth);

    // Unmatched route → platform default
    let d = session.resolve_route(&intent("/unknown/path"));
    assert_eq!(d.profile, Profile::UntrustedRemote);
}

// ── Cache + connectivity lifecycle ───────────────────────────────────

#[test]
fn cache_connectivity_lifecycle() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));

    // Cache content while "online"
    session.cache().store(Profile::App, "/data", b"live-data", "text/html");

    // Online: don't serve cache
    let d = webview_app().with_cache_policy(CachePolicy::CacheFirst);
    assert!(!session.can_serve_offline_for(session.cache(), &d, "/data"));

    // Go offline
    session.set_online(false);

    // Offline with CacheFirst: serve cached
    assert!(session.can_serve_offline_for(session.cache(), &d, "/data"));

    // Offline with OnlineOnly: fail
    let oo = webview_app().with_cache_policy(CachePolicy::OnlineOnly);
    assert!(!session.can_serve_offline_for(session.cache(), &oo, "/data"));

    // Back online
    session.set_online(true);
    assert!(!session.can_serve_offline_for(session.cache(), &d, "/data"));
}

// ── Mutation queue: offline → replay ─────────────────────────────────

#[test]
fn offline_mutation_replay_on_reconnect() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));

    // Go offline, enqueue mutations
    session.set_online(false);
    let _id1 = session.mutation_queue().enqueue("create", serde_json::json!({"item": "x"}));
    let _id2 = session.mutation_queue().enqueue("update", serde_json::json!({"id": 1, "name": "y"}));
    assert_eq!(session.mutation_queue().pending_count(), 2);

    // Come back online, replay
    session.set_online(true);
    let result = session.mutation_queue().replay(|m| {
        Ok(serde_json::Value::String(format!("ok-{}", m.mutation_type)))
    });

    assert_eq!(result.applied.len(), 2);
    assert_eq!(session.mutation_queue().pending_count(), 0);
}

// ── Capability invocation through registry ───────────────────────────

#[test]
fn capability_invocation_with_profile_gating() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));

    struct Cam;
    impl Capability for Cam {
        fn id(&self) -> &CapabilityId {
            Box::leak(Box::new(CapabilityId("camera".into())))
        }
        fn min_profile(&self) -> Profile { Profile::TrustedRemote }
        fn execute(&self, _: &PlatformSession, action: &str, _: serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::Value::String(format!("shot-{action}")))
        }
    }

    session.capabilities().register(Cam);

    // App profile + camera allowed → succeeds
    session.record_navigation("/camera");
    let page = session.active_page_identity().unwrap();
    let route = webview_app()
        .with_profile(Profile::App)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);

    let resp = session.capabilities().invoke(&session, &CapabilityRequest {
        id: "r1".into(), page_identity: page.clone(),
        capability: "camera".into(), action: "capture".into(),
        payload: serde_json::Value::Null,
    }, Some(&route));
    assert!(resp.status.is_ok());

    // UntrustedRemote profile → denied
    let bad_route = remote_fetch()
        .with_profile(Profile::UntrustedRemote)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);

    session.record_navigation("/bad");
    let page2 = session.active_page_identity().unwrap();
    let resp = session.capabilities().invoke(&session, &CapabilityRequest {
        id: "r2".into(), page_identity: page2,
        capability: "camera".into(), action: "capture".into(),
        payload: serde_json::Value::Null,
    }, Some(&bad_route));
    assert!(resp.status.is_err());

    // Stale page → denied
    session.record_navigation("/other"); // page is now stale
    let resp = session.capabilities().invoke(&session, &CapabilityRequest {
        id: "r3".into(), page_identity: page, // stale!
        capability: "camera".into(), action: "capture".into(),
        payload: serde_json::Value::Null,
    }, Some(&route));
    assert!(resp.status.is_err());
}

// ── WebView stack navigation ─────────────────────────────────────────

#[test]
fn webview_stack_navigation_cycle() {
    struct WV { navs: Mutex<Vec<String>>, reloads: Mutex<usize> }
    impl WebViewOps for WV {
        fn navigate(&self, url: &str) { self.navs.lock().unwrap().push(url.to_string()); }
        fn screenshot(&self) -> Vec<u8> { vec![1, 2, 3] }
        fn eval(&self, _: &str) {}
        fn reload(&self) { *self.reloads.lock().unwrap() += 1; }
    }

    let wv = WV { navs: Mutex::new(vec![]), reloads: Mutex::new(0) };
    let mut stack = WebViewStack::new(StackConfig::default());

    // Start → push → push → pop → pop → can't pop root
    stack.init("/app/home");
    stack.push("/app/items", &wv);
    stack.push("/app/items/42", &wv);
    assert_eq!(stack.depth(), 3);

    stack.pop(&wv);
    assert_eq!(stack.depth(), 2);
    stack.pop(&wv);
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.pop(&wv), None); // root
}

// ── Session events + lifecycle ───────────────────────────────────────

#[test]
fn session_lifecycle_events() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let e = events.clone();
    session.on_event(move |ev| {
        e.lock().unwrap().push(format!("{:?}", ev));
        true
    });

    session.on_suspend();
    session.on_resume();
    session.on_shutdown();

    let evts = events.lock().unwrap();
    assert_eq!(evts.len(), 3);
    assert!(evts[0].contains("Background"));
    assert!(evts[1].contains("Foreground"));
    assert!(evts[2].contains("Shutdown"));
}

// ── PatternRouter integration ────────────────────────────────────────

#[test]
fn pattern_router_in_session_chain() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));

    // Register via session.route() shorthand
    session.route("/app/*", webview_app().with_profile(Profile::App));
    session.route("/remote/*", remote_fetch());
    session.route("/auth/*", remote_fetch().with_profile(Profile::Auth));

    // Closure handler
    session.on_navigate(|intent, _| {
        if intent.url.contains("/special/") {
            Some(webview_app().with_profile(Profile::Devtools))
        } else {
            None
        }
    });

    // Pattern wins for /app
    assert_eq!(session.resolve_route(&intent("/app/items")).source, RouteSource::WebviewApp);
    // Pattern wins for /remote
    assert_eq!(session.resolve_route(&intent("/remote/dash")).source, RouteSource::RemoteServer);
    // Closure claims /special
    assert_eq!(session.resolve_route(&intent("/special/debug")).profile, Profile::Devtools);
    // Nothing claims /other → default
    assert_eq!(session.resolve_route(&intent("/other")).profile, Profile::UntrustedRemote);
}

// ── Profile gate full matrix ─────────────────────────────────────────

#[test]
fn profile_gate_full_matrix() {
    // App: everything
    let g = ProfileGate::new(Profile::App);
    for svc in &[Service::Database, Service::Auth, Service::NativeApi, Service::Http] {
        for acc in &[Access::Read, Access::Write, Access::Execute] {
            assert!(g.check(*svc, *acc).is_ok(), "App should allow {svc:?}/{acc:?}");
        }
    }

    // UntrustedRemote: only Http Read
    let g = ProfileGate::new(Profile::UntrustedRemote);
    assert!(g.check(Service::Http, Access::Read).is_ok());
    assert!(g.check(Service::Database, Access::Read).is_err());
    assert!(g.check(Service::Auth, Access::Read).is_err());
    assert!(g.check(Service::NativeApi, Access::Execute).is_err());
}

// ── Protocol selection chain ─────────────────────────────────────────

#[test]
fn protocol_selection_integration() {
    // Priority: decision hint → query param → content detection → default
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    session.route("/api/*", remote_fetch());

    // Client requests JSON via query param
    let intent = NavigationIntent {
        url: "ewe://localhost/api/data?proto=json".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };
    let decision = session.resolve_route(&intent);

    let proto = select_protocol(
        &decision.protocol,       // Default
        Some(&"json".to_string()), // ?proto=json → selects Json
        b"hello",
    );
    assert_eq!(proto, Protocol::Json);

    let (body, ct) = encode_protocol(&proto, b"{\"ok\":true}");
    assert_eq!(ct, "application/primal-json");
    assert_eq!(body, b"{\"ok\":true}");
}

// ── Helpers ──────────────────────────────────────────────────────────

fn intent(url: &str) -> NavigationIntent {
    NavigationIntent {
        url: format!("ewe://localhost{url}"),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    }
}
