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

            // Register routes + their inline responders
            for entry in routes.drain(..) {
                let mut d = entry.decision;
                if let Some(responder) = entry.responder {
                    let id = format!("__route_{}", entry.pattern);
                    d.handler_id = Some(id.clone());
                    session.register_responder(&id, responder);
                }
                session.route(&entry.pattern, d);
            }

            for setup in &setups { setup(&session); }

            app.manage(session);

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(interceptor);
                let initial = index_url.as_deref().unwrap_or("/__platform__/");
                let _ = window.eval(&format!(
                    "location.replace('http://ewe.localhost{}')",
                    initial.trim_end_matches('/')
                ));
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
