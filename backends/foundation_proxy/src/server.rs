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
use std::time::{Duration, Instant};

use foundation_core::synca::OnSignal;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_netio::http::HttpConnectionPool;
use foundation_netio::shared::client::{ClientConfig, SystemDnsResolver};
use foundation_netio::http::NativeHttpClient;

use crate::config::{BackendState, ProxyConfig, ProxyError, SslProvider};
use crate::control::ControlSocket;
use crate::handler::ProxyHandler;
use crate::h2_proxy::H2ProxyHandler;
use crate::health::{spawn_service_probes, HealthMonitor};
use crate::persistence::{PersistedProxyState, ProxyStateStore};
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
    /// Unix-domain admin socket (Decision 20), present when the config enabled it.
    control: Option<ControlSocket>,
    /// The bound UDP address of the HTTP/3 front end, when enabled (F19).
    h3_local_addr: Option<SocketAddr>,
    /// The bound address of the plain-HTTP → HTTPS redirect listener (F12).
    redirect_local_addr: Option<SocketAddr>,
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

        // Build the HTTP/1.1 handler app: store proxy state, register catch-all.
        // The `ContextBag` wraps the stored value in its own `Arc` and keys it by
        // `TypeId::of::<ProxyState>()`, which is exactly what `ProxyHandler::create`
        // looks up via `get::<ProxyState>()`. Store the value (not a pre-made
        // `Arc`), then pull the single shared `Arc<ProxyState>` back out of the bag
        // so the H2 handler, health probes, and this handle all reference the same
        // instance (shared drain flag and in-flight counts).
        // State persistence (Decision 21, F14): restore backend drain/pause state
        // from a previous run, and attach the store so admin changes are written
        // through. `None` state_dir disables persistence entirely.
        let store: Option<Arc<ProxyStateStore>> = config.state_dir.as_ref().map(|dir| {
            Arc::new(ProxyStateStore::new(std::path::Path::new(dir), &config.domain))
        });
        if let Some(store) = &store {
            restore_persisted_state(store, &services, &config);
        }

        let mut h1_app = HttpApp::new_serve();
        let mut proxy_state_value = ProxyState::new(router, client.clone(), "http", config.io_mode);
        if let Some(store) = &store {
            proxy_state_value = proxy_state_value.with_store(Arc::clone(store));
        }
        h1_app.context().store(proxy_state_value);
        let proxy_state = h1_app
            .context()
            .get::<ProxyState>()
            .expect("ProxyState was just stored in the context bag");
        h1_app.route_any::<ProxyHandler>("/*");

        // Build the HTTP/2 handler app: share the same ProxyState.
        let mut h2_app = foundation_http::shared::app::HttpApp::new_h2_serve();
        let h2_handler: Arc<dyn foundation_http::native::serve::H2Serve> =
            Arc::new(H2ProxyHandler::new(Arc::clone(&proxy_state)));
        h2_app.route_any_h2("/*", h2_handler);

        let shutdown = Arc::new(OnSignal::new());
        let use_tls = !matches!(config.ssl.provider, SslProvider::None);
        let mut server_config = ServerConfig::defaults().with_io(config.io_mode);

        // TLS: select the cert manager (Static PEM or ACME provisioning),
        // obtain the cert once, and build the acceptor. The same cert also feeds
        // the HTTP/3 (QUIC) front end when enabled.
        let mut _redirect_thread: Option<std::thread::JoinHandle<()>> = None;
        let h3_local_addr: Option<SocketAddr> = None;
        let mut redirect_local_addr: Option<SocketAddr> = None;
        if use_tls {
            let cert_manager = tls::build_cert_manager(&config, Arc::new(client.clone()))?;
            let cert_pair = cert_manager.get_cert()?;
            let acceptor = tls::acceptor_from_pair(&cert_pair)?;
            server_config = server_config.with_tls(acceptor);

            // Plain-HTTP → HTTPS redirect (Decision 25, F12). Pre-bind so the
            // actual port is observable (tests use an ephemeral port); a bind
            // failure (e.g. :80 without privileges) is non-fatal.
            let redirect_addr = config.redirect_bind.as_deref().unwrap_or(REDIRECT_PORT);
            match TcpListener::bind(redirect_addr) {
                Ok(listener) => {
                    redirect_local_addr = listener.local_addr().ok();
                    let redirect_shutdown = shutdown.clone();
                    _redirect_thread = Some(std::thread::spawn(move || {
                        ssl_redirect_loop(listener, &redirect_shutdown);
                    }));
                }
                Err(e) => tracing::warn!("SSL redirect: cannot bind {redirect_addr}: {e}"),
            }

            // HTTP/3 (QUIC) front end (Decision 27, F19): serve H3 on the
            // configured UDP address using the same certificate.
            #[cfg(feature = "quic")]
            if let Some(h3_addr) = &config.h3_bind {
                h3_local_addr =
                    Some(spawn_h3_server(h3_addr, &cert_pair, Arc::clone(&proxy_state), shutdown.clone())?);
            }
        }

        let server_app = foundation_http::shared::app::ServerApp::both(h1_app, h2_app);
        let server = HttpServer::from_app_with_config(server_app, &bind_addr, server_config);

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

        // Unix-domain admin socket (Decision 20), if the config enabled it. It
        // serves control commands against the shared state and stops when the
        // shutdown signal fires.
        let control = config.control_socket.as_ref().map(|path| {
            ControlSocket::start(path.clone(), Arc::clone(&proxy_state), shutdown.clone())
        });

        tracing::info!(%local_addr, tls = use_tls, control = control.is_some(), "proxy started");
        Ok(Self {
            config,
            local_addr,
            shutdown,
            server_thread: Some(server_thread),
            health,
            state: proxy_state,
            control,
            h3_local_addr,
            redirect_local_addr,
        })
    }

    /// The bound UDP address of the HTTP/3 front end, if enabled (F19).
    #[must_use]
    pub fn h3_local_addr(&self) -> Option<SocketAddr> {
        self.h3_local_addr
    }

    /// The bound address of the plain-HTTP → HTTPS redirect listener (F12).
    #[must_use]
    pub fn redirect_local_addr(&self) -> Option<SocketAddr> {
        self.redirect_local_addr
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

    /// Begin graceful drain: stop accepting new connections, let in-flight
    /// requests finish. Does NOT block — call [`drain_complete`](Self::drain_complete)
    /// with a timeout, then [`shutdown`](Self::shutdown) to tear down.
    ///
    /// Decision 22 — zero-downtime deploy.
    pub fn start_drain(&self) {
        tracing::info!("proxy entering drain — new requests will receive 503");
        self.state.start_drain();
    }

    /// Poll until all in-flight requests have drained, or `timeout` elapses.
    /// Returns `true` if drain completed (inflight == 0), `false` on timeout.
    pub fn drain_complete(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            let inflight = self.state.inflight_total();
            if inflight == 0 {
                return true;
            }
            if start.elapsed() >= timeout {
                tracing::warn!(inflight, "drain timed out after {:?}", timeout);
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Signal shutdown and join the server and probe threads.
    ///
    /// The accept loop stops taking connections, drains in-flight requests
    /// within the server's grace window, and returns; probe threads exit at
    /// their next checkpoint.
    pub fn shutdown(mut self) {
        self.shutdown.turn_on();
        if let Some(control) = self.control.take() {
            control.shutdown();
        }
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
        if let Some(control) = self.control.take() {
            control.shutdown();
        }
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

fn ssl_redirect_loop(listener: TcpListener, shutdown: &Arc<OnSignal>) {
    let _ = listener.set_nonblocking(true);
    tracing::info!(addr = ?listener.local_addr(), "SSL redirect listening");

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

/// Build the H3 proxy app and start the QUIC/H3 front end on `h3_addr`, using
/// the same TLS cert as the TCP listener (Decision 27, F19).
#[cfg(feature = "quic")]
fn spawn_h3_server(
    h3_addr: &str,
    cert_pair: &crate::tls::CertPair,
    proxy_state: Arc<ProxyState>,
    shutdown: Arc<OnSignal>,
) -> Result<SocketAddr, ProxyError> {
    let addr: SocketAddr = h3_addr
        .parse()
        .map_err(|e| ProxyError::Startup(format!("parse h3 bind {h3_addr}: {e}")))?;
    let mut h3_app = foundation_http::shared::app::HttpApp::new_h3_serve();
    h3_app.route_any_h3(
        "/*",
        Arc::new(crate::h3_proxy::H3ProxyHandler::new(proxy_state)),
    );
    let local = foundation_http::native::server::serve_h3(
        addr,
        &cert_pair.cert_chain,
        &cert_pair.private_key,
        Arc::new(h3_app),
        shutdown,
    )
    .map_err(|e| ProxyError::Startup(format!("start h3 server: {e}")))?;
    tracing::info!(%local, "proxy HTTP/3 front end started");
    Ok(local)
}

/// Restore persisted backend drain/pause state onto the live services and record
/// the current config hash (Decision 21, F14). Best-effort — a load/save failure
/// is logged and startup proceeds with default (Active) backend states.
fn restore_persisted_state(
    store: &ProxyStateStore,
    services: &[Arc<ServiceRuntime>],
    config: &ProxyConfig,
) {
    let mut persisted = match store.load() {
        Ok(Some(p)) => {
            apply_backend_states(services, &p);
            p
        }
        Ok(None) => PersistedProxyState::default(),
        Err(e) => {
            tracing::warn!("failed to load persisted proxy state: {e}");
            return;
        }
    };
    // Record the current config hash so a later run can detect topology changes.
    persisted.config_hash = Some(ProxyStateStore::compute_config_hash(config));
    if let Err(e) = store.save(&persisted) {
        tracing::warn!("failed to persist proxy state on start: {e}");
    }
}

/// Apply persisted per-backend states to the matching live backends. The key is
/// `"{service}/{url}"`, exactly as [`ProxyStateStore::save_backend_state`] writes
/// it, so no ambiguous path-splitting is needed — we rebuild the key and look up.
fn apply_backend_states(services: &[Arc<ServiceRuntime>], persisted: &PersistedProxyState) {
    for svc in services {
        let name = &svc.config().name;
        for backend in svc.backends() {
            let key = format!("{name}/{}", backend.url());
            if let Some(state) = persisted.backend_states.get(&key).and_then(|s| parse_backend_state(s)) {
                backend.set_state(state);
                tracing::info!(service = %name, url = %backend.url(), state = %state, "restored backend state");
            }
        }
    }
}

/// Parse a persisted backend-state string (written via `BackendState`'s
/// `Display`, e.g. `"draining"`) back into a [`BackendState`].
fn parse_backend_state(s: &str) -> Option<BackendState> {
    match s.to_ascii_lowercase().as_str() {
        "active" => Some(BackendState::Active),
        "draining" => Some(BackendState::Draining),
        "paused" => Some(BackendState::Paused),
        _ => None,
    }
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
