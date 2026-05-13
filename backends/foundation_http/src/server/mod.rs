//! `HttpServer` — TCP accept loop with valtron-driven keep-alive.
//!
//! Supports both plain TCP and TLS (via `ssl` or any `ssl-*` feature flag).
//! All connections are submitted to the valtron executor via `valtron::send()`
//! — no `BackgroundJobRegistry::submit()` for connection handling.

use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::synca::OnSignal;
use foundation_core::wire::simple_http::timeout::{
    TimeoutCalculator, TimeoutConfig, TimeoutContext,
};
use foundation_core::wire::simple_http::HTTPStreams;

#[cfg(any(
    feature = "ssl",
    feature = "ssl-rustls",
    feature = "ssl-rustls-ring",
    feature = "ssl-rustls-awsrc",
    feature = "ssl-openssl",
    feature = "ssl-native-tls",
))]
use foundation_core::netcap::ssl::SSLAcceptor;
#[cfg(any(
    feature = "ssl",
    feature = "ssl-rustls",
    feature = "ssl-rustls-ring",
    feature = "ssl-rustls-awsrc",
    feature = "ssl-openssl",
    feature = "ssl-native-tls",
))]
use foundation_core::netcap::Connection;

use crate::app::HttpApp;
use crate::serve::respond;

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

mod connection;

use connection::ConnectionHandler;

/// Running HTTP server.
pub struct HttpServer {
    app: Arc<HttpApp>,
    bind_addr: String,
    config: ServerConfig,
}

impl HttpServer {
    /// Create a new HttpServer with default config.
    #[must_use]
    pub fn new(app: HttpApp, addr: &str) -> Self {
        Self::with_config(app, addr, ServerConfig::default())
    }

    /// Create a new HttpServer with custom config.
    #[must_use]
    pub fn with_config(app: HttpApp, addr: &str, config: ServerConfig) -> Self {
        Self {
            app: Arc::new(app),
            bind_addr: addr.to_string(),
            config,
        }
    }

    /// Start serving plain HTTP. Blocks until the shutdown signal is triggered.
    #[tracing::instrument(skip(self, shutdown))]
    pub fn serve(self, shutdown: Arc<OnSignal>) {
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

        self.serve_loop(listener, shutdown, |tcp: TcpStream| {
            RawStream::from_tcp(tcp).map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Start serving from a pre-bound `TcpListener`. Useful for tests
    /// that need to confirm the port is bound before sending requests.
    #[tracing::instrument(skip(self, listener, shutdown))]
    pub fn serve_with_listener(self, listener: std::net::TcpListener, shutdown: Arc<OnSignal>) {
        tracing::info!("Listening on {}", self.bind_addr);
        listener
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");

        self.serve_loop(listener, shutdown, |tcp| {
            RawStream::from_tcp(tcp).map_err(|e| format!("Failed to create RawStream: {e}"))
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

        self.serve_loop(listener, shutdown, move |tcp| {
            let conn = Connection::from(tcp);
            let tls_stream = acceptor
                .accept(conn)
                .map_err(|e| format!("TLS handshake failed: {e}"))?;
            RawStream::from_server_tls(tls_stream)
                .map_err(|e| format!("Failed to create RawStream: {e}"))
        });
    }

    /// Generic accept loop — shared by `serve` and `serve_tls`.
    #[tracing::instrument(skip(self, listener, shutdown, wrap_stream))]
    fn serve_loop(
        self,
        listener: std::net::TcpListener,
        shutdown: Arc<OnSignal>,
        wrap_stream: impl Fn(std::net::TcpStream) -> Result<RawStream, String> + Send + Sync + 'static,
    ) {
        let wrap_stream = Arc::new(wrap_stream);
        let would_block_sleep = self.config.would_block_sleep();
        let accept_error_sleep = self.config.accept_error_sleep();
        let keep_alive_config = self.config.keep_alive.clone();

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

                    // Perform connection setup (TLS handshake if any) in the
                    // accept loop before submitting to valtron.
                    let raw_stream = match wrap_clone(tcp) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!(%client_ip, "Connection setup failed: {e}");
                            continue;
                        }
                    };

                    // raw_stream.set

                    let shared_stream = SharedByteBufferStream::rwrite(raw_stream);
                    let streams = HTTPStreams::new(shared_stream.clone());

                    let handler = ConnectionHandler::new(
                        self.app.clone(),
                        streams,
                        shared_stream.clone(),
                        client_ip.clone(),
                        keep_alive_config.clone(),
                    );

                    match foundation_core::valtron::send(handler) {
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

        tracing::info!("Server stopped");
    }
}
