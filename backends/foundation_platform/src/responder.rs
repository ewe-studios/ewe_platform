use std::path::Path;

use foundation_nostd::embeddable::EmbeddableDirectory;
use foundation_ui_traits::*;
use tauri::http::{header, Response, StatusCode};

use crate::pattern;
use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

pub struct WebviewApp<A: EmbeddableDirectory + Send + Sync + 'static> {
    assets: A,
}

impl<A: EmbeddableDirectory + Send + Sync + 'static> WebviewApp<A> {
    pub fn new(assets: A) -> Self { Self { assets } }
}

impl<A: EmbeddableDirectory + Send + Sync + 'static> RouteResponder for WebviewApp<A> {
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
        match self.assets.read_utf8_for(&file) {
            Some(data) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, ct).body(data).unwrap(),
            None => Response::builder().status(StatusCode::NOT_FOUND).header(header::CONTENT_TYPE, "text/plain")
                .body(format!("Not Found: {file}").into_bytes()).unwrap(),
        }
    }
}
