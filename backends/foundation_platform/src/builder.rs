use tauri::{App, Context, Manager, Runtime};

use crate::ewe;
use crate::session::PlatformSession;

struct RouteEntry {
    pattern: String,
    decision: foundation_ui_traits::RouteDecision,
}

type SetupCallback = Box<dyn Fn(&PlatformSession) + Send + Sync + 'static>;

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

            // Route registration
            for entry in &routes { session.route(&entry.pattern, entry.decision.clone()); }
            for setup in &setups { setup(&session); }

            // Wire real backend transport via Tauri AppHandle.
            // WASM: returns an HTML page that signals the in-WebView WASM
            //   app (already loaded from initial index.html). The page
            //   contains a <script type=module> that imports and calls
            //   init() on the WASM wrapper — the wasm entrypoint renders.
            let wasm = move |route: &str| -> Vec<u8> {
                let wasm_bin = "platform_dashboard";
                format!(
                    "<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                     <meta name=viewport content='width=device-width,initial-scale=1'>\
                     <style>body{{margin:0;padding:0;background:#0a0a1a}}</style></head>\
                     <body><div id=s style='color:#8892b0;font-family:sans-serif;font-size:12px;padding:16px'>Loading {route}...</div>\
                     <script type=module src='./{wasm_bin}.js'></script>\
                     <script type=module>import{{init}}from'./{wasm_bin}.js';init().then(function(){{document.getElementById('s').textContent='OK-{route}'}}).catch(function(e){{document.getElementById('s').textContent=String(e)}})</script></body></html>"
                ).into_bytes()
            };
            let h2 = handle.clone();
            let ipc = move |target: Option<&str>, route: &str| -> Vec<u8> {
                let t = target.unwrap_or("shell");
                use tauri::Emitter; let _ = h2.emit("platform:ipc", serde_json::json!({"target":t,"route":route}));
                format!("<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                         <style>body{{font-family:sans-serif;padding:16px;background:#0a1a2a;color:#ccd6f6}}\
                         h1{{color:#64ffda;font-size:18px}}</style></head>\
                         <body><h1>{route}</h1><p>IPC Shell → {t}</p></body></html>").into_bytes()
            };
            let remote = move |route: &str| -> Vec<u8> {
                format!("<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                         <style>body{{font-family:sans-serif;padding:16px;background:#0a1a2a;color:#ccd6f6}}\
                         h1{{color:#64ffda;font-size:18px}}</style></head>\
                         <body><h1>{route}</h1><p>Remote Server fetch</p></body></html>").into_bytes()
            };
            session.set_backend(Box::new(crate::backend::ClosureTransport::new(wasm, ipc, remote)));

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
