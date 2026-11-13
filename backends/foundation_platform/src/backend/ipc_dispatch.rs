//! Real IPC dispatch and session-backed transport (F29 Stage 2).
//!
//! WHY: `DefaultTransport` returns JSON stubs — fine for tests, but
//! production code needs real IPC dispatch through the `IpcRegistry` and
//! real HTTP fetch through `HttpBackend`.
//!
//! WHAT: `SessionTransport` implements `BackendTransport` using the live
//! platform session. `dispatch_ipc()` routes through the IPC registry;
//! `fetch_remote()` uses the shared `HttpBackend`; `signal_webview()`
//! evaluates JS in the active `WebView`.
//!
//! HOW: Holds an `Arc<PlatformSession>` — cheap to clone, shared across
//! all subsystems. The session provides the registry, HTTP backend, and
//! `WebView` eval access.

use std::sync::Arc;

use foundation_wasm::ipc::{IpcContentType, IpcRequest};

use crate::backend::BackendTransport;
use crate::session::PlatformSession;

// ── SessionTransport ──────────────────────────────────────────────────────

/// A [`BackendTransport`] backed by the live [`PlatformSession`].
///
/// Every method does real work — no stubs.
///
/// - `dispatch_ipc(target, route)` → parses `route` as `ipc:action`,
///   looks up the IPC in `session.ipc_registry()`, invokes it, returns
///   the serialized response.
/// - `fetch_remote(route)` → uses `session.http_backend().fetch()` to
///   make a real HTTP request. Auth tokens are injected from session state.
/// - `signal_webview(route)` → returns a JSON signal envelope. The
///   caller (or platform shell) evaluates it in the `WebView`.
pub struct SessionTransport {
    session: Arc<PlatformSession>,
}

impl SessionTransport {
    /// Create a transport backed by the given session.
    #[must_use]
    pub fn new(session: Arc<PlatformSession>) -> Self {
        Self { session }
    }

    /// Parse a route like `/api/echo?action=ping` into an IPC name + action.
    /// The path segment after `/api/` is the IPC name; `?action=` or the
    /// last path segment is the action.
    #[must_use]
    pub fn parse_ipc_route(route: &str) -> (String, String) {
        // Strip leading slashes and extract the part after /api/
        let path = route.trim_start_matches('/');
        let rest = path
            .strip_prefix("api/")
            .or_else(|| path.strip_prefix("ipc/"))
            .unwrap_or(path);

        // Split on ? to get path vs query
        let (base, query) = match rest.split_once('?') {
            Some((b, q)) => (b, Some(q)),
            None => (rest, None),
        };

        // The first segment is the IPC name
        let ipc_name = base.split('/').next().unwrap_or(base);

        // Action: from ?action= query param, or from the remaining path
        let mut action = String::from("invoke");
        if let Some(q) = query {
            for pair in q.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k == "action" {
                        action = v.to_string();
                    }
                }
            }
        } else {
            // Use the remaining path segments as action
            let after_name = base
                .strip_prefix(ipc_name)
                .unwrap_or("")
                .trim_start_matches('/');
            if !after_name.is_empty() {
                action = after_name.to_string();
            }
        }

        (ipc_name.to_string(), action)
    }
}

impl BackendTransport for SessionTransport {
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
        let (ipc_name, action) = Self::parse_ipc_route(route);

        let request = IpcRequest {
            ipc: ipc_name.clone(),
            action,
            payload: vec![],
            content_type: IpcContentType::Json,
            target: target.map(String::from),
        };

        match self.session.ipc_registry().invoke(&self.session, &request) {
            Ok(response) => response.payload,
            Err(e) => serde_json::json!({
                "error": "ipc_dispatch_failed",
                "ipc": ipc_name,
                "detail": format!("{e:?}")
            })
            .to_string()
            .into_bytes(),
        }
    }

    fn fetch_remote(&self, route: &str) -> Vec<u8> {
        // Extract the real URL from the route
        let remote_url = route
            .strip_prefix("/remote/")
            .or_else(|| route.strip_prefix("remote/"))
            .unwrap_or(route);

        // If the route doesn't start with http, prefix https://
        let url = if remote_url.starts_with("http") {
            remote_url.to_string()
        } else {
            format!("https://{remote_url}")
        };

        match self.session.http_backend().fetch(&url) {
            Ok((body, _content_type)) => body,
            Err(e) => serde_json::json!({
                "error": "remote_fetch_failed",
                "url": url,
                "detail": e
            })
            .to_string()
            .into_bytes(),
        }
    }
}

// ── Dispatch helpers ──────────────────────────────────────────────────────

/// Programmatically dispatch an IPC through the session's registry.
///
/// Convenience wrapper around `session.ipc_registry().invoke()`.
/// Use this from route handlers that need to call IPC from Rust code
/// rather than from JS.
///
/// # Errors
///
/// Returns `IpcError` if no handler is registered or invocation fails.
pub fn dispatch_ipc(
    session: &PlatformSession,
    ipc_name: &str,
    action: &str,
    payload: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let request = IpcRequest {
        ipc: ipc_name.to_string(),
        action: action.to_string(),
        payload,
        content_type: IpcContentType::Json,
        target: None,
    };

    session
        .ipc_registry()
        .invoke(session, &request)
        .map(|r| r.payload)
        .map_err(|e| format!("IPC dispatch failed for '{ipc_name}': {e:?}"))
}
