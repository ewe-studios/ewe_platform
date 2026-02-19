//! Connection management for HTTP client.
//!
//! This module provides URL parsing and TCP/TLS connection establishment.

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
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct HttpClientConnection {
    stream: RawStream,
}

#[cfg(not(target_arch = "wasm32"))]
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
                        return Self::upgrade_to_tls(connection, &host);
                    }
                    // Create plain RawStream from Connection
                    let stream = RawStream::from_connection(connection)
                        .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?;
                    return Ok(HttpClientConnection { stream });
                }
                Err(e) => {
                    last_error = Some(e);
                    continue; // Try next address
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
    fn upgrade_to_tls(connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        let connector = SSLConnector::new();
        let (tls_stream, _addr) = connector
            .from_tcp_stream(host.to_string(), connection)
            .map_err(|e: Box<dyn std::error::Error + Send + Sync>| {
                HttpClientError::TlsHandshakeFailed(e.to_string())
            })?;

        // Create RawStream from ClientSSLStream (which is the return type from from_tcp_stream)
        let stream = RawStream::from_client_tls(tls_stream)
            .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?;

        Ok(HttpClientConnection { stream })
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
    pub fn stream(&self) -> &RawStream {
        &self.stream
    }

    /// Returns a mutable reference to the underlying stream.
    pub fn stream_mut(&mut self) -> &mut RawStream {
        &mut self.stream
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
    pub fn take_stream(self) -> RawStream {
        self.stream
    }
}
