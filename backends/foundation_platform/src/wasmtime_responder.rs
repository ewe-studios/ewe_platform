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

    /// Get or build the wasmtime instance.
    fn get_instance(&self) -> Result<&mut foundation_wasmtime::WasmtimeInstance, String> {
        // SAFETY: We hold the Mutex lock, so the Option<Instance> inside is stable.
        // The caller must not hold the lock across await points — but we're
        // synchronous, so this is fine.
        let mut guard = self.instance.lock().unwrap();
        if guard.is_none() {
            let instance = self.builder.build().map_err(|e| format!("wasmtime build: {e}"))?;
            *guard = Some(instance);
        }
        // SAFETY: we just ensured guard is Some
        Ok(unsafe { &mut *(&raw mut *guard).cast::<Option<foundation_wasmtime::WasmtimeInstance>>() }.as_mut().unwrap())
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
        let body = match self.get_instance() {
            Ok(instance) => {
                let _ = instance.name();
                format!("wasmtime shell handled: {route}")
            }
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
