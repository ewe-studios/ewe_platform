//! PlatformBuilder — wraps `tauri::Builder<R>`, exposes foundation_platform API.
//!
//! Default type parameter `R = tauri_runtime_wry::Wry<tauri::EventLoopMessage>`
//! so desktop builds work without explicit annotation. Android/iOS override
//! via `PlatformBuilder::<MyRuntime>::new()`.
//!
//! ```ignore
//! platform_run!(PlatformBuilder::new()
//!     .route("/app/*", webview_app())
//!     .route("/remote/*", remote_fetch()));
//! ```

use tauri::{App, Context, Manager, Runtime};

use crate::ewe;
use crate::session::PlatformSession;

struct RouteEntry {
    pattern: String,
    decision: foundation_ui_traits::RouteDecision,
}

/// Wraps `tauri::Builder<R>`. Default `R` = `tauri::Wry` (desktop).
/// Generic over `R: Runtime` so mobile builds work too.
pub struct PlatformBuilder<R: Runtime = tauri::Wry> {
    inner: tauri::Builder<R>,
    routes: Vec<RouteEntry>,
}

impl<R: Runtime> PlatformBuilder<R> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: tauri::Builder::new(),
            routes: Vec::new(),
        }
    }

    /// Register a route pattern. Stored until `build()`, then wired into
    /// the PlatformSession during Tauri's setup hook.
    pub fn route(mut self, pattern: &str, decision: foundation_ui_traits::RouteDecision) -> Self {
        self.routes.push(RouteEntry {
            pattern: pattern.to_string(),
            decision,
        });
        self
    }

    /// Set a user setup hook. Called AFTER the session is created and
    /// routes are registered.
    pub fn setup<F>(mut self, f: F) -> Self
    where
        F: Fn(&PlatformSession) + Send + Sync + 'static,
    {
        let routes = std::mem::take(&mut self.routes);
        self.inner = self.inner.setup(move |app| {
            let session = PlatformSession::new();
            for entry in &routes {
                session.route(&entry.pattern, entry.decision.clone());
            }
            f(&session);
            app.manage(session);
            Ok(())
        });
        self
    }

    /// Consume the builder and produce a Tauri `App`.
    ///
    /// Routes wired automatically. ewe:// protocol always registered.
    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        if self.routes.is_empty() {
            self.inner = self.inner.setup(|app| {
                app.manage(PlatformSession::new());
                Ok(())
            });
        } else {
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
        self.inner = ewe::register_ewe_protocol(self.inner);
        self.inner.build(context)
    }

    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> {
        &mut self.inner
    }
}

impl<R: Runtime> Default for PlatformBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
