use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use foundation_wasm::{CapabilityContentType, CapabilityRequest};
use tauri::{App, Context, Manager, Runtime};

use crate::ewe;
use crate::injector::{InjectedScript, ScriptInjector, ScriptInjectorPlugin};
use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

/// On Android, Tauri's `resource_dir()` returns `asset://localhost/` — a content
/// URI, not a real filesystem path. `std::fs::read()` can't use it. We extract
/// `bundle.resources` from the APK into AppData so `MobileDirectory` responders
/// can serve files via `std::fs::read()` as they do on desktop.
///
/// Returns the `resource_root` that should be used — on desktop it's the original
/// `resource_dir()`, on Android it's the AppData directory with extracted files.
fn resolve_resource_root<R: Runtime>(app: &App<R>) -> PathBuf {
    let resource_root = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| env::current_dir().unwrap_or_default());

    // Desktop / dev: resource_dir is a real filesystem path.
    if resource_root.exists() && resource_root.is_dir() {
        return resource_root;
    }

    // Mobile: resource_dir is a content URI — extract assets to AppData.
    let app_data = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| env::current_dir().unwrap_or_default());

    let resolver = app.asset_resolver();
    for asset_entry in resolver.iter() {
        let asset_key: String = (*asset_entry.0).to_string();
        let dest = app_data.join(&asset_key);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Some(asset) = resolver.get(asset_key) {
            let _ = std::fs::write(&dest, &asset.bytes);
        }
    }

    app_data
}

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
    #[must_use]
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

    #[must_use]
    pub fn route(mut self, pattern: &str, decision: foundation_ui_traits::RouteDecision) -> Self {
        self.routes.push(RouteEntry { pattern: pattern.to_string(), decision, responder: None });
        self
    }

    /// Register a route WITH its responder — all in one declarative call.
    #[must_use]
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

    #[must_use]
    pub fn setup<F: Fn(&PlatformSession) + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.setups.push(Box::new(f));
        self
    }

    #[must_use]
    pub fn index_as(mut self, route: &str) -> Self {
        self.index_url = Some(route.to_string());
        self
    }

    /// Set the resource root for disk-based script resolution (F24).
    #[must_use]
    pub fn resource_root(mut self, root: PathBuf) -> Self {
        self.script_injector.resource_root = root;
        self
    }

    /// Register a custom script for injection into every webview (F24).
    #[must_use]
    pub fn inject_script(mut self, script: InjectedScript) -> Self {
        self.script_injector.register(script);
        self
    }

    /// Inject all standard platform runtime scripts with disk→static fallback
    /// chains (F24). Registers: `scheme_interceptor`, `foundation_wasm`,
    /// `foundation_wasm_ui`, `capability_bridge`.
    #[must_use]
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

    /// Register a `ScriptInjectorPlugin` (F24).
    #[must_use]
    pub fn register_plugin(mut self, plugin: &impl ScriptInjectorPlugin) -> Self {
        plugin.inject_scripts(&mut self.script_injector);
        self
    }

    /// Build the platform application.
    ///
    /// # Errors
    ///
    /// Returns [`tauri::Error`] if Tauri initialisation fails.
    #[allow(clippy::needless_pass_by_value)]
    pub fn build(mut self, context: Context<R>) -> tauri::Result<App<R>> {
        let mut routes = std::mem::take(&mut self.routes);
        let setups = std::mem::take(&mut self.setups);
        let _index_url = self.index_url.clone();
        let script_injector = std::mem::take(&mut self.script_injector);

        self.inner = self.inner.setup(move |app| {
            let resource_root = resolve_resource_root(app);

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

            // F35: Inject Tauri window manager into the session.
            // The pool tracks labels; the manager creates real Tauri windows.
            session.window_manager().set_ops(
                crate::window::TauriWindowOps::new(app.handle().clone()).into_boxed()
            );

            // F24: resolve and eval all platform runtime scripts.
            // On mobile (Android/iOS), `window.eval()` treats the injected JS as a
            // classic script — the `export` keyword causes a SyntaxError. We strip
            // `export` statements on mobile so the `globalThis.FoundationWasmRuntime`
            // mirror path works (the scripts are designed for both ESM and IIFE).
            let scripts = session.script_injector().resolve_all();
            if let Some(window) = app.get_webview_window("main") {
                let is_mobile = cfg!(target_os = "android") || cfg!(target_os = "ios");
                for script in &scripts {
                    let to_eval = if is_mobile {
                        // Strip top-level `export` so classic-script eval works on WebView.
                        script.replace("export ", "// export ")
                    } else {
                        script.clone()
                    };
                    if !to_eval.is_empty() {
                        let _ = window.eval(&to_eval);
                    }
                }
            }

            app.manage(session);
            Ok(())
        });

        self.inner = self.inner.invoke_handler(tauri::generate_handler![
            __ewe_capabilities,
            __ewe_ipc,
            __ewe_ipc_stream
        ]);

        self.inner = ewe::register_ewe_protocol(self.inner);
        self.inner.build(context)
    }

    #[must_use]
    pub fn invoke_handler<H>(mut self, handler: H) -> Self
    where H: Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
    { self.inner = self.inner.invoke_handler(handler); self }

    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> { &mut self.inner }
}

impl<R: Runtime> Default for PlatformBuilder<R> { fn default() -> Self { Self::new() } }

// ── F23 Tauri command: __ewe_capabilities ────────────────────────────────

/// The Tauri command bridge for capability invocations (F23).
///
/// JS calls `window.__TAURI_INTERNALS__.invoke('__ewe_capabilities', { capability, action, payload })`.
/// Looks up the capability in the `CapabilityRegistry` and delegates to the handler.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
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

    // Look up the handler in the platform registry and invoke with session access.
    let handler = session
        .get_capability(&request.capability)
        .ok_or_else(|| format!("unknown capability: {}", request.capability))?;

    let response = handler
        .invoke_with_session(&session, &request)
        .map_err(|e| format!("capability error: {e:?}"))?;

    String::from_utf8(response.payload).map_err(|e| format!("invalid UTF-8 response: {e}"))
}

// ── F25 Tauri command: __ewe_ipc ────────────────────────────────────────

/// The Tauri command bridge for IPC invocations (F25).
///
/// JS calls `window.__TAURI_INTERNALS__.invoke('__ewe_ipc', { ipc, action, payload })`
/// which lands here. The command looks up the IPC in the `IpcRegistry`
/// and delegates to the handler.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn __ewe_ipc(
    session: tauri::State<'_, Arc<PlatformSession>>,
    ipc: String,
    action: String,
    payload: Option<Vec<u8>>,
    content_type: Option<String>,
) -> Result<Vec<u8>, String> {
    let ct = match content_type.as_deref() {
        Some("arrow" | "application/vnd.apache.arrow.batch") => {
            foundation_wasm::ipc::IpcContentType::Arrow
        }
        Some("binary" | "application/octet-stream") => {
            foundation_wasm::ipc::IpcContentType::Binary
        }
        _ => foundation_wasm::ipc::IpcContentType::Json,
    };

    let request = foundation_wasm::ipc::IpcRequest {
        ipc,
        action,
        payload: payload.unwrap_or_default(),
        content_type: ct,
        target: None,
    };

    let response = session
        .ipc_registry()
        .invoke(&session, &request)
        .map_err(|e| format!("ipc error: {e:?}"))?;

    Ok(response.payload)
}

// ── F26 Tauri command: __ewe_ipc_stream ──────────────────────────────────

/// The Tauri command for server→client streaming IPC (F26).
///
/// JS creates a `Channel<IpcStreamChunk>`, passes it via `invoke()`, and
/// Tauri deserializes it from the `"__CHANNEL__:ID"` wire format.
/// The command looks up the `StreamingIpc` handler, calls `stream()`,
/// and drains the receiver into the Tauri Channel.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn __ewe_ipc_stream(
    session: tauri::State<'_, Arc<PlatformSession>>,
    ipc: String,
    action: String,
    payload: Vec<u8>,
    stream_channel: tauri::ipc::Channel<crate::ipc::streaming::IpcStreamChunk>,
) -> Result<(), String> {
    use foundation_wasm::ipc::{IpcContentType, IpcRequest};
    use crate::ipc::streaming::IpcStreamReceiver;

    let request = IpcRequest {
        ipc: ipc.clone(),
        action,
        payload,
        content_type: IpcContentType::Json,
        target: None,
    };

    let handler = session.stream_registry().get(&request.ipc)
        .ok_or_else(|| format!("unknown streaming ipc: {ipc}"))?;

    let ipc_stream = handler.stream(&session, &request)
        .map_err(|e| format!("stream error: {e:?}"))?;

    loop {
        match &ipc_stream.receiver {
            IpcStreamReceiver::Sync(queue) => {
                match queue.pop() {
                    Ok(Ok(chunk)) => {
                        stream_channel.send(chunk)
                            .map_err(|_| "channel closed".to_string())?;
                    }
                    Ok(Err(e)) => {
                        let _ = stream_channel.send(crate::ipc::streaming::IpcStreamChunk {
                            data: format!("error: {e:?}").into_bytes(),
                            sequence: u64::MAX,
                            progress: None,
                        });
                        return Err(format!("stream error: {e:?}"));
                    }
                    Err(_) => return Ok(()),
                }
            }
        }
    }
}
