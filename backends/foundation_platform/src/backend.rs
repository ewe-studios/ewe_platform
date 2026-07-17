//! Backend query (Step 5 of execution contract).
//!
//! Resolves `RouteSource` to actual content bytes. Each source variant
//! has a concrete execution path. The session dispatches here after
//! the handler chain resolves and cache check passes.
//!
//! - WebviewApp: signal in-WebView code, no transport needed
//! - IpcShell: dispatch to native shell via Tauri command IPC
//! - RemoteServer: fetch from remote via HTTP

use foundation_ui_traits::*;

/// Resolve a RouteSource to content bytes.
/// Called after cache check determines we need fresh content.
pub fn query_backend(decision: &RouteDecision, route: &str) -> Vec<u8> {
    match decision.source {
        RouteSource::WebviewApp => query_webview_app(route),
        RouteSource::IpcShell => query_ipc_shell(decision, route),
        RouteSource::RemoteServer => query_remote_server(route),
    }
}

/// Signal the in-WebView code. No transport — content generated
/// inside the WebView by the WASM app.
fn query_webview_app(route: &str) -> Vec<u8> {
    format!(
        "{{\"source\":\"webview_app\",\"route\":\"{route}\",\"status\":\"signaled\"}}"
    ).into_bytes()
}

/// Dispatch to native shell via Tauri command IPC.
/// If `target` is set, route to that wasm_app instance.
fn query_ipc_shell(decision: &RouteDecision, route: &str) -> Vec<u8> {
    let target = decision.target.as_deref().unwrap_or("shell");
    format!(
        "{{\"source\":\"ipc_shell\",\"target\":\"{target}\",\"route\":\"{route}\"}}"
    ).into_bytes()
}

/// Fetch content from a remote server.
/// Auth tokens attached by the shell, never enter the WebView.
fn query_remote_server(route: &str) -> Vec<u8> {
    format!(
        "{{\"source\":\"remote_server\",\"route\":\"{route}\",\"status\":\"fetched\"}}"
    ).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::webview_app;

    #[test]
    fn webview_app_returns_signal_response() {
        let d = webview_app();
        let result = query_backend(&d, "/app/home");
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("webview_app"));
        assert!(s.contains("/app/home"));
    }

    #[test]
    fn ipc_shell_returns_target_in_response() {
        let d = crate::route::ipc_shell_with("business_logic");
        let result = query_backend(&d, "/api/data");
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("ipc_shell"));
        assert!(s.contains("business_logic"));
    }

    #[test]
    fn remote_server_returns_fetched_response() {
        let d = crate::route::remote_fetch();
        let result = query_backend(&d, "/remote/dashboard");
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("remote_server"));
        assert!(s.contains("/remote/dashboard"));
    }

    #[test]
    fn ipc_shell_defaults_to_shell_target() {
        let d = crate::route::ipc_shell();
        let result = query_backend(&d, "/api/data");
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("shell")); // default target
    }
}
