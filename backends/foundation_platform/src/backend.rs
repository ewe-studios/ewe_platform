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

/// A test transport that returns controlled responses per mode.
pub struct TestTransport {
    wasm_response: Vec<u8>,
    ipc_response: Vec<u8>,
    remote_response: Vec<u8>,
    pub ipc_calls: std::sync::Mutex<Vec<(Option<String>, String)>>,
    pub remote_calls: std::sync::Mutex<Vec<String>>,
}

impl TestTransport {
    pub fn new() -> Self {
        Self {
            wasm_response: b"wasm-rendered".to_vec(),
            ipc_response: b"ipc-result".to_vec(),
            remote_response: b"remote-fetched".to_vec(),
            ipc_calls: std::sync::Mutex::new(Vec::new()),
            remote_calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn with_remote(mut self, response: &[u8]) -> Self {
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

// Tests moved to tests/backend_suite.rs
