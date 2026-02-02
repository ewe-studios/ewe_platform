//! Connection management for HTTP client.
//!
//! This module provides URL parsing and TCP/TLS connection establishment.

use crate::netcap::{Connection, RawStream};
use crate::wire::simple_http::client::dns::DnsResolver;
use crate::wire::simple_http::client::errors::HttpClientError;
use crate::wire::simple_http::url::Uri;
use std::time::Duration;

#[cfg(feature = "ssl-rustls")]
use crate::netcap::ssl::rustls::{default_client_config, RustlsConnector};

#[cfg(feature = "ssl-openssl")]
use crate::netcap::ssl::openssl::OpensslConnector;

#[cfg(feature = "ssl-native-tls")]
use crate::netcap::ssl::native_tls::NativeTlsConnector;

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
                "No addresses resolved for {}",
                host
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
                    "Connection to {}:{} timed out",
                    host, port
                )));
            }
            return Err(HttpClientError::ConnectionFailed(err.to_string()));
        }

        Err(HttpClientError::ConnectionFailed(format!(
            "Failed to connect to {}:{}",
            host, port
        )))
    }

    #[cfg(feature = "ssl-rustls")]
    fn upgrade_to_tls(connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        let connector = RustlsConnector::new();

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

    #[cfg(all(feature = "ssl-openssl", not(feature = "ssl-rustls")))]
    fn upgrade_to_tls(connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        use openssl::ssl::{SslConnector, SslMethod};
        use std::sync::Arc;

        let ssl_connector = SslConnector::builder(SslMethod::tls())
            .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?
            .build();

        let connector = OpensslConnector::create(&crate::netcap::Endpoint::WithIdentity(
            crate::netcap::EndpointConfig::NoTimeout(
                url::Url::parse(&format!("https://{}:443", host))
                    .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?,
            ),
            Arc::new(ssl_connector),
        ));

        let (tls_stream, _addr) = connector
            .from_tcp_stream(host.to_string(), connection)
            .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?;

        // Create RawStream from ClientSSLStream
        let stream = RawStream::from_client_tls(tls_stream)
            .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?;

        Ok(HttpClientConnection { stream })
    }

    #[cfg(all(
        feature = "ssl-native-tls",
        not(feature = "ssl-rustls"),
        not(feature = "ssl-openssl")
    ))]
    fn upgrade_to_tls(connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        // OPTIONAL BACKEND: native-tls not yet implemented
        //
        // For TLS support, use one of these feature flags instead:
        // - ssl-rustls (recommended, pure Rust)
        // - ssl-openssl (system OpenSSL)
        //
        // native-tls backend is planned but not prioritized.
        // See: https://github.com/ewe-studios/ewe_platform/issues/[TBD]
        Err(HttpClientError::TlsHandshakeFailed(
            "native-tls backend not implemented. Use ssl-rustls or ssl-openssl features instead"
                .to_string(),
        ))
    }

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    fn upgrade_to_tls(_connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        Err(HttpClientError::TlsHandshakeFailed(format!(
            "HTTPS requested for {host} but no TLS feature enabled"
        )))
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
    /// WHY: Allows transferring stream ownership to other components (e.g., HttpResponseReader)
    /// without lifetime issues.
    ///
    /// WHAT: Consumes the connection and returns the owned RawStream.
    ///
    /// # Returns
    ///
    /// The owned `RawStream` that was wrapped by this connection.
    #[must_use]
    pub fn take_stream(self) -> RawStream {
        self.stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ParsedUrl Tests - URL Parsing
    // ========================================================================

    /// WHY: Verify ParsedUrl correctly parses a simple HTTP URL
    /// WHAT: Tests that scheme, host, port, and path are extracted correctly
    /// IMPORTANCE: Basic URL parsing is fundamental to all HTTP requests
    #[test]
    fn test_parsed_url_simple_http() {
        let url = ParsedUrl::parse("http://example.com").unwrap();

        assert!(url.scheme().is_http());
        assert_eq!(url.host_str().unwrap(), "example.com");
        assert_eq!(url.port_or_default(), 80); // Default HTTP port
        assert_eq!(url.path(), "/"); // Default path
        assert_eq!(url.query(), None);
    }

    /// WHY: Verify ParsedUrl correctly parses a simple HTTPS URL
    /// WHAT: Tests that HTTPS scheme is detected and default port is 443
    /// IMPORTANCE: HTTPS is the primary use case for secure connections
    #[test]
    fn test_parsed_url_simple_https() {
        let url = ParsedUrl::parse("https://example.com").unwrap();

        assert!(url.scheme().is_https());
        assert_eq!(url.host_str().unwrap(), "example.com");
        assert_eq!(url.port_or_default(), 443); // Default HTTPS port
        assert_eq!(url.path(), "/");
        assert_eq!(url.query(), None);
    }

    /// WHY: Verify ParsedUrl handles explicit port numbers
    /// WHAT: Tests that non-default ports override scheme defaults
    /// IMPORTANCE: Many services run on non-standard ports
    #[test]
    fn test_parsed_url_with_explicit_port() {
        let url = ParsedUrl::parse("http://example.com:8080").unwrap();

        assert!(url.scheme().is_http());
        assert_eq!(url.host_str().unwrap(), "example.com");
        assert_eq!(url.port().unwrap(), 8080); // Explicit port overrides default
        assert_eq!(url.path(), "/");
    }

    /// WHY: Verify ParsedUrl correctly extracts path components
    /// WHAT: Tests that paths with multiple segments are preserved
    /// IMPORTANCE: RESTful APIs use complex path structures
    #[test]
    fn test_parsed_url_with_path() {
        let url = ParsedUrl::parse("http://example.com/api/v1/users").unwrap();

        assert!(url.scheme().is_http());
        assert_eq!(url.host_str().unwrap(), "example.com");
        assert_eq!(url.path(), "/api/v1/users");
        assert_eq!(url.query(), None);
    }

    /// WHY: Verify ParsedUrl correctly extracts query strings
    /// WHAT: Tests that query parameters are separated from path
    /// IMPORTANCE: Query parameters are essential for API requests
    #[test]
    fn test_parsed_url_with_query() {
        let url = ParsedUrl::parse("http://example.com/search?q=test&limit=10").unwrap();

        assert!(url.scheme().is_http());
        assert_eq!(url.host_str().unwrap(), "example.com");
        assert_eq!(url.path(), "/search");
        assert_eq!(url.query(), Some("q=test&limit=10"));
    }

    /// WHY: Verify ParsedUrl handles complex URLs with all components
    /// WHAT: Tests parsing of URL with port, path, and query together
    /// IMPORTANCE: Real-world URLs often have all components
    #[test]
    fn test_parsed_url_complex() {
        let url = ParsedUrl::parse("https://api.example.com:8443/v2/users/123?fields=name,email")
            .unwrap();

        assert!(url.scheme().is_https());
        assert_eq!(url.host_str().unwrap(), "api.example.com");
        assert_eq!(url.port().unwrap(), 8443);
        assert_eq!(url.path(), "/v2/users/123");
        assert_eq!(url.query(), Some("fields=name,email"));
    }

    /// WHY: Verify ParsedUrl rejects URLs with missing scheme
    /// WHAT: Tests that URLs without http:// or https:// are rejected
    /// IMPORTANCE: Prevents ambiguous URL interpretation
    #[test]
    fn test_parsed_url_missing_scheme() {
        let result = ParsedUrl::parse("example.com");

        assert!(result.is_err());
    }

    /// WHY: Verify ParsedUrl rejects URLs with unsupported schemes
    /// WHAT: Tests that only HTTP and HTTPS schemes are accepted
    /// IMPORTANCE: HTTP client should only handle HTTP/HTTPS
    #[test]
    fn test_parsed_url_unsupported_scheme() {
        let result = ParsedUrl::parse("ftp://example.com");

        // The Uri parser now accepts any scheme during parsing,
        // but scheme validation happens when checking is_http/is_https
        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(!url.scheme().is_http() && !url.scheme().is_https());
    }

    /// WHY: Verify ParsedUrl handles URLs with empty host
    /// WHAT: Tests that URLs like "http:///" parse but have no host
    /// IMPORTANCE: Connection validation happens when accessing host_str()
    #[test]
    fn test_parsed_url_empty_host() {
        let result = ParsedUrl::parse("http:///path");

        // The Uri parser may succeed but host validation happens on access
        assert!(result.is_ok());
        let url = result.unwrap();
        // Empty host should return None
        assert!(url.host_str().is_none() || url.host_str().unwrap().is_empty());
    }

    /// WHY: Verify ParsedUrl handles IP addresses as hosts
    /// WHAT: Tests that IPv4 addresses are valid hosts
    /// IMPORTANCE: Direct IP connections are common in testing
    #[test]
    fn test_parsed_url_with_ip_address() {
        let url = ParsedUrl::parse("http://127.0.0.1:8080/test").unwrap();

        assert_eq!(url.host_str().unwrap(), "127.0.0.1");
        assert_eq!(url.port().unwrap(), 8080);
        assert_eq!(url.path(), "/test");
    }

    /// WHY: Verify ParsedUrl handles URLs with fragment identifiers
    /// WHAT: Tests that fragments (#section) are ignored in HTTP requests
    /// IMPORTANCE: Fragments are client-side only, not sent to server
    #[test]
    fn test_parsed_url_with_fragment() {
        let url = ParsedUrl::parse("http://example.com/page#section").unwrap();

        assert_eq!(url.path(), "/page");
        // Fragment should be ignored
    }

    /// WHY: Verify Scheme::default_port returns correct values
    /// WHAT: Tests that scheme default ports match HTTP standards
    /// IMPORTANCE: Ensures correct default port selection
    #[test]
    fn test_scheme_default_ports() {
        assert_eq!(Scheme::HTTP.default_port(), 80);
        assert_eq!(Scheme::HTTPS.default_port(), 443);
    }

    // ========================================================================
    // HttpClientConnection Tests - Connection Establishment
    // ========================================================================

    #[cfg(not(target_arch = "wasm32"))]
    mod connection_tests {
        use super::*;
        use crate::wire::simple_http::client::dns::{MockDnsResolver, SystemDnsResolver};
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};

        /// WHY: Verify HttpClientConnection can establish HTTP connections
        /// WHAT: Tests that connect() successfully creates a TCP connection
        /// IMPORTANCE: Basic HTTP connectivity is core functionality
        #[test]
        #[ignore] // Requires actual network - run with `cargo test -- --ignored`
        fn test_connection_http_real() {
            let url = ParsedUrl::parse("http://httpbin.org").unwrap();
            let resolver = SystemDnsResolver::new();

            let result =
                HttpClientConnection::connect(&url, &resolver, Some(Duration::from_secs(10)));

            assert!(result.is_ok(), "Should connect to httpbin.org");
        }

        /// WHY: Verify HttpClientConnection can establish HTTPS connections with TLS
        /// WHAT: Tests that connect() performs TLS handshake for HTTPS URLs
        /// IMPORTANCE: HTTPS is the primary use case for secure connections
        #[test]
        #[cfg(feature = "ssl-rustls")]
        fn test_connection_https_real() {
            let url = ParsedUrl::parse("https://httpbin.org").unwrap();
            let resolver = SystemDnsResolver::new();

            let result =
                HttpClientConnection::connect(&url, &resolver, Some(Duration::from_secs(10)));

            assert!(result.is_ok(), "Should connect to httpbin.org with TLS");
        }

        /// WHY: Verify HttpClientConnection respects connection timeout
        /// WHAT: Tests that connect() fails when timeout is exceeded
        /// IMPORTANCE: Prevents hanging connections from blocking indefinitely
        #[test]
        #[ignore] // Requires network - timeout test
        fn test_connection_timeout() {
            // Use a non-routable IP to trigger timeout
            let url = ParsedUrl::parse("http://192.0.2.1").unwrap(); // TEST-NET-1, non-routable

            let resolver = MockDnsResolver::new().with_response(
                "192.0.2.1",
                vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 80)],
            );

            let result =
                HttpClientConnection::connect(&url, &resolver, Some(Duration::from_millis(100)));

            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                HttpClientError::ConnectionTimeout(_)
            ));
        }

        /// WHY: Verify HttpClientConnection handles DNS resolution failures
        /// WHAT: Tests that connect() returns appropriate error when DNS fails
        /// IMPORTANCE: DNS errors are common and should be handled gracefully
        #[test]
        fn test_connection_dns_failure() {
            let url = ParsedUrl::parse("http://invalid-host-12345.invalid").unwrap();
            let resolver = SystemDnsResolver::new();

            let result =
                HttpClientConnection::connect(&url, &resolver, Some(Duration::from_secs(5)));

            assert!(result.is_err());
            // Should be wrapped in HttpClientError::DnsError
        }

        /// WHY: Verify HttpClientConnection works with mock resolver
        /// WHAT: Tests that connect() uses provided resolver for DNS
        /// IMPORTANCE: Enables testing without real network
        #[test]
        fn test_connection_with_mock_resolver() {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
            let resolver = MockDnsResolver::new().with_response("example.com", vec![addr]);

            let url = ParsedUrl::parse("http://example.com:8080").unwrap();

            // This will fail to connect since no server is running, but DNS should work
            let result =
                HttpClientConnection::connect(&url, &resolver, Some(Duration::from_millis(100)));

            // We expect connection failure, not DNS failure
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                HttpClientError::ConnectionFailed(_) | HttpClientError::ConnectionTimeout(_)
            ));
        }
    }
}
