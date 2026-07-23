//! Embedded WASM app assets for Android (F22 fallback).
//!
//! On Android, Tauri does NOT extract `bundle.resources` to the filesystem.
//! We embed the WASM app files at compile time and serve them directly
//! from the `ewe://` protocol handler.

use foundation_platform::pattern;
use foundation_platform::route_handler::RouteResponder;
use foundation_ui_traits::{NavigationIntent, RouteDecision};
use foundation_platform::PlatformSession;
use tauri::http::{header, Response, StatusCode};
use std::path::Path;

/// A `RouteResponder` that serves files from compile-time-embedded bytes.
pub struct EmbeddedAppResponder {
    /// Relative file path → bytes.
    files: &'static [(&'static str, &'static [u8])],
}

impl EmbeddedAppResponder {
    pub const fn new(files: &'static [(&'static str, &'static [u8])]) -> Self {
        Self { files }
    }
}

impl RouteResponder for EmbeddedAppResponder {
    fn respond(
        &self,
        intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> Response<Vec<u8>> {
        let raw_path = pattern::extract_path(&intent.url).trim_start_matches('/').to_string();

        // Resolve to a concrete file.
        let lookup = if Path::new(&raw_path).extension().is_some() {
            // Concrete file with extension: strip the app prefix segment
            // e.g. "app/bundle.js" → "bundle.js"
            let parts: Vec<&str> = raw_path.split('/').filter(|s| !s.is_empty()).collect();
            if parts.len() > 1 { parts[1..].join("/") } else { raw_path.clone() }
        } else if raw_path.ends_with('/') || !raw_path.contains('.') {
            // Directory or bare app name → serve index.html
            "index.html".to_string()
        } else {
            raw_path.clone()
        };

        // Search embedded files.
        for (name, data) in self.files {
            if *name == lookup || format!("/{}", name) == raw_path || *name == raw_path {
                let ct = match Path::new(name).extension().and_then(|e| e.to_str()) {
                    Some("wasm") => "application/wasm",
                    Some("js") => "application/javascript; charset=utf-8",
                    _ => "text/html; charset=utf-8",
                };
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, ct)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(data.to_vec())
                    .unwrap();
            }
        }

        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(format!("Not Found: {lookup}").into_bytes())
            .unwrap()
    }
}

// ── Embedded app assets ──────────────────────────────────────────────

static APP_FILES: &[(&str, &[u8])] = &[
    ("index.html", include_bytes!("../public/app/index.html")),
    ("bundle.js", include_bytes!("../public/app/bundle.js")),
    ("platform_dashboard.js", include_bytes!("../public/app/platform_dashboard.js")),
    ("platform_dashboard.wasm", include_bytes!("../public/app/platform_dashboard.wasm")),
];

static APP_HELLO_FILES: &[(&str, &[u8])] = &[
    ("index.html", include_bytes!("../public/app-hello/index.html")),
    ("bundle.js", include_bytes!("../public/app-hello/bundle.js")),
    ("hello_dashboard.js", include_bytes!("../public/app-hello/hello_dashboard.js")),
    ("hello_dashboard.wasm", include_bytes!("../public/app-hello/hello_dashboard.wasm")),
];

pub fn embedded_app() -> EmbeddedAppResponder {
    EmbeddedAppResponder::new(APP_FILES)
}

pub fn embedded_app_hello() -> EmbeddedAppResponder {
    EmbeddedAppResponder::new(APP_HELLO_FILES)
}
