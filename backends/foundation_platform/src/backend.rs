//! Backend query (Step 5 of execution contract).
//!
//! Resolves `RouteSource` to actual content bytes. Each source variant
//! has a concrete execution path through the [`BackendTransport`] trait.
//! The session dispatches here after the handler chain resolves and
//! cache check passes.
//!
//! - WebviewApp: signal in-WebView WASM code via transport
//! - IpcShell: dispatch to native shell via Tauri command IPC
//! - RemoteServer: fetch from remote via HTTP

use foundation_ui_traits::*;

// ── Backend transport trait ─────────────────────────────────────────────

/// Pluggable backend dispatch. One method per [`RouteSource`].
///
/// Tests inject [`TestTransport`]; the real Tauri app injects an
/// `AppHandle`-backed transport. The session delegates to whichever
/// transport is registered.
pub trait BackendTransport: Send + Sync + 'static {
    /// Signal the WASM app running inside the WebView. The platform
    /// returns a JSON signal envelope that the WebView's bootstrap JS
    /// dispatches to the WASM app's route handler.
    fn signal_webview(&self, route: &str) -> Vec<u8>;

    /// Dispatch to the native shell via Tauri command IPC.
    /// `target` names the wasm_app instance; `None` means the root shell.
    fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8>;

    /// Fetch content from a remote server over HTTP.
    /// Auth tokens are attached by the shell — they never enter the WebView.
    fn fetch_remote(&self, route: &str) -> Vec<u8>;
}

/// Default transport — returns signal JSON stubs suitable for testing
/// and for apps that haven't registered a real transport yet.
///
/// In production, the Tauri setup hook injects an `AppHandle`-backed
/// transport that does real IPC dispatch and HTTP fetch.
#[derive(Debug, Clone, Copy)]
pub struct DefaultTransport;

impl BackendTransport for DefaultTransport {
    fn signal_webview(&self, route: &str) -> Vec<u8> {
        serde_json::json!({
            "type": "webview_app_signal",
            "route": route,
            "action": "render_in_webview"
        })
        .to_string()
        .into_bytes()
    }

    fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8> {
        let target = target.unwrap_or("shell");
        serde_json::json!({
            "type": "ipc_shell_dispatch",
            "target": target,
            "route": route,
            "action": "invoke_native_command"
        })
        .to_string()
        .into_bytes()
    }

    fn fetch_remote(&self, route: &str) -> Vec<u8> {
        serde_json::json!({
            "type": "remote_server_fetch",
            "route": route,
            "action": "http_fetch"
        })
        .to_string()
        .into_bytes()
    }
}

// ── Backend-aware query ─────────────────────────────────────────────────

/// Resolve a RouteSource to content bytes through the registered transport.
/// Called after cache check determines we need fresh content.
pub fn query_backend(
    transport: &dyn BackendTransport,
    decision: &RouteDecision,
    route: &str,
) -> Vec<u8> {
    match decision.source {
        RouteSource::WebviewApp => transport.signal_webview(route),
        RouteSource::IpcShell => transport.dispatch_ipc(decision.target.as_deref(), route),
        RouteSource::RemoteServer => transport.fetch_remote(route),
    }
}

/// Fallback transport used when session has no custom transport registered.
pub static DEFAULT_TRANSPORT: DefaultTransport = DefaultTransport;

// ── Closure-based transport ──────────────────────────────────────────────

/// A [`BackendTransport`] built from closures. The Tauri setup hook
/// constructs this with closures that capture the `AppHandle` for real
/// IPC dispatch and HTTP fetch.
pub struct ClosureTransport {
    wasm_fn: Box<dyn Fn(&str) -> Vec<u8> + Send + Sync>,
    ipc_fn: Box<dyn Fn(Option<&str>, &str) -> Vec<u8> + Send + Sync>,
    remote_fn: Box<dyn Fn(&str) -> Vec<u8> + Send + Sync>,
}

impl ClosureTransport {
    pub fn new(
        wasm: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static,
        ipc: impl Fn(Option<&str>, &str) -> Vec<u8> + Send + Sync + 'static,
        remote: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static,
    ) -> Self {
        Self {
            wasm_fn: Box::new(wasm),
            ipc_fn: Box::new(ipc),
            remote_fn: Box::new(remote),
        }
    }
}

impl BackendTransport for ClosureTransport {
    fn signal_webview(&self, route: &str) -> Vec<u8> {
        (self.wasm_fn)(route)
    }

    fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8> {
        (self.ipc_fn)(target, route)
    }

    fn fetch_remote(&self, route: &str) -> Vec<u8> {
        (self.remote_fn)(route)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::webview_app;

    // A test transport that returns controlled responses per mode.
    struct TestTransport {
        wasm_response: Vec<u8>,
        ipc_response: Vec<u8>,
        remote_response: Vec<u8>,
        ipc_calls: std::sync::Mutex<Vec<(Option<String>, String)>>,
        remote_calls: std::sync::Mutex<Vec<String>>,
    }

    impl TestTransport {
        fn new() -> Self {
            Self {
                wasm_response: b"wasm-rendered".to_vec(),
                ipc_response: b"ipc-result".to_vec(),
                remote_response: b"remote-fetched".to_vec(),
                ipc_calls: std::sync::Mutex::new(Vec::new()),
                remote_calls: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn with_remote(mut self, response: &[u8]) -> Self {
            self.remote_response = response.to_vec();
            self
        }
    }

    impl BackendTransport for TestTransport {
        fn signal_webview(&self, route: &str) -> Vec<u8> {
            format!("wasm:{}", route).into_bytes()
        }

        fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8> {
            self.ipc_calls
                .lock()
                .unwrap()
                .push((target.map(String::from), route.to_string()));
            self.ipc_response.clone()
        }

        fn fetch_remote(&self, route: &str) -> Vec<u8> {
            self.remote_calls.lock().unwrap().push(route.to_string());
            self.remote_response.clone()
        }
    }

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

        // WebviewApp → signal_webview
        let d = webview_app();
        let content = query_backend(&transport, &d, "/app/home");
        assert_eq!(content, b"wasm:/app/home");

        // IpcShell → dispatch_ipc
        let d = crate::route::ipc_shell_with("business_logic");
        let content = query_backend(&transport, &d, "/api/data");
        assert_eq!(content, b"ipc-result");
        let calls = transport.ipc_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0.as_deref(), Some("business_logic"));
        assert_eq!(calls[0].1, "/api/data");

        // RemoteServer → fetch_remote
        let d = crate::route::remote_fetch();
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
        let d = crate::route::ipc_shell_with("business_logic");
        let result = query_backend(&DefaultTransport, &d, "/api/data");
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("ipc_shell_dispatch"));
        assert!(s.contains("business_logic"));
    }

    #[test]
    fn remote_server_returns_fetched_response() {
        let d = crate::route::remote_fetch();
        let result = query_backend(&DefaultTransport, &d, "/remote/dashboard");
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("remote_server_fetch"));
        assert!(s.contains("/remote/dashboard"));
    }

    #[test]
    fn ipc_shell_defaults_to_shell_target() {
        let d = crate::route::ipc_shell();
        let result = query_backend(&DefaultTransport, &d, "/api/data");
        let s = String::from_utf8(result).unwrap();
        assert!(s.contains("shell")); // default target
    }
}
