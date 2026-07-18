//! PlatformBuilder — wraps `tauri::Builder<R>`, exposes foundation_platform API.
//!
//! Default type parameter `R = Wry` (desktop). Android/iOS override via turbofish.
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

type SetupCallback = Box<dyn Fn(&PlatformSession) + Send + Sync + 'static>;

/// Wraps `tauri::Builder<R>`. Default `R` = `tauri::Wry` (desktop).
/// Generic over `R: Runtime` so mobile builds work too.
pub struct PlatformBuilder<R: Runtime = tauri::Wry> {
    inner: tauri::Builder<R>,
    routes: Vec<RouteEntry>,
    setups: Vec<SetupCallback>,
}

impl<R: Runtime> PlatformBuilder<R> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: tauri::Builder::new(),
            routes: Vec::new(),
            setups: Vec::new(),
        }
    }

    /// Register a route pattern. Stored until `build()`, then wired into
    /// the PlatformSession during Tauri's setup hook.
    #[must_use]
    pub fn route(mut self, pattern: &str, decision: foundation_ui_traits::RouteDecision) -> Self {
        self.routes.push(RouteEntry {
            pattern: pattern.to_string(),
            decision,
        });
        self
    }

    /// Register a user setup callback. Called AFTER the session is created
    /// and routes are registered. Multiple calls chain — each callback runs
    /// in order.
    #[must_use]
    pub fn setup<F>(mut self, f: F) -> Self
    where
        F: Fn(&PlatformSession) + Send + Sync + 'static,
    {
        self.setups.push(Box::new(f));
        self
    }

    /// Consume the builder and produce a Tauri `App`.
    ///
    /// This is the **single place** the session is created, routes are
    /// registered, setup callbacks run, and `app.manage(session)` is called.
    /// ewe:// protocol is always registered.
    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        let routes = std::mem::take(&mut self.routes);
        let setups = std::mem::take(&mut self.setups);

        self.inner = self.inner.setup(move |app| {
            let session = PlatformSession::new();

            // Register every route declared via .route()
            for entry in &routes {
                session.route(&entry.pattern, entry.decision.clone());
            }

            // Call every .setup() callback in registration order
            for setup in &setups {
                setup(&session);
            }

            app.manage(session);
            Ok(())
        });

        self.inner = ewe::register_ewe_protocol(self.inner);
        self.inner.build(context)
    }

    /// Access the inner `tauri::Builder` for advanced configuration
    /// (plugins, custom Tauri setup, etc.).
    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> {
        &mut self.inner
    }
}

impl<R: Runtime> Default for PlatformBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
