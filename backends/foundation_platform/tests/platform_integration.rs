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
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

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
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Register routes for different surfaces
    session.route("/app/*", webview_app().with_profile(Profile::App));
    session.route(
        "/remote/*",
        remote_fetch()
            .with_profile(Profile::TrustedRemote)
            .with_cache_policy(CachePolicy::NetworkFirst),
    );
    session.route(
        "/cached/*",
        remote_fetch().with_cache_policy(CachePolicy::CacheFirst),
    );
    session.route("/auth/*", remote_fetch().with_profile(Profile::Auth));

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
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Cache content while "online"
    session
        .cache()
        .store(Profile::App, "/data", b"live-data", "text/html");

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
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Go offline, enqueue mutations
    session.set_online(false);
    let _id1 = session
        .mutation_queue()
        .enqueue("create", serde_json::json!({"item": "x"}));
    let _id2 = session
        .mutation_queue()
        .enqueue("update", serde_json::json!({"id": 1, "name": "y"}));
    assert_eq!(session.mutation_queue().pending_count(), 2);

    // Come back online, replay
    session.set_online(true);
    let result = session
        .mutation_queue()
        .replay(|m| Ok(serde_json::Value::String(format!("ok-{}", m.mutation_type))));

    assert_eq!(result.applied.len(), 2);
    assert_eq!(session.mutation_queue().pending_count(), 0);
}

// ── Capability invocation through registry ───────────────────────────

#[test]
fn capability_invocation_with_profile_gating() {
    use foundation_platform::capability::PlatformIpc;
    use foundation_ui_traits::{CapabilityId, Profile};
    use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    struct Cam;
    impl Ipc<Vec<u8>, Vec<u8>> for Cam {
        fn name(&self) -> &str {
            "camera"
        }
        fn kind(&self) -> IpcKind {
            IpcKind::Capability
        }
        fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
            let action = &request.action;
            Ok(IpcResponse {
                payload: format!("shot-{action}").into_bytes(),
                content_type: request.content_type,
            })
        }
    }
    impl PlatformIpc for Cam {
        fn capability_id(&self) -> &CapabilityId {
            Box::leak(Box::new(CapabilityId("camera".into())))
        }
        fn min_profile(&self) -> Profile {
            Profile::TrustedRemote
        }
    }

    session.capabilities().register(Cam);

    // App profile + camera allowed → succeeds
    session.record_navigation("/camera");
    let page = session.active_page_identity().unwrap();
    let route = webview_app()
        .with_profile(Profile::App)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);

    let req = IpcRequest {
        ipc: "camera".into(),
        action: "capture".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
        target: None,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    session
        .capabilities()
        .invoke(&session, &req, &page, Some(&route), move |r| {
            let _ = tx.send(r);
        })
        .unwrap();
    assert!(rx.recv().unwrap().is_ok());

    // UntrustedRemote profile → denied (security reject — invoke returns Err)
    let bad_route = remote_fetch()
        .with_profile(Profile::UntrustedRemote)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);
    session.record_navigation("/bad");
    let page2 = session.active_page_identity().unwrap();
    assert!(session
        .capabilities()
        .invoke(&session, &req, &page2, Some(&bad_route), |_| {})
        .is_err());

    // Stale page → denied (security reject — invoke returns Err)
    session.record_navigation("/other");
    assert!(session
        .capabilities()
        .invoke(&session, &req, &page, Some(&route), |_| {})
        .is_err());
}

// ── WebView stack navigation ─────────────────────────────────────────

#[test]
fn webview_stack_navigation_cycle() {
    struct WV {
        navs: Mutex<Vec<String>>,
        reloads: Mutex<usize>,
    }
    impl WebViewOps for WV {
        fn navigate(&self, url: &str) {
            self.navs.lock().unwrap().push(url.to_string());
        }
        fn screenshot(&self) -> Vec<u8> {
            vec![1, 2, 3]
        }
        fn eval(&self, _: &str) {}
        fn reload(&self) {
            *self.reloads.lock().unwrap() += 1;
        }
    }

    let wv = WV {
        navs: Mutex::new(vec![]),
        reloads: Mutex::new(0),
    };
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
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
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
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

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
    assert_eq!(
        session.resolve_route(&intent("/app/items")).source,
        RouteSource::WebviewApp
    );
    // Pattern wins for /remote
    assert_eq!(
        session.resolve_route(&intent("/remote/dash")).source,
        RouteSource::RemoteServer
    );
    // Closure claims /special
    assert_eq!(
        session.resolve_route(&intent("/special/debug")).profile,
        Profile::Devtools
    );
    // Nothing claims /other → default
    assert_eq!(
        session.resolve_route(&intent("/other")).profile,
        Profile::UntrustedRemote
    );
}

// ── Profile gate full matrix ─────────────────────────────────────────

#[test]
fn profile_gate_full_matrix() {
    // App: everything
    let g = ProfileGate::new(Profile::App);
    for svc in &[
        Service::Database,
        Service::Auth,
        Service::NativeApi,
        Service::Http,
    ] {
        for acc in &[Access::Read, Access::Write, Access::Execute] {
            assert!(
                g.check(*svc, *acc).is_ok(),
                "App should allow {svc:?}/{acc:?}"
            );
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
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
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
        &decision.protocol,        // Default
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

// ── Presentation mode integration: full execute_decision pipeline ───────

/// A simple RouteResponder that returns a fixed response.
struct TestResponder {
    body: String,
}
impl RouteResponder for TestResponder {
    fn respond(
        &self,
        _intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", "text/html")
            .body(self.body.clone().into_bytes())
            .unwrap()
    }
}

#[test]
fn presentation_push_creates_new_stack_slot() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Register a push-mode route with a responder.
    session.register_route_with(
        "/nav_push",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/nav_push"),
        TestResponder {
            body: "push".into(),
        },
    );

    // Execute the decision — this should call record_presentation(Push)
    // which pushes a new slot onto the WebViewStack.
    let decision = session.resolve_route(&intent("/nav_push"));
    let _response = session.execute_decision(&decision, &intent("/nav_push"));

    // Verify: Push should have added a slot.
    let stack = session.webview_stack();
    assert_eq!(
        stack.depth(),
        1,
        "Push should initialize the stack with one slot"
    );
    assert!(stack.active_route().is_some());
}

#[test]
fn presentation_morph_does_not_create_new_slot() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Root first to have one slot
    session.register_route_with(
        "/nav_root",
        webview_app()
            .with_presentation(Presentation::Root)
            .with_handler("__route_/nav_root"),
        TestResponder {
            body: "root".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_root"));
    let _ = session.execute_decision(&d, &intent("/nav_root"));

    let before = session.webview_stack().depth();

    // Now morph — should keep same depth
    session.register_route_with(
        "/nav_morph",
        webview_app()
            .with_presentation(Presentation::Morph)
            .with_handler("__route_/nav_morph"),
        TestResponder {
            body: "morph".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_morph"));
    let _ = session.execute_decision(&d, &intent("/nav_morph"));

    let after = session.webview_stack().depth();
    assert_eq!(
        after, before,
        "Morph should not create a new slot (before={before}, after={after})"
    );
}

#[test]
fn presentation_replace_swaps_content_in_place() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Push then replace
    session.register_route_with(
        "/nav_push",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/nav_push"),
        TestResponder {
            body: "push".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_push"));
    let _ = session.execute_decision(&d, &intent("/nav_push"));
    let depth_after_push = session.webview_stack().depth();

    // Replace at the same depth
    session.register_route_with(
        "/nav_replace",
        webview_app()
            .with_presentation(Presentation::Replace)
            .with_handler("__route_/nav_replace"),
        TestResponder {
            body: "replace".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_replace"));
    let _ = session.execute_decision(&d, &intent("/nav_replace"));

    let depth_after_replace = session.webview_stack().depth();
    assert_eq!(
        depth_after_replace, depth_after_push,
        "Replace should keep same depth"
    );
    // Active route should now be the replaced one
    let active = session
        .webview_stack()
        .active_route()
        .map(|s| s.to_string())
        .unwrap_or_default();
    assert!(
        active.contains("nav_replace"),
        "Active route should be nav_replace, got: {active}"
    );
}

#[test]
fn presentation_root_clears_the_whole_stack() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Root first as base
    session.register_route_with(
        "/home",
        webview_app()
            .with_presentation(Presentation::Root)
            .with_handler("__route_/home"),
        TestResponder {
            body: "home".into(),
        },
    );
    let d = session.resolve_route(&intent("/home"));
    let _ = session.execute_decision(&d, &intent("/home"));
    assert_eq!(session.webview_stack().depth(), 1);

    // Push 1
    session.register_route_with(
        "/push1",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/push1"),
        TestResponder { body: "p1".into() },
    );
    let d = session.resolve_route(&intent("/push1"));
    let _ = session.execute_decision(&d, &intent("/push1"));
    assert_eq!(session.webview_stack().depth(), 2);

    // Push 2
    session.register_route_with(
        "/push2",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/push2"),
        TestResponder { body: "p2".into() },
    );
    let d = session.resolve_route(&intent("/push2"));
    let _ = session.execute_decision(&d, &intent("/push2"));
    assert_eq!(session.webview_stack().depth(), 3);

    // Root clears back to 1
    session.register_route_with(
        "/login",
        webview_app()
            .with_presentation(Presentation::Root)
            .with_handler("__route_/login"),
        TestResponder {
            body: "login".into(),
        },
    );
    let d = session.resolve_route(&intent("/login"));
    let _ = session.execute_decision(&d, &intent("/login"));
    assert_eq!(session.webview_stack().depth(), 1);
    assert_eq!(session.webview_stack().active(), 0);
    assert_eq!(session.webview_stack().active_route(), Some("/login"));
}

#[test]
fn presentation_external_does_not_affect_stack() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Push first to set base
    session.register_route_with(
        "/nav_push",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/nav_push"),
        TestResponder {
            body: "push".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_push"));
    let _ = session.execute_decision(&d, &intent("/nav_push"));

    let depth_before = session.webview_stack().depth();

    // External should NOT change the stack (it goes to the system browser)
    session.register_route_with(
        "/nav_external",
        webview_app()
            .with_presentation(Presentation::External)
            .with_handler("__route_/nav_external"),
        TestResponder {
            body: "external".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_external"));
    let _ = session.execute_decision(&d, &intent("/nav_external"));

    assert_eq!(
        session.webview_stack().depth(),
        depth_before,
        "External navigation should not change the stack"
    );
}

#[test]
fn all_six_presentation_modes_execute_decision_pipeline() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Sequence: Root → Push → Morph → Modal → Replace → External
    // Root: depth 1
    session.register_route_with(
        "/root",
        webview_app()
            .with_presentation(Presentation::Root)
            .with_handler("__route_/root"),
        TestResponder {
            body: "root".into(),
        },
    );
    let d = session.resolve_route(&intent("/root"));
    let _ = session.execute_decision(&d, &intent("/root"));
    assert_eq!(session.webview_stack().depth(), 1);

    // Push: depth 2
    session.register_route_with(
        "/push",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/push"),
        TestResponder {
            body: "push".into(),
        },
    );
    let d = session.resolve_route(&intent("/push"));
    let _ = session.execute_decision(&d, &intent("/push"));
    assert_eq!(session.webview_stack().depth(), 2);

    // Morph: depth stays 2
    session.register_route_with(
        "/morph",
        webview_app()
            .with_presentation(Presentation::Morph)
            .with_handler("__route_/morph"),
        TestResponder {
            body: "morph".into(),
        },
    );
    let d = session.resolve_route(&intent("/morph"));
    let _ = session.execute_decision(&d, &intent("/morph"));
    assert_eq!(session.webview_stack().depth(), 2, "Morph: depth unchanged");

    // Modal: depth 3
    session.register_route_with(
        "/modal",
        webview_app()
            .with_presentation(Presentation::Modal)
            .with_handler("__route_/modal"),
        TestResponder {
            body: "modal".into(),
        },
    );
    let d = session.resolve_route(&intent("/modal"));
    let _ = session.execute_decision(&d, &intent("/modal"));
    assert_eq!(session.webview_stack().depth(), 3, "Modal: adds slot");

    // Replace: depth stays 3
    session.register_route_with(
        "/replace",
        webview_app()
            .with_presentation(Presentation::Replace)
            .with_handler("__route_/replace"),
        TestResponder {
            body: "replace".into(),
        },
    );
    let d = session.resolve_route(&intent("/replace"));
    let _ = session.execute_decision(&d, &intent("/replace"));
    assert_eq!(
        session.webview_stack().depth(),
        3,
        "Replace: depth unchanged"
    );

    // External: depth stays 3 (no stack change)
    session.register_route_with(
        "/external",
        webview_app()
            .with_presentation(Presentation::External)
            .with_handler("__route_/external"),
        TestResponder {
            body: "external".into(),
        },
    );
    let d = session.resolve_route(&intent("/external"));
    let _ = session.execute_decision(&d, &intent("/external"));
    assert_eq!(
        session.webview_stack().depth(),
        3,
        "External: depth unchanged"
    );
}

#[test]
fn presentation_modal_adds_slot_and_pool_label_like_push() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Push for base slot
    session.register_route_with(
        "/nav_push",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/nav_push"),
        TestResponder {
            body: "push".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_push"));
    let _ = session.execute_decision(&d, &intent("/nav_push"));
    assert_eq!(session.webview_stack().depth(), 1);

    // Modal with a target label
    session.register_route_with(
        "/nav_modal",
        webview_app()
            .with_presentation(Presentation::Modal)
            .with_target("modal_settings")
            .with_handler("__route_/nav_modal"),
        TestResponder {
            body: "modal".into(),
        },
    );
    let d = session.resolve_route(&intent("/nav_modal"));
    let _ = session.execute_decision(&d, &intent("/nav_modal"));

    let stack = session.webview_stack();
    assert_eq!(stack.depth(), 2, "Modal should add a slot");
    // Pool should have the modal label if pool is enabled
    if let Some(pool) = stack.pool() {
        assert!(
            pool.get("modal_settings").is_some(),
            "Pool should have modal_settings entry"
        );
    }
}

#[test]
fn presentation_decisions_flow_through_handler_chain_correctly() {
    // Verify that resolve_route returns the correct Presentation even when
    // the handler chain has multiple entries and the matching is by pattern.
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // Register multiple patterns with different presentations
    session.route(
        "/app/*",
        webview_app().with_presentation(Presentation::Morph),
    );
    session.route(
        "/push/*",
        webview_app().with_presentation(Presentation::Push),
    );
    session.route(
        "/modal/*",
        webview_app().with_presentation(Presentation::Modal),
    );
    session.route(
        "/replace/*",
        webview_app().with_presentation(Presentation::Replace),
    );

    // These don't have responders (no handle_id), so execute_decision would fail.
    // But resolve_route works regardless — test the resolve path independently.
    let morph_d = session.resolve_route(&intent("/app/settings"));
    assert_eq!(morph_d.presentation, Presentation::Morph);

    let push_d = session.resolve_route(&intent("/push/new_page"));
    assert_eq!(push_d.presentation, Presentation::Push);

    let modal_d = session.resolve_route(&intent("/modal/confirm"));
    assert_eq!(modal_d.presentation, Presentation::Modal);

    let replace_d = session.resolve_route(&intent("/replace/current"));
    assert_eq!(replace_d.presentation, Presentation::Replace);

    // An unmatched same-origin `ewe://` URL falls through the handler chain to
    // `default_decision_for`, which returns `remote_fetch()` — an UntrustedRemote
    // sandbox whose presentation is `Morph` (see route::remote_fetch). It is NOT
    // External: External is reserved for off-origin http(s) URLs.
    let unknown_d = session.resolve_route(&intent("/unknown/path"));
    assert_eq!(
        unknown_d.presentation,
        Presentation::Morph,
        "unmatched ewe:// falls back to remote_fetch (Morph), not External"
    );
    assert_eq!(unknown_d.profile, Profile::UntrustedRemote);
    assert_eq!(unknown_d.source, RouteSource::RemoteServer);

    // Only off-origin http(s) URLs resolve to External (system browser).
    let external_d = session.resolve_route(&NavigationIntent {
        url: "https://example.com/docs".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    });
    assert_eq!(
        external_d.presentation,
        Presentation::External,
        "off-origin https:// must resolve to External"
    );
}

#[test]
fn record_presentation_directly_works_for_all_modes() {
    // Bypass the handler chain entirely — test record_presentation directly.
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    // First call always inits (depth 0 → init)
    // We do this through execute_decision's record_presentation path.
    // But we can also verify the stack directly.

    // Init via Root
    session.register_route_with(
        "/home",
        webview_app()
            .with_presentation(Presentation::Root)
            .with_handler("__route_/home"),
        TestResponder {
            body: "home".into(),
        },
    );
    let d = session.resolve_route(&intent("/home"));
    let _ = session.execute_decision(&d, &intent("/home"));
    assert_eq!(session.webview_stack().depth(), 1, "Root initializes to 1");
    assert_eq!(session.webview_stack().active_route(), Some("/home"));

    // Push — should go to depth 2
    session.register_route_with(
        "/push1",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/push1"),
        TestResponder {
            body: "push1".into(),
        },
    );
    let d = session.resolve_route(&intent("/push1"));
    let _ = session.execute_decision(&d, &intent("/push1"));
    assert_eq!(session.webview_stack().depth(), 2, "Push 1 → depth 2");
    assert_eq!(session.webview_stack().active_route(), Some("/push1"));

    // Push again — should go to depth 3
    session.register_route_with(
        "/push2",
        webview_app()
            .with_presentation(Presentation::Push)
            .with_handler("__route_/push2"),
        TestResponder {
            body: "push2".into(),
        },
    );
    let d = session.resolve_route(&intent("/push2"));
    let _ = session.execute_decision(&d, &intent("/push2"));
    assert_eq!(session.webview_stack().depth(), 3, "Push 2 → depth 3");
    assert_eq!(session.webview_stack().active_route(), Some("/push2"));

    // Replace — same depth, different route
    session.register_route_with(
        "/replace_me",
        webview_app()
            .with_presentation(Presentation::Replace)
            .with_handler("__route_/replace_me"),
        TestResponder {
            body: "replace".into(),
        },
    );
    let d = session.resolve_route(&intent("/replace_me"));
    let _ = session.execute_decision(&d, &intent("/replace_me"));
    assert_eq!(session.webview_stack().depth(), 3, "Replace keeps depth 3");
    assert_eq!(session.webview_stack().active_route(), Some("/replace_me"));

    // Root — resets to 1
    session.register_route_with(
        "/login",
        webview_app()
            .with_presentation(Presentation::Root)
            .with_handler("__route_/login"),
        TestResponder {
            body: "login".into(),
        },
    );
    let d = session.resolve_route(&intent("/login"));
    let _ = session.execute_decision(&d, &intent("/login"));
    assert_eq!(session.webview_stack().depth(), 1, "Root resets to 1");
    assert_eq!(session.webview_stack().active_route(), Some("/login"));
    assert_eq!(session.webview_stack().active(), 0);
}

// ── F41: IPC stream integration tests ───────────────────────────────────

#[test]
fn host_stream_registry_create_write_read_close() {
    let reg = foundation_platform::handle::HostStreamRegistry::new();
    let (id, _queue) = reg.create();

    assert!(id > 0, "stream ID should be non-zero");
    assert!(reg.write(id, Ok(b"chunk1".to_vec())));
    assert!(reg.write(id, Ok(b"chunk2".to_vec())));

    let c1 = reg.read(id).unwrap().unwrap();
    assert_eq!(c1, b"chunk1");
    let c2 = reg.read(id).unwrap().unwrap();
    assert_eq!(c2, b"chunk2");
    assert!(reg.read(id).is_none()); // empty

    assert!(reg.close(id));
    assert!(!reg.close(id)); // already removed
    assert!(reg.read(999).is_none()); // nonexistent
}

#[test]
fn host_stream_registry_multiple_streams_independent() {
    let reg = foundation_platform::handle::HostStreamRegistry::new();
    let (s1, _q1) = reg.create();
    let (s2, _q2) = reg.create();

    assert_ne!(s1, s2);
    reg.write(s1, Ok(b"stream-one".to_vec()));
    reg.write(s2, Ok(b"stream-two".to_vec()));

    assert_eq!(reg.read(s1).unwrap().unwrap(), b"stream-one");
    assert_eq!(reg.read(s2).unwrap().unwrap(), b"stream-two");
    assert!(reg.read(s1).is_none());
    assert!(reg.read(s2).is_none());
}

#[test]
fn host_stream_registry_error_propagation() {
    let reg = foundation_platform::handle::HostStreamRegistry::new();
    let (id, _q) = reg.create();

    reg.write(
        id,
        Err(foundation_wasm::ipc::IpcError::ExecutionFailed(
            "fail".into(),
        )),
    );
    match reg.read(id).unwrap() {
        Err(foundation_wasm::ipc::IpcError::ExecutionFailed) => {}
        other => panic!("expected ExecutionFailed, got {other:?}"),
    }
}

#[test]
fn capability_handle_registry_round_trip() {
    let reg = foundation_platform::handle::CapabilityHandleRegistry::new();
    let token = reg.insert("camera_session".to_string());

    let result = reg.with::<String, _>(token, |s| {
        assert_eq!(s, "camera_session");
        s.push_str("-modified");
        42u32
    });
    assert_eq!(result, Some(42u32));

    let readback = reg.with::<String, _>(token, |s| s.clone());
    assert_eq!(readback, Some("camera_session-modified".to_string()));

    let removed = reg.remove(token);
    assert!(removed.is_some());
    assert!(reg.with::<String, ()>(token, |_| ()).is_none());
}
