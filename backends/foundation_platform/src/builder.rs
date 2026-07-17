//! PlatformBuilder — wraps `tauri::Builder`, exposes foundation_platform API.
//!
//! User code never touches Tauri directly. Routes and capabilities registered
//! on the builder are auto-wired into the session at startup.
//!
//! ```ignore
//! PlatformBuilder::new()
//!     .route("/app/*", webview_app())
//!     .route("/remote/*", remote_fetch())
//!     .build(tauri::generate_context!())
//!     .unwrap()
//!     .run(|_handle, event| { ... });
//! ```

use std::sync::Arc;
use tauri::{App, Context};

use crate::ewe;
use crate::session::PlatformSession;

type R = tauri_runtime_wry::Wry<tauri::EventLoopMessage>;

/// Route registration stored before the session is created.
struct RouteEntry {
    pattern: String,
    decision: foundation_ui_traits::RouteDecision,
}

/// Wraps `tauri::Builder`. Routes registered here are auto-wired into
/// the PlatformSession at startup.
pub struct PlatformBuilder {
    inner: tauri::Builder<R>,
    routes: Vec<RouteEntry>,
}

impl PlatformBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: tauri::Builder::new(),
            routes: Vec::new(),
        }
    }

    /// Register a route pattern. Stored until build(), then wired into
    /// the PlatformSession during Tauri's setup hook.
    pub fn route(mut self, pattern: &str, decision: foundation_ui_traits::RouteDecision) -> Self {
        self.routes.push(RouteEntry {
            pattern: pattern.to_string(),
            decision,
        });
        self
    }

    /// Set a custom Tauri setup hook. Called AFTER the session is created
    /// and routes are registered. Use for capabilities, custom commands, etc.
    pub fn setup<F>(mut self, f: F) -> Self
    where
        F: Fn(&PlatformSession) -> Result<(), Box<dyn std::error::Error>> + Send + Sync + 'static,
    {
        let routes = std::mem::take(&mut self.routes);
        self.inner = self.inner.setup(move |app| {
            // 1. Create session
            let session = PlatformSession::new();

            // 2. Register routes
            for entry in &routes {
                session.route(&entry.pattern, entry.decision.clone());
            }

            // 3. Run user setup
            f(&session)?;

            // 4. Store session in Tauri state
            app.manage(session);
            Ok(())
        });
        self
    }

    /// Consume the builder and produce a Tauri `App`.
    /// If no explicit setup() was called, creates a default setup that
    /// just registers the routes.
    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        // If user didn't call setup(), wire routes into a default setup
        if self.routes.is_empty() {
            // No routes, no setup — just build
            self.inner = self.inner.setup(|app| {
                let session = PlatformSession::new();
                app.manage(session);
                Ok(())
            });
        } else {
            // Routes registered without explicit setup — wire them automatically
            let routes = std::mem::take(&mut self.routes);
            self.inner = self.inner.setup(move |app| {
                let session = PlatformSession::new();
                for entry in &routes {
                    session.route(&entry.pattern, entry.decision.clone());
                }
                app.manage(session);
                Ok(())
            });
        }

        // Register ewe:// custom protocol
        self.inner = ewe::register_ewe_protocol(self.inner);

        self.inner.build(context)
    }

    /// Access the inner `tauri::Builder` for advanced customization.
    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> {
        &mut self.inner
    }
}

impl Default for PlatformBuilder {
    fn default() -> Self {
        Self::new()
    }
}
