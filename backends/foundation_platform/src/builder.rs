use tauri::{App, Context, Manager, Runtime};
use foundation_ui_traits::*;

use crate::ewe;
use crate::responder::WebviewApp;
use crate::session::PlatformSession;

struct RouteEntry {
    pattern: String,
    decision: foundation_ui_traits::RouteDecision,
}

type SetupCallback = Box<dyn Fn(&PlatformSession) + Send + Sync + 'static>;

const PLATFORM_SCHEME_INTERCEPTOR_JS: &str =
    foundation_wasm_ui::embedded::PLATFORM_SCHEME_INTERCEPTOR_JS;

pub struct PlatformBuilder<R: Runtime = tauri::Wry> {
    inner: tauri::Builder<R>,
    routes: Vec<RouteEntry>,
    setups: Vec<SetupCallback>,
}

impl<R: Runtime> PlatformBuilder<R> {
    #[must_use] pub fn new() -> Self { Self { inner: tauri::Builder::new(), routes: Vec::new(), setups: Vec::new() } }

    #[must_use]
    pub fn route(mut self, pattern: &str, decision: foundation_ui_traits::RouteDecision) -> Self {
        self.routes.push(RouteEntry { pattern: pattern.to_string(), decision }); self
    }

    #[must_use]
    pub fn setup<F: Fn(&PlatformSession) + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.setups.push(Box::new(f)); self
    }

    /// Build the Tauri App. Session is created, routes registered, ewe://
    /// protocol registered. The initial WebView URL is set to
    /// `http://ewe.localhost/__platform__/index.html` so scripts load
    /// with proper Content-Type (WebViewAssetLoader strips them).
    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        let routes = std::mem::take(&mut self.routes);
        let setups = std::mem::take(&mut self.setups);
        let interceptor = PLATFORM_SCHEME_INTERCEPTOR_JS;

        self.inner = self.inner.setup(move |app| {
            let session = PlatformSession::new();
            let handle = app.handle().clone();

            // Route registration — each WebviewApp route gets a file-serving responder.
            // IpcShell and RemoteServer routes expect the user to register their own
            // responder via session.register_responder() in .setup().
            let public_dir = std::path::PathBuf::from(
                std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()
            ).join("public");

            for entry in &routes {
                let pattern = entry.pattern.clone();
                let mut d = entry.decision.clone();

                if matches!(d.source, RouteSource::WebviewApp) {
                    let id = format!("handler_{}", entry.pattern);
                    let responder: Box<dyn crate::route_handler::RouteResponder> =
                        Box::new(WebviewApp::new(public_dir.clone()));
                    d.handler_id = Some(id.clone());
                    session.register_responder(&id, responder);
                }
                // IpcShell/RemoteServer: handler_id already set by user via .with_handler()
                // User registers responder in .setup() callback.

                session.route(&pattern, d);
            }
            for setup in &setups { setup(&session); }

            app.manage(session);

            // Inject platform scheme interceptor
            if let Some(window) = app.get_webview_window("main") { let _ = window.eval(interceptor); }
            Ok(())
        });

        self.inner = ewe::register_ewe_protocol(self.inner);
        self.inner.build(context)
    }

    #[must_use]
    pub fn invoke_handler<H: Send + Sync + 'static>(mut self, handler: H) -> Self
    where H: Fn(tauri::ipc::Invoke<R>) -> bool,
    { self.inner = self.inner.invoke_handler(handler); self }

    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> { &mut self.inner }
}

impl<R: Runtime> Default for PlatformBuilder<R> { fn default() -> Self { Self::new() } }
