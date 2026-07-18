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
    index_url: Option<String>,
}

impl<R: Runtime> PlatformBuilder<R> {
    #[must_use] pub fn new() -> Self { Self { inner: tauri::Builder::new(), routes: Vec::new(), setups: Vec::new(), index_url: None } }

    #[must_use]
    pub fn route(mut self, pattern: &str, decision: foundation_ui_traits::RouteDecision) -> Self {
        self.routes.push(RouteEntry { pattern: pattern.to_string(), decision }); self
    }

    #[must_use]
    pub fn setup<F: Fn(&PlatformSession) + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.setups.push(Box::new(f)); self
    }

    /// Set the initial page URL for the main WebView window.
    /// e.g. `.index_as("/app/")` → loads `http://ewe.localhost/app/`.
    /// Serves through ewe_handler so `document.baseURI` is properly set
    /// (WebViewAssetLoader sets it to `about:blank`, breaking ES module imports).
    #[must_use]
    pub fn index_as(mut self, route: &str) -> Self {
        self.index_url = Some(route.to_string()); self
    }

    /// Build the Tauri App. Session is created, routes registered, ewe://
    /// protocol registered. The initial WebView URL is served through
    /// ewe_handler (bypasses WebViewAssetLoader which strips scripts).
    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        let routes = std::mem::take(&mut self.routes);
        let setups = std::mem::take(&mut self.setups);
        let interceptor = PLATFORM_SCHEME_INTERCEPTOR_JS;
        let index_url = self.index_url.clone();

        self.inner = self.inner.setup(move |app| {
            let session = PlatformSession::new();
            let handle = app.handle().clone();

            // Route registration — each WebviewApp route gets a file-serving responder
            // from its own public/{app-name}/ subdirectory.
            let base_public = std::path::PathBuf::from(
                std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()
            ).join("public");

            for entry in &routes {
                let pattern = entry.pattern.clone();
                let mut d = entry.decision.clone();

                if matches!(d.source, RouteSource::WebviewApp) {
                    // Derive subdirectory from the route prefix: "/app/dashboard/*" → "app-dashboard"
                    let app_name = pattern.trim_matches('/').replace("/", "-").trim_end_matches("-*").to_string();
                    let id = format!("handler_{}", pattern);
                    let asset_dir = base_public.join(&app_name);
                    // Fall back to base public/ if no per-app subdirectory exists
                    let dir = if asset_dir.exists() { asset_dir } else { base_public.clone() };
                    let responder: Box<dyn crate::route_handler::RouteResponder> =
                        Box::new(WebviewApp::new(dir));
                    d.handler_id = Some(id.clone());
                    session.register_responder(&id, responder);
                }

                session.route(&pattern, d);
            }
            for setup in &setups { setup(&session); }

            app.manage(session);

            // Inject platform scheme interceptor + navigate to initial URL
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(interceptor);
                let initial = index_url.as_deref().unwrap_or("/__platform__/");
                let redirect_js = format!(
                    "location.replace('http://ewe.localhost{}')",
                    initial.trim_end_matches('/')
                );
                let _ = window.eval(&redirect_js);
            }
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
