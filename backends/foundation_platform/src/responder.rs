//! Route responders — the concrete handlers behind each RouteSource.
//!
//! Each responder owns the logic for its response type. When a route
//! resolves, `execute_decision()` looks up the registered responder
//! and calls `respond()` with `NavigationIntent`, `RouteDecision`,
//! and `PlatformSession`. The responder returns a `tauri::http::Response`.

use foundation_ui_traits::*;
use tauri::http::{header, Response, StatusCode};

use crate::pattern;
use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

/// WASM app responder — serves static files from a directory on disk.
/// `/app/` → `index.html`, `/app/foo.wasm` → `foo.wasm`.
/// Sets correct Content-Type for .wasm, .js, .html files.
pub struct WebviewApp {
    pub asset_dir: std::path::PathBuf,
}

impl WebviewApp {
    #[must_use]
    pub fn new(asset_dir: impl Into<std::path::PathBuf>) -> Self {
        Self { asset_dir: asset_dir.into() }
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
        let file = if std::path::Path::new(&path).extension().is_some() { path } else { "index.html".to_string() };
        let ct = match std::path::Path::new(&file).extension().and_then(|e| e.to_str()) {
            Some("wasm") => "application/wasm",
            Some("js") => "application/javascript",
            _ => "text/html; charset=utf-8",
        };
        match std::fs::read(self.asset_dir.join(&file)) {
            Ok(data) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, ct)
                .body(data).unwrap(),
            Err(_) => {
                let body = format!("Not Found: {file}").into_bytes();
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(body).unwrap()
            }
        }
    }
}
