use foundation_ui_traits::*;
use tauri::{App, Context, Manager, Runtime};

use crate::ewe;
use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

struct RouteEntry {
    pattern: String,
    decision: foundation_ui_traits::RouteDecision,
    responder: Option<Box<dyn RouteResponder>>,
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
    pub fn new() -> Self {
        Self { inner: tauri::Builder::new(), routes: Vec::new(), setups: Vec::new(), index_url: None }
    }

    pub fn route(mut self, pattern: &str, decision: foundation_ui_traits::RouteDecision) -> Self {
        self.routes.push(RouteEntry { pattern: pattern.to_string(), decision, responder: None });
        self
    }

    /// Register a route WITH its responder — all in one declarative call.
    pub fn route_with(
        mut self,
        pattern: &str,
        decision: foundation_ui_traits::RouteDecision,
        responder: impl RouteResponder,
    ) -> Self {
        self.routes.push(RouteEntry {
            pattern: pattern.to_string(),
            decision,
            responder: Some(Box::new(responder)),
        });
        self
    }

    pub fn setup<F: Fn(&PlatformSession) + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.setups.push(Box::new(f));
        self
    }

    pub fn index_as(mut self, route: &str) -> Self {
        self.index_url = Some(route.to_string());
        self
    }

    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        let mut routes = std::mem::take(&mut self.routes);
        let setups = std::mem::take(&mut self.setups);
        let interceptor = PLATFORM_SCHEME_INTERCEPTOR_JS;
        let index_url = self.index_url.clone();

        self.inner = self.inner.setup(move |app| {
            let session = PlatformSession::new();

            // Register routes + their inline responders.
            // For patterns like /app/*, also register /app and /app/
            // so the initial URL ewe://localhost/app matches.
            for entry in routes.drain(..) {
                let mut d = entry.decision.clone();
                let handler_id = entry.responder.map(|r| {
                    let rid = format!("__route_{}", entry.pattern);
                    session.register_responder(&rid, r);
                    rid
                });
                d.handler_id = handler_id;
                session.route(&entry.pattern, d.clone());

                // Also register prefix-only routes for wildcard patterns
                if entry.pattern.ends_with("/*") {
                    let prefix = entry.pattern.trim_end_matches('*');
                    session.route(prefix.trim_end_matches('/'), d.clone());
                }
            }

            for setup in &setups { setup(&session); }

            app.manage(session);

            // Inject platform scheme interceptor. The initial URL is set
            // via tauri.conf.json (patched by codegen), so this is for
            // subsequent link-click handling only.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(interceptor);
            }
            Ok(())
        });

        self.inner = ewe::register_ewe_protocol(self.inner);
        self.inner.build(context)
    }

    pub fn invoke_handler<H: Send + Sync + 'static>(mut self, handler: H) -> Self
    where H: Fn(tauri::ipc::Invoke<R>) -> bool,
    { self.inner = self.inner.invoke_handler(handler); self }

    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> { &mut self.inner }
}

impl<R: Runtime> Default for PlatformBuilder<R> { fn default() -> Self { Self::new() } }
