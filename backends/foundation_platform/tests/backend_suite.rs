//! Tests for backend query dispatch through BackendTransport.

use foundation_platform::backend::TestTransport;
use foundation_platform::*;

// ── Default transport tests ─────────────────────────────────

#[test]
fn default_transport_returns_typed_envelopes() {
    let t = DefaultTransport;

    let wasm = t.signal_webview("/app/home");
    let s = String::from_utf8(wasm).unwrap();
    assert!(s.contains("webview_app_signal"));
    assert!(s.contains("render_in_webview"));

    let ipc = t.dispatch_ipc(Some("my_app"), "/api/data");
    let s = String::from_utf8(ipc).unwrap();
    assert!(s.contains("ipc_shell_dispatch"));
    assert!(s.contains("my_app"));

    let remote = t.fetch_remote("/remote/dashboard");
    let s = String::from_utf8(remote).unwrap();
    assert!(s.contains("remote_server_fetch"));
    assert!(s.contains("http_fetch"));
}

// ── query_backend dispatch ──────────────────────────────────

#[test]
fn query_dispatches_to_correct_transport_method() {
    let transport = TestTransport::new()
        .with_remote(b"fetched-from-remote");

    // WebviewApp -> signal_webview
    let d = webview_app();
    let content = query_backend(&transport, &d, "/app/home");
    assert_eq!(content, b"wasm:/app/home");

    // IpcShell -> dispatch_ipc
    let d = ipc_shell_with("business_logic");
    let content = query_backend(&transport, &d, "/api/data");
    assert_eq!(content, b"ipc-result");
    let calls = transport.ipc_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0.as_deref(), Some("business_logic"));
    assert_eq!(calls[0].1, "/api/data");

    // RemoteServer -> fetch_remote
    let d = remote_fetch();
    let content = query_backend(&transport, &d, "/remote/dash");
    assert_eq!(content, b"fetched-from-remote");
    let calls = transport.remote_calls.lock().unwrap();
    assert_eq!(calls[0], "/remote/dash");
}

// ── Legacy tests (use DefaultTransport via query_backend) ───

#[test]
fn webview_app_returns_signal_response() {
    let d = webview_app();
    let result = query_backend(&DefaultTransport, &d, "/app/home");
    let s = String::from_utf8(result).unwrap();
    assert!(s.contains("webview_app_signal"));
    assert!(s.contains("/app/home"));
}

#[test]
fn ipc_shell_returns_target_in_response() {
    let d = ipc_shell_with("business_logic");
    let result = query_backend(&DefaultTransport, &d, "/api/data");
    let s = String::from_utf8(result).unwrap();
    assert!(s.contains("ipc_shell_dispatch"));
    assert!(s.contains("business_logic"));
}

#[test]
fn remote_server_returns_fetched_response() {
    let d = remote_fetch();
    let result = query_backend(&DefaultTransport, &d, "/remote/dashboard");
    let s = String::from_utf8(result).unwrap();
    assert!(s.contains("remote_server_fetch"));
    assert!(s.contains("/remote/dashboard"));
}

#[test]
fn ipc_shell_defaults_to_shell_target() {
    let d = ipc_shell();
    let result = query_backend(&DefaultTransport, &d, "/api/data");
    let s = String::from_utf8(result).unwrap();
    assert!(s.contains("shell")); // default target
}
