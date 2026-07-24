use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_core::valtron::valtron;
use foundation_platform::capability::PlatformIpc;
use foundation_platform::*;
use foundation_ui_traits::{CapabilityId, NavigationIntent, Profile, RouteDecision};
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

// Public: the generated modules are part of this crate's surface. `build_at`
// is the unmounted escape hatch for embeddings with no PlatformAssetManager,
// and it is real API even though this example only uses `build`.
pub mod generated;

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
        ("/presentation/", "🎬 Presentation"),
        ("/remote/example.com", "🌐 Remote"),
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
        request: &IpcRequest<Vec<u8>>,
        callback: foundation_platform::ipc::IpcCallback,
    ) -> Result<(), IpcError> {
        callback(Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        }));
        Ok(())
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
        request: &IpcRequest<Vec<u8>>,
        callback: foundation_platform::ipc::IpcCallback,
    ) -> Result<(), IpcError> {
        callback(self.invoke(request));
        Ok(())
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
        request: &IpcRequest<Vec<u8>>,
        callback: foundation_platform::ipc::IpcCallback,
    ) -> Result<(), IpcError> {
        callback(self.invoke(request));
        Ok(())
    }
}

// ── Capability handler ───────────────────────────────────────────────

struct EchoCap;
impl Ipc<Vec<u8>, Vec<u8>> for EchoCap {
    fn name(&self) -> &str {
        "echo_cap"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Capability
    }
    fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}
impl PlatformIpc for EchoCap {
    fn capability_id(&self) -> &CapabilityId {
        Box::leak(Box::new(CapabilityId("echo".into())))
    }
    fn min_profile(&self) -> Profile {
        Profile::TrustedRemote
    }
}

// ── IPC Demo page responders ─────────────────────────────────────────

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
        var result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {{ ipc: 'system', action: 'get_info', payload: [], content_type: 'application/json' }});
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
            var result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', { ipc: 'ticker', action: 'tick', payload: [], content_type: 'application/json' });
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

// ── Presentation demo page (F35 validation) ──────────────────────────

/// Serves an interactive page that exercises all 6 Presentation modes.
/// Each button triggers a navigation to a route that uses a different
/// presentation mode and returns HTML showing which mode was activated.
struct PresentationDemoPage;
impl RouteResponder for PresentationDemoPage {
    fn respond(
        &self,
        _intent: &NavigationIntent,
        _decision: &RouteDecision,
        session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        let stack = session.webview_stack();
        let depth = stack.depth();
        let active = stack.active_route().unwrap_or("(none)").to_string();
        let session_id = format!("{:?}", session.session_id());

        let body = format!(
            r##"<h1>Presentation Mode Tests</h1>
<p>Stack: <b>{depth}</b> slots · Active: <b>{active}</b></p>
<p>Session: <b>{session_id}</b></p>
<h2>All 6 Presentation Modes</h2>
<a href="ewe://localhost/nav_push">Push — new slot + screenshot old</a>
<a href="ewe://localhost/nav_modal">Modal — new slot like Push</a>
<a href="ewe://localhost/nav_morph">Morph — in-place DOM swap</a>
<a href="ewe://localhost/nav_replace">Replace — swap current slot</a>
<a href="ewe://localhost/nav_root">Root — clear stack + new root</a>
<a href="https://github.com">External — system browser</a>
<h2>Navigation Tests</h2>
<a href="ewe://localhost/app/">Push: WASM App</a>
<a href="ewe://localhost/app-hello/">Push: Hello App</a>
<a href="ewe://localhost/api/invoke">Push: IPC Invoke</a>
<button onclick="history.back()">← Back (pop)</button>
<h2>Stack State</h2>
<pre id="stack" style="font-size:11px">Loading...</pre>
<script>
setInterval(function(){{
    document.getElementById('stack').textContent =
        'Depth: <stack_depth>' + ' · Mode: <last_mode>';
}}, 500);
</script>"##,
        );
        let html = page_html(
            "Presentation Tests",
            &body,
            &nav_buttons("/presentation/"),
            "",
        );
        html_response(html)
    }
}

/// Responder that reports which presentation mode was used.
/// Each mode gets a visually distinct page with a monotonic counter so
/// navigation is observable — a Push that created a new WebView, a Morph
/// that reused one, and a back/pop that returned to the previous slot are
/// all distinguishable by what's shown.
struct ModeReportPage;
impl RouteResponder for ModeReportPage {
    fn respond(
        &self,
        intent: &NavigationIntent,
        decision: &RouteDecision,
        session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        let route = foundation_platform::pattern::extract_path(&intent.url);
        let stack = session.webview_stack();
        let depth = stack.depth();
        let active = stack.active_route().unwrap_or("(none)").to_string();
        drop(stack);

        static VISIT: AtomicU64 = AtomicU64::new(0);
        let tick = VISIT.fetch_add(1, Ordering::Relaxed);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        // Visual identity per mode — same colors as the dashboard buttons
        let (mode_name, emoji, color, bg) = match decision.presentation {
            foundation_ui_traits::Presentation::Push => ("Push", "➕", "#ff79c6", "#1a0a1a"),
            foundation_ui_traits::Presentation::Modal => ("Modal", "🪟", "#bd93f9", "#121024"),
            foundation_ui_traits::Presentation::Morph => ("Morph", "🔄", "#50fa7b", "#0a1a0a"),
            foundation_ui_traits::Presentation::Replace => ("Replace", "🔀", "#ffb86c", "#1a120a"),
            foundation_ui_traits::Presentation::Root => ("Root", "🧹", "#ff5555", "#1a0a0a"),
            _ => ("?", "❓", "#ccc", "#111"),
        };

        let body = format!(
            r##"<h1>{emoji} {mode_name}</h1>
<div style="background:{bg};border:2px solid {color};border-radius:8px;padding:16px;margin:12px 0">
  <p>Route: <b style="color:#64ffda">{route}</b></p>
  <p>Stack depth: <b style="font-size:24px;color:{color}">{depth}</b></p>
  <p>Active: <b>{active}</b></p>
  <p style="color:#8892b0;font-size:11px">render #{tick} · {ts}</p>
</div>
<div style="margin-top:12px">
  <a href="ewe://localhost/nav_push">➕ Push</a>
  <a href="ewe://localhost/nav_morph">🔄 Morph</a>
  <a href="ewe://localhost/nav_replace">🔀 Replace</a>
  <a href="ewe://localhost/nav_root">🧹 Root</a>
</div>
<div style="margin-top:16px">
  <a href="ewe://localhost/presentation/">← Back to Demo Index</a>
</div>"##,
        );
        let html = page_html(
            &format!("{emoji} {mode_name}"),
            &body,
            &nav_buttons(&route),
            "",
        );
        html_response(html)
    }
}

// ── Setup ────────────────────────────────────────────────────────────

fn setup_routes(session: Arc<PlatformSession>) {
    // Each responder mounts at its own app's active version directory and
    // reads through the session's asset manager (F40). On Android that is
    // the only path that reaches APK-bundled assets — they are never
    // extracted to disk, so a plain std::fs read would serve nothing.
    let app_responder = generated::app::AppAssets::build(Arc::clone(&session));

    session.register_route_with(
        "/app/*",
        webview_app().with_profile(Profile::App),
        app_responder,
    );

    // F42: Register native IPC handlers from foundation_platform_native.
    foundation_platform_native::native::modal::register(Arc::clone(&session));

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

    // Platform RemoteProxy — uses session.http_backend() for real HTTP fetch.
    session.register_route_with(
        "/remote/*",
        remote_fetch().with_profile(Profile::TrustedRemote),
        RemoteProxy::new(),
    );

    // API fallback
    session.register_route_with("/api/*", ipc_shell_with("shell"), IpcInvokePage);

    // ── Presentation mode demo routes (F35 validation) ───────────────
    // Each route uses a different Presentation variant so we can verify
    // the WebViewStack behaves correctly for all 6 modes.
    session.register_route_with(
        "/presentation/",
        webview_app().with_profile(Profile::App),
        PresentationDemoPage,
    );
    session.register_route_with(
        "/nav_push",
        webview_app()
            .with_profile(Profile::App)
            .with_presentation(foundation_ui_traits::Presentation::Push),
        ModeReportPage,
    );
    session.register_route_with(
        "/nav_modal",
        webview_app()
            .with_profile(Profile::App)
            .with_presentation(foundation_ui_traits::Presentation::Modal),
        ModeReportPage,
    );
    session.register_route_with(
        "/nav_morph",
        webview_app()
            .with_profile(Profile::App)
            .with_presentation(foundation_ui_traits::Presentation::Morph),
        ModeReportPage,
    );
    session.register_route_with(
        "/nav_replace",
        webview_app()
            .with_profile(Profile::App)
            .with_presentation(foundation_ui_traits::Presentation::Replace),
        ModeReportPage,
    );
    session.register_route_with(
        "/nav_root",
        webview_app()
            .with_profile(Profile::App)
            .with_presentation(foundation_ui_traits::Presentation::Root),
        ModeReportPage,
    );

    println!("[platform_android] Session: {:?}", session.session_id());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[valtron(tracing = "trace")]
pub fn run() {
    platform_run!(PlatformBuilder::new()
        .inject_platform_runtimes()
        // Baked at compile time (F40): the only origin OTA manifests and
        // bundles may come from. No IPC, script, or manifest field can
        // change it — that takes a new binary.
        .ota_manifest_domain("cdn.ewe.studio")
        // The trust anchor. `keys/ota_public.key` is written by build.rs on
        // the first build and committed; the private seed stays in CI.
        .ota_manifest_key_from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/keys/ota_public.key"
        )))
        .setup(setup_routes));
}
