//! `ProxyServer` — the running proxy instance.
//!
//! WHY: `ProxyConfig::start` must actually stand up a data plane: bind a
//! listener, serve requests through the [`ProxyHandler`], run health probes, and
//! hand back a handle that can be shut down cleanly.
//!
//! WHAT: [`ProxyServer::start`] validates the config (no duplicate hosts), builds
//! the shared [`ProxyState`], binds the front-end listener, spawns the HTTP
//! server and per-backend health probes, and returns a [`ProxyServer`].
//! [`ProxyServer::shutdown`] turns off the accept loop and joins everything.
//!
//! HOW: The HTTP front end is `foundation_http::HttpServer`, which submits each
//! connection to the valtron pool. **The caller must have initialised a valtron
//! pool** (`foundation_core::valtron::initialize_pool`) and hold its guard for
//! the server's lifetime, exactly as the framework's own server tests do.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_netio::http::HttpConnectionPool;
use foundation_netio::shared::client::{ClientConfig, SystemDnsResolver};
use foundation_netio::http::NativeHttpClient;

use crate::config::{ProxyConfig, ProxyError, SslProvider};
use crate::handler::ProxyHandler;
use crate::health::{spawn_service_probes, HealthMonitor};
use crate::router::Router;
use crate::runtime::ServiceRuntime;
use crate::state::ProxyState;
use crate::tls;

const DEFAULT_BIND: &str = "0.0.0.0:80";

/// A running proxy server instance.
pub struct ProxyServer {
    config: ProxyConfig,
    local_addr: SocketAddr,
    shutdown: Arc<OnSignal>,
    server_thread: Option<JoinHandle<()>>,
    health: Vec<HealthMonitor>,
    state: Arc<ProxyState>,
}

impl std::fmt::Debug for ProxyServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyServer")
            .field("local_addr", &self.local_addr)
            .field("services", &self.config.services.len())
            .finish()
    }
}

impl ProxyServer {
    /// Start the proxy from a validated configuration.
    ///
    /// # Errors
    /// Returns [`ProxyError::Config`] on a duplicate host, or
    /// [`ProxyError::Startup`] if the front-end listener cannot bind.
    ///
    /// # Panics
    /// Never panics. (The valtron pool must already be initialised by the
    /// caller; without it the front end cannot submit connections.)
    pub fn start(config: ProxyConfig) -> Result<Self, ProxyError> {
        tracing::info!(domain = %config.domain, services = config.services.len(), "starting proxy");

        // Validate: no duplicate routes. A route is a (host, path_prefix) pair —
        // Decision 14 routes on host PLUS path_prefix, so two services may share a
        // host as long as their prefixes differ (e.g. `/api` and `/` on one host).
        // Only an identical (host, prefix) pair is a genuine conflict.
        let mut routes = HashSet::new();
        for svc in &config.services {
            let key = (svc.host.clone(), svc.path_prefix.clone());
            if !routes.insert(key) {
                return Err(ProxyError::Config(format!(
                    "duplicate route: host {} with path_prefix {:?}",
                    svc.host, svc.path_prefix
                )));
            }
        }

        // Build runtime services, router, shared client, and state.
        let services: Vec<Arc<ServiceRuntime>> = config
            .services
            .iter()
            .cloned()
            .map(|svc| Arc::new(ServiceRuntime::new(svc)))
            .collect();
        let router = Arc::new(Router::new(services.clone()));

        let pool = Arc::new(HttpConnectionPool::default());
        let client = NativeHttpClient::with_config_and_pool(
            ClientConfig::default(),
            pool,
            SystemDnsResolver::default(),
        );

        // Bind the front-end listener (ephemeral port support for tests).
        let bind_addr = config.bind_addr.clone().unwrap_or_else(|| DEFAULT_BIND.to_string());
        let listener = TcpListener::bind(&bind_addr)
            .map_err(|e| ProxyError::Startup(format!("bind {bind_addr}: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| ProxyError::Startup(format!("local_addr: {e}")))?;

        // Build the HTTP app: store proxy state, then register the catch-all
        // handler (its factory reads the state out of the bag at registration).
        // Read the Arc back so `ProxyServer` shares the exact state the handler
        // sees, for control/inspection.
        let mut app = HttpApp::new_serve();
        app.context()
            .store(ProxyState::new(router, client.clone(), "http", config.io_mode));
        let state = app
            .context()
            .get::<ProxyState>()
            .expect("ProxyState was just stored");
        app.route_any::<ProxyHandler>("/*");

        let shutdown = Arc::new(OnSignal::new());
        let use_tls = !matches!(config.ssl.provider, SslProvider::None);
        // The accept path uses the same I/O mode as the dialed upstream legs (F50).
        let mut server_config = ServerConfig::defaults().with_io(config.io_mode);

        // TLS: build acceptor and configure.
        let mut _redirect_thread: Option<std::thread::JoinHandle<()>> = None;
        if use_tls {
            let acceptor = tls::build_acceptor(&config.ssl)?;
            server_config = server_config.with_tls(acceptor);

            // SSL redirect: spawn a plain-HTTP listener on :80 that 301s to https.
            let redirect_shutdown = shutdown.clone();
            _redirect_thread = Some(std::thread::spawn(move || {
                ssl_redirect_loop(&redirect_shutdown);
            }));
        }

        let server = HttpServer::with_config(app, &bind_addr, server_config);

        // Serve on a dedicated thread with the pre-bound listener.
        let serve_shutdown = shutdown.clone();
        let server_thread = std::thread::spawn(move || {
            if use_tls {
                server.serve_tls_with_listener(&listener, &serve_shutdown);
            } else {
                server.serve_with_listener(&listener, &serve_shutdown);
            }
        });

        // Spawn health probes for every service that configured one.
        let health: Vec<HealthMonitor> = services
            .iter()
            .map(|svc| spawn_service_probes(svc, client.clone(), shutdown.clone()))
            .collect();

        tracing::info!(%local_addr, tls = use_tls, "proxy started");
        Ok(Self {
            config,
            local_addr,
            shutdown,
            server_thread: Some(server_thread),
            health,
            state,
        })
    }

    /// The address the front end is bound to.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    #[must_use]
    pub fn config(&self) -> &ProxyConfig {
        &self.config
    }

    /// Access the shared runtime state (router, backends) for control/inspection.
    #[must_use]
    pub fn state(&self) -> &Arc<ProxyState> {
        &self.state
    }

    /// Signal shutdown and join the server and probe threads.
    ///
    /// The accept loop stops taking connections, drains in-flight requests
    /// within the server's grace window, and returns; probe threads exit at
    /// their next checkpoint.
    pub fn shutdown(mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.server_thread.take() {
            let _ = handle.join();
        }
        for monitor in self.health.drain(..) {
            monitor.join();
        }
    }
}

impl Drop for ProxyServer {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.server_thread.take() {
            let _ = handle.join();
        }
        for monitor in self.health.drain(..) {
            monitor.join();
        }
    }
}

/// SSL redirect: plain-HTTP listener on :80 that answers every request with
/// `301 Moved Permanently` to `https://{host}{path}`.
///
/// WHY: Decision 25 — when TLS is enabled, plain-HTTP traffic must be
/// redirected rather than dropped or answered with connection-refused.
///
/// HOW: Reads just the request line + Host header, constructs the redirect,
/// and closes the connection.  No buffering, no keep-alive, no full HTTP parse.
const REDIRECT_PORT: &str = "0.0.0.0:80";

fn ssl_redirect_loop(shutdown: &Arc<OnSignal>) {
    let listener = match TcpListener::bind(REDIRECT_PORT) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("SSL redirect: cannot bind {REDIRECT_PORT}: {e}");
            return;
        }
    };
    let _ = listener.set_nonblocking(true);
    tracing::info!("SSL redirect listening on {REDIRECT_PORT}");

    let mut buf = [0u8; 4096];
    loop {
        if shutdown.probe() {
            return;
        }
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                match stream.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        let head = String::from_utf8_lossy(&buf[..n]);
                        let host = extract_header(&head, "host:").unwrap_or("localhost");
                        let path = extract_path(&head);
                        let redirect = format!(
                            "HTTP/1.1 301 Moved Permanently\r\n\
                             Location: https://{host}{path}\r\n\
                             Connection: close\r\n\r\n"
                        );
                        let _ = stream.write_all(redirect.as_bytes());
                    }
                    _ => {}
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                tracing::error!("SSL redirect accept error: {e}");
            }
        }
    }
}

/// Best-effort: extract the first header value matching `prefix:` (case-insensitive).
fn extract_header<'a>(head: &'a str, prefix: &str) -> Option<&'a str> {
    let prefix_lower = prefix.to_lowercase();
    head.lines()
        .find(|line| line.to_lowercase().starts_with(&prefix_lower))
        .and_then(|line| line[prefix.len()..].trim().split(',').next())
        .map(|v| v.trim())
}

/// Extract the path from an HTTP request line (e.g. "GET /path HTTP/1.1" → "/path").
fn extract_path(head: &str) -> &str {
    head.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_header_finds_host() {
        let head = "GET /some/path HTTP/1.1\r\nHost: example.com:443\r\nConnection: close\r\n\r\n";
        assert_eq!(extract_header(head, "host:"), Some("example.com:443"));
    }

    #[test]
    fn extract_header_case_insensitive() {
        let head = "GET / HTTP/1.1\r\nHOST: myapp.local\r\n\r\n";
        assert_eq!(extract_header(head, "host:"), Some("myapp.local"));
    }

    #[test]
    fn extract_header_absent_returns_none() {
        let head = "GET / HTTP/1.1\r\nConnection: close\r\n\r\n";
        assert_eq!(extract_header(head, "host:"), None);
    }

    #[test]
    fn extract_header_strips_whitespace() {
        let head = "GET / HTTP/1.1\r\nHost:   padded.example.com  \r\n\r\n";
        assert_eq!(extract_header(head, "host:"), Some("padded.example.com"));
    }

    #[test]
    fn extract_path_from_get() {
        assert_eq!(extract_path("GET /api/health HTTP/1.1\r\nHost: x\r\n\r\n"), "/api/health");
    }

    #[test]
    fn extract_path_root() {
        assert_eq!(extract_path("GET / HTTP/1.1\r\n\r\n"), "/");
    }

    #[test]
    fn extract_path_fallback() {
        assert_eq!(extract_path(""), "/");
    }

    #[test]
    fn redirect_response_has_correct_status() {
        let redirect = format!(
            "HTTP/1.1 301 Moved Permanently\r\n\
             Location: https://{}{}\r\n\
             Connection: close\r\n\r\n",
            "example.com",
            "/login"
        );
        assert!(redirect.starts_with("HTTP/1.1 301"));
        assert!(redirect.contains("Location: https://example.com/login"));
        assert!(redirect.contains("Connection: close"));
    }
}
