//! Route responders — the concrete handlers behind each RouteSource.
//!
//! The platform provides the `RouteResponder` trait and `WebviewApp`.
//! Users implement `IpcShell` and `RemoteServer` responders for their app.
//! Each route declaration carries its handler via `route_with()`.

use std::path::Path;

use foundation_ui_traits::*;
use tauri::http::{header, Response, StatusCode};

use crate::pattern;
use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

/// WASM app responder — serves static files from an asset directory.
/// Reads `index.html` for bare paths, `.wasm`, `.js` with correct Content-Type.
///
/// ```ignore
/// #[derive(EmbedDirectoryAs)]
/// #[source = "public/app"]
/// struct AppAssets;
///
/// builder.route_with("/app/*", webview_app(),
///     WebviewApp::new(Box::new(AppAssets)));
/// ```
pub struct WebviewApp {
    assets: Box<dyn AssetReader>,
}

/// Internal trait — erases the `EmbeddableDirectory` concrete type
/// behind a `Send + Sync` boundary so `WebviewApp` satisfies
/// `RouteResponder: Send + Sync`.
trait AssetReader: Send + Sync {
    fn read(&self, file: &str) -> Option<Vec<u8>>;
}

/// Blanket impl: any `EmbeddableDirectory` that is `Send + Sync` works.
impl<T> AssetReader for T
where
    T: foundation_nostd::embeddable::EmbeddableDirectory + Send + Sync,
{
    fn read(&self, file: &str) -> Option<Vec<u8>> {
        self.read_utf8_for(file)
    }
}

impl WebviewApp {
    pub fn new(assets: impl foundation_nostd::embeddable::EmbeddableDirectory + Send + Sync + 'static) -> Self {
        Self { assets: Box::new(assets) }
    }
}

impl RouteResponder for WebviewApp {
    fn respond(
        &self,
        intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> Response<Vec<u8>> {
        let path = pattern::extract_path(&intent.url).trim_start_matches('/').to_string();
        let file = if Path::new(&path).extension().is_some() { path } else { "index.html".to_string() };
        let ct = match Path::new(&file).extension().and_then(|e| e.to_str()) {
            Some("wasm") => "application/wasm",
            Some("js") => "application/javascript",
            _ => "text/html; charset=utf-8",
        };
        match self.assets.read(&file) {
            Some(data) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, ct)
                .body(data).unwrap(),
            None => {
                let body = format!("Not Found: {file}").into_bytes();
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(body).unwrap()
            }
        }
    }
}
