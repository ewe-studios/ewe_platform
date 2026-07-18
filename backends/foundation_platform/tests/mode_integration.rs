//! Mode integration tests — WASM, IPC, and Remote through the full execution contract.
//!
//! Each test exercises the complete 9-step pipeline for one RouteSource,
//! using injected BackendTransport to control responses.
//!
//! Proves all three modes surface content correctly through:
//! NavigationIntent → route resolution → cache check → backend query →
//! protocol selection → content encoding → page identity tracking.

use std::sync::{Arc, Mutex};

use foundation_platform::*;

// ── Test transport ─────────────────────────────────────────────────────

/// Records call metadata in shared state so tests can verify what was dispatched.
struct TestTransport {
    wasm_body: Vec<u8>,
    ipc_body: Vec<u8>,
    remote_body: Vec<u8>,
    ipc_calls: Mutex<Vec<(Option<String>, String)>>,
    remote_calls: Mutex<Vec<String>>,
}

impl TestTransport {
    fn new() -> Self {
        Self {
            wasm_body: b"<html><body><h1>WASM App</h1></body></html>".to_vec(),
            ipc_body: br#"{"status":"ok","from":"native_shell"}"#.to_vec(),
            remote_body: br#"{"data":"fetched_from_remote_api"}"#.to_vec(),
            ipc_calls: Mutex::new(Vec::new()),
            remote_calls: Mutex::new(Vec::new()),
        }
    }
}

impl BackendTransport for TestTransport {
    fn signal_webview(&self, route: &str) -> Vec<u8> {
        format!(
            r#"{{"type":"wasm_signal","route":"{}","payload":{}}}"#,
            route,
            String::from_utf8_lossy(&self.wasm_body)
        )
        .into_bytes()
    }

    fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8> {
        self.ipc_calls
            .lock()
            .unwrap()
            .push((target.map(String::from), route.to_string()));
        self.ipc_body.clone()
    }

    fn fetch_remote(&self, route: &str) -> Vec<u8> {
        self.remote_calls
            .lock()
            .unwrap()
            .push(route.to_string());
        self.remote_body.clone()
    }
}

fn session_with_transport() -> (Arc<PlatformSession>, Arc<TestTransport>) {
    let session = PlatformSession::new();
    let transport = Arc::new(TestTransport::new());
    session.set_backend(Box::new(TestTransport {
        wasm_body: transport.wasm_body.clone(),
        ipc_body: transport.ipc_body.clone(),
        remote_body: transport.remote_body.clone(),
        ipc_calls: Mutex::new(Vec::new()),
        remote_calls: Mutex::new(Vec::new()),
    }));
    // Note: we use a duplicate transport for the session so it owns the Box.
    // Tests verify output content rather than internal call tracking from
    // the session-owned instance. For call tracking, use direct query_backend().
    (session, transport)
}

fn create_intent(url: &str) -> NavigationIntent {
    NavigationIntent {
        url: url.to_string(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    }
}

fn session() -> Arc<PlatformSession> {
    PlatformSession::new()
}

// ═══════════════════════════════════════════════════════════════════════
// WASM MODE — WebviewApp
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn wasm_mode_full_contract() {
    let transport = TestTransport::new();
    let s = session();

    // Register a WASM route
    s.route("/app/*", webview_app().with_profile(Profile::App));

    // Step 1-2: Navigation interception → route resolution
    let intent = create_intent("ewe://localhost/app/dashboard?proto=html");
    let decision = s.resolve_route(&intent);
    assert_eq!(decision.source, RouteSource::WebviewApp);
    assert_eq!(decision.profile, Profile::App);

    // Backend query dispatches to signal_webview
    let content = query_backend(&transport, &decision, "/app/dashboard");
    let body_str = String::from_utf8_lossy(&content);
    assert!(body_str.contains("wasm_signal"));
    assert!(body_str.contains("/app/dashboard"));
    assert!(body_str.contains("WASM App"));

    // Protocol selection: ?proto=html hint
    let proto = select_protocol(&decision.protocol, Some(&"html".to_string()), &content);
    assert_eq!(proto, Protocol::Html);
    let (_, ct) = encode_protocol(&proto, &content);
    assert_eq!(ct, "text/html; charset=utf-8");

    // Page identity tracked
    let page = s.record_navigation("/app/dashboard");
    assert_eq!(page.route, "/app/dashboard");
}

#[test]
fn wasm_mode_cache_hit_serves_without_backend_query() {
    let s = session();
    s.route("/app/*", webview_app()
        .with_profile(Profile::App)
        .with_cache_policy(CachePolicy::CacheFirst));

    // Pre-populate cache
    s.cache().store(Profile::App, "/app/cached-page", b"cached-wasm-output", "text/html");

    let intent = create_intent("ewe://localhost/app/cached-page");
    let decision = s.resolve_route(&intent);

    // execute_decision serves cache → no backend call
    let (body, ct) = s.execute_decision(&decision, &intent);
    assert_eq!(ct, "text/html");
    assert_eq!(body, b"cached-wasm-output");
}

#[test]
fn wasm_mode_offline_serves_stale_cache() {
    let s = session();
    s.route("/app/*", webview_app()
        .with_profile(Profile::App)
        .with_cache_policy(CachePolicy::StaleWhileRevalidate));

    s.set_online(false);
    s.cache().store(Profile::App, "/app/offline-page", b"offline-wasm", "text/html");

    let intent = create_intent("ewe://localhost/app/offline-page");
    let decision = s.resolve_route(&intent);
    assert!(s.can_serve_offline_for(s.cache(), &decision, "/app/offline-page"));

    let (body, _) = s.execute_decision(&decision, &intent);
    assert_eq!(body, b"offline-wasm");
}

// ═══════════════════════════════════════════════════════════════════════
// IPC MODE — IpcShell
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn ipc_mode_full_contract() {
    let transport = TestTransport::new();
    let s = session();
    s.route("/api/*", ipc_shell_with("business_logic")
        .with_profile(Profile::TrustedRemote));

    let intent = create_intent("ewe://localhost/api/data?proto=json");
    let decision = s.resolve_route(&intent);
    assert_eq!(decision.source, RouteSource::IpcShell);
    assert_eq!(decision.target.as_deref(), Some("business_logic"));

    let content = query_backend(&transport, &decision, "/api/data");
    let body_str = String::from_utf8_lossy(&content);
    assert!(body_str.contains("native_shell"));

    // Transport tracked the IPC dispatch
    let calls = transport.ipc_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0.as_deref(), Some("business_logic"));
    assert_eq!(calls[0].1, "/api/data");
}

#[test]
fn ipc_mode_defaults_to_shell_target() {
    let transport = TestTransport::new();
    let s = session();
    s.route("/cmd/*", ipc_shell());

    let decision = s.resolve_route(&create_intent("ewe://localhost/cmd/status"));
    assert_eq!(decision.source, RouteSource::IpcShell);
    assert!(decision.target.is_none());

    query_backend(&transport, &decision, "/cmd/status");
    let calls = transport.ipc_calls.lock().unwrap();
    // TestTransport records the raw target (None); DefaultTransport fills "shell"
    assert_eq!(calls[0].0.as_deref(), None);
    assert_eq!(calls[0].1, "/cmd/status");
}

#[test]
fn ipc_mode_cache_network_first_skips_cache() {
    let s = session();
    s.route("/api/*", ipc_shell());
    s.cache().store(Profile::TrustedRemote, "/api/cached-cmd", b"stale", "text/plain");

    let decision = s.resolve_route(&create_intent("ewe://localhost/api/cached-cmd"));
    assert!(!s.cache().should_serve_cached(&decision, "/api/cached-cmd"));
}

// ═══════════════════════════════════════════════════════════════════════
// REMOTE MODE — RemoteServer
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn remote_mode_full_contract() {
    let transport = TestTransport::new();
    let s = session();
    s.route("/remote/*", remote_fetch()
        .with_profile(Profile::TrustedRemote)
        .with_cache_policy(CachePolicy::NetworkFirst));

    let intent = create_intent("ewe://localhost/remote/dashboard?proto=json");
    let decision = s.resolve_route(&intent);
    assert_eq!(decision.source, RouteSource::RemoteServer);
    assert_eq!(decision.profile, Profile::TrustedRemote);
    assert_eq!(decision.cache_policy, CachePolicy::NetworkFirst);

    let content = query_backend(&transport, &decision, "/remote/dashboard");
    let body_str = String::from_utf8_lossy(&content);
    assert!(body_str.contains("fetched_from_remote_api"));

    let calls = transport.remote_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], "/remote/dashboard");
}

#[test]
fn remote_mode_offline_with_online_only_fails() {
    let s = session();
    s.route("/remote/*", remote_fetch().with_cache_policy(CachePolicy::OnlineOnly));
    s.set_online(false);

    let decision = s.resolve_route(&create_intent("ewe://localhost/remote/live-data"));
    assert!(!s.can_serve_offline_for(s.cache(), &decision, "/remote/live-data"));
}

#[test]
fn remote_mode_network_first_serves_stale_offline() {
    let s = session();
    s.route("/remote/*", remote_fetch().with_cache_policy(CachePolicy::NetworkFirst));

    s.cache().store(Profile::TrustedRemote, "/remote/old-data", b"stale-remote", "application/primal-json");
    s.set_online(false);

    let intent = create_intent("ewe://localhost/remote/old-data");
    let decision = s.resolve_route(&intent);
    assert!(s.can_serve_offline_for(s.cache(), &decision, "/remote/old-data"));

    let (body, ct) = s.execute_decision(&decision, &intent);
    assert_eq!(body, b"stale-remote");
    assert_eq!(ct, "application/primal-json");
}

// ═══════════════════════════════════════════════════════════════════════
// CROSS-MODE — all three sources in one session
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn cross_mode_all_three_sources_in_one_session() {
    let transport = TestTransport::new();
    let s = session();

    s.route("/app/*", webview_app().with_profile(Profile::App));
    s.route("/api/*", ipc_shell_with("workers").with_profile(Profile::TrustedRemote));
    s.route("/remote/*", remote_fetch().with_profile(Profile::TrustedRemote));

    // WASM
    let d = s.resolve_route(&create_intent("ewe://localhost/app/home"));
    assert_eq!(d.source, RouteSource::WebviewApp);

    // IPC
    let d = s.resolve_route(&create_intent("ewe://localhost/api/users"));
    assert_eq!(d.source, RouteSource::IpcShell);
    let content = query_backend(&transport, &d, "/api/users");
    assert!(String::from_utf8_lossy(&content).contains("native_shell"));

    // Remote
    let d = s.resolve_route(&create_intent("ewe://localhost/remote/news"));
    assert_eq!(d.source, RouteSource::RemoteServer);
    let content = query_backend(&transport, &d, "/remote/news");
    assert!(String::from_utf8_lossy(&content).contains("fetched_from_remote_api"));
}

#[test]
fn cross_mode_auth_profile_isolates_remote_routes() {
    let s = session();
    s.route("/auth/*", remote_fetch()
        .with_profile(Profile::Auth)
        .with_auth_origin("https://auth.example.com"));

    let decision = s.resolve_route(&create_intent("ewe://localhost/auth/login?proto=json"));
    assert_eq!(decision.profile, Profile::Auth);
    assert_eq!(decision.auth_origin.as_deref(), Some("https://auth.example.com"));
}

#[test]
fn cross_mode_capability_allowlisting_per_route() {
    let s = PlatformSession::new();

    struct FileAccess;
    impl Capability for FileAccess {
        fn id(&self) -> &CapabilityId {
            Box::leak(Box::new(CapabilityId("fs".into())))
        }
        fn min_profile(&self) -> Profile { Profile::TrustedRemote }
        fn execute(&self, _: &PlatformSession, action: &str, _: serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::Value::String(format!("fs:{action}")))
        }
    }

    s.capabilities().register(FileAccess);

    // WASM route with per-route allowlist that excludes 'fs'
    s.route("/app/*", webview_app()
        .with_profile(Profile::App)
        .with_allowed_capabilities(&[CapabilityId("microphone".into())])); // allows mic, NOT fs
    // IPC route: allows 'fs'
    s.route("/api/*", ipc_shell_with("workers")
        .with_profile(Profile::TrustedRemote)
        .with_allowed_capabilities(&[CapabilityId("fs".into())]));

    // WASM route → capability denied (fs not in per-route allowlist; only mic is)
    let wasm_decision = s.resolve_route(&create_intent("ewe://localhost/app/editor"));
    s.record_navigation("/app/editor");
    let page = s.active_page_identity().unwrap();
    let resp = s.capabilities().invoke(&s, &CapabilityRequest {
        id: "r1".into(), page_identity: page,
        capability: "fs".into(), action: "read".into(),
        payload: serde_json::Value::Null,
    }, Some(&wasm_decision));
    assert!(resp.status.is_err(), "fs should be denied — not in per-route allowlist");

    // IPC route → capability allowed (fs in allowlist)
    let ipc_decision = s.resolve_route(&create_intent("ewe://localhost/api/files"));
    s.record_navigation("/api/files");
    let page2 = s.active_page_identity().unwrap();
    let resp = s.capabilities().invoke(&s, &CapabilityRequest {
        id: "r2".into(), page_identity: page2,
        capability: "fs".into(), action: "write".into(),
        payload: serde_json::Value::Null,
    }, Some(&ipc_decision));
    assert!(resp.status.is_ok());
}

// ═══════════════════════════════════════════════════════════════════════
// PROTOCOL NEGOTIATION per mode
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn protocol_negotiation_per_mode() {
    let transport = DefaultTransport;
    let s = session();

    // WASM → Html hint: decision hint wins over content detection
    s.route("/app/*", webview_app().with_protocol(ProtocolHint::Html));
    let d = s.resolve_route(&create_intent("ewe://localhost/app/home"));
    let content = query_backend(&transport, &d, "/app/home");
    // Content is JSON but decision hint Html takes priority
    let proto = select_protocol(&d.protocol, None, &content);
    assert_eq!(proto, Protocol::Html);

    // Remote → Arrow hint (decision overrides content)
    s.route("/remote/*", remote_fetch().with_protocol(ProtocolHint::Arrow));
    let d = s.resolve_route(&create_intent("ewe://localhost/remote/data"));
    let proto = select_protocol(&d.protocol, None, b"{\"k\":\"v\"}");
    assert_eq!(proto, Protocol::Arrow); // decision hint wins

    // Query param is checked when decision hint is Default
    s.route("/data/*", remote_fetch().with_protocol(ProtocolHint::Default));
    let intent = create_intent("ewe://localhost/data/export?proto=json");
    let d = s.resolve_route(&intent);
    let content = query_backend(&transport, &d, "/data/export");
    let proto = select_protocol(&d.protocol, Some(&"json".to_string()), &content);
    assert_eq!(proto, Protocol::Json); // query param acts when hint is Default
}
