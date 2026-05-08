//! Static file handler — serves files from a directory with path traversal prevention.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::{
    Http11, RenderHttp, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse,
    SendSafeBody, Status,
};

use crate::context::ContextBag;
use crate::serve::{ConnectionResult, Serve, ServeFactory};

/// MIME type detection based on file extension.
fn mime_type(path: &Path) -> &str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") => "text/plain",
        Some("xml") => "application/xml",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Handler that serves static files from a root directory.
pub struct StaticFileHandler {
    root: PathBuf,
}

impl StaticFileHandler {
    /// Create a new `StaticFileHandler` serving from the given root directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn serve_file(&self, path: &str, conn: &mut SharedByteBufferStream<RawStream>) -> ConnectionResult {
        // Prevent path traversal
        let safe_path = path.trim_start_matches('/');
        if safe_path.contains("..") {
            return crate::serve::respond::text(conn, 403, "Forbidden")
                .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
        }

        let file_path = self.root.join(safe_path);

        // Verify the resolved path is still under root
        if !file_path.starts_with(&self.root) {
            return crate::serve::respond::text(conn, 403, "Forbidden")
                .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
        }

        // Check if it's a directory — try index.html
        if file_path.is_dir() {
            let index_path = file_path.join("index.html");
            return Self::serve_file_path(&index_path, conn);
        }

        Self::serve_file_path(&file_path, conn)
    }

    fn serve_file_path(path: &Path, conn: &mut SharedByteBufferStream<RawStream>) -> ConnectionResult {
        let Ok(mut file) = File::open(path) else {
            return crate::serve::respond::not_found(conn)
                .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
        };

        let mut contents = Vec::new();
        if let Err(e) = file.read_to_end(&mut contents) {
            tracing::error!("Failed to read static file {path:?}: {e}");
            return crate::serve::respond::server_error(conn, None)
                .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
        }

        let ct = mime_type(path).to_string();
        let response = SimpleOutgoingResponse::builder()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_TYPE, ct)
            .with_body(SendSafeBody::Bytes(contents))
            .build()
            .expect("valid response");

        match Http11::response(response).http_render_to_writer(conn) {
            Ok(_) => ConnectionResult::Keep,
            Err(e) => {
                tracing::error!("Failed to render static file response: {e}");
                ConnectionResult::Close(None)
            }
        }
    }
}

impl ServeFactory for StaticFileHandler {
    fn create(_bag: &ContextBag) -> Self {
        panic!("StaticFileHandler requires a root directory — use StaticFileHandler::new() and register manually");
    }
}

impl Serve for StaticFileHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let full_url = &req.request_url.url;
        let path = full_url.split('?').next().unwrap_or(full_url);
        self.serve_file(path, &mut conn)
    }
}
