//! ewe:// handler integration — exercises full request → response pipeline.
//!
//! Proves that EweUrl parsing, session route resolution, backend query,
//! protocol selection, and content encoding all compose through the handler.
//! Does NOT require a running Tauri app — tests the handler logic directly.

use foundation_platform::*;

/// Simulate the full ewe:// request flow:
/// Parse URI → resolve route → query backend → select protocol → encode
#[test]
fn full_ewe_pipeline_webview_app() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    session.route("/app/*", webview_app().with_profile(Profile::App));

    // Step 1-2: Parse URI, resolve route
    let intent = NavigationIntent {
        url: "ewe://localhost/app/items?proto=json".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };
    let decision = session.resolve_route(&intent);
    assert_eq!(decision.source, RouteSource::WebviewApp);

    // Step 3: Cache check — NetworkFirst for WebviewApp? No — CacheFirst
    // Actually webview_app() defaults to CacheFirst
    let should_cache = session.cache().should_serve_cached(&decision, "/app/items");
    assert!(!should_cache); // no cached entry yet

    // Step 5: Backend query
    let content = query_backend(&DefaultTransport, &decision, "/app/items");
    assert!(!content.is_empty());

    // Step 6: Protocol selection (query param = json)
    let proto = select_protocol(&ProtocolHint::Default, Some(&"json".to_string()), &content);
    assert_eq!(proto, Protocol::Json);

    // Step 7: Encode
    let (_body, ct) = encode_protocol(&proto, &content);
    assert_eq!(ct, "application/primal-json");

    // Step 9: Record navigation
    let page = session.record_navigation("/app/items");
    assert_eq!(page.route, "/app/items");
}

#[test]
fn full_ewe_pipeline_remote_server() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    session.route("/remote/*", remote_fetch()
        .with_profile(Profile::TrustedRemote)
        .with_cache_policy(CachePolicy::NetworkFirst));

    let intent = NavigationIntent {
        url: "ewe://localhost/remote/dashboard?proto=arrow".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };
    let decision = session.resolve_route(&intent);
    assert_eq!(decision.source, RouteSource::RemoteServer);
    assert_eq!(decision.profile, Profile::TrustedRemote);
    assert_eq!(decision.cache_policy, CachePolicy::NetworkFirst);

    // Backend query
    let content = query_backend(&DefaultTransport, &decision, "/remote/dashboard");
    let s = String::from_utf8(content.clone()).unwrap();
    assert!(s.contains("remote_server"));

    // Protocol selection with arrow hint
    let proto = select_protocol(&ProtocolHint::Default, Some(&"arrow".to_string()), &content);
    assert_eq!(proto, Protocol::Arrow);
}

#[test]
fn full_ewe_pipeline_ipc_shell_with_target() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    // Route that explicitly targets a wasm_app
    session.route("/api/*", ipc_shell_with("business_logic"));

    let intent = NavigationIntent {
        url: "ewe://localhost/api/data".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };
    let decision = session.resolve_route(&intent);
    assert_eq!(decision.source, RouteSource::IpcShell);

    // Verify target was resolved
    let content = query_backend(&DefaultTransport, &decision, "/api/data");
    let s = String::from_utf8(content.clone()).unwrap();
    assert!(s.contains("business_logic"));
}

#[test]
fn cache_intercepts_before_backend_query() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    session.route("/cached/*", remote_fetch()
        .with_cache_policy(CachePolicy::CacheFirst));

    // Pre-populate cache
    session.cache().store(Profile::TrustedRemote, "/cached/data", b"cached-content", "text/html");

    let intent = NavigationIntent {
        url: "ewe://localhost/cached/data".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };
    let decision = session.resolve_route(&intent);

    // Cache check should return true — skip backend query
    assert!(session.cache().should_serve_cached(&decision, "/cached/data"));

    // The actual cached entry is available
    let entry = session.cache().get(Profile::TrustedRemote, "/cached/data").unwrap();
    assert_eq!(entry.body, b"cached-content");
    assert_eq!(entry.content_type, "text/html");
}

#[test]
fn capability_gating_in_full_pipeline() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    session.route("/camera/*", webview_app()
        .with_profile(Profile::App)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]));

    let intent = NavigationIntent {
        url: "ewe://localhost/camera/viewfinder".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };
    let decision = session.resolve_route(&intent);
    assert_eq!(decision.source, RouteSource::WebviewApp);
    assert!(decision.capabilities.iter().any(|c| c.0 == "camera"));
}

#[test]
fn offline_route_fails_with_online_only() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));
    session.route("/live/*", remote_fetch()
        .with_cache_policy(CachePolicy::OnlineOnly));

    let intent = NavigationIntent {
        url: "ewe://localhost/live/scores".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };
    let decision = session.resolve_route(&intent);

    // OnlineOnly never caches — no cache entry exists
    let has_cache = session.cache().get(decision.profile, "/live/scores").is_some();
    assert!(!has_cache);

    // OnlineOnly fails offline
    session.set_online(false);
    let can_serve = session.cache().can_serve_offline(&decision, "/live/scores");
    assert!(!can_serve);
}

#[test]
fn mutation_queue_integrates_with_connectivity() {
    let session = PlatformSession::new(std::path::PathBuf::from("."));

    // Go offline, enqueue
    session.set_online(false);
    session.mutation_queue().enqueue("create", serde_json::json!({"item": "x"}));
    session.mutation_queue().enqueue("update", serde_json::json!({"id": 1}));
    assert_eq!(session.mutation_queue().pending_count(), 2);

    // Come online, replay
    session.set_online(true);
    let applied = std::sync::Mutex::new(Vec::new());
    let result = session.mutation_queue().replay(|m| {
        applied.lock().unwrap().push(m.mutation_type.clone());
        Ok(serde_json::Value::String(format!("processed-{}", m.id)))
    });

    assert_eq!(result.applied.len(), 2);
    assert_eq!(session.mutation_queue().pending_count(), 0);
}
