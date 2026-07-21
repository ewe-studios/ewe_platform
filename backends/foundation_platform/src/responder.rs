use std::path::{Path, PathBuf};

use foundation_nostd::embeddable::EmbeddableDirectory;
use foundation_nostd::mobile::MobileDirectory;
use foundation_ui_traits::{NavigationIntent, RouteDecision};
use tauri::http::{header, Response, StatusCode};

use crate::pattern;
use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

/// Extension trait adding disk-backed file reads to `MobileDirectory`.
/// Lives here (not in `foundation_nostd`) because `std::fs` requires `std`.
pub trait MobileDisk: MobileDirectory {
    fn root_path(&self) -> PathBuf {
        PathBuf::from(self.root_str())
    }

    fn read_utf8_for(&self, target: &str) -> Option<Vec<u8>> {
        let location = self.root_path().join(target);
        if !location.exists() || location.is_dir() {
            return None;
        }
        std::fs::read(location).ok()
    }
}

// Blanket impl: every MobileDirectory automatically gets MobileDisk.
impl<T: MobileDirectory + ?Sized> MobileDisk for T {}

/// Embedded asset responder — uses `EmbedDirectoryAs` (compile-time bytes).
pub struct WebviewApp<A: EmbeddableDirectory + Send + Sync + 'static> {
    assets: A,
}

/// Mobile asset responder — uses `MobileDirectory` + `MobileDisk` (F22).
pub struct MobileApp<A: MobileDisk + Send + Sync + 'static> {
    assets: A,
}

impl<A: EmbeddableDirectory + Send + Sync + 'static> WebviewApp<A> {
    pub fn new(assets: A) -> Self { Self { assets } }
}

impl<A: MobileDisk + Send + Sync + 'static> MobileApp<A> {
    pub fn new(assets: A) -> Self { Self { assets } }
}

fn serve_response(assets: &impl MobileDisk, intent: &NavigationIntent) -> Response<Vec<u8>> {
    let path = pattern::extract_path(&intent.url).trim_start_matches('/').to_string();
    let file = if Path::new(&path).extension().is_some() { path } else { "index.html".to_string() };
    let ct = match Path::new(&file).extension().and_then(|e| e.to_str()) {
        Some("wasm") => "application/wasm",
        Some("js") => "application/javascript",
        _ => "text/html; charset=utf-8",
    };
    match assets.read_utf8_for(&file) {
        Some(data) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, ct).body(data).unwrap(),
        None => Response::builder().status(StatusCode::NOT_FOUND).header(header::CONTENT_TYPE, "text/plain")
            .body(format!("Not Found: {file}").into_bytes()).unwrap(),
    }
}

impl<A: EmbeddableDirectory + Send + Sync + 'static> RouteResponder for WebviewApp<A> {
    fn respond(&self, intent: &NavigationIntent, _decision: &RouteDecision, _session: &PlatformSession) -> Response<Vec<u8>> {
        let path = pattern::extract_path(&intent.url).trim_start_matches('/').to_string();
        let file = if Path::new(&path).extension().is_some() { path } else { "index.html".to_string() };
        let ct = match Path::new(&file).extension().and_then(|e| e.to_str()) {
            Some("wasm") => "application/wasm",
            Some("js") => "application/javascript",
            _ => "text/html; charset=utf-8",
        };
        match self.assets.read_utf8_for(&file) {
            Some(data) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, ct).body(data).unwrap(),
            None => Response::builder().status(StatusCode::NOT_FOUND).header(header::CONTENT_TYPE, "text/plain")
                .body(format!("Not Found: {file}").into_bytes()).unwrap(),
        }
    }
}

impl<A: MobileDisk + Send + Sync + 'static> RouteResponder for MobileApp<A> {
    fn respond(&self, intent: &NavigationIntent, _decision: &RouteDecision, _session: &PlatformSession) -> Response<Vec<u8>> {
        serve_response(&self.assets, intent)
    }
}

// ── RemoteProxy (F29 Stage 5) ──────────────────────────────────────────

/// A `RouteResponder` that fetches remote URLs and wraps them in an iframe
/// page with the floating navigation toolbar.
///
/// Usage:
/// ```ignore
/// session.register_route_with(
///     "/remote/*",
///     remote_fetch().with_profile(Profile::TrustedRemote),
///     RemoteProxy::new(),
/// );
/// ```
///
/// The response is an HTML page containing:
/// - The floating-nav.js toolbar (back, home, refresh)
/// - The scheme interceptor (catches `ewe://` links inside the iframe)
/// - A full-height iframe with the remote content (sandboxed)
pub struct RemoteProxy;

impl RemoteProxy {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build the wrapper HTML page with floating nav, scheme interceptor,
    /// and a sandboxed iframe containing the remote content.
    fn wrap_remote(path: &str, _remote_url: &str, body: &[u8], _content_type: &str) -> String {
        let interceptor = foundation_wasm_ui::embedded::PLATFORM_SCHEME_INTERCEPTOR_JS;
        let floating_nav = foundation_wasm_ui::embedded::FLOATING_NAV_JS;

        // Try to decode as UTF-8 for srcdoc; fall back to a plain message.
        let safe_body = String::from_utf8_lossy(body);
        let escaped = html_escape(&safe_body);
        // Escape the escaped content for safe embedding in srcdoc attribute.
        let srcdoc_safe = escaped
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        format!(
            r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Remote: {path}</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:system-ui,sans-serif;background:#0a0a1a;color:#ccd6f6;min-height:100vh}}
#iframe-container{{width:100%;height:calc(100vh - 52px);border:none;overflow:hidden}}
iframe{{width:100%;height:100%;border:none}}
</style>
<script>{interceptor}</script>
<script>{floating_nav}</script>
</head><body>
<div id="iframe-container"><iframe sandbox="allow-scripts allow-same-origin allow-forms" srcdoc="{srcdoc_safe}"></iframe></div>
</body></html>"#,
        )
    }
}

impl Default for RemoteProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteResponder for RemoteProxy {
    fn respond(
        &self,
        intent: &NavigationIntent,
        _decision: &RouteDecision,
        session: &PlatformSession,
    ) -> Response<Vec<u8>> {
        let path = crate::pattern::extract_path(&intent.url);

        // Extract the remote target from the route: /remote/{url}
        let remote_path = path
            .strip_prefix("/remote/")
            .or_else(|| path.strip_prefix("remote/"))
            .unwrap_or(&path);

        let remote_url = if remote_path.starts_with("http") {
            remote_path.to_string()
        } else {
            format!("https://{remote_path}")
        };

        // Fetch via the session's shared HTTP backend.
        match session.http_backend().fetch(&remote_url) {
            Ok((body, content_type)) => {
                let html = Self::wrap_remote(&path, &remote_url, &body, &content_type);
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(html.into_bytes())
                    .unwrap()
            }
            Err(e) => {
                let body = format!(
                    "<!DOCTYPE html><html><head><meta charset=utf-8><title>Remote Error</title></head>\
                     <body style='font-family:sans-serif;padding:20px;background:#0a0a1a;color:#ccd6f6'>\
                     <h1>Remote Fetch Error</h1><pre>{e}</pre></body></html>"
                );
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(body.into_bytes())
                    .unwrap()
            }
        }
    }
}

/// Minimal HTML escaper for safe embedding in attributes.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
