//! Connection management for HTTP client.
//!
//! This module provides URL parsing and TCP/TLS connection establishment.

use crate::wire::simple_http::client::pool::ConnectionPool;
use crate::wire::simple_http::client::SystemDnsResolver;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::{Connection, RawStream};
use crate::wire::simple_http::client::dns::DnsResolver;
use crate::wire::simple_http::client::errors::HttpClientError;
use crate::wire::simple_http::url::Uri;
use std::time::Duration;

use crate::netcap::ssl::SSLConnector;

// Re-export Uri as ParsedUrl for backward compatibility
/// Backward compatibility alias for Uri.
///
/// **Deprecated**: Use `crate::wire::simple_http::url::Uri` instead.
pub type ParsedUrl = Uri;

// Re-export Scheme from url module
pub use crate::wire::simple_http::url::Scheme;

/// HTTP client connection wrapping `netcap::RawStream`.
///
/// Provides automatic buffering, address tracking, and convenient Read/Write traits
/// over plain TCP or TLS connections.
#[derive(Debug, Clone)]
pub struct HttpClientConnection {
    stream: SharedByteBufferStream<RawStream>,
    host: String,
    port: u16,
}

impl DerefMut for HttpClientConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl Deref for HttpClientConnection {
    type Target = SharedByteBufferStream<RawStream>;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl HttpClientConnection {
    /// Establishes a connection to the given URL.
    ///
    /// # Type Parameters
    ///
    /// * `R` - DNS resolver type implementing `DnsResolver` trait
    ///
    /// # Arguments
    ///
    /// * `url` - Parsed URL to connect to
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `timeout` - Optional connection timeout
    ///
    /// # Returns
    ///
    /// An established `HttpClientConnection` with automatic buffering.
    ///
    /// # Errors
    ///
    /// Returns errors for:
    /// - DNS resolution failures
    /// - TCP connection failures
    /// - TLS handshake failures
    /// - Connection timeout
    pub fn connect<R: DnsResolver>(
        url: &ParsedUrl,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        // Get host as string (required for DNS resolution)
        let host = url
            .host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host".to_string()))?;

        // Get port (with default from scheme)
        let port = url.port_or_default();

        // Step 1: Resolve DNS
        let addrs = resolver.resolve(&host, port)?;

        if addrs.is_empty() {
            return Err(HttpClientError::ConnectionFailed(format!(
                "No addresses resolved for {host}"
            )));
        }

        // Step 2: Try connecting to each resolved address
        let mut last_error = None;

        for addr in addrs {
            let conn_result = if let Some(timeout_duration) = timeout {
                Connection::with_timeout(addr, timeout_duration)
            } else {
                Connection::without_timeout(addr)
            };

            match conn_result {
                Ok(connection) => {
                    // Step 3: Upgrade to TLS if HTTPS, or create plain RawStream
                    if url.scheme().is_https() {
                        return Self::upgrade_to_tls(connection, &host, port);
                    }
                    // Create plain RawStream from Connection
                    let stream = SharedByteBufferStream::rwrite(
                        RawStream::from_connection(connection)
                            .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?,
                    );
                    return Ok(HttpClientConnection { stream, host, port });
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        // All connection attempts failed
        if let Some(err) = last_error {
            if err.to_string().contains("timeout") || err.to_string().contains("timed out") {
                return Err(HttpClientError::ConnectionTimeout(format!(
                    "Connection to {host}:{port} timed out"
                )));
            }
            return Err(HttpClientError::ConnectionFailed(err.to_string()));
        }

        Err(HttpClientError::ConnectionFailed(format!(
            "Failed to connect to {host}:{port}"
        )))
    }

    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    fn upgrade_to_tls(
        connection: Connection,
        host: &str,
        port: u16,
    ) -> Result<Self, HttpClientError> {
        let connector = SSLConnector::new();
        let (tls_stream, _addr) = connector
            .from_tcp_stream(host.to_string(), connection)
            .map_err(|e: Box<dyn std::error::Error + Send + Sync>| {
                HttpClientError::TlsHandshakeFailed(e.to_string())
            })?;

        // Create RawStream from ClientSSLStream (which is the return type from from_tcp_stream)
        let stream = SharedByteBufferStream::rwrite(
            RawStream::from_client_tls(tls_stream)
                .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?,
        );

        Ok(HttpClientConnection {
            stream,
            host: host.into(),
            port,
        })
    }

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    fn upgrade_to_tls(_connection: Connection, _host: &str) -> Result<Self, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    /// Returns a reference to the underlying stream.
    #[must_use]
    pub fn stream(&self) -> &SharedByteBufferStream<RawStream> {
        &self.stream
    }

    /// Returns a mutable reference to the underlying stream.
    pub fn stream_mut(&mut self) -> &mut SharedByteBufferStream<RawStream> {
        &mut self.stream
    }

    // Clone and return the underlying SharedByteBufferStream.
    // This method consumes the HttpClientConnection to avoid borrowing issues,
    // allowing callers to move the connection in and obtain a cloned handle
    // to the same underlying stream for independent use.
    #[must_use]
    pub fn clone_stream(&self) -> SharedByteBufferStream<RawStream> {
        self.stream.clone()
    }

    /// Takes ownership of the underlying stream, consuming the connection.
    ///
    /// WHY: Allows transferring stream ownership to other components (e.g., `HttpResponseReader`)
    /// without lifetime issues.
    ///
    /// WHAT: Consumes the connection and returns the owned `RawStream`.
    ///
    /// # Returns
    ///
    /// The owned `RawStream` that was wrapped by this connection.
    #[must_use]
    pub fn take_stream(self) -> SharedByteBufferStream<RawStream> {
        self.stream
    }
}

/// A pool-aware HTTP connection factory.
///
/// Wraps the shared `ConnectionPool` and will attempt to reuse an existing
/// `RawStream` from the pool for the target host/port before creating a new
/// connection via `HttpClientConnection::connect`.
#[derive(Clone, Debug)]
pub struct HttpConnectionPool<R: DnsResolver> {
    pool: Arc<ConnectionPool>,
    resolver: Arc<R>,
}

impl Default for HttpConnectionPool<SystemDnsResolver> {
    fn default() -> Self {
        Self {
            pool: Arc::new(ConnectionPool::default()),
            resolver: Arc::new(SystemDnsResolver::new()),
        }
    }
}

impl<R: DnsResolver> HttpConnectionPool<R> {
    /// Create a new `HttpConnectionPool` from an existing `ConnectionPool`.
    #[must_use]
    pub fn new(pool: ConnectionPool, resolver: R) -> Self {
        Self {
            pool: Arc::new(pool),
            resolver: Arc::new(resolver),
        }
    }

    /// Create a new `HttpConnectionPool` from an `Arc<ConnectionPool>`.
    #[must_use]
    pub fn from_arc(pool: Arc<ConnectionPool>, resolver: Arc<R>) -> Self {
        Self { pool, resolver }
    }

    /// Acquire a `HttpClientConnection` for the given URL.
    ///
    /// The method will:
    /// 1. Try to obtain a pooled `RawStream` for the URL's host/port.
    /// 2. If a pooled stream is available, wrap it into `HttpClientConnection`.
    /// 3. Otherwise, establish a new connection via `HttpClientConnection::connect`.
    ///
    /// Note: This function assumes the underlying `ConnectionPool` exposes at least
    /// some method to retrieve and return `RawStream` instances keyed by host/port.
    /// The exact pool API may differ; callers can adapt or extend this wrapper as needed.
    ///
    /// # Errors
    ///
    /// Returns an `Err(HttpClientError)` when:
    /// - the provided `url` is invalid (missing host),
    /// - DNS resolution fails,
    /// - establishing a new TCP/TLS connection fails or times out.
    pub fn create_http_connection(
        &self,
        url: &ParsedUrl,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        // Extract host/port for pool lookup
        let host = url
            .host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host".to_string()))?;
        let port = url.port_or_default();

        // Try to obtain a pooled RawStream. Most pool implementations provide some
        // variant of `get`, `acquire`, or `take`. We optimistically call `checkout`.
        //
        // If your actual `ConnectionPool` API uses different method names, update
        // the calls here to match the real signatures.
        if let Some(stream) = self.pool.checkout(host.as_str(), port) {
            // Wrap the pooled RawStream into our HttpClientConnection and return.
            return Ok(HttpClientConnection { stream, host, port });
        }

        // No pooled connection available, create a fresh one.
        HttpClientConnection::connect(url, &*self.resolver, timeout)
    }

    /// Return a `RawStream` back into the pool for reuse.
    ///
    /// This is a best-effort helper that calls into the pool's `put`/`release`
    /// style API. Adjust the call if your pool uses a different method name.
    pub fn return_to_pool(&self, conn: HttpClientConnection) {
        // Destructure to move fields out without partially borrowing `conn`.
        let HttpClientConnection { host, port, stream } = conn;
        // Attempt to put the stream back; ignore failures to avoid panics.
        self.pool.checkin(host.as_str(), port, stream);
    }
}
