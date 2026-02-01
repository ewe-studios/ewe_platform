//! Connection management for HTTP client.
//!
//! This module provides URL parsing and TCP/TLS connection establishment.

use crate::netcap::Connection;
use crate::wire::simple_http::client::dns::DnsResolver;
use crate::wire::simple_http::client::errors::HttpClientError;
use std::time::Duration;

#[cfg(feature = "ssl-rustls")]
use crate::netcap::ssl::rustls::{default_client_config, RustlsConnector};

#[cfg(feature = "ssl-openssl")]
use crate::netcap::ssl::openssl::OpensslConnector;

#[cfg(feature = "ssl-native-tls")]
use crate::netcap::ssl::native_tls::NativeTlsConnector;

/// URL scheme (HTTP or HTTPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// HTTP (unencrypted)
    Http,
    /// HTTPS (TLS encrypted)
    Https,
}

impl Scheme {
    /// Returns the default port for this scheme.
    #[must_use] 
    pub fn default_port(&self) -> u16 {
        match self {
            Scheme::Http => 80,
            Scheme::Https => 443,
        }
    }
}

/// Parsed URL components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    /// URL scheme (HTTP or HTTPS)
    pub scheme: Scheme,
    /// Hostname or IP address
    pub host: String,
    /// Port number
    pub port: u16,
    /// Path component (always starts with /)
    pub path: String,
    /// Query string (without the ?)
    pub query: Option<String>,
}

impl ParsedUrl {
    /// Parses a URL string into components.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to parse (e.g., "<http://example.com/path?query>")
    ///
    /// # Returns
    ///
    /// A `ParsedUrl` with all components extracted.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if:
    /// - URL format is invalid
    /// - Scheme is not HTTP or HTTPS
    /// - Host is missing or empty
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::connection::ParsedUrl;
    ///
    /// let url = ParsedUrl::parse("http://example.com/path").unwrap();
    /// assert_eq!(url.host, "example.com");
    /// assert_eq!(url.port, 80);
    /// assert_eq!(url.path, "/path");
    /// ```
    pub fn parse(url: &str) -> Result<Self, HttpClientError> {
        // Simple URL parsing without external dependencies
        // Format: scheme://host[:port][/path][?query][#fragment]

        // Find scheme separator
        let scheme_end = url.find("://").ok_or_else(|| {
            HttpClientError::InvalidUrl("Missing scheme (use http:// or https://)".to_string())
        })?;

        let scheme_str = &url[..scheme_end];
        let scheme = match scheme_str.to_lowercase().as_str() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            _ => return Err(HttpClientError::InvalidScheme(scheme_str.to_string())),
        };

        let after_scheme = &url[scheme_end + 3..]; // Skip "://"

        // Split off fragment (we ignore it)
        let without_fragment = after_scheme.split('#').next().unwrap_or(after_scheme);

        // Split into authority (host:port) and path+query
        let (authority, path_and_query) = if let Some(slash_pos) = without_fragment.find('/') {
            (
                &without_fragment[..slash_pos],
                &without_fragment[slash_pos..],
            )
        } else {
            (without_fragment, "/")
        };

        // Parse authority (host[:port])
        let (host, port) = if let Some(colon_pos) = authority.rfind(':') {
            // Has explicit port
            let host = &authority[..colon_pos];
            let port_str = &authority[colon_pos + 1..];
            let port = port_str
                .parse::<u16>()
                .map_err(|_| HttpClientError::InvalidUrl(format!("Invalid port: {port_str}")))?;
            (host, port)
        } else {
            // Use default port
            (authority, scheme.default_port())
        };

        // Validate host is not empty
        if host.is_empty() {
            return Err(HttpClientError::InvalidUrl("Empty host".to_string()));
        }

        // Split path and query
        let (path, query) = if let Some(query_pos) = path_and_query.find('?') {
            let path = &path_and_query[..query_pos];
            let query = &path_and_query[query_pos + 1..];
            (
                path,
                if query.is_empty() {
                    None
                } else {
                    Some(query.to_string())
                },
            )
        } else {
            (path_and_query, None)
        };

        Ok(ParsedUrl {
            scheme,
            host: host.to_string(),
            port,
            path: path.to_string(),
            query,
        })
    }
}

/// HTTP client connection wrapping `netcap::Connection`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct HttpClientConnection {
    connection: Connection,
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
    /// An established `HttpClientConnection`.
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
        // Step 1: Resolve DNS
        let addrs = resolver.resolve(&url.host, url.port)?;

        if addrs.is_empty() {
            return Err(HttpClientError::ConnectionFailed(format!(
                "No addresses resolved for {}",
                url.host
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
                    // Step 3: Upgrade to TLS if HTTPS
                    if url.scheme == Scheme::Https {
                        return Self::upgrade_to_tls(connection, &url.host);
                    }
                    return Ok(HttpClientConnection { connection });
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
                    url.host, url.port
                )));
            }
            return Err(HttpClientError::ConnectionFailed(err.to_string()));
        }

        Err(HttpClientError::ConnectionFailed(format!(
            "Failed to connect to {}:{}",
            url.host, url.port
        )))
    }

    #[cfg(feature = "ssl-rustls")]
    fn upgrade_to_tls(connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        let connector = RustlsConnector::new();

        let (_tls_stream, _addr) = connector
            .from_tcp_stream(host.to_string(), connection)
            .map_err(|e: Box<dyn std::error::Error + Send + Sync>| {
                HttpClientError::TlsHandshakeFailed(e.to_string())
            })?;

        // TODO: We need to convert RustTlsClientStream back to Connection
        // For now, this is a limitation of the current netcap design
        Err(HttpClientError::TlsHandshakeFailed(
            "TLS stream conversion not yet implemented".to_string(),
        ))
    }

    #[cfg(all(feature = "ssl-openssl", not(feature = "ssl-rustls")))]
    fn upgrade_to_tls(connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        // TODO: Implement OpenSSL TLS upgrade
        Err(HttpClientError::TlsHandshakeFailed(
            "OpenSSL TLS not yet implemented".to_string(),
        ))
    }

    #[cfg(all(
        feature = "ssl-native-tls",
        not(feature = "ssl-rustls"),
        not(feature = "ssl-openssl")
    ))]
    fn upgrade_to_tls(connection: Connection, host: &str) -> Result<Self, HttpClientError> {
        // TODO: Implement native-tls TLS upgrade
        Err(HttpClientError::TlsHandshakeFailed(
            "native-tls not yet implemented".to_string(),
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

    /// Returns a reference to the underlying connection.
    #[must_use] 
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Returns a mutable reference to the underlying connection.
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
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

        assert_eq!(url.scheme, Scheme::Http);
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 80); // Default HTTP port
        assert_eq!(url.path, "/"); // Default path
        assert_eq!(url.query, None);
    }

    /// WHY: Verify ParsedUrl correctly parses a simple HTTPS URL
    /// WHAT: Tests that HTTPS scheme is detected and default port is 443
    /// IMPORTANCE: HTTPS is the primary use case for secure connections
    #[test]
    fn test_parsed_url_simple_https() {
        let url = ParsedUrl::parse("https://example.com").unwrap();

        assert_eq!(url.scheme, Scheme::Https);
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 443); // Default HTTPS port
        assert_eq!(url.path, "/");
        assert_eq!(url.query, None);
    }

    /// WHY: Verify ParsedUrl handles explicit port numbers
    /// WHAT: Tests that non-default ports override scheme defaults
    /// IMPORTANCE: Many services run on non-standard ports
    #[test]
    fn test_parsed_url_with_explicit_port() {
        let url = ParsedUrl::parse("http://example.com:8080").unwrap();

        assert_eq!(url.scheme, Scheme::Http);
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 8080); // Explicit port overrides default
        assert_eq!(url.path, "/");
    }

    /// WHY: Verify ParsedUrl correctly extracts path components
    /// WHAT: Tests that paths with multiple segments are preserved
    /// IMPORTANCE: RESTful APIs use complex path structures
    #[test]
    fn test_parsed_url_with_path() {
        let url = ParsedUrl::parse("http://example.com/api/v1/users").unwrap();

        assert_eq!(url.scheme, Scheme::Http);
        assert_eq!(url.host, "example.com");
        assert_eq!(url.path, "/api/v1/users");
        assert_eq!(url.query, None);
    }

    /// WHY: Verify ParsedUrl correctly extracts query strings
    /// WHAT: Tests that query parameters are separated from path
    /// IMPORTANCE: Query parameters are essential for API requests
    #[test]
    fn test_parsed_url_with_query() {
        let url = ParsedUrl::parse("http://example.com/search?q=test&limit=10").unwrap();

        assert_eq!(url.scheme, Scheme::Http);
        assert_eq!(url.host, "example.com");
        assert_eq!(url.path, "/search");
        assert_eq!(url.query, Some("q=test&limit=10".to_string()));
    }

    /// WHY: Verify ParsedUrl handles complex URLs with all components
    /// WHAT: Tests parsing of URL with port, path, and query together
    /// IMPORTANCE: Real-world URLs often have all components
    #[test]
    fn test_parsed_url_complex() {
        let url = ParsedUrl::parse("https://api.example.com:8443/v2/users/123?fields=name,email")
            .unwrap();

        assert_eq!(url.scheme, Scheme::Https);
        assert_eq!(url.host, "api.example.com");
        assert_eq!(url.port, 8443);
        assert_eq!(url.path, "/v2/users/123");
        assert_eq!(url.query, Some("fields=name,email".to_string()));
    }

    /// WHY: Verify ParsedUrl rejects URLs with missing scheme
    /// WHAT: Tests that URLs without http:// or https:// are rejected
    /// IMPORTANCE: Prevents ambiguous URL interpretation
    #[test]
    fn test_parsed_url_missing_scheme() {
        let result = ParsedUrl::parse("example.com");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HttpClientError::InvalidUrl(_)
        ));
    }

    /// WHY: Verify ParsedUrl rejects URLs with unsupported schemes
    /// WHAT: Tests that only HTTP and HTTPS schemes are accepted
    /// IMPORTANCE: HTTP client should only handle HTTP/HTTPS
    #[test]
    fn test_parsed_url_unsupported_scheme() {
        let result = ParsedUrl::parse("ftp://example.com");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HttpClientError::InvalidScheme(_)
        ));
    }

    /// WHY: Verify ParsedUrl rejects URLs with empty host
    /// WHAT: Tests that URLs like "http://" or "http:///" are rejected
    /// IMPORTANCE: Connection requires a valid hostname
    #[test]
    fn test_parsed_url_empty_host() {
        let result = ParsedUrl::parse("http:///path");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HttpClientError::InvalidUrl(_)
        ));
    }

    /// WHY: Verify ParsedUrl handles IP addresses as hosts
    /// WHAT: Tests that IPv4 addresses are valid hosts
    /// IMPORTANCE: Direct IP connections are common in testing
    #[test]
    fn test_parsed_url_with_ip_address() {
        let url = ParsedUrl::parse("http://127.0.0.1:8080/test").unwrap();

        assert_eq!(url.host, "127.0.0.1");
        assert_eq!(url.port, 8080);
        assert_eq!(url.path, "/test");
    }

    /// WHY: Verify ParsedUrl handles URLs with fragment identifiers
    /// WHAT: Tests that fragments (#section) are ignored in HTTP requests
    /// IMPORTANCE: Fragments are client-side only, not sent to server
    #[test]
    fn test_parsed_url_with_fragment() {
        let url = ParsedUrl::parse("http://example.com/page#section").unwrap();

        assert_eq!(url.path, "/page");
        // Fragment should be ignored
    }

    /// WHY: Verify Scheme::default_port returns correct values
    /// WHAT: Tests that scheme default ports match HTTP standards
    /// IMPORTANCE: Ensures correct default port selection
    #[test]
    fn test_scheme_default_ports() {
        assert_eq!(Scheme::Http.default_port(), 80);
        assert_eq!(Scheme::Https.default_port(), 443);
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
        #[ignore] // Requires actual network and TLS feature
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
            let url = ParsedUrl {
                scheme: Scheme::Http,
                host: "192.0.2.1".to_string(), // TEST-NET-1, non-routable
                port: 80,
                path: "/".to_string(),
                query: None,
            };

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

            let url = ParsedUrl {
                scheme: Scheme::Http,
                host: "example.com".to_string(),
                port: 8080,
                path: "/".to_string(),
                query: None,
            };

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
