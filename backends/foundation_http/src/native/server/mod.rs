//! `HttpServer` — TCP accept loop with valtron-driven keep-alive.
//!
//! Supports both plain TCP and TLS (via `ssl` or any `ssl-*` feature flag).
//! All connections are submitted to the valtron executor via `valtron::send()`
//! — no `BackgroundJobRegistry::submit()` for connection handling.

use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_iogate::ServerIo;
use foundation_netio::netcap::{ConnectionContext, RawStream};
use foundation_core::synca::{OnSignal, WaitGroup};
use foundation_netio::simple_http::shared::timeout::{
    ExpectContinueConfig, TimeoutCalculator, TimeoutConfig, TimeoutContext,
};

#[cfg(any(
    feature = "ssl",
    feature = "ssl-rustls",
    feature = "ssl-rustls-ring",
    feature = "ssl-rustls-awsrc",
    feature = "ssl-openssl",
    feature = "ssl-native-tls",
))]
use foundation_netio::netcap::ssl::SSLAcceptor;
use foundation_netio::netcap::Connection;

use crate::shared::app::{HttpApp, ServerApp};
use crate::shared::serve::{respond, Serve};

// ---------------------------------------------------------------------------
// KeepAliveConfig

/// Configuration for keep-alive timing on idle connections.
///
/// WHY: All connections use the valtron keep-alive handler. This struct
/// controls how long idle connections are kept alive and how aggressively
/// the executor polls them.
///
/// NOTE: Now uses `TimeoutCalculator` as the single source of truth for
/// timeout values. The calculator provides dynamic timeout calculation
/// based on context.
#[derive(Clone)]
pub struct KeepAliveConfig {
    /// Timeout calculator for dynamic timeout calculation.
    pub timeout_calculator: TimeoutCalculator,
    /// Number of consecutive polls with no data before switching from
    /// `Pending` to `Delayed` (default: 200).
    pub escalation_threshold: u32,
    /// Maximum number of complete delay cycles without data before closing
    /// the connection (default: 200). A "cycle" = `escalation_threshold` polls.
    pub max_delay_cycles: u32,
}

impl KeepAliveConfig {
    /// Create a config with default values.
    #[must_use]
    pub fn defaults() -> Self {
        // Server-specific timeout config with longer timeouts for connections
        let timeout_config = TimeoutConfig {
            min_read_timeout: Duration::from_secs(120), // 3 mins idle timeout
            max_read_timeout: Duration::from_secs(300), // 5 mins max read
            ..TimeoutConfig::default()
        };

        Self {
            timeout_calculator: TimeoutCalculator::with_config(timeout_config),
            escalation_threshold: 50,
            max_delay_cycles: 100,
        }
    }

    /// Create a config with a custom timeout calculator.
    #[must_use]
    pub fn with_timeout_calculator(mut self, calculator: TimeoutCalculator) -> Self {
        self.timeout_calculator = calculator;
        self
    }

    /// Set the idle timeout (convenience method that updates calculator config).
    ///
    /// WHY: Backward compatibility - users can still call `with_idle_timeout()`
    /// but now it updates the underlying calculator.
    #[must_use]
    pub fn with_idle_timeout(mut self, dur: Duration) -> Self {
        // Create new config with updated idle timeout
        let new_config = TimeoutConfig {
            min_read_timeout: dur,
            ..*self.timeout_calculator.config()
        };
        self.timeout_calculator = TimeoutCalculator::with_config(new_config);
        self
    }

    /// Get the minimum delay from calculator.
    #[must_use]
    pub fn min_delay(&self) -> Duration {
        // Use sleep duration for default context as base min delay
        self.timeout_calculator
            .calculate_sleep_duration(&TimeoutContext::default())
    }

    /// Get the maximum delay from calculator.
    #[must_use]
    pub fn max_delay(&self) -> Duration {
        // Max delay is based on max read timeout
        self.timeout_calculator.config().max_read_timeout
    }

    /// Get the idle timeout from calculator.
    #[must_use]
    pub fn idle_timeout(&self) -> Duration {
        // Idle timeout is the read timeout for default context
        self.timeout_calculator
            .calculate_read_timeout(&TimeoutContext::default())
    }

    /// Set the 100-continue expect delay configuration.
    ///
    /// WHY: Controls how long the server waits for the client body after
    /// sending 100 Continue, with configurable base delay, max attempts,
    /// and per-attempt penalty reduction.
    #[must_use]
    pub fn with_expect_continue(mut self, config: ExpectContinueConfig) -> Self {
        let mut tc = *self.timeout_calculator.config();
        tc.expect_continue = config;
        self.timeout_calculator = TimeoutCalculator::with_config(tc);
        self
    }
}

impl Default for KeepAliveConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Configuration for the HTTP server's accept loop timing.
///
/// WHY: All durations are configurable — no hardcoded values.
/// WHAT: `ServerConfig::default()` provides sane defaults; users can
/// override individual fields via builder-style methods.
///
/// NOTE: Now uses `TimeoutCalculator` as the single source of truth for
/// timeout values, providing dynamic timeout calculation based on context.
pub struct ServerConfig {
    /// Timeout calculator for dynamic timeout calculation.
    pub timeout_calculator: TimeoutCalculator,
    /// Keep-alive configuration. Always present with defaults.
    pub keep_alive: KeepAliveConfig,
    /// Maximum body size in bytes (default: 10 MB).
    pub max_body_bytes: usize,
    /// Grace window for draining in-flight connections after shutdown is
    /// signalled (Decision 12 §10). Once the accept loop stops, the server waits
    /// up to this long for active connections to finish before force-closing
    /// whatever remains. Default: 30s.
    pub shutdown_grace: Duration,
    /// How accepted sockets acquire bytes (Feature 48). Default
    /// [`ServerIo::Std`] — plain `read(2)`, no reactor, so nothing changes for a
    /// caller who does not ask. [`ServerIo::Completion`] opts the read path into
    /// the io_uring completion inbox and errors at startup if the kernel cannot.
    pub io_mode: ServerIo,
    /// TLS acceptor for encrypted connections.
    #[cfg(any(
        feature = "ssl",
        feature = "ssl-rustls",
        feature = "ssl-rustls-ring",
        feature = "ssl-rustls-awsrc",
        feature = "ssl-openssl",
        feature = "ssl-native-tls",
    ))]
    pub tls_acceptor: Option<Arc<SSLAcceptor>>,
}

impl ServerConfig {
    /// Create a config with default values.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            timeout_calculator: TimeoutCalculator::default(),
            keep_alive: KeepAliveConfig::defaults(),
            max_body_bytes: 10 * 1024 * 1024, // 10 MB
            shutdown_grace: Duration::from_secs(30),
            io_mode: ServerIo::Std,
            #[cfg(any(
                feature = "ssl",
                feature = "ssl-rustls",
                feature = "ssl-rustls-ring",
                feature = "ssl-rustls-awsrc",
                feature = "ssl-openssl",
                feature = "ssl-native-tls",
            ))]
            tls_acceptor: None,
        }
    }

    /// Create a config with a custom timeout calculator.
    #[must_use]
    pub fn with_timeout_calculator(mut self, calculator: TimeoutCalculator) -> Self {
        // Also update keep_alive to use the same calculator
        self.keep_alive = self.keep_alive.with_timeout_calculator(calculator.clone());
        self.timeout_calculator = calculator;
        self
    }

    /// Get the `WouldBlock` sleep duration from calculator.
    #[must_use]
    pub fn would_block_sleep(&self) -> Duration {
        // Use sleep duration for default context
        self.timeout_calculator
            .calculate_sleep_duration(&TimeoutContext::default())
    }

    /// Get the accept error sleep duration from calculator.
    /// Uses a longer duration for error recovery.
    #[must_use]
    pub fn accept_error_sleep(&self) -> Duration {
        // Use 2x the base sleep duration for error recovery
        let base = self
            .timeout_calculator
            .calculate_sleep_duration(&TimeoutContext::default());
        base * 2
    }

    /// Set the `WouldBlock` sleep duration (convenience method).
    ///
    /// WHY: Backward compatibility - updates the underlying calculator config.
    #[must_use]
    pub fn with_would_block_sleep(mut self, dur: Duration) -> Self {
        let new_config = TimeoutConfig {
            min_sleep_duration: dur,
            ..*self.timeout_calculator.config()
        };
        self.timeout_calculator = TimeoutCalculator::with_config(new_config);
        self
    }

    /// Set the accept error sleep duration (convenience method).
    ///
    /// WHY: Backward compatibility - updates the underlying calculator config.
    #[must_use]
    pub fn with_accept_error_sleep(mut self, dur: Duration) -> Self {
        // Note: accept_error_sleep is derived from would_block_sleep * 2
        // So we set would_block_sleep to half the desired error sleep
        let new_config = TimeoutConfig {
            min_sleep_duration: dur / 2,
            ..*self.timeout_calculator.config()
        };
        self.timeout_calculator = TimeoutCalculator::with_config(new_config);
        self
    }

    /// Set the keep-alive config, replacing the entire config.
    #[must_use]
    pub fn with_keep_alive(mut self, config: KeepAliveConfig) -> Self {
        self.keep_alive = config;
        self
    }

    /// Convenience: set both min and max delay to the same value.
    #[must_use]
    pub fn with_poll_interval(mut self, dur: Duration) -> Self {
        // Update the keep_alive calculator
        let new_config = TimeoutConfig {
            min_sleep_duration: dur,
            max_sleep_duration: dur,
            ..*self.keep_alive.timeout_calculator.config()
        };
        self.keep_alive.timeout_calculator = TimeoutCalculator::with_config(new_config);
        self
    }

    /// Convenience: set the idle timeout.
    #[must_use]
    pub fn with_idle_timeout(mut self, dur: Duration) -> Self {
        self.keep_alive = self.keep_alive.with_idle_timeout(dur);
        self
    }

    /// Set the maximum body size in bytes.
    #[must_use]
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// Set the graceful-shutdown drain grace window (Decision 12 §10).
    #[must_use]
    pub fn with_shutdown_grace(mut self, dur: Duration) -> Self {
        self.shutdown_grace = dur;
        self
    }

    /// Choose how accepted sockets acquire bytes (Feature 48).
    ///
    /// WHY: an operator who wants the io_uring completion read path must be able
    /// to demand it — and be told at startup if the kernel cannot deliver —
    /// rather than getting a silent default.
    ///
    /// WHAT: sets the [`ServerIo`] mode. [`ServerIo::Std`] (the default) keeps
    /// today's plain `read(2)` behaviour; [`ServerIo::Completion`] arms the
    /// completion inbox and fails at `serve` time on a non-io_uring kernel.
    ///
    /// HOW: the accept loop hands the mode to `foundation_iogate` on unix; on
    /// non-unix targets the mode is inert and serving stays on `read(2)`.
    #[must_use]
    pub fn with_io(mut self, io_mode: ServerIo) -> Self {
        self.io_mode = io_mode;
        self
    }

    /// Set the TLS acceptor for encrypted connections.
    #[cfg(any(
        feature = "ssl",
        feature = "ssl-rustls",
        feature = "ssl-rustls-ring",
        feature = "ssl-rustls-awsrc",
        feature = "ssl-openssl",
        feature = "ssl-native-tls",
    ))]
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

/// Build the accept-time [`Connection`] for `tcp` under `io_mode`.
///
/// WHY: the accept loop should know only which [`ServerIo`] mode it serves, not
/// reactors or completion sockets. This is the one seam where the mode turns
/// into a concrete connection (Feature 48).
///
/// WHAT: on unix, delegates to `foundation_iogate::accept_connection` — a plain
/// `Connection::Tcp` for [`ServerIo::Std`], a `Connection::Completion` otherwise.
/// On non-unix there is no reactor, so the mode is inert and the result is always
/// a plain `Connection::Tcp`.
///
/// # Errors
/// The reactor/registration error surfaced by the I/O gate, as a `String`.
#[cfg(unix)]
fn build_connection(
    tcp: TcpStream,
    addr: std::net::SocketAddr,
    io_mode: ServerIo,
) -> Result<Connection, String> {
    foundation_iogate::accept_connection(tcp, addr, io_mode)
        .map_err(|e| format!("iogate accept failed: {e}"))
}

#[cfg(not(unix))]
fn build_connection(
    tcp: TcpStream,
    _addr: std::net::SocketAddr,
    _io_mode: ServerIo,
) -> Result<Connection, String> {
    Ok(Connection::from(tcp))
}

mod connection;
mod h2_connection;
mod protocol_detect;

use protocol_detect::ProtocolDetectHandler;

/// Running HTTP server.
pub struct HttpServer {
    app: ServerApp,
    bind_addr: String,
    config: ServerConfig,
}

impl HttpServer {
    /// Create a new `HttpServer` with default config (HTTP/1.1 only).
    #[must_use]
    pub fn new(app: HttpApp<Arc<dyn Serve>>, addr: &str) -> Self {
        Self::with_config(app, addr, ServerConfig::default())
    }

    /// Create a new `HttpServer` with custom config (HTTP/1.1 only).
    #[must_use]
    pub fn with_config(app: HttpApp<Arc<dyn Serve>>, addr: &str, config: ServerConfig) -> Self {
        Self { app: ServerApp::Http1(Arc::new(app)), bind_addr: addr.to_string(), config }
    }

    /// Create from a [`ServerApp`] with default config.
    #[must_use]
    pub fn from_app(app: ServerApp, addr: &str) -> Self {
        Self { app, bind_addr: addr.to_string(), config: ServerConfig::default() }
    }

    /// Create from a [`ServerApp`] with custom config.
    #[must_use]
    pub fn from_app_with_config(app: ServerApp, addr: &str, config: ServerConfig) -> Self {
        Self { app, bind_addr: addr.to_string(), config }
    }

    /// Start serving plain HTTP. Blocks until the shutdown signal is triggered.
    ///
    /// # Panics
    ///
    /// Panics if the TCP listener cannot be set to non-blocking mode.
    #[tracing::instrument(skip(self, shutdown))]
    pub fn serve(self, shutdown: &Arc<OnSignal>) {
        let listener = match std::net::TcpListener::bind(&self.bind_addr) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind to {}: {}", self.bind_addr, e);
                return;
            }
        };

        tracing::info!("Listening on {}", self.bind_addr);
        listener
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");

        self.init_io();
        let io_mode = self.config.io_mode;
        self.serve_loop(&listener, shutdown, move |tcp, addr| {
            let conn = build_connection(tcp, addr, io_mode)?;
            RawStream::from_connection(conn)
                .map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Start serving from a pre-bound `TcpListener`. Useful for tests
    /// that need to confirm the port is bound before sending requests.
    ///
    /// # Panics
    ///
    /// Panics if the TCP listener cannot be set to non-blocking mode.
    #[tracing::instrument(skip(self, listener, shutdown))]
    pub fn serve_with_listener(self, listener: &std::net::TcpListener, shutdown: &Arc<OnSignal>) {
        tracing::info!("Listening on {}", self.bind_addr);
        listener
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");

        self.init_io();
        let io_mode = self.config.io_mode;
        self.serve_loop(listener, shutdown, move |tcp, addr| {
            let conn = build_connection(tcp, addr, io_mode)?;
            RawStream::from_connection(conn)
                .map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Start serving HTTPS. Blocks until the shutdown signal is triggered.
    ///
    /// Requires an `ssl` or `ssl-*` feature flag and a `tls_acceptor`
    /// configured in `ServerConfig`.
    #[cfg(any(
        feature = "ssl",
        feature = "ssl-rustls",
        feature = "ssl-rustls-ring",
        feature = "ssl-rustls-awsrc",
        feature = "ssl-openssl",
        feature = "ssl-native-tls",
    ))]
    #[tracing::instrument(skip(self, shutdown))]
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
        listener
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");

        self.init_io();
        let io_mode = self.config.io_mode;
        self.serve_loop(&listener, &shutdown, move |tcp, addr| {
            // rustls reads *through* this Connection, so a completion-backed
            // Connection gives TLS the completion read path for free.
            let conn = build_connection(tcp, addr, io_mode)?;
            let tls_stream = acceptor
                .accept(conn)
                .map_err(|e| format!("TLS handshake failed: {e}"))?;
            RawStream::from_server_tls(tls_stream)
                .map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Serve HTTPS over a caller-bound listener (the TLS analogue of
    /// [`serve_with_listener`](Self::serve_with_listener)). Lets the caller bind
    /// first — e.g. to discover an ephemeral port before announcing the URL.
    /// Blocks until the shutdown signal is triggered.
    ///
    /// Requires an `ssl`/`ssl-*` feature and a `tls_acceptor` in `ServerConfig`.
    #[cfg(any(
        feature = "ssl",
        feature = "ssl-rustls",
        feature = "ssl-rustls-ring",
        feature = "ssl-rustls-awsrc",
        feature = "ssl-openssl",
        feature = "ssl-native-tls",
    ))]
    #[tracing::instrument(skip(self, listener, shutdown))]
    pub fn serve_tls_with_listener(self, listener: &std::net::TcpListener, shutdown: &Arc<OnSignal>) {
        let acceptor = match &self.config.tls_acceptor {
            Some(a) => a.clone(),
            None => {
                tracing::error!("TLS acceptor not configured — call ServerConfig::with_tls()");
                return;
            }
        };
        tracing::info!("Listening on {} (TLS)", self.bind_addr);
        listener
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");
        self.init_io();
        let io_mode = self.config.io_mode;
        self.serve_loop(listener, shutdown, move |tcp, addr| {
            // rustls reads *through* this Connection, so a completion-backed
            // Connection gives TLS the completion read path for free.
            let conn = build_connection(tcp, addr, io_mode)?;
            let tls_stream = acceptor
                .accept(conn)
                .map_err(|e| format!("TLS handshake failed: {e}"))?;
            RawStream::from_server_tls(tls_stream)
                .map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Initialise the shared reactor for the configured [`ServerIo`] mode.
    ///
    /// WHY: an operator who asks for [`ServerIo::Completion`] must get io_uring
    /// or a hard failure at startup — never a server that logs one line and then
    /// silently serves nothing (Feature 48, no-silent-defaults).
    ///
    /// WHAT: a no-op for [`ServerIo::Std`] and on non-unix targets; otherwise it
    /// initialises the process-wide reactor on the required backend.
    ///
    /// HOW: delegates to `foundation_iogate::init_reactor_for`.
    ///
    /// # Panics
    /// Panics, naming the requested mode, if the reactor cannot be initialised on
    /// the backend the mode demands (e.g. `Completion` on a kernel without
    /// io_uring) — the same fail-fast contract as the `set_nonblocking` setup
    /// above.
    #[cfg(unix)]
    fn init_io(&self) {
        foundation_iogate::init_reactor_for(self.config.io_mode).unwrap_or_else(|e| {
            panic!(
                "server I/O mode '{}' could not be initialised: {e}",
                self.config.io_mode
            )
        });
    }

    /// Non-unix targets have no reactor; the I/O mode is inert.
    #[cfg(not(unix))]
    fn init_io(&self) {}

    /// Generic accept loop — shared by `serve` and `serve_tls`.
    #[tracing::instrument(skip(self, listener, shutdown, wrap_stream))]
    fn serve_loop(
        self,
        listener: &std::net::TcpListener,
        shutdown: &Arc<OnSignal>,
        wrap_stream: impl Fn(std::net::TcpStream, std::net::SocketAddr) -> Result<RawStream, String>
            + Send
            + Sync
            + 'static,
    ) {
        let wrap_stream = Arc::new(wrap_stream);
        let would_block_sleep = self.config.would_block_sleep();
        let accept_error_sleep = self.config.accept_error_sleep();
        let keep_alive_config = self.config.keep_alive.clone();
        let shutdown_grace = self.config.shutdown_grace;

        // Tracks connections currently owned by a valtron handler task. Each
        // handler holds a guard that decrements on completion, so the drain
        // phase below can wait for in-flight connections (Decision 12 §10).
        let active = WaitGroup::new();

        loop {
            if shutdown.probe() {
                tracing::info!("Shutdown signal received, stopping accept loop");
                break;
            }

            match listener.accept() {
                Ok((tcp, addr)) => {
                    tracing::trace!("Accepted connection from {addr}");

                    // Set socket to non-blocking mode for valtron executor compatibility
                    if let Err(e) = tcp.set_nonblocking(true) {
                        tracing::error!("Failed to set non-blocking mode: {e}");
                        continue;
                    }

                    let client_ip = addr.ip().to_string();
                    let wrap_clone = wrap_stream.clone();

                    // Build the connection-scoped context once at accept time;
                    // every request read on this connection shares a clone of it
                    // (Decision 12 §13). TLS/ALPN/QUIC fields stay at their empty
                    // defaults here — the HTTP/1.1 accept path knows only the peer
                    // address; richer fields are populated by the TLS/h2/h3 front
                    // ends as those transports land.
                    let connection = Arc::new(ConnectionContext {
                        peer_addr: Some(addr),
                        ..ConnectionContext::default()
                    });

                    // Perform connection setup (TLS handshake if any) in the
                    // accept loop before submitting to valtron.
                    let raw_stream = match wrap_clone(tcp, addr) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!(%client_ip, "Connection setup failed: {e}");
                            continue;
                        }
                    };

                    // raw_stream.set

                    let shared_stream = SharedByteBufferStream::rwrite(raw_stream);

                    // Count this connection as active; the guard travels with the
                    // connection into whichever protocol handler claims it, and
                    // decrements when that task completes (Decision 12 §10).
                    active.add(1);
                    let guard = active.guard();

                    // The socket is non-blocking and the peer has usually sent
                    // nothing yet, so HTTP/1.1-vs-h2c detection cannot happen
                    // here without stalling the accept loop. Hand the connection
                    // to a task that peeks, decides, and spawns the real handler.
                    let detector = ProtocolDetectHandler::new(
                        shared_stream.clone(),
                        self.app.clone(),
                        client_ip.clone(),
                        connection,
                        shutdown.clone(),
                        guard,
                        keep_alive_config.clone(),
                    );

                    match foundation_core::valtron::send(detector) {
                        Ok(()) => {
                            tracing::trace!(%client_ip, "Submitted connection to valtron executor");
                        }
                        Err(e) => {
                            tracing::error!(
                                %client_ip,
                                err = ?e,
                                "Failed to submit connection to valtron"
                            );
                            let _ = respond::text(
                                &mut shared_stream.clone(),
                                503,
                                "Service Unavailable",
                            );
                        }
                    }
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

        // Drain phase (Decision 12 §10): the accept loop has stopped. Handlers
        // already saw the shutdown signal and stop taking new keep-alive requests
        // at their idle checkpoint; wait up to the grace window for in-flight
        // connections (including streaming) to finish. Past the deadline we stop
        // waiting and return — leftover tasks will end at their next checkpoint.
        tracing::info!(grace_ms = shutdown_grace.as_millis(), "Draining in-flight connections");
        if active.wait_timeout(shutdown_grace) {
            tracing::info!("All in-flight connections drained cleanly");
        } else {
            tracing::warn!(
                grace_ms = shutdown_grace.as_millis(),
                "Drain grace elapsed; force-closing remaining connections"
            );
        }

        tracing::info!("Server stopped");
    }
}

// Re-export timeout types so users can configure expect-continue behavior.
pub mod timeout {
    pub use foundation_netio::simple_http::shared::timeout::ExpectContinueConfig;
}
