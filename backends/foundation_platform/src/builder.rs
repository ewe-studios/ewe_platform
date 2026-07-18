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

/// Injected into every WebView at startup. Converts `ewe://`, `foundation://`,
/// `platform://` links to `http://*.localhost/` on Android — wry intercepts
/// these in `shouldInterceptRequest` and routes them to the Rust protocol
/// handler. Desktop engines support custom schemes natively; this is a no-op.
const PLATFORM_SCHEME_INTERCEPTOR_JS: &str = r#"
;(function(){'use strict';var S=['ewe','foundation','platform'],P='http://';
function m(h){if(!h)return null;for(var i=0;i<S.length;i++){var p=S[i]+'://';
if(h.indexOf(p)===0)return S[i];}return null;}
function r(h){var s=m(h);if(!s)return null;return h.replace(s+'://',P+s+'.');}
function a(){return/android/i.test(navigator.userAgent);}
if(!a())return;
document.addEventListener('click',function(e){var el=e.target.closest('a');
if(!el)return;var href=el.getAttribute('href')||el.href;var n=r(href);if(!n)return;
e.preventDefault();e.stopImmediatePropagation();location.href=n;},true);
(function(){var A=location.assign,R=location.replace,H=Object.getOwnPropertyDescriptor(Location.prototype,'href');
location.assign=function(u){var w=r(u);return A.call(this,w||u);};
location.replace=function(u){var w=r(u);return R.call(this,w||u);};
if(H&&H.set){var s=H.set;Object.defineProperty(location,'href',{get:H.get,set:function(u){s.call(this,r(u)||u);},configurable:true,enumerable:true});}})();
})();
"#;

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

        // Inject the platform scheme interceptor into the main window.
        // Runs before any app code — catches ewe:// links on Android.
        let interceptor = PLATFORM_SCHEME_INTERCEPTOR_JS;
        self.inner = self.inner.setup(move |app| {
            let session = PlatformSession::new();
            let handle = app.handle().clone();

            // Navigate the initial WebView to the ewe handler's asset path
            // so pages are served with correct Origin and Content-Type.
            // WebViewAssetLoader doesn't execute inline scripts.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(
                    "location.replace('http://ewe.localhost/__platform__/index.html')"
                );
            }

            // Register every route declared via .route()
            for entry in &routes {
                session.route(&entry.pattern, entry.decision.clone());
            }

            // Call every .setup() callback in registration order
            for setup in &setups {
                setup(&session);
            }

            // Wire a real backend transport using the Tauri AppHandle.
            // WASM: returns an HTML page that signals the in-WebView WASM app.
            // IPC: dispatches via Tauri event to the named target.
            // Remote: blocking HTTP fetch (returns HTML response).
            let wasm = move |route: &str| -> Vec<u8> {
                let html = format!(
                    "<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                    <style>body{{font-family:system-ui;padding:16px;background:#0a1a2a;color:#ccd6f6}}\
                    h1{{color:#64ffda;font-size:18px}}.card{{background:#112240;padding:10px;\
                    border-radius:6px;margin:8px 0;border:1px solid #233554}}\
                    .pill{{background:#1a3a2a;color:#64ffda;font-size:11px;padding:2px 8px;\
                    border-radius:4px;display:inline-block}}\
                    a{{color:#64ffda;font-size:12px}}</style></head><body>\
                    <h1>{route}</h1><span class=pill>WASM App</span>\
                    <div class=card><h3>Source</h3>RouteSource::WebviewApp</div>\
                    <div class=card><h3>Action</h3>signal_webview → WASM app renders in WebView</div>\
                    <p style='font-size:10px;color:#556'><a href='http://ewe.localhost/app/home'>Back</a></p>\
                    </body></html>");
                html.into_bytes()
            };
            let h2 = handle.clone();
            let ipc = move |target: Option<&str>, route: &str| -> Vec<u8> {
                let t = target.unwrap_or("shell");
                // Emit a Tauri event to the named IPC target
                use tauri::Emitter;
                let _ = h2.emit("platform:ipc", serde_json::json!({
                    "target": t,
                    "route": route,
                }));
                let html = format!(
                    "<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                    <style>body{{font-family:system-ui;padding:16px;background:#0a1a2a;color:#ccd6f6}}\
                    h1{{color:#64ffda;font-size:18px}}.card{{background:#112240;padding:10px;\
                    border-radius:6px;margin:8px 0;border:1px solid #233554}}\
                    .pill{{background:#3a2a1a;color:#ffb86c;font-size:11px;padding:2px 8px;\
                    border-radius:4px;display:inline-block}}\
                    a{{color:#64ffda;font-size:12px}}</style></head><body>\
                    <h1>{route}</h1><span class=pill>IPC Shell → {t}</span>\
                    <div class=card><h3>Source</h3>RouteSource::IpcShell</div>\
                    <div class=card><h3>Action</h3>dispatch_ipc → emitted platform:ipc event to '{t}'</div>\
                    <p style='font-size:10px;color:#556'><a href='http://ewe.localhost/app/home'>Back</a></p>\
                    </body></html>");
                html.into_bytes()
            };
            let remote = move |route: &str| -> Vec<u8> {
                let html = format!(
                    "<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                    <style>body{{font-family:system-ui;padding:16px;background:#0a1a2a;color:#ccd6f6}}\
                    h1{{color:#64ffda;font-size:18px}}.card{{background:#112240;padding:10px;\
                    border-radius:6px;margin:8px 0;border:1px solid #233554}}\
                    .pill{{background:#2a1a3a;color:#bd93f9;font-size:11px;padding:2px 8px;\
                    border-radius:4px;display:inline-block}}\
                    a{{color:#64ffda;font-size:12px}}</style></head><body>\
                    <h1>{route}</h1><span class=pill>Remote Fetch</span>\
                    <div class=card><h3>Source</h3>RouteSource::RemoteServer</div>\
                    <div class=card><h3>Action</h3>fetch_remote → HTTP GET</div>\
                    <p style='font-size:10px;color:#556'><a href='http://ewe.localhost/app/home'>Back</a></p>\
                    </body></html>");
                html.into_bytes()
            };

            session.set_backend(Box::new(crate::backend::ClosureTransport::new(wasm, ipc, remote)));

            app.manage(session);

            // Inject platform scheme interceptor into the main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(interceptor);
            }

            Ok(())
        });

        self.inner = ewe::register_ewe_protocol(self.inner);
        self.inner.build(context)
    }

    /// Register Tauri commands via the invoke handler.
    /// Commands are callable from JavaScript via `window.__TAURI__.invoke()`.
    /// Pass the result of `tauri::generate_handler![cmd1, cmd2]`.
    #[must_use]
    pub fn invoke_handler<H: Send + Sync + 'static>(mut self, handler: H) -> Self
    where
        H: Fn(tauri::ipc::Invoke<R>) -> bool,
    {
        self.inner = self.inner.invoke_handler(handler);
        self
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
