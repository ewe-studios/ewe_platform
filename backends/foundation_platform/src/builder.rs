use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use foundation_wasm::{CapabilityContentType, CapabilityRequest};
use tauri::{App, Context, Manager, Runtime};

use crate::ewe;
use crate::injector::{InjectedScript, ScriptInjector, ScriptInjectorPlugin};
use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

struct RouteEntry {
    pattern: String,
    decision: foundation_ui_traits::RouteDecision,
    responder: Option<Box<dyn RouteResponder>>,
}

type SetupCallback = Box<dyn Fn(&PlatformSession) + Send + Sync + 'static>;

pub struct PlatformBuilder<R: Runtime = tauri::Wry> {
    inner: tauri::Builder<R>,
    routes: Vec<RouteEntry>,
    setups: Vec<SetupCallback>,
    index_url: Option<String>,
    /// Scripts to inject into every webview (F24).
    script_injector: ScriptInjector,
    /// Whether platform runtimes have been auto-registered.
    runtimes_injected: bool,
}

impl<R: Runtime> PlatformBuilder<R> {
    pub fn new() -> Self {
        Self {
            inner: tauri::Builder::new(),
            routes: Vec::new(),
            setups: Vec::new(),
            index_url: None,
            script_injector: ScriptInjector::new(PathBuf::new()),
            runtimes_injected: false,
        }
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

    /// Set the resource root for disk-based script resolution (F24).
    pub fn resource_root(mut self, root: PathBuf) -> Self {
        self.script_injector.resource_root = root;
        self
    }

    /// Register a custom script for injection into every webview (F24).
    pub fn inject_script(mut self, script: InjectedScript) -> Self {
        self.script_injector.register(script);
        self
    }

    /// Inject all standard platform runtime scripts with disk→static fallback
    /// chains (F24). Registers: scheme_interceptor, foundation_wasm,
    /// foundation_wasm_ui, capability_bridge.
    pub fn inject_platform_runtimes(mut self) -> Self {
        self.runtimes_injected = true;
        let old = std::mem::replace(
            &mut self.script_injector,
            ScriptInjector::new(PathBuf::new()),
        );
        let root = old.resource_root.clone();
        self.script_injector = ScriptInjector::with_platform_runtimes(root);
        for script in old.scripts {
            self.script_injector.register(script);
        }
        self
    }

    /// Register a ScriptInjectorPlugin (F24).
    pub fn register_plugin(mut self, plugin: impl ScriptInjectorPlugin) -> Self {
        plugin.inject_scripts(&mut self.script_injector);
        self
    }

    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        let mut routes = std::mem::take(&mut self.routes);
        let setups = std::mem::take(&mut self.setups);
        let index_url = self.index_url.clone();
        let script_injector = std::mem::take(&mut self.script_injector);

        self.inner = self.inner.setup(move |app| {
            let resource_root = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| env::current_dir().unwrap_or_default());

            // Use resolved resource root if none set explicitly
            let injector = if script_injector.resource_root == PathBuf::new() {
                ScriptInjector::with_platform_runtimes(resource_root.clone())
            } else {
                script_injector
            };

            let session = PlatformSession::new(resource_root, injector);

            for entry in routes.drain(..) {
                let mut d = entry.decision.clone();
                let handler_id = entry.responder.map(|r| {
                    let rid = format!("__route_{}", entry.pattern);
                    session.register_responder(&rid, r);
                    rid
                });
                d.handler_id = handler_id;
                session.route(&entry.pattern, d.clone());

                if entry.pattern.ends_with("/*") {
                    let prefix = entry.pattern.trim_end_matches('*');
                    session.route(prefix.trim_end_matches('/'), d.clone());
                }
            }

            for setup in &setups { setup(&session); }

            // F24: resolve and eval all platform runtime scripts
            let scripts = session.script_injector().resolve_all();
            if let Some(window) = app.get_webview_window("main") {
                for script in &scripts {
                    let _ = window.eval(script);
                }
            }

            app.manage(session);
            Ok(())
        });

        self.inner = self.inner.invoke_handler(tauri::generate_handler![__ewe_capabilities]);

        self.inner = ewe::register_ewe_protocol(self.inner);
        self.inner.build(context)
    }

    pub fn invoke_handler<H: Send + Sync + 'static>(mut self, handler: H) -> Self
    where H: Fn(tauri::ipc::Invoke<R>) -> bool,
    { self.inner = self.inner.invoke_handler(handler); self }

    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> { &mut self.inner }
}

impl<R: Runtime> Default for PlatformBuilder<R> { fn default() -> Self { Self::new() } }

// ── F23 Tauri command: __ewe_capabilities ────────────────────────────────

/// The Tauri command bridge for portable WasmCapability invocations (F23).
///
/// JS calls `window.__TAURI_INTERNALS__.invoke('__ewe_capabilities', { capability, action, payload })`
/// which lands here. The command looks up the capability in the
/// `CapabilityRegistry` and delegates to the handler.
#[tauri::command]
fn __ewe_capabilities(
    session: tauri::State<'_, Arc<PlatformSession>>,
    capability: String,
    action: String,
    payload: String,
) -> Result<String, String> {
    let request = CapabilityRequest {
        capability,
        action,
        payload: payload.into_bytes(),
        content_type: CapabilityContentType::Json,
    };

    let response = session
        .invoke_wasm_capability(&request)
        .map_err(|e| format!("capability error: {:?}", e))?;

    String::from_utf8(response.payload).map_err(|e| format!("invalid UTF-8 response: {e}"))
}
