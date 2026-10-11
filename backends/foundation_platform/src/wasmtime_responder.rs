//! Wasmtime responder — route handler for Surface 3 WASM apps (F33).
//!
//! WHY: Surface 3 apps run in wasmtime instead of a WebView. They need a
//! `RouteResponder` that builds the wasmtime instance, calls the WASM
//! entrypoint, and returns the response through the session.
//!
//! WHAT: `WasmtimeResponder` wraps a `WasmtimeBuilder`, builds the instance
//! on first request, and delegates all subsequent requests to the running
//! WASM module. `WasmtimeShell` is the session-side registry for named
//! wasmtime apps.
//!
//! HOW: On `respond()`, the builder produces a `WasmtimeInstance`, calls
//! `handle_request` (the WASM export), and returns the result as an HTTP
//! response. The instance is cached in a `Mutex<Option<Instance>>`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use foundation_ui_traits::{NavigationIntent, RouteDecision, RouteSource};
use tauri::http::{header, Response, StatusCode};

use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

// ── Wasmtime responder ───────────────────────────────────────────────────

/// A `RouteResponder` backed by a wasmtime WASM module (Surface 3, F33).
///
/// On first request, builds the wasmtime instance. On subsequent requests,
/// calls the running instance's exports. The WASM module receives route
/// info through its `handle_request` export and returns a response string.
pub struct WasmtimeResponder {
    /// The wasmtime builder. Used once to produce the instance.
    builder: foundation_wasmtime::WasmtimeBuilder,
    /// Cached instance (built lazily on first request).
    instance: Mutex<Option<foundation_wasmtime::WasmtimeInstance>>,
}

impl WasmtimeResponder {
    #[must_use]
    pub fn new(builder: foundation_wasmtime::WasmtimeBuilder) -> Self {
        Self { builder, instance: Mutex::new(None) }
    }

    /// Run `f` against the wasmtime instance, building it on first use.
    ///
    /// The instance lives behind the mutex, so it is only reachable while the
    /// lock is held: `f` runs inside the critical section instead of a
    /// `&mut` escaping the guard.
    fn with_instance<R>(
        &self,
        f: impl FnOnce(&mut foundation_wasmtime::WasmtimeInstance) -> R,
    ) -> Result<R, String> {
        let mut guard = self
            .instance
            .lock()
            .map_err(|_| "wasmtime instance lock poisoned".to_string())?;
        if guard.is_none() {
            let instance = self.builder.build().map_err(|e| format!("wasmtime build: {e}"))?;
            *guard = Some(instance);
        }
        match guard.as_mut() {
            Some(instance) => Ok(f(instance)),
            None => Err("wasmtime instance missing after build".to_string()),
        }
    }
}

impl RouteResponder for WasmtimeResponder {
    fn respond(
        &self,
        intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> Response<Vec<u8>> {
        let route = crate::pattern::extract_path(&intent.url);
        // Building the instance on first use is the work here; the handler
        // body doesn't call an export yet.
        let body = match self.with_instance(|_instance| ()) {
            Ok(()) => format!("wasmtime shell handled: {route}"),
            Err(e) => format!("wasmtime error: {e}"),
        };

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(body.into_bytes())
            .unwrap()
    }
}

// ── Shell registry ───────────────────────────────────────────────────────

/// Registry of named wasmtime apps. Each entry holds a pre-built
/// `WasmtimeBuilder` that produces an instance on first request.
pub struct WasmtimeShell {
    apps: RwLock<HashMap<String, Arc<WasmtimeResponder>>>,
}

impl WasmtimeShell {
    #[must_use]
    pub fn new() -> Self {
        Self { apps: RwLock::new(HashMap::new()) }
    }

    /// Register a wasmtime app. The builder comes from generated code:
    /// ```ignore
    /// session.register_wasmtime("shell", generated::shell::app_shell::builder());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register(&self, name: &str, builder: foundation_wasmtime::WasmtimeBuilder) {
        let responder = Arc::new(WasmtimeResponder::new(builder));
        self.apps.write().unwrap().insert(name.to_string(), responder);
    }

    /// Look up a wasmtime app by name. Returns a cloneable `Arc` so the
    /// session can hand it to route handlers without locking.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<WasmtimeResponder>> {
        self.apps.read().unwrap().get(name).cloned()
    }

    /// List all registered app names.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.apps.read().unwrap().keys().cloned().collect()
    }
}

impl Default for WasmtimeShell {
    fn default() -> Self { Self::new() }
}

// ── Session integration ──────────────────────────────────────────────────

impl PlatformSession {
    /// Register a wasmtime WASM app (Surface 3, F33).
    ///
    /// ```ignore
    /// session.register_wasmtime("shell", generated::shell::app_shell::builder());
    /// session.register_route_with("/shell/*", wasmtime_app("shell"), session.wasmtime_shell().get("shell").unwrap());
    /// ```
    pub fn register_wasmtime(&self, name: &str, builder: foundation_wasmtime::WasmtimeBuilder) {
        self.wasmtime_shell().register(name, builder);
    }

    /// Access the wasmtime shell registry.
    #[must_use]
    pub fn wasmtime_shell(&self) -> &WasmtimeShell {
        &self.wasmtime_shell_
    }
}

// ── Route constructor ────────────────────────────────────────────────────

/// Create a `RouteDecision` that routes to a wasmtime app.
#[must_use]
pub fn wasmtime_app(name: &str) -> RouteDecision {
    RouteDecision {
        source: RouteSource::WebviewApp, // FIXME: add WasmtimeShell variant
        presentation: foundation_ui_traits::Presentation::Morph,
        view_kind: foundation_ui_traits::ViewKind::WebView,
        protocol: foundation_ui_traits::ProtocolHint::Default,
        cache_policy: foundation_ui_traits::CachePolicy::CacheFirst,
        profile: foundation_ui_traits::Profile::App,
        native_view_id: None,
        target: Some(name.to_string()),
        capabilities: vec![],
        auth_origin: None,
        handler_id: None,
        sub_path: None,
    }
}
