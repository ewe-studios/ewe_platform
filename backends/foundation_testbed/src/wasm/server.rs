//! HTTP server for web test modes, built on `foundation_http`.
//!
//! WHY: Browser tests need an HTTP server to serve the integration directory.
//! WHAT: Wraps `foundation_http`'s `HttpServer` with `StaticFileHandler`.
//! HOW: Creates an `HttpApp` with `StaticFileHandler` registered on `/*`,
//!      starts the server on a thread, and provides shutdown via `OnSignal`.

use std::path::Path;
use std::sync::Arc;
use std::thread;

use foundation_core::synca::OnSignal;
use foundation_http::native::handlers::static_file::StaticFileHandler;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use tracing::debug;

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Running test server.
pub struct TestServer {
    port: u16,
    shutdown_signal: Arc<OnSignal>,
    server_thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    /// Returns the bound port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the URL for a given path on this server.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("http://localhost:{}/{}", self.port, path.trim_start_matches('/'))
    }

    /// Signal the server to stop and wait for the thread to finish.
    pub fn shutdown(mut self) {
        self.shutdown_signal.turn_on();
        if let Some(handle) = self.server_thread.take() {
            let _ = handle.join();
        }
        debug!("Test server shut down");
    }
}

/// Start a static file server serving the integration directory.
///
/// The server binds to a random available port on 127.0.0.1 using
/// `foundation_http`'s `HttpServer` with a `StaticFileHandler`.
///
/// # Errors
///
/// Returns an error if no available port can be found.
pub fn start_serving(integration_dir: &Path) -> Result<TestServer> {
    let port = portpicker::pick_unused_port()
        .ok_or_else(|| WasmTestbedError::NoPortAvailable.trace())?;

    let shutdown = Arc::new(OnSignal::new());
    let shutdown_clone = shutdown.clone();
    let root = integration_dir.to_path_buf();
    let addr = format!("127.0.0.1:{port}");

    let handle = thread::spawn(move || {
        let mut app = HttpApp::new_serve();
        let handler: Arc<dyn Serve> = Arc::new(StaticFileHandler::new(&root));
        app.router.add_route_any("/*", &handler);

        let server = app.server(&addr);
        server.serve(&shutdown_clone);
    });

    tracing::info!("Test server listening on http://127.0.0.1:{port}");

    Ok(TestServer {
        port,
        shutdown_signal: shutdown,
        server_thread: Some(handle),
    })
}
