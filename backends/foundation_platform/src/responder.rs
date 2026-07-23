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
///
/// When `mount` is set (F40), the responder knows which app it serves, so its
/// reads go through the session's [`PlatformAssetManager`]. On Android that is
/// the only way to reach bundled bytes — they are never on disk, so
/// `std::fs::read()` sees nothing.
///
/// The route-relative path comes from `RouteDecision::sub_path`, which the
/// router already computed when it matched the pattern. Re-deriving it here
/// would be a second implementation of prefix matching that can disagree with
/// the first: a bare `strip_prefix("app")` also fires on `app-hello`, quietly
/// handing one app's request to another. The router matches whole segments
/// against a literal pattern, so it cannot make that mistake — and there is
/// no reason for two places to know where a route's prefix ends.
pub struct MobileApp<A: MobileDisk + Send + Sync + 'static> {
    assets: A,
    /// The app id this responder serves, when it is mounted at one.
    mount: Option<String>,
}

impl<A: EmbeddableDirectory + Send + Sync + 'static> WebviewApp<A> {
    pub fn new(assets: A) -> Self { Self { assets } }
}

impl<A: MobileDisk + Send + Sync + 'static> MobileApp<A> {
    /// Serve `assets` with request paths resolved verbatim against their root.
    pub fn new(assets: A) -> Self { Self { assets, mount: None } }

    /// Serve `assets` as the app `app_id` (F40).
    ///
    /// `app_id` is the key used to resolve the app's active version through
    /// the session's asset manager. Where the route's prefix ends is the
    /// router's business, not this responder's — see `RouteDecision::sub_path`.
    pub fn mounted_at(assets: A, app_id: impl Into<String>) -> Self {
        Self { assets, mount: Some(app_id.into()) }
    }
}

/// Where a `MobileApp` reads its bytes from.
///
/// The manager path and the disk path resolve *different* namespaces — VFS
/// paths relative to an app's active version versus filesystem paths under
/// the responder's own root — so probing goes through one seam rather than
/// being inlined at each of the four call sites below.
struct AssetReader<'a, A: MobileDisk> {
    assets: &'a A,
    mount: Option<&'a str>,
    manager: Option<std::sync::Arc<crate::assets::PlatformAssetManager>>,
}

impl<A: MobileDisk> AssetReader<'_, A> {
    fn read(&self, target: &str) -> Option<Vec<u8>> {
        match (&self.manager, self.mount) {
            // Mounted with a manager: the VFS resolves delta (OTA'd) over
            // base (bundle), which is what makes Android work at all.
            (Some(manager), Some(app_id)) => manager.read_app_file(app_id, target).ok(),
            // No manager: plain disk under the responder's own root.
            _ => self.assets.read_utf8_for(target),
        }
    }
}

fn serve_response<A: MobileDisk>(
    reader: &AssetReader<'_, A>,
    intent: &NavigationIntent,
    decision: &RouteDecision,
) -> Response<Vec<u8>> {
    // The router already worked out where this route's prefix ends when it
    // matched the pattern; `sub_path` is that answer, already normalised —
    // no leading or trailing slash, and empty when the request is for the
    // route root. Only fall back to the raw URL when there is none: an
    // unmounted responder, or one invoked outside the router. Normalise that
    // the same way so one set of rules covers both.
    let path = match (&decision.sub_path, reader.mount) {
        (Some(sub), Some(_)) => sub.clone(),
        _ => pattern::extract_path(&intent.url).trim_matches('/').to_string(),
    };

    let file = if path.is_empty() {
        // Nothing left after the prefix: this is the route root, so serve the
        // entry point. No separate trailing-slash case — `/app` and `/app/`
        // both normalise to the same empty remainder.
        "index.html".to_string()

    // A concrete file with an extension — serve exactly that, or 404. Falling
    // back to index.html here would hand a `.wasm` request an HTML body.
    } else if Path::new(&path).extension().is_some() {
        path.clone()

    // Try {path} as a file, then {path}/index.html, then the entry point
    // (SPA client-side routing).
    } else if reader.read(&path).is_some() {
        path.clone()
    } else {
        let spa_index = format!("{path}/index.html");
        if reader.read(&spa_index).is_some() {
            spa_index
        } else if reader.mount.is_some() {
            // Already inside one app — its index is the SPA entry point.
            "index.html".to_string()
        } else {
            // Unmounted: the first segment names the app, pre-F40 style.
            let root = path.split('/').next().unwrap_or("");
            format!("{root}/index.html")
        }
    };
    let ct = match Path::new(&file).extension().and_then(|e| e.to_str()) {
        Some("wasm") => "application/wasm",
        Some("js") => "application/javascript",
        _ => "text/html; charset=utf-8",
    };
    match reader.read(&file) {
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
    fn respond(&self, intent: &NavigationIntent, decision: &RouteDecision, session: &PlatformSession) -> Response<Vec<u8>> {
        let reader = AssetReader {
            assets: &self.assets,
            mount: self.mount.as_deref(),
            manager: session.asset_manager(),
        };
        serve_response(&reader, intent, decision)
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

        // Single-pass srcdoc-safe escape: encode only the characters that
        // would break the attribute value.
        let srcdoc_safe = String::from_utf8_lossy(body)
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
body{{font-family:system-ui,sans-serif;background:#fff;color:#333;min-height:100vh}}
#iframe-container{{width:100%;height:100vh;border:none;overflow:hidden}}
iframe{{width:100%;height:100%;border:none}}
</style>
<script>{interceptor}</script>
</head><body>
<div id="iframe-container"><iframe sandbox="allow-scripts allow-same-origin allow-forms" srcdoc="{srcdoc_safe}"></iframe></div>
<script>{floating_nav}</script>
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
