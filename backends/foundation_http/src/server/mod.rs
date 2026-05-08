//! `HttpServer` — TCP accept loop with `BackgroundJobRegistry` thread pool.
//!
//! Supports both plain TCP and TLS (via `ssl-rustls` or `ssl-openssl` feature flags).

use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::BackgroundJobRegistry;
use foundation_core::wire::simple_http::{
    Http11, RenderHttp, SimpleOutgoingResponse, HTTPStreams,
};

#[cfg(any(feature = "tls", feature = "openssl-tls"))]
use foundation_core::netcap::ssl::SSLAcceptor;
#[cfg(any(feature = "tls", feature = "openssl-tls"))]
use foundation_core::netcap::Connection;

use crate::app::HttpApp;
use crate::middleware::MiddlewareResult;
use crate::reader::read_next_request;
use crate::serve::{ConnectionResult, respond};

/// Configuration for the HTTP server's accept loop timing.
///
/// WHY: All durations are configurable — no hardcoded values.
/// WHAT: `ServerConfig::default()` provides sane defaults; users can
/// override individual fields via builder-style methods.
pub struct ServerConfig {
    /// Sleep duration when accept returns `WouldBlock` (default: 10ms).
    pub would_block_sleep: Duration,
    /// Sleep duration after an accept error (default: 100ms).
    pub accept_error_sleep: Duration,
    /// Keep-alive timeout for idle connections. If not set, connections
    /// block indefinitely waiting for the next request.
    pub keep_alive_timeout: Option<Duration>,
    /// Maximum body size in bytes (default: 10 MB).
    pub max_body_bytes: usize,
    /// TLS acceptor for encrypted connections.
    #[cfg(any(feature = "tls", feature = "openssl-tls"))]
    pub tls_acceptor: Option<Arc<SSLAcceptor>>,
}

impl ServerConfig {
    /// Create a config with default values.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            would_block_sleep: Duration::from_millis(10),
            accept_error_sleep: Duration::from_millis(100),
            keep_alive_timeout: None,
            max_body_bytes: 10 * 1024 * 1024, // 10 MB
            #[cfg(any(feature = "tls", feature = "openssl-tls"))]
            tls_acceptor: None,
        }
    }

    /// Set the `WouldBlock` sleep duration.
    #[must_use]
    pub fn with_would_block_sleep(mut self, dur: Duration) -> Self {
        self.would_block_sleep = dur;
        self
    }

    /// Set the accept error sleep duration.
    #[must_use]
    pub fn with_accept_error_sleep(mut self, dur: Duration) -> Self {
        self.accept_error_sleep = dur;
        self
    }

    /// Set the keep-alive timeout for idle connections.
    #[must_use]
    pub fn with_keep_alive_timeout(mut self, timeout: Duration) -> Self {
        self.keep_alive_timeout = Some(timeout);
        self
    }

    /// Set the maximum body size in bytes.
    #[must_use]
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// Set the TLS acceptor for encrypted connections.
    #[cfg(any(feature = "tls", feature = "openssl-tls"))]
    #[must_use]
    pub fn with_tls(mut self, acceptor: Arc<SSLAcceptor>) -> Self {
        self.tls_acceptor = Some(acceptor);
        self
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Running HTTP server.
pub struct HttpServer {
    app: Arc<HttpApp>,
    bg: Arc<BackgroundJobRegistry>,
    bind_addr: String,
    config: ServerConfig,
}

impl HttpServer {
    /// Create a new HttpServer with default config.
    #[must_use]
    pub fn new(app: HttpApp, bg: Arc<BackgroundJobRegistry>, addr: &str) -> Self {
        Self::with_config(app, bg, addr, ServerConfig::default())
    }

    /// Create a new HttpServer with custom config.
    #[must_use]
    pub fn with_config(app: HttpApp, bg: Arc<BackgroundJobRegistry>, addr: &str, config: ServerConfig) -> Self {
        Self {
            app: Arc::new(app),
            bg,
            bind_addr: addr.to_string(),
            config,
        }
    }

    /// Start serving plain HTTP. Blocks until the shutdown signal is triggered.
    pub fn serve(self, shutdown: Arc<OnSignal>) {
        let listener = match std::net::TcpListener::bind(&self.bind_addr) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind to {}: {}", self.bind_addr, e);
                return;
            }
        };

        tracing::info!("Listening on {}", self.bind_addr);
        listener.set_nonblocking(true).expect("Failed to set non-blocking");

        self.serve_loop(listener, shutdown, |tcp| {
            // No TLS handshake — use plain connection directly.
            RawStream::from_tcp(tcp).map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Start serving HTTPS. Blocks until the shutdown signal is triggered.
    ///
    /// Requires a TLS feature flag (`tls` or `openssl-tls`) and a `tls_acceptor`
    /// configured in `ServerConfig`.
    #[cfg(any(feature = "tls", feature = "openssl-tls"))]
    pub fn serve_tls(self, shutdown: Arc<OnSignal>) {
        let acceptor = match &self.config.tls_acceptor {
            Some(a) => a.clone(),
            None => {
                tracing::error!("TLS acceptor not configured — call ServerConfig::with_tls()");
                return;
            }
        };

        let listener = match std::net::TcpListener::bind(&self.bind_addr) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind to {}: {}", self.bind_addr, e);
                return;
            }
        };

        tracing::info!("Listening on {} (TLS)", self.bind_addr);
        listener.set_nonblocking(true).expect("Failed to set non-blocking");

        self.serve_loop(listener, shutdown, move |tcp| {
            let conn = Connection::from(tcp);
            let tls_stream = acceptor
                .accept(conn)
                .map_err(|e| format!("TLS handshake failed: {e}"))?;
            RawStream::from_server_tls(tls_stream).map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Generic accept loop — shared by `serve` and `serve_tls`.
    fn serve_loop(
        self,
        listener: std::net::TcpListener,
        shutdown: Arc<OnSignal>,
        wrap_stream: impl Fn(std::net::TcpStream) -> Result<RawStream, String> + Send + Sync + 'static,
    ) {
        let wrap_stream = Arc::new(wrap_stream);
        let would_block_sleep = self.config.would_block_sleep;
        let accept_error_sleep = self.config.accept_error_sleep;

        loop {
            if shutdown.probe() {
                tracing::info!("Shutdown signal received, stopping accept loop");
                break;
            }

            match listener.accept() {
                Ok((tcp, addr)) => {
                    // Set keep-alive read timeout if configured.
                    if let Some(ka) = self.config.keep_alive_timeout {
                        let _ = tcp.set_read_timeout(Some(ka));
                    }

                    let wrap_clone = wrap_stream.clone();
                    let app_clone = self.app.clone();
                    let bg_clone = self.bg.clone();
                    let shutdown_clone = shutdown.clone();

                    bg_clone.submit(move || {
                        // Perform TLS handshake (if any) on the worker thread.
                        let raw_stream = match wrap_clone(tcp) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!("Connection setup failed: {e}");
                                return;
                            }
                        };

                        let client_ip = addr.ip().to_string();
                        let shared_stream = SharedByteBufferStream::rwrite(raw_stream);
                        let streams = HTTPStreams::new(shared_stream.clone());

                        handle_connection(&app_clone, streams, shared_stream, &client_ip, &shutdown_clone);
                    }).unwrap_or_else(|e| {
                        tracing::error!("Failed to submit connection to worker pool: {e}");
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(would_block_sleep);
                }
                Err(e) => {
                    tracing::error!("Accept error: {e}");
                    std::thread::sleep(accept_error_sleep);
                }
            }
        }

        tracing::info!("Server stopped");
    }
}

/// Handle a single TCP connection.
fn handle_connection(
    app: &HttpApp,
    streams: HTTPStreams<RawStream>,
    conn: SharedByteBufferStream<RawStream>,
    client_ip: &str,
    shutdown: &OnSignal,
) {
    let bag = app.context().clone();

    loop {
        if shutdown.probe() {
            break;
        }

        let mut req = match read_next_request(&streams, client_ip) {
            Some(Ok(r)) => r,
            Some(Err(_)) => {
                let _ = respond::text(&mut conn.clone(), 400, "Bad Request");
                break;
            }
            None => break,
        };

        // Execute middleware chain
        let mut middleware_response: Option<SimpleOutgoingResponse> = None;
        for mw in app.middleware_chain() {
            match mw.handle(&bag, &mut req) {
                MiddlewareResult::Continue => {}
                MiddlewareResult::Response(resp) => {
                    middleware_response = Some(resp);
                    break;
                }
            }
        }

        if let Some(resp) = middleware_response {
            let _ = Http11::response(resp).http_render_to_writer(&mut conn.clone());
            continue;
        }

        // Route dispatch
        let method = &req.method;
        let path = &req.request_url.url;

        match app.router().dispatch(method, path) {
            Some(handler) => {
                let result = handler.serve(bag.clone(), req, conn.clone());

                match result {
                    ConnectionResult::Keep => {}
                    ConnectionResult::Take => break,
                    ConnectionResult::Close(err) => {
                        if let Some(e) = &err {
                            tracing::error!("Connection closed with error: {e}");
                        }
                        break;
                    }
                }
            }
            None => {
                let _ = respond::not_found(&mut conn.clone());
            }
        }
    }
}
