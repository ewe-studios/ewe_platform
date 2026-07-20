use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_core::valtron::valtron;
use foundation_platform::backend::http::HttpBackend;
use foundation_platform::capability::PlatformCapability;
use foundation_platform::*;
use foundation_ui_traits::{CapabilityId, NavigationIntent, Profile, RouteDecision};
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};
use foundation_wasm::{CapabilityError, CapabilityRequest, CapabilityResponse, WasmCapability};

mod generated;

// ── HTML helpers ──────────────────────────────────────────────────────

/// Wrap a body + optional extra head scripts into a full page.
fn page_html(title: &str, body: &str, nav: &str, extra_head: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>{title}</title>\
         <style>body{{font-family:sans-serif;padding:16px;background:#0a0a1a;color:#ccd6f6}}h1{{color:#64ffda;font-size:18px}}button{{background:#bd93f9;border:none;padding:10px 16px;margin:4px;color:#0a0a1a;border-radius:6px;font-size:14px;cursor:pointer}}button:hover{{opacity:.85}}pre{{background:#112240;padding:12px;border-radius:6px;overflow-x:auto;min-height:80px;max-height:300px;font-size:12px}}a{{padding:6px 10px;margin:3px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:5px;text-decoration:none;font-size:12px;display:inline-block}}a:hover{{background:#233554}}</style>\
         <script>{interceptor}</script>{extra_head}</head><body>{body}{nav}</body></html>",
        interceptor = foundation_wasm_ui::embedded::PLATFORM_SCHEME_INTERCEPTOR_JS,
    )
}

fn nav_buttons(current: &str) -> String {
    let pages = [
        ("/app/", "🏠 App"),
        ("/app-hello/", "👋 Hello"),
        ("/remote/news", "🌐 Remote"),
        ("/api/invoke", "⚡ Invoke"),
        ("/api/system", "💻 System"),
        ("/api/events", "📡 Events"),
        ("/api/capability", "🔐 Capability"),
    ];
    pages
        .iter()
        .filter(|(p, _)| !current.starts_with(p) && !p.starts_with(current))
        .fold(String::new(), |s, (href, label)| {
            s + &format!("<a href=\"ewe://localhost{href}\">{label}</a>")
        })
}

/// Helper to build an HTTP response from HTML.
fn html_response(html: String) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(html.into_bytes())
        .unwrap()
}

// ── IPC handlers ─────────────────────────────────────────────────────

struct EchoIpc;
impl foundation_wasm::ipc::Ipc for EchoIpc {
    fn name(&self) -> &str {
        "echo"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Query
    }
    fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}
impl foundation_platform::ipc::PlatformIpc for EchoIpc {
    fn invoke_with_session(
        &self,
        _: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}

struct SystemInfoIpc;
impl foundation_wasm::ipc::Ipc for SystemInfoIpc {
    fn name(&self) -> &str {
        "system"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Query
    }
    fn invoke(&self, _request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let info = serde_json::json!({
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "pid": std::process::id(),
            "cwd": std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default(),
        });
        Ok(IpcResponse {
            payload: serde_json::to_vec(&info).unwrap(),
            content_type: IpcContentType::Json,
        })
    }
}
impl foundation_platform::ipc::PlatformIpc for SystemInfoIpc {
    fn invoke_with_session(
        &self,
        _session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        self.invoke(request)
    }
}

/// A ticker IPC — returns an incrementing counter on each invoke.
/// Used by the /api/events page to demonstrate polling-based updates.
struct TickerIpc {
    counter: AtomicU64,
}
impl foundation_wasm::ipc::Ipc for TickerIpc {
    fn name(&self) -> &str {
        "ticker"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Query
    }
    fn invoke(&self, _request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        let payload = serde_json::json!({ "tick": count });
        Ok(IpcResponse {
            payload: serde_json::to_vec(&payload).unwrap(),
            content_type: IpcContentType::Json,
        })
    }
}
impl foundation_platform::ipc::PlatformIpc for TickerIpc {
    fn invoke_with_session(
        &self,
        _session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        self.invoke(request)
    }
}

// ── Capability handler ───────────────────────────────────────────────

struct EchoCap;
impl WasmCapability for EchoCap {
    fn name(&self) -> &str {
        "echo_cap"
    }
    fn invoke_capability(
        &self,
        request: &CapabilityRequest<Vec<u8>>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> {
        Ok(CapabilityResponse {
            capability: request.capability.clone(),
            action: request.action.clone(),
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}
impl PlatformCapability for EchoCap {
    fn capability_id(&self) -> &CapabilityId {
        Box::leak(Box::new(CapabilityId("echo".into())))
    }
    fn min_profile(&self) -> Profile {
        Profile::TrustedRemote
    }
}

// ── IPC Demo page responders ─────────────────────────────────────────

struct RemoteProxy {
    http: Arc<HttpBackend>,
}
impl RouteResponder for RemoteProxy {
    fn respond(
        &self,
        intent: &NavigationIntent,
        _: &RouteDecision,
        _: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        let path = foundation_platform::pattern::extract_path(&intent.url);
        // Map ewe://localhost/remote/{url_path} → https://{url_path}
        let remote_path = path.strip_prefix("/remote/").unwrap_or(&path);
        let remote_url = if remote_path.starts_with("http") {
            remote_path.to_string()
        } else {
            format!("https://{remote_path}")
        };

        match self.http.fetch(&remote_url) {
            Ok((body, content_type)) => {
                let safe_body = String::from_utf8_lossy(&body);
                let html = iframe_wrapper(&path, &remote_url, &safe_body, &nav_buttons("/remote/"));
                tauri::http::Response::builder()
                    .status(200)
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(html.into_bytes())
                    .unwrap()
            }
            Err(e) => {
                let body = format!("<h1>Remote Fetch Error</h1><pre>{e}</pre>");
                html_response(page_html(&path, &body, &nav_buttons("/remote/"), ""))
            }
        }
    }
}

/// Wrap remote content in an iframe with shared nav bar so users can
/// navigate back to other ewe:// pages.
fn iframe_wrapper(path: &str, remote_url: &str, body: &str, nav: &str) -> String {
    let interceptor = foundation_wasm_ui::embedded::PLATFORM_SCHEME_INTERCEPTOR_JS;
    // Encode the body so it renders safely inside the page.
    // In production we would use srcdoc on the iframe, but for now
    // we inline it with a sandbox.
    format!(
        "<!DOCTYPE html><html><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>Remote: {path}</title>\
         <style>body{{margin:0;padding:0;background:#0a0a1a;color:#ccd6f6;font-family:sans-serif}}iframe{{width:100%;height:calc(100vh - 60px);border:none}}#nav{{display:flex;flex-wrap:wrap;padding:8px;background:#112240;gap:4px;min-height:44px;align-items:center}}\
         a{{padding:5px 8px;background:#1a2a4a;color:#64ffda;border:1px solid #233554;border-radius:4px;text-decoration:none;font-size:11px}}a:hover{{background:#233554}}\
         .url{{font-size:10px;color:#8892b0;margin-left:8px}}</style>\
         <script>{interceptor}</script></head><body>\
         <div id=nav>{nav}<span class=url>⤷ {remote_url}</span></div>\
         <iframe sandbox='allow-scripts allow-same-origin' srcdoc='{escaped_body}'></iframe>\
         </body></html>",
        interceptor = interceptor,
        escaped_body = html_escape(&body),
    )
}

/// Minimal HTML escaper for safe inlining into srcdoc.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

struct IpcInvokePage;
impl RouteResponder for IpcInvokePage {
    fn respond(
        &self,
        _intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        let body = r##"
<h1>⚡ IPC Invoke — /api/invoke</h1>
<button onclick="pingEcho()">Ping Echo IPC</button>
<button onclick="getSystem()">Get System Info</button>
<pre id="output">Click a button to invoke an IPC handler...</pre>
<script>
var out = document.getElementById('output');
async function _invoke(ipc, action, payload) {
    out.textContent = 'Invoking ' + ipc + '...';
    try {
        var data = Array.from(new TextEncoder().encode(payload ? JSON.stringify(payload) : '{}'));
        var result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', { ipc: ipc, action: action, payload: data, content_type: 'application/json' });
        out.textContent = JSON.stringify(JSON.parse(new TextDecoder().decode(new Uint8Array(result))), null, 2);
    } catch(e) { out.textContent = 'IPC Error: ' + e; }
}
function pingEcho() { _invoke('echo', 'ping', { msg: 'hello from ewe://' }); }
function getSystem() { _invoke('system', 'get_info'); }
</script>"##;
        html_response(page_html(
            "IPC Invoke",
            body,
            &nav_buttons("/api/invoke"),
            "",
        ))
    }
}

struct IpcSystemPage;
impl RouteResponder for IpcSystemPage {
    fn respond(
        &self,
        _intent: &NavigationIntent,
        _decision: &RouteDecision,
        session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        let ids = session.ipc_registry().names().join(", ");
        let caps = session.capabilities().names().join(", ");
        let body = format!(
            r##"
<h1>💻 System — /api/system</h1>
<p>Registered IPCs: <b>{ids}</b></p>
<p>Registered capabilities: <b>{caps}</b></p>
<p>Session ID: <b>{sid:?}</b></p>
<button onclick="invokeSystem()">Invoke System IPC</button>
<pre id="output">Click the button to query system info via IPC...</pre>
<script>
var out = document.getElementById('output');
async function invokeSystem() {{
    out.textContent = 'Invoking system IPC...';
    try {{
        var result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {{ ipc: 'system', action: 'get_info', content_type: 'application/json' }});
        out.textContent = JSON.stringify(JSON.parse(new TextDecoder().decode(new Uint8Array(result))), null, 2);
    }} catch(e) {{ out.textContent = 'Error: ' + e; }}
}}
</script>"##,
            ids = ids,
            caps = caps,
            sid = session.session_id(),
        );
        html_response(page_html(
            "System Info",
            &body,
            &nav_buttons("/api/system"),
            "",
        ))
    }
}

struct IpcEventsPage;
impl RouteResponder for IpcEventsPage {
    fn respond(
        &self,
        _intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        let body = r##"
<h1>📡 Events — /api/events</h1>
<p>Polling the ticker IPC every second to simulate push-based updates.</p>
<button onclick="startPolling()">Start Polling</button>
<button onclick="stopPolling()">Stop Polling</button>
<pre id="output">Click Start Polling...</pre>
<script>
var out = document.getElementById('output');
var timer = null;
var ticks = [];
function startPolling() {
    if (timer) return;
    out.textContent = 'Polling...';
    timer = setInterval(async function() {
        try {
            var result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', { ipc: 'ticker', action: 'tick', content_type: 'application/json' });
            var data = JSON.parse(new TextDecoder().decode(new Uint8Array(result)));
            ticks.push(data.tick);
            if (ticks.length > 20) ticks.shift();
            out.textContent = 'Ticks received: [' + ticks.join(', ') + ']';
        } catch(e) { out.textContent = 'Error: ' + e; }
    }, 1000);
}
function stopPolling() {
    if (timer) { clearInterval(timer); timer = null; }
    out.textContent = 'Stopped. ' + ticks.length + ' ticks received.';
}
</script>"##;
        html_response(page_html("Events", body, &nav_buttons("/api/events"), ""))
    }
}

struct IpcCapabilityPage;
impl RouteResponder for IpcCapabilityPage {
    fn respond(
        &self,
        _intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        let body = r##"
<h1>🔐 Capability — /api/capability</h1>
<p>Invoke the echo capability through the security gated path.</p>
<button onclick="invokeCap()">Invoke Echo Capability</button>
<pre id="output">Click the button...</pre>
<script>
var out = document.getElementById('output');
async function invokeCap() {
    out.textContent = 'Invoking echo capability...';
    try {
        var result = await window.__TAURI_INTERNALS__.invoke('__ewe_capabilities', { capability: 'echo', action: 'ping', payload: JSON.stringify({ msg: 'cap-test' }) });
        out.textContent = JSON.stringify(JSON.parse(result), null, 2);
    } catch(e) { out.textContent = 'Capability Error: ' + e; }
}
</script>"##;
        html_response(page_html(
            "Capability",
            body,
            &nav_buttons("/api/capability"),
            "",
        ))
    }
}

// ── Setup ────────────────────────────────────────────────────────────

fn setup_routes(session: &PlatformSession) {
    let app_responder = generated::app::AppAssets::build();
    let hello_responder = generated::app_hello::AppAssets::build();

    session.register_route_with(
        "/app/*",
        webview_app().with_profile(Profile::App),
        app_responder,
    );
    session.register_route_with(
        "/app-hello/*",
        webview_app().with_profile(Profile::App),
        hello_responder,
    );

    // Register IPC handlers on the session.
    session.register_ipc(EchoIpc);
    session.register_ipc(SystemInfoIpc);
    session.register_ipc(TickerIpc {
        counter: AtomicU64::new(0),
    });

    // Register a capability on the session.
    session.register_capability(EchoCap);

    // IPC demo routes — each serves an interactive HTML page with JS that
    // calls __TAURI_INTERNALS__.invoke() to exercise the IPC bridge.
    session.register_route_with("/api/invoke", ipc_shell_with("invoke"), IpcInvokePage);
    session.register_route_with("/api/system", ipc_shell_with("system"), IpcSystemPage);
    session.register_route_with("/api/events", ipc_shell_with("events"), IpcEventsPage);
    session.register_route_with(
        "/api/capability",
        ipc_shell_with("capability"),
        IpcCapabilityPage,
    );

    // Real remote proxy — fetches external URLs via HTTP and wraps in iframe.
    let http = Arc::new(HttpBackend::new());
    session.register_route_with(
        "/remote/*",
        remote_fetch().with_profile(Profile::TrustedRemote),
        RemoteProxy { http },
    );

    // API fallback
    session.register_route_with("/api/*", ipc_shell_with("shell"), IpcInvokePage);

    println!("[platform_android] Session: {:?}", session.session_id());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[valtron]
pub fn run() {
    platform_run!(PlatformBuilder::new()
        .inject_platform_runtimes()
        .setup(setup_routes));
}
