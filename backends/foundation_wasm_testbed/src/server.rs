//! Minimal static file HTTP server for web test modes.
//!
//! WHY: Browser tests need an HTTP server to serve the integration directory.
//! WHAT: A simple static file server using tiny_http.
//! HOW: Binds to a random port, serves files from the integration directory
//!      with path traversal prevention and correct MIME types.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use tiny_http::{Header, Request, Response, Server, StatusCode};
use tracing::{debug, info};

/// MIME type detection based on file extension.
fn mime_type(path: &Path) -> &str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("js" | "mjs") => "application/javascript",
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
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Running test server.
pub struct TestServer {
    port: u16,
    shutdown_signal: Arc<std::sync::atomic::AtomicBool>,
    server_thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    /// Returns the bound port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the URL for a given path on this server.
    pub fn url(&self, path: &str) -> String {
        format!("http://localhost:{}/{}", self.port, path.trim_start_matches('/'))
    }

    /// Signal the server to stop and wait for the thread to finish.
    pub fn shutdown(mut self) {
        self.shutdown_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(handle) = self.server_thread.take() {
            let _ = handle.join();
        }
        debug!("Test server shut down");
    }
}

/// Start a static file server serving the integration directory.
///
/// The server binds to a random available port on 127.0.0.1.
///
/// # Errors
///
/// Returns an error if no available port can be found or the server fails to start.
pub fn start_serving(integration_dir: &Path) -> anyhow::Result<TestServer> {
    let port = portpicker::pick_unused_port()
        .ok_or_else(|| anyhow::anyhow!("No available port found"))?;

    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let root = integration_dir.to_path_buf();

    let handle = thread::spawn(move || {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
        let server = match Server::http(addr) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to start HTTP server on {addr}: {e}");
                return;
            }
        };

        info!("Test server listening on http://127.0.0.1:{port}");

        for request in server.incoming_requests() {
            if shutdown_clone.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            match handle_request(request, &root) {
                Ok(()) => {}
                Err(e) => {
                    tracing::error!("Error handling request: {e}");
                }
            }
        }

        info!("Test server stopped");
    });

    Ok(TestServer {
        port,
        shutdown_signal: shutdown,
        server_thread: Some(handle),
    })
}

/// Handle a single HTTP request.
fn handle_request(request: Request, root: &Path) -> anyhow::Result<()> {
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url);

    // Prevent path traversal
    let safe_path = path.trim_start_matches('/');
    if safe_path.contains("..") {
        let response = Response::from_string("Forbidden")
            .with_status_code(StatusCode(403));
        let _ = request.respond(response);
        return Ok(());
    }

    let file_path = root.join(safe_path);

    // Verify resolved path is still under root
    if !file_path.starts_with(root) {
        let response = Response::from_string("Forbidden")
            .with_status_code(StatusCode(403));
        let _ = request.respond(response);
        return Ok(());
    }

    // If directory, try index.html
    let file_path = if file_path.is_dir() {
        file_path.join("index.html")
    } else {
        file_path
    };

    // Read file
    let content = match std::fs::read(&file_path) {
        Ok(data) => data,
        Err(_) => {
            let response = Response::from_string("Not Found")
                .with_status_code(StatusCode(404));
            let _ = request.respond(response);
            return Ok(());
        }
    };

    let ct = mime_type(&file_path);

    let response = Response::from_data(content)
        .with_status_code(StatusCode(200))
        .with_header(Header::from_bytes("Content-Type", ct).expect("valid Content-Type header"));

    let _ = request.respond(response);
    debug!("Served {} ({})", safe_path, ct);
    Ok(())
}
