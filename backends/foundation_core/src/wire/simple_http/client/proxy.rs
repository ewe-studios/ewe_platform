use crate::wire::simple_http::url::Scheme;
/// Proxy support for HTTP client.
///
/// WHY: HTTP clients often need to route requests through proxy servers for
/// security, monitoring, or network architecture requirements.
///
/// WHAT: Provides proxy configuration (HTTP, HTTPS, SOCKS5), URL parsing,
/// environment variable detection, and authentication support.
///
/// HOW: Extends HttpConnectionPool with proxy connection methods that establish
/// CONNECT tunnels through proxies, parse responses with HttpResponseReader,
/// and optionally upgrade to TLS for target servers.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::proxy::{ProxyConfig, ProxyProtocol};
///
/// // Parse proxy URL
/// let proxy = ProxyConfig::parse("http://proxy.example.com:8080").unwrap();
///
/// // With authentication
/// let proxy = ProxyConfig::parse("http://user:pass@proxy.example.com:8080").unwrap();
///
/// // Check if host should bypass proxy
/// std::env::set_var("NO_PROXY", "localhost,.internal.com");
/// assert!(ProxyConfig::should_bypass("localhost"));
/// assert!(ProxyConfig::should_bypass("api.internal.com"));
/// std::env::remove_var("NO_PROXY");
/// ```
use crate::wire::simple_http::HttpClientError;

/// Proxy protocol type.
///
/// WHY: Different proxy types require different connection logic (HTTP CONNECT vs SOCKS5).
///
/// WHAT: Enum representing HTTP, HTTPS, and SOCKS5 proxy protocols.
///
/// HOW: Simple enum with three variants, SOCKS5 feature-gated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyProtocol {
    /// HTTP proxy using CONNECT method for HTTPS targets
    Http,

    /// HTTPS proxy (TLS to proxy, then CONNECT tunnel)
    Https,

    /// SOCKS5 proxy (feature-gated, requires "socks5" feature)
    #[cfg(feature = "socks5")]
    Socks5,
}

/// Proxy authentication credentials.
///
/// WHY: Many proxy servers require authentication.
///
/// WHAT: Username and password for proxy authentication, with Base64 encoding for headers.
///
/// HOW: Simple struct with two String fields, provides to_basic_auth() for header generation.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::proxy::ProxyAuth;
///
/// let auth = ProxyAuth::new("user", "password");
/// let header_value = auth.to_basic_auth();
/// assert_eq!(header_value, "dXNlcjpwYXNzd29yZA=="); // Base64("user:password")
/// ```
#[derive(Debug, Clone)]
pub struct ProxyAuth {
    /// Username for proxy authentication
    pub username: String,

    /// Password for proxy authentication
    pub password: String,
}

impl ProxyAuth {
    /// Create new proxy authentication credentials.
    ///
    /// WHY: Constructor for ergonomic credential creation.
    ///
    /// WHAT: Accepts username and password as any type convertible to String.
    ///
    /// HOW: Converts inputs with .into() for flexibility.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::proxy::ProxyAuth;
    ///
    /// let auth = ProxyAuth::new("user", "password");
    /// assert_eq!(auth.username, "user");
    /// assert_eq!(auth.password, "password");
    /// ```
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Encode credentials as Basic auth header value (Base64).
    ///
    /// WHY: Proxy-Authorization header requires Base64-encoded "username:password".
    ///
    /// WHAT: Returns Base64 string ready for Proxy-Authorization header.
    ///
    /// HOW: Format as "username:password", encode with base64 STANDARD engine.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::proxy::ProxyAuth;
    ///
    /// let auth = ProxyAuth::new("user", "password");
    /// assert_eq!(auth.to_basic_auth(), "dXNlcjpwYXNzd29yZA==");
    /// ```
    #[must_use]
    pub fn to_basic_auth(&self) -> String {
        use base64::{engine::general_purpose, Engine as _};
        let credentials = format!("{}:{}", self.username, self.password);
        general_purpose::STANDARD.encode(credentials.as_bytes())
    }
}

/// Proxy configuration.
///
/// WHY: Encapsulates all proxy settings in one place for easy passing to connection methods.
///
/// WHAT: Complete proxy configuration including protocol, host, port, and optional authentication.
///
/// HOW: Builder pattern with public fields for direct access, methods for ergonomic construction.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::proxy::{ProxyConfig, ProxyProtocol};
///
/// // Simple proxy
/// let proxy = ProxyConfig::new(ProxyProtocol::Http, "proxy.example.com", 8080);
///
/// // With authentication
/// let proxy = ProxyConfig::new(ProxyProtocol::Http, "proxy.example.com", 8080)
///     .with_auth("user", "password");
///
/// // Parse from URL
/// let proxy = ProxyConfig::parse("http://user:pass@proxy.example.com:8080").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Proxy protocol type (HTTP, HTTPS, or SOCKS5)
    pub protocol: ProxyProtocol,

    /// Proxy server hostname
    pub host: String,

    /// Proxy server port
    pub port: u16,

    /// Optional authentication credentials
    pub auth: Option<ProxyAuth>,
}

impl ProxyConfig {
    /// Create new proxy configuration.
    ///
    /// WHY: Constructor for manual proxy configuration.
    ///
    /// WHAT: Creates ProxyConfig with specified protocol, host, and port (no auth).
    ///
    /// HOW: Simple struct construction with None for auth field.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::proxy::{ProxyConfig, ProxyProtocol};
    ///
    /// let proxy = ProxyConfig::new(ProxyProtocol::Http, "proxy.example.com", 8080);
    /// assert_eq!(proxy.host, "proxy.example.com");
    /// assert_eq!(proxy.port, 8080);
    /// assert!(proxy.auth.is_none());
    /// ```
    #[must_use]
    pub fn new(protocol: ProxyProtocol, host: impl Into<String>, port: u16) -> Self {
        Self {
            protocol,
            host: host.into(),
            port,
            auth: None,
        }
    }

    /// Add authentication credentials.
    ///
    /// WHY: Builder pattern for adding auth to proxy configuration.
    ///
    /// WHAT: Adds ProxyAuth to this config, returns self for chaining.
    ///
    /// HOW: Creates ProxyAuth from username/password, sets auth field.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::proxy::{ProxyConfig, ProxyProtocol};
    ///
    /// let proxy = ProxyConfig::new(ProxyProtocol::Http, "proxy.example.com", 8080)
    ///     .with_auth("user", "password");
    ///
    /// assert!(proxy.auth.is_some());
    /// assert_eq!(proxy.auth.unwrap().username, "user");
    /// ```
    #[must_use]
    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth = Some(ProxyAuth::new(username, password));
        self
    }

    /// Parse proxy URL string into ProxyConfig.
    ///
    /// WHY: Users configure proxies via URL strings (environment variables, config files).
    ///
    /// WHAT: Parses various proxy URL formats into ProxyConfig struct.
    ///
    /// HOW: Split on :// for protocol, @ for auth, : for host:port.
    ///
    /// Supported formats:
    /// - `http://proxy.com:8080`
    /// - `http://user:pass@proxy.com:8080`
    /// - `https://proxy.com:8443`
    /// - `socks5://proxy.com:1080` (requires "socks5" feature)
    ///
    /// # Errors
    ///
    /// Returns `InvalidProxyUrl` for:
    /// - Missing protocol separator (://)
    /// - Unsupported protocol
    /// - Invalid auth format
    /// - Missing port
    /// - Invalid port number
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::proxy::ProxyConfig;
    ///
    /// let proxy = ProxyConfig::parse("http://proxy.com:8080").unwrap();
    /// assert_eq!(proxy.host, "proxy.com");
    /// assert_eq!(proxy.port, 8080);
    ///
    /// let proxy = ProxyConfig::parse("http://user:pass@proxy.com:8080").unwrap();
    /// assert!(proxy.auth.is_some());
    /// ```
    pub fn parse(url: &str) -> Result<Self, HttpClientError> {
        // Step 1: Split protocol (http://, https://, socks5://)
        let (protocol_str, rest) = url.split_once("://").ok_or_else(|| {
            HttpClientError::InvalidProxyUrl("Missing protocol separator ://".to_string())
        })?;

        let protocol = match protocol_str.to_lowercase().as_str() {
            "http" => ProxyProtocol::Http,
            "https" => ProxyProtocol::Https,
            #[cfg(feature = "socks5")]
            "socks5" => ProxyProtocol::Socks5,
            #[cfg(not(feature = "socks5"))]
            "socks5" => {
                return Err(HttpClientError::InvalidProxyUrl(
                    "SOCKS5 support requires 'socks5' feature".to_string(),
                ))
            }
            other => {
                return Err(HttpClientError::InvalidProxyUrl(format!(
                    "Unsupported proxy protocol: {}",
                    other
                )))
            }
        };

        // Step 2: Check for auth (user:pass@host:port)
        // Use rsplit_once to find the LAST @ (in case password contains @)
        let (auth, host_port) = if let Some((auth_str, host_port)) = rest.rsplit_once('@') {
            let (username, password) = auth_str.split_once(':').ok_or_else(|| {
                HttpClientError::InvalidProxyUrl(
                    "Invalid auth format, expected user:pass".to_string(),
                )
            })?;

            (Some(ProxyAuth::new(username, password)), host_port)
        } else {
            (None, rest)
        };

        // Step 3: Parse host:port
        let (host, port_str) = host_port.rsplit_once(':').ok_or_else(|| {
            HttpClientError::InvalidProxyUrl("Missing port in proxy URL".to_string())
        })?;

        let port: u16 = port_str.parse().map_err(|_| {
            HttpClientError::InvalidProxyUrl(format!("Invalid port number: {}", port_str))
        })?;

        Ok(ProxyConfig {
            protocol,
            host: host.to_string(),
            port,
            auth,
        })
    }

    /// Detect proxy from environment variables based on scheme.
    ///
    /// WHY: Standard Unix proxy configuration via environment variables.
    ///
    /// WHAT: Checks HTTP_PROXY/HTTPS_PROXY based on target URL scheme.
    ///
    /// HOW: Match on scheme, try uppercase first, fallback to lowercase.
    ///
    /// Checks:
    /// - For http:// targets: `HTTP_PROXY` or `http_proxy`
    /// - For https:// targets: `HTTPS_PROXY` or `https_proxy`
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::proxy::ProxyConfig;
    /// use foundation_core::wire::simple_http::url::Scheme;
    ///
    /// std::env::set_var("HTTP_PROXY", "http://proxy.com:8080");
    /// let proxy = ProxyConfig::from_env(&Scheme::Http);
    /// assert!(proxy.is_some());
    /// std::env::remove_var("HTTP_PROXY");
    /// ```
    #[must_use]
    pub fn from_env(scheme: &Scheme) -> Option<Self> {
        if scheme.is_http() {
            Self::from_env_var("HTTP_PROXY").or_else(|| Self::from_env_var("http_proxy"))
        } else if scheme.is_https() {
            Self::from_env_var("HTTPS_PROXY").or_else(|| Self::from_env_var("https_proxy"))
        } else {
            None
        }
    }

    /// Check if host should bypass proxy (NO_PROXY list).
    ///
    /// WHY: Some hosts should not go through proxy (localhost, internal hosts).
    ///
    /// WHAT: Checks NO_PROXY/no_proxy comma-separated list for matches.
    ///
    /// HOW: Split on comma, check exact match or domain suffix match.
    ///
    /// Supports:
    /// - Exact match: `localhost`, `127.0.0.1`
    /// - Domain suffix: `.example.com` matches `api.example.com`
    /// - Wildcard: `*` matches all hosts
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::proxy::ProxyConfig;
    ///
    /// std::env::set_var("NO_PROXY", "localhost,.example.com");
    /// assert!(ProxyConfig::should_bypass("localhost"));
    /// assert!(ProxyConfig::should_bypass("api.example.com"));
    /// assert!(!ProxyConfig::should_bypass("other.com"));
    /// std::env::remove_var("NO_PROXY");
    /// ```
    #[must_use]
    pub fn should_bypass(host: &str) -> bool {
        let no_proxy = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();

        if no_proxy.is_empty() {
            return false;
        }

        for pattern in no_proxy.split(',') {
            let pattern = pattern.trim();

            // Wildcard matches everything
            if pattern == "*" {
                return true;
            }

            // Exact match
            if host == pattern {
                return true;
            }

            // Domain suffix match (.example.com matches api.example.com)
            if pattern.starts_with('.') && host.ends_with(pattern) {
                return true;
            }

            // Domain suffix without leading dot (example.com matches api.example.com)
            if host.ends_with(&format!(".{}", pattern)) {
                return true;
            }
        }

        false
    }

    /// Helper to read and parse proxy from single environment variable.
    fn from_env_var(var: &str) -> Option<Self> {
        std::env::var(var)
            .ok()
            .and_then(|url| Self::parse(&url).ok())
    }
}
