//! HTTP server for web test modes, built on `foundation_http`.
//!
//! WHY: Browser tests need an HTTP server to serve the integration directory,
//! and every suite needs a local endpoint it can call (spec 88 invariant I5).
//! WHAT: Wraps `foundation_http`'s `HttpServer` with `StaticFileHandler` on
//! `/*` and [`EchoHandler`] on `/echo`.
//! HOW: Creates an `HttpApp` with both routes, starts the server on a thread,
//! and provides shutdown via `ShutdownSignal`.

use std::path::Path;
use std::sync::Arc;
use std::thread;

use base64::Engine as _;
use foundation_core::io::readers::DataBytesIterator;
use foundation_http::native::handlers::static_file::StaticFileHandler;
use foundation_http::native::serve::Serve;
use foundation_http::native::writer::{send_json, send_text};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::{
    RawStream, SendSafeBody, SharedByteBufferStream, ShutdownSignal, SimpleIncomingRequest,
};
use serde_json::json;
use tracing::debug;

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Limits of the test server.
///
/// Every field has a builder method and a documented default (owner rule 8).
#[derive(Clone, Debug)]
pub struct ServerLimits {
    /// The largest body the `/echo` route accepts, in bytes. A larger body gets
    /// 413. Default 1 MiB: the order of a default nginx `client_max_body_size`,
    /// and enough for a suite that echoes a request body once.
    pub max_echo_body: usize,
}

impl Default for ServerLimits {
    fn default() -> Self {
        Self {
            max_echo_body: 1024 * 1024,
        }
    }
}

impl ServerLimits {
    /// The default limits. See the field docs for each default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::max_echo_body`].
    #[must_use]
    pub fn with_max_echo_body(mut self, bytes: usize) -> Self {
        self.max_echo_body = bytes;
        self
    }
}

/// Answers `/echo` with the request it received (spec 88 invariant I5).
///
/// The body is a JSON object: `method`, `path`, `headers`, `body` as base64 and
/// `bytes` (the body length). A body over the limit gets 413.
pub struct EchoHandler {
    max_body: usize,
}

impl EchoHandler {
    /// An echo handler that accepts up to `max_body` bytes.
    #[must_use]
    pub fn new(max_body: usize) -> Self {
        Self { max_body }
    }
}

impl Serve for EchoHandler {
    fn serve(
        &self,
        ctx: Ctx,
        request: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> foundation_http::shared::serve::ConnectionResult {
        // The connection reader streams the body, so read it the way the
        // client's own body reader does: one chunk at a time, and stop at the
        // limit instead of buffering an unbounded body.
        let (body, too_large) = match request.body {
            None | Some(SendSafeBody::None | SendSafeBody::Stream(None)) => (Vec::new(), false),
            Some(SendSafeBody::Text(text)) => {
                let bytes = text.into_bytes();
                let too_large = bytes.len() > self.max_body;
                (bytes, too_large)
            }
            Some(SendSafeBody::Bytes(bytes)) => {
                let too_large = bytes.len() > self.max_body;
                (bytes, too_large)
            }
            Some(SendSafeBody::Stream(Some(iter))) => {
                let mut bytes = Vec::new();
                let mut too_large = false;
                for chunk in DataBytesIterator::new(iter) {
                    match chunk {
                        Ok(data) => {
                            bytes.extend_from_slice(&data);
                            if bytes.len() > self.max_body {
                                too_large = true;
                                break;
                            }
                        }
                        Err(e) => {
                            return send_text(
                                conn,
                                400,
                                &format!("echo could not read the body: {e}"),
                                &ctx,
                            );
                        }
                    }
                }
                (bytes, too_large)
            }
            Some(_) => {
                return send_text(
                    conn,
                    400,
                    "echo accepts a body with a length or a byte stream, not a chunked or event stream",
                    &ctx,
                );
            }
        };
        if too_large {
            // The body is still arriving: answer and close, so the unread
            // bytes never become the next request line.
            return match send_text(conn, 413, "echo body too large", &ctx) {
                foundation_http::shared::serve::ConnectionResult::Keep => {
                    foundation_http::shared::serve::ConnectionResult::Close(None)
                }
                other => other,
            };
        }
        let headers: serde_json::Map<String, serde_json::Value> = request
            .headers
            .iter()
            .map(|(name, values)| (name.to_string(), json!(values.join(", "))))
            .collect();
        let response = json!({
            "method": request.method.to_string(),
            "path": request.request_uri.path(),
            "headers": headers,
            "body": base64::engine::general_purpose::STANDARD.encode(&body),
            "bytes": body.len(),
        });
        send_json(conn, 200, &response, &ctx)
    }
}

/// Running test server.
pub struct TestServer {
    port: u16,
    shutdown_signal: Arc<ShutdownSignal>,
    server_thread: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    /// Returns the bound port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the URL for a given path on this server.
    ///
    /// The host is the same as [`Self::base_url`], so a page and the endpoint a
    /// case calls share one origin and the browser needs no cross-origin grant.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url(), path.trim_start_matches('/'))
    }

    /// The origin every suite calls: `http://127.0.0.1:<port>`.
    ///
    /// It uses the loopback address, not `localhost`, so a fetch never waits on
    /// a name lookup.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
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
/// The `/echo` route uses the default limits. Use
/// [`start_serving_with_echo`] to set them.
///
/// # Errors
///
/// Returns an error if no available port can be found.
pub fn start_serving(integration_dir: &Path) -> Result<TestServer> {
    start_serving_with_echo(integration_dir, &ServerLimits::default())
}

/// Start a static file server with the `/echo` route and the given limits.
///
/// The server binds to a random available port on 127.0.0.1. `/echo` answers
/// before `/*`, so a file named `echo` never shadows the route.
///
/// # Errors
///
/// Returns an error if no available port can be found.
pub fn start_serving_with_echo(
    integration_dir: &Path,
    limits: &ServerLimits,
) -> Result<TestServer> {
    let port =
        portpicker::pick_unused_port().ok_or_else(|| WasmTestbedError::NoPortAvailable.trace())?;

    let shutdown = Arc::new(ShutdownSignal::new());
    let shutdown_clone = shutdown.clone();
    let root = integration_dir.to_path_buf();
    let max_echo_body = limits.max_echo_body;
    let addr = format!("127.0.0.1:{port}");

    let handle = thread::spawn(move || {
        let mut app = HttpApp::new_serve();
        let echo: Arc<dyn Serve> = Arc::new(EchoHandler::new(max_echo_body));
        app.router.add_route_any("/echo", &echo);
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
