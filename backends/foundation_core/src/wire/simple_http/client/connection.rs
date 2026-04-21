//! Connection management for HTTP client.
//!
//! This module provides URL parsing and TCP/TLS connection establishment.

use crate::wire::simple_http::client::pool::ConnectionPool;
use crate::wire::simple_http::client::SystemDnsResolver;
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::{Connection, RawStream};
use crate::wire::simple_http::client::dns::DnsResolver;
use crate::wire::simple_http::url::Uri;
use crate::wire::simple_http::HttpClientError;
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
    pub stream: SharedByteBufferStream<RawStream>,
    host: String,
    port: u16,
}

impl DerefMut for HttpClientConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl HttpClientConnection {
    /// Drain any remaining data from a stream before returning it to the pool.
    ///
    /// This ensures the connection is in a clean state for reuse. After no-body
    /// responses (204, 205, HEAD), the HTTP reader may leave unread bytes or
    /// intermediate state in the underlying `SharedByteBufferStream`. Draining
    /// consumes any remaining buffered data and resets the stream position.
    ///
    /// # Arguments
    ///
    /// * `stream` - The stream to drain
    pub fn drain_stream(&mut self) {
        // Read any remaining buffered data into a small buffer
        // This ensures the HTTP response reader state machine is fully consumed
        let mut drain_buf = [0u8; 1024];
        loop {
            match self.stream.read(&mut drain_buf) {
                Ok(0) => break,    // EOF - stream fully drained
                Ok(_) => continue, // More data read, keep draining
                Err(_) => break,   // Read error - stop draining to avoid blocking
            }
        }
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
        Self::connect_with_tls_config(url, resolver, timeout, None)
    }

    pub fn connect_with_tls_config<R: DnsResolver>(
        url: &ParsedUrl,
        resolver: &R,
        timeout: Option<Duration>,
        tls_connector: Option<&SSLConnector>,
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
                    // Step 3: Upgrade to TLS if HTTPS or WSS, or create plain RawStream
                    if url.scheme().is_https() || url.scheme().is_wss() {
                        return Self::upgrade_to_tls_with_config(connection, &host, port, tls_connector);
                    }
                    // Create plain RawStream from Connection (for http:// and ws://)
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
        Self::upgrade_to_tls_with_config(connection, host, port, None)
    }

    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    fn upgrade_to_tls_with_config(
        connection: Connection,
        host: &str,
        port: u16,
        tls_connector: Option<&SSLConnector>,
    ) -> Result<Self, HttpClientError> {
        let default_connector;
        let connector = match tls_connector {
            Some(c) => c,
            None => {
                default_connector = SSLConnector::new();
                &default_connector
            }
        };
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

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    fn upgrade_to_tls_with_config(
        _connection: Connection,
        _host: &str,
        _port: u16,
        _tls_connector: Option<&SSLConnector>,
    ) -> Result<Self, HttpClientError> {
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

    /// Establishes an HTTP connection to the given host and port.
    ///
    /// WHY: Proxy connections need to connect to host:port directly without URL parsing.
    ///
    /// WHAT: Creates a plain TCP connection for HTTP (no TLS).
    ///
    /// HOW: DNS resolution → TCP connection → wrap in `SharedByteBufferStream`.
    ///
    /// # Arguments
    ///
    /// * `host` - Hostname or IP address
    /// * `port` - Port number
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `timeout` - Optional connection timeout
    ///
    /// # Returns
    ///
    /// An established `HttpClientConnection` with plain TCP connection.
    ///
    /// # Errors
    ///
    /// Returns errors for:
    /// - DNS resolution failures
    /// - TCP connection failures
    /// - Connection timeout
    pub fn connect_http_host_port<R: DnsResolver>(
        host: impl Into<String>,
        port: u16,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        let host = host.into();

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
                    // Create plain RawStream from Connection (no TLS)
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

    /// Establishes an HTTPS connection to the given host and port.
    ///
    /// WHY: Proxy connections need to connect to host:port directly without URL parsing.
    ///
    /// WHAT: Creates a TLS connection for HTTPS.
    ///
    /// HOW: DNS resolution → TCP connection → TLS handshake → wrap in `SharedByteBufferStream`.
    ///
    /// # Arguments
    ///
    /// * `host` - Hostname or IP address
    /// * `port` - Port number
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `timeout` - Optional connection timeout
    ///
    /// # Returns
    ///
    /// An established `HttpClientConnection` with TLS connection.
    ///
    /// # Errors
    ///
    /// Returns errors for:
    /// - DNS resolution failures
    /// - TCP connection failures
    /// - TLS handshake failures
    /// - Connection timeout
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    pub fn connect_https_host_port<R: DnsResolver>(
        host: impl Into<String>,
        port: u16,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        let host = host.into();

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
                    // Upgrade to TLS
                    return Self::upgrade_to_tls(connection, &host, port);
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

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    pub fn connect_https_host_port<R: DnsResolver>(
        _host: impl Into<String>,
        _port: u16,
        _resolver: &R,
        _timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }
}

/// A pool-aware HTTP connection factory.
///
/// Wraps the shared `ConnectionPool` and will attempt to reuse an existing
/// `RawStream` from the pool for the target host/port before creating a new
/// connection via `HttpClientConnection::connect`.
#[derive(Clone)]
pub struct HttpConnectionPool<R: DnsResolver> {
    pool: Arc<ConnectionPool>,
    resolver: Arc<R>,
    tls_connector: Option<Arc<SSLConnector>>,
}

impl<R: DnsResolver> std::fmt::Debug for HttpConnectionPool<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpConnectionPool")
            .field("pool", &self.pool)
            .field("tls_connector", &self.tls_connector.as_ref().map(|_| "SSLConnector(...)"))
            .finish()
    }
}

impl<R: DnsResolver + Default> Default for HttpConnectionPool<R> {
    fn default() -> Self {
        Self {
            pool: Arc::new(ConnectionPool::default()),
            resolver: Arc::new(R::default()),
            tls_connector: None,
        }
    }
}

impl HttpConnectionPool<SystemDnsResolver> {
    #[allow(dead_code)]
    fn system() -> Self {
        Self {
            pool: Arc::new(ConnectionPool::default()),
            resolver: Arc::new(SystemDnsResolver::new()),
            tls_connector: None,
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
            tls_connector: None,
        }
    }

    /// Create a new `HttpConnectionPool` with a custom TLS connector.
    #[must_use]
    pub fn with_tls_connector(mut self, connector: SSLConnector) -> Self {
        self.tls_connector = Some(Arc::new(connector));
        self
    }

    /// Returns the TLS connector if one is configured.
    #[must_use]
    pub fn tls_connector(&self) -> Option<&Arc<SSLConnector>> {
        self.tls_connector.as_ref()
    }

    /// Create a new `HttpConnectionPool` from an `Arc<ConnectionPool>`.
    #[must_use]
    pub fn from_arc(pool: Arc<ConnectionPool>, resolver: Arc<R>) -> Self {
        Self { pool, resolver, tls_connector: None }
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
        HttpClientConnection::connect_with_tls_config(url, &*self.resolver, timeout, self.tls_connector.as_deref())
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

    /// Establishes HTTP CONNECT tunnel through HTTP proxy.
    ///
    /// WHY: HTTP proxies use CONNECT method to tunnel HTTPS connections.
    ///
    /// WHAT: Connects to proxy, sends CONNECT request, validates 200 OK response.
    ///
    /// HOW: Connect to proxy → send CONNECT request → parse HTTP response → return tunnel connection.
    ///
    /// # Arguments
    ///
    /// * `proxy` - Proxy configuration (must be `ProxyProtocol::Http`)
    /// * `target_host` - Target server hostname
    /// * `target_port` - Target server port
    /// * `timeout` - Optional connection timeout
    ///
    /// # Returns
    ///
    /// A plain TCP `Connection` representing the tunnel to the target.
    ///
    /// # Errors
    ///
    /// Returns errors for:
    /// - Proxy connection failures
    /// - CONNECT request send failures
    /// - Non-200 proxy responses
    /// - Invalid proxy responses
    pub fn connect_via_http_proxy(
        &self,
        proxy: &crate::wire::simple_http::client::ProxyConfig,
        target_host: &str,
        target_port: u16,
        timeout: Option<Duration>,
    ) -> Result<Connection, HttpClientError> {
        use crate::wire::simple_http::client::ProxyProtocol;
        use crate::wire::simple_http::{HttpResponseReader, SimpleHttpBody};
        use std::io::Write;

        // Verify proxy protocol
        if !matches!(proxy.protocol, ProxyProtocol::Http) {
            return Err(HttpClientError::InvalidProxyUrl(
                "Expected HTTP proxy protocol".to_string(),
            ));
        }

        // Step 1: DNS resolution for proxy
        let addrs = self.resolver.resolve(&proxy.host, proxy.port)?;
        if addrs.is_empty() {
            return Err(HttpClientError::ConnectionFailed(format!(
                "No addresses resolved for proxy {}",
                proxy.host
            )));
        }

        // Step 2: Connect to proxy server (plain TCP)
        let mut connection = None;
        let mut last_error = None;

        for addr in addrs {
            let conn_result = if let Some(timeout_duration) = timeout {
                Connection::with_timeout(addr, timeout_duration)
            } else {
                Connection::without_timeout(addr)
            };

            match conn_result {
                Ok(conn) => {
                    connection = Some(conn);
                    break;
                }
                Err(e) => last_error = Some(e),
            }
        }

        let connection = connection.ok_or_else(|| {
            HttpClientError::ProxyConnectionFailed(format!(
                "Failed to connect to proxy {}: {}",
                proxy.host,
                last_error.map(|e| e.to_string()).unwrap_or_default()
            ))
        })?;

        // Step 3: Wrap temporarily to send CONNECT and read response
        let raw_stream = RawStream::from_connection(connection.try_clone().map_err(|e| {
            HttpClientError::ProxyConnectionFailed(format!("Failed to clone connection: {e}"))
        })?)
        .map_err(|e| HttpClientError::ProxyConnectionFailed(e.to_string()))?;

        let mut proxy_stream = SharedByteBufferStream::rwrite(raw_stream);

        // Step 4: Send CONNECT request
        let connect_request = if let Some(ref auth) = proxy.auth {
            format!(
                "CONNECT {}:{} HTTP/1.1\r\n\
                 Host: {}:{}\r\n\
                 Proxy-Authorization: Basic {}\r\n\
                 \r\n",
                target_host,
                target_port,
                target_host,
                target_port,
                auth.to_basic_auth()
            )
        } else {
            format!(
                "CONNECT {target_host}:{target_port} HTTP/1.1\r\n\
                 Host: {target_host}:{target_port}\r\n\
                 \r\n"
            )
        };

        proxy_stream
            .write_all(connect_request.as_bytes())
            .map_err(|e| HttpClientError::ProxyConnectionFailed(e.to_string()))?;

        proxy_stream
            .flush()
            .map_err(|e| HttpClientError::ProxyConnectionFailed(e.to_string()))?;

        // Step 5: Parse proxy response
        let stream_clone = proxy_stream.clone();
        let mut reader = HttpResponseReader::new(stream_clone, SimpleHttpBody::default());

        // Read intro line (status)
        let intro_response = reader
            .next()
            .ok_or_else(|| HttpClientError::ProxyTunnelFailed {
                status: 0,
                message: "No response from proxy".to_string(),
            })?
            .map_err(|e| HttpClientError::ProxyTunnelFailed {
                status: 0,
                message: format!("Failed to parse proxy response: {e}"),
            })?;

        // Extract status code from Intro variant
        use crate::wire::simple_http::IncomingResponseParts;
        let status_code = match intro_response {
            IncomingResponseParts::Intro(status, _proto, _text) => {
                let code: usize = status.into();
                code as u16
            }
            _ => {
                return Err(HttpClientError::ProxyTunnelFailed {
                    status: 0,
                    message: "Expected Intro response part from proxy".to_string(),
                })
            }
        };

        // Step 6: Verify 200 OK status
        if status_code != 200 {
            return Err(HttpClientError::ProxyTunnelFailed {
                status: status_code,
                message: format!("Proxy returned status {status_code}"),
            });
        }

        // Step 7: Drain headers from reader
        while let Some(Ok(part)) = reader.next() {
            use crate::wire::simple_http::IncomingResponseParts;
            match part {
                IncomingResponseParts::Headers(_) => continue,
                IncomingResponseParts::NoBody | IncomingResponseParts::SKIP => break,
                _ => break,
            }
        }

        // Step 8: Return the original Connection (tunnel is established)
        Ok(connection)
    }

    /// Establishes HTTP CONNECT tunnel through HTTPS proxy.
    ///
    /// WHY: HTTPS proxies require TLS connection to proxy before CONNECT tunnel.
    ///
    /// WHAT: Connects to proxy with TLS, sends CONNECT request, validates response.
    ///
    /// HOW: TLS connect to proxy → send CONNECT request → parse HTTP response → return tunnel stream.
    ///
    /// # Arguments
    ///
    /// * `proxy` - Proxy configuration (must be `ProxyProtocol::Https`)
    /// * `target_host` - Target server hostname
    /// * `target_port` - Target server port
    /// * `timeout` - Optional connection timeout
    ///
    /// # Returns
    ///
    /// An established `HttpClientConnection` representing the tunnel to the target.
    ///
    /// # Errors
    ///
    /// Returns errors for:
    /// - Proxy connection failures
    /// - TLS handshake failures
    /// - CONNECT request send failures
    /// - Non-200 proxy responses
    /// - Invalid proxy responses
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    pub fn connect_via_https_proxy(
        &self,
        proxy: &crate::wire::simple_http::client::ProxyConfig,
        target_host: &str,
        target_port: u16,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        use crate::wire::simple_http::client::ProxyProtocol;
        use crate::wire::simple_http::{HttpResponseReader, SimpleHttpBody};
        use std::io::Write;

        // Verify proxy protocol
        if !matches!(proxy.protocol, ProxyProtocol::Https) {
            return Err(HttpClientError::InvalidProxyUrl(
                "Expected HTTPS proxy protocol".to_string(),
            ));
        }

        // Step 1: Connect to proxy server with TLS
        let mut proxy_conn = HttpClientConnection::connect_https_host_port(
            &proxy.host,
            proxy.port,
            &*self.resolver,
            timeout,
        )?;

        // Step 2: Send CONNECT request
        let connect_request = if let Some(ref auth) = proxy.auth {
            format!(
                "CONNECT {}:{} HTTP/1.1\r\n\
                 Host: {}:{}\r\n\
                 Proxy-Authorization: Basic {}\r\n\
                 \r\n",
                target_host,
                target_port,
                target_host,
                target_port,
                auth.to_basic_auth()
            )
        } else {
            format!(
                "CONNECT {target_host}:{target_port} HTTP/1.1\r\n\
                 Host: {target_host}:{target_port}\r\n\
                 \r\n"
            )
        };

        proxy_conn
            .write_all(connect_request.as_bytes())
            .map_err(|e| HttpClientError::ProxyConnectionFailed(e.to_string()))?;

        proxy_conn
            .flush()
            .map_err(|e| HttpClientError::ProxyConnectionFailed(e.to_string()))?;

        // Step 3: Parse proxy response
        let stream = proxy_conn.clone_stream();
        let mut reader = HttpResponseReader::new(stream, SimpleHttpBody::default());

        // Read intro line (status)
        let intro_response = reader
            .next()
            .ok_or_else(|| HttpClientError::ProxyTunnelFailed {
                status: 0,
                message: "No response from proxy".to_string(),
            })?
            .map_err(|e| HttpClientError::ProxyTunnelFailed {
                status: 0,
                message: format!("Failed to parse proxy response: {e}"),
            })?;

        // Extract status code from Intro variant
        use crate::wire::simple_http::IncomingResponseParts;
        let status_code = match intro_response {
            IncomingResponseParts::Intro(status, _proto, _text) => {
                let code: usize = status.into();
                code as u16
            }
            _ => {
                return Err(HttpClientError::ProxyTunnelFailed {
                    status: 0,
                    message: "Expected Intro response part from proxy".to_string(),
                })
            }
        };

        // Step 4: Verify 200 OK status
        if status_code != 200 {
            return Err(HttpClientError::ProxyTunnelFailed {
                status: status_code,
                message: format!("Proxy returned status {status_code}"),
            });
        }

        // Step 5: Return the established tunnel connection
        // The stream is now tunneled to target_host:target_port through the proxy
        Ok(HttpClientConnection {
            stream: proxy_conn.stream,
            host: target_host.to_string(),
            port: target_port,
        })
    }

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    pub fn connect_via_https_proxy(
        &self,
        _proxy: &crate::wire::simple_http::client::ProxyConfig,
        _target_host: &str,
        _target_port: u16,
        _timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    /// Creates an HTTP connection, optionally through a proxy.
    ///
    /// WHY: Provides unified connection creation that handles direct connections,
    /// proxy tunnels, and `NO_PROXY` bypass logic.
    ///
    /// WHAT: Determines effective connection strategy based on proxy config and
    /// `NO_PROXY` settings, then establishes the appropriate connection type.
    ///
    /// HOW:
    /// 1. Check `NO_PROXY` bypass list
    /// 2. If bypassed, create direct connection
    /// 3. Otherwise, establish proxy tunnel based on protocol
    /// 4. For HTTPS targets through proxy, upgrade tunnel to TLS
    ///
    /// # Arguments
    ///
    /// * `url` - Target URL to connect to
    /// * `proxy` - Optional proxy configuration
    /// * `timeout` - Optional connection timeout
    ///
    /// # Returns
    ///
    /// An established `HttpClientConnection` ready for HTTP requests.
    ///
    /// # Errors
    ///
    /// Returns errors for:
    /// - Invalid URL (missing host)
    /// - DNS resolution failures
    /// - Connection failures
    /// - Proxy tunnel establishment failures
    /// - TLS handshake failures
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Direct connection (no proxy)
    /// let conn = pool.create_connection_with_proxy(&url, None, timeout)?;
    ///
    /// // Through HTTP proxy
    /// let proxy = ProxyConfig::parse("http://proxy.example.com:8080")?;
    /// let conn = pool.create_connection_with_proxy(&url, Some(&proxy), timeout)?;
    ///
    /// // Proxy with NO_PROXY bypass
    /// std::env::set_var("NO_PROXY", "localhost,.internal.com");
    /// let conn = pool.create_connection_with_proxy(&url, Some(&proxy), timeout)?;
    /// // Will bypass proxy for localhost and *.internal.com
    /// ```
    pub fn create_connection_with_proxy(
        &self,
        url: &ParsedUrl,
        proxy: Option<&crate::wire::simple_http::client::ProxyConfig>,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        use crate::wire::simple_http::client::{ProxyConfig, ProxyProtocol};

        // Extract target host and port
        let target_host = url
            .host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host".to_string()))?;
        let target_port = url.port_or_default();

        // Check if proxy should be bypassed
        if let Some(proxy_config) = proxy {
            if ProxyConfig::should_bypass(&target_host) {
                // Bypass proxy, create direct connection
                return self.create_http_connection(url, timeout);
            }

            // Establish proxy tunnel based on protocol
            match proxy_config.protocol {
                ProxyProtocol::Http => {
                    // HTTP proxy returns plain Connection
                    let tunnel_connection = self.connect_via_http_proxy(
                        proxy_config,
                        &target_host,
                        target_port,
                        timeout,
                    )?;

                    // If target is HTTPS, upgrade the tunnel Connection to TLS
                    if url.scheme().is_https() {
                        return HttpClientConnection::upgrade_to_tls_with_config(
                            tunnel_connection,
                            &target_host,
                            target_port,
                            self.tls_connector.as_deref(),
                        );
                    }

                    // HTTP target - wrap plain Connection in HttpClientConnection
                    let stream = SharedByteBufferStream::rwrite(
                        RawStream::from_connection(tunnel_connection)
                            .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?,
                    );
                    Ok(HttpClientConnection {
                        stream,
                        host: target_host.clone(),
                        port: target_port,
                    })
                }
                ProxyProtocol::Https => {
                    // HTTPS proxy returns HttpClientConnection (TLS to proxy)
                    let tunnel_conn = self.connect_via_https_proxy(
                        proxy_config,
                        &target_host,
                        target_port,
                        timeout,
                    )?;

                    // HTTPS targets through HTTPS proxy would require double-TLS (not yet supported)
                    if url.scheme().is_https() {
                        return Err(HttpClientError::NotSupported);
                    }

                    // HTTP target through HTTPS proxy - return as-is
                    Ok(tunnel_conn)
                }
                #[cfg(feature = "socks5")]
                ProxyProtocol::Socks5 => Err(HttpClientError::Socks5Error(
                    "SOCKS5 proxy not yet implemented".to_string(),
                )),
            }
        } else {
            // No proxy, create direct connection
            self.create_http_connection(url, timeout)
        }
    }
}
