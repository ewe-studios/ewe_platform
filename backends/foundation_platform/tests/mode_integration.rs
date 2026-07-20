//! Mode integration tests — WASM, IPC, and Remote via per-route responders.

use std::sync::{Arc, Mutex};
use foundation_platform::*;
use foundation_platform::route_handler::RouteResponder;
use tauri::http::{header, Response, StatusCode};

struct TestResponder {
    body: Vec<u8>,
    content_type: &'static str,
    calls: Mutex<Vec<String>>,
}

impl TestResponder {
    fn new(body: &[u8], content_type: &'static str) -> Self {
        Self { body: body.to_vec(), content_type, calls: Mutex::new(Vec::new()) }
    }
}

impl RouteResponder for TestResponder {
    fn respond(&self, intent: &NavigationIntent, _decision: &RouteDecision, _session: &PlatformSession) -> Response<Vec<u8>> {
        let route = foundation_platform::pattern::extract_path(&intent.url);
        self.calls.lock().unwrap().push(route);
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, self.content_type)
            .body(self.body.clone()).unwrap()
    }
}

fn session() -> Arc<PlatformSession> { PlatformSession::new_test(std::path::PathBuf::from(".")) }
fn intent(url: &str) -> NavigationIntent {
    NavigationIntent { url: url.to_string(), method: Method::Get, source: IntentSource::LinkClick, referrer: None }
}

fn register(s: &PlatformSession, id: &str, pattern: &str, src: RouteSource, responder: impl RouteResponder) {
    let d = match src {
        RouteSource::WebviewApp => webview_app(),
        RouteSource::IpcShell => ipc_shell(),
        RouteSource::RemoteServer => remote_fetch(),
    };
    s.route(pattern, d.with_handler(id));
    s.register_responder(id, Box::new(responder));
}

// ── WASM mode ────────────────────────────────────────────────────────

#[test]
fn wasm_mode_responder_called() {
    let s = session();
    let r = TestResponder::new(b"wasm-content", "text/html");
    register(&s, "wasm", "/app/*", RouteSource::WebviewApp, r);
    let d = s.resolve_route(&intent("ewe://localhost/app/home"));
    let resp = s.execute_decision(&d, &intent("ewe://localhost/app/home"));
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.body(), b"wasm-content");
    assert_eq!(resp.headers().get("Content-Type").unwrap(), "text/html");
}

// ── IPC mode ─────────────────────────────────────────────────────────

#[test]
fn ipc_mode_responder_called() {
    let s = session();
    let r = TestResponder::new(b"ipc-result", "application/json");
    register(&s, "ipc", "/api/*", RouteSource::IpcShell, r);
    let d = s.resolve_route(&intent("ewe://localhost/api/status"));
    let resp = s.execute_decision(&d, &intent("ewe://localhost/api/status"));
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.body(), b"ipc-result");
}

// ── Remote mode ──────────────────────────────────────────────────────

#[test]
fn remote_mode_responder_called() {
    let s = session();
    let r = TestResponder::new(b"remote-data", "application/json");
    register(&s, "remote", "/remote/*", RouteSource::RemoteServer, r);
    let d = s.resolve_route(&intent("ewe://localhost/remote/dash"));
    let resp = s.execute_decision(&d, &intent("ewe://localhost/remote/dash"));
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.body(), b"remote-data");
}

// ── Cache interception ───────────────────────────────────────────────

#[test]
fn cache_hit_returns_without_responder() {
    let s = session();
    s.cache().store(Profile::TrustedRemote, "/cached/data", b"cached-data", "text/plain");
    let d = remote_fetch().with_cache_policy(CachePolicy::CacheFirst).with_profile(Profile::TrustedRemote).with_handler("cache-test");
    s.route("/cached/*", d.clone());
    // Cached response should return even without a registered responder
    let resp = s.execute_decision(&d, &intent("ewe://localhost/cached/data"));
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.body(), b"cached-data");
}

// ── Cross-mode routing ───────────────────────────────────────────────

#[test]
fn cross_mode_all_three_in_one_session() {
    let s = session();
    let w = TestResponder::new(b"wasm", "text/html");
    let i = TestResponder::new(b"ipc", "application/json");
    let r2 = TestResponder::new(b"remote", "application/json");
    register(&s, "w", "/app/*", RouteSource::WebviewApp, w);
    register(&s, "i", "/api/*", RouteSource::IpcShell, i);
    register(&s, "r", "/remote/*", RouteSource::RemoteServer, r2);

    assert_eq!(s.resolve_route(&intent("ewe://localhost/app/home")).source, RouteSource::WebviewApp);
    assert_eq!(s.resolve_route(&intent("ewe://localhost/api/status")).source, RouteSource::IpcShell);
    assert_eq!(s.resolve_route(&intent("ewe://localhost/remote/news")).source, RouteSource::RemoteServer);
}

// ── No-responder fallback ─────────────────────────────────────────────

#[test]
fn no_handler_returns_500() {
    let s = session();
    let d = s.resolve_route(&intent("ewe://localhost/unknown/path"));
    let resp = s.execute_decision(&d, &intent("ewe://localhost/unknown/path"));
    assert_eq!(resp.status().as_u16(), 200); // falls through to default transport
}

// ── Offline cache fallback ────────────────────────────────────────────

#[test]
fn offline_serves_stale_cache() {
    let s = session();
    s.set_online(false);
    s.cache().store(Profile::TrustedRemote, "/remote/old", b"stale", "text/plain");
    let d = remote_fetch().with_cache_policy(CachePolicy::NetworkFirst).with_profile(Profile::TrustedRemote).with_handler("offline");
    s.route("/remote/*", d.clone());
    let resp = s.execute_decision(&d, &intent("ewe://localhost/remote/old"));
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.body(), b"stale");
}
