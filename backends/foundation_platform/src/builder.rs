use std::path::PathBuf;
use std::sync::Arc;

use foundation_wasm::ipc::{IpcContentType, IpcRequest};
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
    /// The only CDN domain OTA manifests may be fetched from (F40).
    /// Baked here at compile time so no runtime input can redirect updates.
    ota_manifest_domain: Option<String>,
    /// Ed25519 public key that authenticates OTA manifests (F40).
    ota_manifest_key: Option<[u8; 32]>,
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
            ota_manifest_domain: None,
            ota_manifest_key: None,
        }
    }

    /// Bake the CDN domain OTA manifests must come from (F40).
    ///
    /// WHY: the manifest URL is derived as `https://{domain}/ewe-manifest.json`
    /// and never taken from user input, IPC, or the manifest itself. A
    /// compromised route handler therefore cannot aim the updater at an
    /// attacker's host, and the value is visible in the shipped binary rather
    /// than being configurable on-device. Changing it takes a new release.
    ///
    /// This is a routing guard, not a trust anchor — content authenticity
    /// comes from [`Self::ota_manifest_key`].
    #[must_use]
    pub fn ota_manifest_domain(mut self, domain: &str) -> Self {
        self.ota_manifest_domain = Some(domain.to_string());
        self
    }

    /// Bake the Ed25519 public key that authenticates OTA manifests (F40).
    ///
    /// WHY: this key is the trust anchor for the whole update path. It proves
    /// *we* declared the SHA-256 hashes in a manifest; those hashes then prove
    /// the downloaded bytes match that declaration. Without it, a compromised
    /// CDN can serve any bundle it likes with hashes it computed itself.
    ///
    /// Only the public half ships. The private seed lives in CI secrets.
    #[must_use]
    pub fn ota_manifest_key(mut self, key: [u8; 32]) -> Self {
        self.ota_manifest_key = Some(key);
        self
    }

    /// Bake the OTA public key from a base64 key file read at compile time.
    ///
    /// Pair with `concat!(env!("CARGO_MANIFEST_DIR"), "/keys/ota_public.key")`
    /// so the bytes come from the repository rather than the device.
    ///
    /// # Panics
    ///
    /// Panics if `contents` is not exactly 32 base64-decoded bytes. This is a
    /// build-configuration error — a malformed key would silently disable
    /// signature verification, so failing at startup is the safe outcome.
    #[must_use]
    pub fn ota_manifest_key_from_str(self, contents: &str) -> Self {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(contents.trim())
            .expect("OTA public key is not valid base64");
        let key = <[u8; 32]>::try_from(bytes.as_slice())
            .expect("OTA public key must be exactly 32 bytes");
        self.ota_manifest_key(key)
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
        let ota_manifest_domain = self.ota_manifest_domain.clone();
        let ota_manifest_key = self.ota_manifest_key;

        self.inner = self.inner.setup(move |app| {
            // The bundle version is already baked into every Tauri build:
            // tauri.conf.json → PackageInfo::version → APK versionName.
            // Tauri's schema guarantees it is semver.
            let bundle_version = app.package_info().version.to_string();

            let manager = Arc::new(crate::assets::PlatformAssetManager::initialize(
                app,
                &bundle_version,
                ota_manifest_domain.clone(),
                ota_manifest_key,
            ));
            let resource_root = manager.base_root().to_path_buf();

            // Use resolved resource root if none set explicitly
            let injector = if script_injector.resource_root == PathBuf::new() {
                ScriptInjector::with_platform_runtimes(resource_root.clone())
            } else {
                script_injector
            };

            let session = PlatformSession::new(resource_root, injector);
            // Installed before the setup callbacks run so `session.app_root()`
            // resolves version directories while routes are being registered.
            session.set_asset_manager(manager);

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

            // F24: capture the main window label for IPC context scoping.
            let main_label = app.get_webview_window("main")
                .map(|w| w.label().to_string())
                .unwrap_or_else(|| "main".to_string());
            session.set_main_webview_label(&main_label);

            app.manage(session);
            Ok(())
        });

        self.inner = self.inner.invoke_handler(tauri::generate_handler![
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

// ── F41 unified Tauri command: __ewe_ipc ─────────────────────────────────

/// The unified Tauri command for IPC + capability invocations (F41).
///
/// JS calls `window.__TAURI_INTERNALS__.invoke('__ewe_ipc', { ipc, action, payload })`.
/// The command builds an `IpcInvokeContext` from the Tauri window, dispatches
/// through `session.invoke_ipc()`, and returns the response bytes.
///
/// Replaces both `__ewe_ipc` (F25) and `__ewe_capabilities` (F23).
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn __ewe_ipc(
    session: tauri::State<'_, Arc<PlatformSession>>,
    ipc: String,
    action: String,
    payload: Option<Vec<u8>>,
    content_type: Option<String>,
) -> Result<Vec<u8>, String> {
    use crate::handle::IpcInvokeContext;

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

    let ctx = IpcInvokeContext {
        page: session.active_page_identity().unwrap_or(foundation_ui_traits::PageIdentity {
            session_id: foundation_ui_traits::SessionId(0),
            route: String::new(),
            visit_id: 0,
        }),
        webview_label: session.main_webview_label(),
    };

    let response = session
        .invoke_ipc(&ctx, &request)
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
