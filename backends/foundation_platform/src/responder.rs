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
