---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/proxy-support"
this_file: "specifications/02-build-http-client/features/proxy-support/feature.md"

status: pending
priority: medium
created: 2026-02-28

depends_on:
  - connection

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0
---

# Proxy Support Feature

## Overview

Add comprehensive proxy support for the HTTP client, including HTTP CONNECT tunneling, HTTPS proxies, and SOCKS5 proxies. This feature enables transparent proxy usage with automatic environment variable detection and per-request overrides.

## Dependencies

This feature depends on:
- `connection` - Uses HttpClientConnection and HttpConnectionPool

This feature is required by:
- None (enhances existing public-api)

---

# DETAILED IMPLEMENTATION PLAN

## Architecture Overview

### Core Strategy

**Extend existing infrastructure** rather than create new parallel systems:

1. **HttpClientConnection** - Add `connect_http_host_port()` and `connect_https_host_port()` methods
2. **HttpConnectionPool** - Add proxy connection methods that leverage pool/resolver
3. **SharedByteBufferStream<RawStream>** - Already cloneable/movable, perfect for proxying
4. **HttpResponseReader** - Parse CONNECT responses with existing HTTP parser
5. **No TaskIterator** - Proxy CONNECT is synchronous blocking (happens before request)

### Key Insight

Proxy connection establishment is **blocking and synchronous** - it happens once before the HTTP request, not during it. No need for async state machines.

---

## Phase 1: Core Data Structures (proxy.rs)

### 1.1 ProxyProtocol Enum

```rust
/// WHY: Different proxy types require different connection logic
/// WHAT: Enum representing HTTP, HTTPS, and SOCKS5 proxy protocols
/// HOW: Simple enum with three variants, SOCKS5 feature-gated
#[derive(Debug, Clone, PartialEq)]
pub enum ProxyProtocol {
    /// HTTP proxy using CONNECT method for HTTPS targets
    Http,

    /// HTTPS proxy (TLS to proxy, then CONNECT tunnel)
    Https,

    /// SOCKS5 proxy (feature-gated)
    #[cfg(feature = "socks5")]
    Socks5,
}
```

**Implementation notes:**
- Simple enum, no methods needed
- SOCKS5 variant gated with `#[cfg(feature = "socks5")]`
- Derive Debug, Clone, PartialEq

### 1.2 ProxyAuth Struct

```rust
/// WHY: Proxy servers often require authentication
/// WHAT: Username and password for proxy authentication
/// HOW: Encode as Base64 for Proxy-Authorization header
#[derive(Debug, Clone)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}

impl ProxyAuth {
    /// Create new proxy authentication
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Encode as Basic auth header value (Base64)
    ///
    /// WHY: Proxy-Authorization header uses Basic auth format
    /// WHAT: Base64 encode "username:password"
    /// HOW: Use base64::engine::general_purpose::STANDARD
    #[must_use]
    pub fn to_basic_auth(&self) -> String {
        use base64::{Engine as _, engine::general_purpose};
        let credentials = format!("{}:{}", self.username, self.password);
        general_purpose::STANDARD.encode(credentials.as_bytes())
    }
}
```

**Implementation notes:**
- Reuse base64 crate already used in auth-helpers
- to_basic_auth() returns ready-to-use header value
- No password obfuscation in Debug (avoid logging credentials)

### 1.3 ProxyConfig Struct

```rust
/// WHY: Encapsulate all proxy configuration in one place
/// WHAT: Complete proxy configuration including protocol, host, port, and auth
/// HOW: Builder pattern for ergonomic configuration
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub auth: Option<ProxyAuth>,
}

impl ProxyConfig {
    /// Create new proxy config
    #[must_use]
    pub fn new(protocol: ProxyProtocol, host: impl Into<String>, port: u16) -> Self {
        Self {
            protocol,
            host: host.into(),
            port,
            auth: None,
        }
    }

    /// Add authentication
    #[must_use]
    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth = Some(ProxyAuth::new(username, password));
        self
    }
}
```

**Implementation notes:**
- Public fields for direct access
- Builder pattern with with_auth()
- auth is Optional - most proxies don't require it

---

## Phase 2: Proxy URL Parsing

### 2.1 ProxyConfig::parse Method

```rust
impl ProxyConfig {
    /// Parse proxy URL string into ProxyConfig
    ///
    /// WHY: Users configure proxies via URL strings (env vars, config files)
    /// WHAT: Parse formats: http://host:port, http://user:pass@host:port, socks5://...
    /// HOW: Split on :// for protocol, @ for auth, : for host:port
    ///
    /// Supported formats:
    /// - http://proxy.com:8080
    /// - http://user:pass@proxy.com:8080
    /// - https://proxy.com:8443
    /// - socks5://proxy.com:1080 (requires "socks5" feature)
    ///
    /// # Errors
    ///
    /// Returns InvalidProxyUrl for malformed URLs
    pub fn parse(url: &str) -> Result<Self, HttpClientError> {
        // 1. Split protocol (http://, https://, socks5://)
        let (protocol_str, rest) = url.split_once("://")
            .ok_or_else(|| HttpClientError::InvalidProxyUrl("Missing protocol separator ://".to_string()))?;

        let protocol = match protocol_str.to_lowercase().as_str() {
            "http" => ProxyProtocol::Http,
            "https" => ProxyProtocol::Https,
            #[cfg(feature = "socks5")]
            "socks5" => ProxyProtocol::Socks5,
            #[cfg(not(feature = "socks5"))]
            "socks5" => return Err(HttpClientError::InvalidProxyUrl(
                "SOCKS5 support requires 'socks5' feature".to_string()
            )),
            other => return Err(HttpClientError::InvalidProxyUrl(
                format!("Unsupported proxy protocol: {}", other)
            )),
        };

        // 2. Check for auth (user:pass@host:port)
        let (auth, host_port) = if let Some(at_pos) = rest.find('@') {
            let auth_str = &rest[..at_pos];
            let host_port = &rest[at_pos + 1..];

            let (username, password) = auth_str.split_once(':')
                .ok_or_else(|| HttpClientError::InvalidProxyUrl("Invalid auth format".to_string()))?;

            (Some(ProxyAuth::new(username, password)), host_port)
        } else {
            (None, rest)
        };

        // 3. Parse host:port
        let (host, port) = host_port.rsplit_once(':')
            .ok_or_else(|| HttpClientError::InvalidProxyUrl("Missing port".to_string()))?;

        let port: u16 = port.parse()
            .map_err(|_| HttpClientError::InvalidProxyUrl("Invalid port number".to_string()))?;

        Ok(ProxyConfig {
            protocol,
            host: host.to_string(),
            port,
            auth,
        })
    }
}
```

**Implementation notes:**
- Use split_once() for clean parsing
- Parse auth before host:port
- Validate protocol is supported
- Feature gate SOCKS5 with helpful error message

---

## Phase 3: Environment Variable Detection

### 3.1 Environment Detection Methods

```rust
impl ProxyConfig {
    /// Detect proxy from environment variables based on scheme
    ///
    /// WHY: Standard Unix proxy configuration via env vars
    /// WHAT: Check HTTP_PROXY/HTTPS_PROXY based on target scheme
    /// HOW: Try uppercase first, fallback to lowercase
    pub fn from_env(scheme: &Scheme) -> Option<Self> {
        match scheme {
            Scheme::Http => Self::from_env_var("HTTP_PROXY")
                .or_else(|| Self::from_env_var("http_proxy")),
            Scheme::Https => Self::from_env_var("HTTPS_PROXY")
                .or_else(|| Self::from_env_var("https_proxy")),
        }
    }

    /// Check if host should bypass proxy (NO_PROXY list)
    ///
    /// WHY: Some hosts should not go through proxy (localhost, internal hosts)
    /// WHAT: Check NO_PROXY/no_proxy comma-separated list
    /// HOW: Split on comma, check exact match or domain suffix match
    ///
    /// Supports:
    /// - Exact match: localhost, 127.0.0.1
    /// - Domain suffix: .example.com matches api.example.com
    /// - Wildcard: * matches all hosts
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

            // Domain suffix without leading dot
            if host.ends_with(&format!(".{}", pattern)) {
                return true;
            }
        }

        false
    }

    fn from_env_var(var: &str) -> Option<Self> {
        std::env::var(var)
            .ok()
            .and_then(|url| Self::parse(&url).ok())
    }
}
```

**Implementation notes:**
- Case-insensitive fallback (uppercase → lowercase)
- NO_PROXY supports wildcard, exact, domain suffix
- Silently ignore parse errors in env vars

---

## Phase 4: Error Type Extensions

### 4.1 Add Proxy Error Variants

```rust
// In errors.rs - add to HttpClientError enum

#[derive(From, Debug)]
pub enum HttpClientError {
    // ... existing variants ...

    /// Proxy connection failed
    #[from(ignore)]
    ProxyConnectionFailed(String),

    /// Proxy authentication failed (407)
    #[from(ignore)]
    ProxyAuthenticationFailed(String),

    /// Proxy tunnel establishment failed
    #[from(ignore)]
    ProxyTunnelFailed {
        status: u16,
        message: String,
    },

    /// Invalid proxy URL format
    #[from(ignore)]
    InvalidProxyUrl(String),

    /// SOCKS5 protocol error
    #[from(ignore)]
    Socks5Error(String),
}

impl Display for HttpClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            // ... existing arms ...

            Self::ProxyConnectionFailed(msg) => {
                write!(f, "Proxy connection failed: {}", msg)
            }
            Self::ProxyAuthenticationFailed(msg) => {
                write!(f, "Proxy authentication failed: {}", msg)
            }
            Self::ProxyTunnelFailed { status, message } => {
                write!(f, "Proxy tunnel failed with status {}: {}", status, message)
            }
            Self::InvalidProxyUrl(msg) => {
                write!(f, "Invalid proxy URL: {}", msg)
            }
            Self::Socks5Error(msg) => {
                write!(f, "SOCKS5 error: {}", msg)
            }
        }
    }
}
```

---

## Phase 5: Extend HttpClientConnection

### 5.1 Add connect_http_host_port Method

```rust
// In connection.rs - add to impl HttpClientConnection

impl HttpClientConnection {
    /// Connect to HTTP endpoint by host and port
    ///
    /// WHY: Proxy needs to connect to proxy server directly, not via URL
    /// WHAT: Direct TCP connection without URL parsing
    /// HOH: DNS resolve + Connection::with_timeout + wrap in SharedByteBufferStream
    pub fn connect_http_host_port<R: DnsResolver>(
        host: &str,
        port: u16,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        // Step 1: DNS resolution
        let addrs = resolver.resolve(host, port)?;

        if addrs.is_empty() {
            return Err(HttpClientError::ConnectionFailed(format!(
                "No addresses resolved for {}:{}", host, port
            )));
        }

        // Step 2: Try connecting to each address
        let mut last_error = None;

        for addr in addrs {
            let conn_result = if let Some(timeout_duration) = timeout {
                Connection::with_timeout(addr, timeout_duration)
            } else {
                Connection::without_timeout(addr)
            };

            match conn_result {
                Ok(connection) => {
                    // Create plain RawStream from Connection
                    let stream = SharedByteBufferStream::rwrite(
                        RawStream::from_connection(connection)
                            .map_err(|e| HttpClientError::ConnectionFailed(e.to_string()))?,
                    );
                    return Ok(HttpClientConnection {
                        stream,
                        host: host.to_string(),
                        port,
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        // All connection attempts failed
        if let Some(err) = last_error {
            return Err(HttpClientError::ConnectionFailed(err.to_string()));
        }

        Err(HttpClientError::ConnectionFailed(format!(
            "Failed to connect to {}:{}", host, port
        )))
    }

    /// Connect to HTTPS endpoint by host and port
    ///
    /// WHY: HTTPS proxy needs TLS connection to proxy server
    /// WHAT: TCP + TLS handshake
    /// HOW: Same as connect_http_host_port + SSLConnector upgrade
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    pub fn connect_https_host_port<R: DnsResolver>(
        host: &str,
        port: u16,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        // Step 1: DNS + TCP (same as HTTP)
        let addrs = resolver.resolve(host, port)?;

        if addrs.is_empty() {
            return Err(HttpClientError::ConnectionFailed(format!(
                "No addresses resolved for {}:{}", host, port
            )));
        }

        let mut last_error = None;

        for addr in addrs {
            let conn_result = if let Some(timeout_duration) = timeout {
                Connection::with_timeout(addr, timeout_duration)
            } else {
                Connection::without_timeout(addr)
            };

            match conn_result {
                Ok(connection) => {
                    // Step 2: TLS upgrade
                    let connector = SSLConnector::new();
                    let (tls_stream, _addr) = connector
                        .from_tcp_stream(host.to_string(), connection)
                        .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?;

                    let stream = SharedByteBufferStream::rwrite(
                        RawStream::from_client_tls(tls_stream)
                            .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?,
                    );

                    return Ok(HttpClientConnection {
                        stream,
                        host: host.to_string(),
                        port,
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        if let Some(err) = last_error {
            return Err(HttpClientError::ConnectionFailed(err.to_string()));
        }

        Err(HttpClientError::ConnectionFailed(format!(
            "Failed to connect to {}:{}", host, port
        )))
    }

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    pub fn connect_https_host_port<R: DnsResolver>(
        _host: &str,
        _port: u16,
        _resolver: &R,
        _timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }
}
```

**Implementation notes:**
- Mirrors existing connect() logic but without URL parsing
- Reuses DNS resolver and Connection patterns
- TLS upgrade uses same SSLConnector pattern
- Feature-gated for TLS support

---

## Phase 6: Extend HttpConnectionPool - HTTP Proxy

### 6.1 Add connect_through_http_proxy Method

```rust
// In connection.rs - add to impl HttpConnectionPool<R>

impl<R: DnsResolver> HttpConnectionPool<R> {
    /// Establish HTTP CONNECT tunnel through proxy
    ///
    /// WHY: HTTPS requires tunneling through HTTP proxy
    /// WHAT: Connect to proxy, send CONNECT, parse response with HttpResponseReader
    /// HOW: connect_http_host_port + write CONNECT + HttpResponseReader + return tunnel
    pub fn connect_through_http_proxy(
        &self,
        proxy_config: &ProxyConfig,
        target_host: &str,
        target_port: u16,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        use std::io::Write;

        // Step 1: Connect to proxy server
        let mut proxy_conn = HttpClientConnection::connect_http_host_port(
            &proxy_config.host,
            proxy_config.port,
            &*self.resolver,
            timeout,
        )?;

        // Step 2: Build CONNECT request
        let mut request = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n",
            target_host, target_port, target_host, target_port
        );

        // Add Proxy-Authorization if auth configured
        if let Some(ref auth) = proxy_config.auth {
            request.push_str(&format!(
                "Proxy-Authorization: Basic {}\r\n",
                auth.to_basic_auth()
            ));
        }

        request.push_str("\r\n");

        // Step 3: Send CONNECT request
        proxy_conn.stream_mut().write_all(request.as_bytes())
            .map_err(|e| HttpClientError::ProxyConnectionFailed(
                format!("Failed to send CONNECT: {}", e)
            ))?;

        proxy_conn.stream_mut().flush()
            .map_err(|e| HttpClientError::ProxyConnectionFailed(
                format!("Failed to flush CONNECT: {}", e)
            ))?;

        // Step 4: Parse CONNECT response with HttpResponseReader
        let cloned_stream = proxy_conn.clone_stream();
        let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
            cloned_stream,
            SimpleHttpBody
        );

        // Step 5: Get status from intro
        let status_code = match reader.next() {
            Some(Ok(IncomingResponseParts::Intro(status, _proto, _reason))) => {
                status.into_usize() as u16
            }
            Some(Err(e)) => {
                return Err(HttpClientError::ProxyConnectionFailed(
                    format!("Failed to parse CONNECT response: {:?}", e)
                ));
            }
            _ => {
                return Err(HttpClientError::ProxyConnectionFailed(
                    "Invalid CONNECT response format".to_string()
                ));
            }
        };

        // Step 6: Drain headers (we don't need them)
        while let Some(Ok(part)) = reader.next() {
            match part {
                IncomingResponseParts::Headers(_) => continue,
                IncomingResponseParts::NoBody | IncomingResponseParts::SKIP => break,
                _ => break,
            }
        }

        // Step 7: Check status code
        match status_code {
            200 => {
                // Success! Tunnel established, return connection
                Ok(proxy_conn)
            }
            407 => {
                Err(HttpClientError::ProxyAuthenticationFailed(
                    "Proxy authentication required".to_string()
                ))
            }
            code => {
                Err(HttpClientError::ProxyTunnelFailed {
                    status: code,
                    message: format!("CONNECT failed with status {}", code),
                })
            }
        }
    }
}
```

**Implementation notes:**
- Uses HttpClientConnection::connect_http_host_port (leverages resolver)
- Writes CONNECT as raw string (simple HTTP/1.1)
- Clones stream for HttpResponseReader (non-destructive parsing)
- Original proxy_conn still usable after parsing
- Returns HttpClientConnection ready for TLS upgrade

---

## Phase 7: Extend HttpConnectionPool - HTTPS Proxy

### 7.1 Add connect_through_https_proxy Method

```rust
impl<R: DnsResolver> HttpConnectionPool<R> {
    /// Establish tunnel through HTTPS proxy (double TLS)
    ///
    /// WHY: HTTPS proxies require TLS to proxy, then CONNECT
    /// WHAT: TLS to proxy + CONNECT + return connection for target TLS
    /// HOW: connect_https_host_port + CONNECT over TLS + HttpResponseReader
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    pub fn connect_through_https_proxy(
        &self,
        proxy_config: &ProxyConfig,
        target_host: &str,
        target_port: u16,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        use std::io::Write;

        // Step 1: Connect to proxy with TLS
        let mut proxy_conn = HttpClientConnection::connect_https_host_port(
            &proxy_config.host,
            proxy_config.port,
            &*self.resolver,
            timeout,
        )?;

        // Step 2: Build CONNECT request
        let mut request = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n",
            target_host, target_port, target_host, target_port
        );

        if let Some(ref auth) = proxy_config.auth {
            request.push_str(&format!(
                "Proxy-Authorization: Basic {}\r\n",
                auth.to_basic_auth()
            ));
        }

        request.push_str("\r\n");

        // Step 3: Send CONNECT over TLS connection
        proxy_conn.stream_mut().write_all(request.as_bytes())
            .map_err(|e| HttpClientError::ProxyConnectionFailed(
                format!("Failed to send CONNECT over TLS: {}", e)
            ))?;

        proxy_conn.stream_mut().flush()
            .map_err(|e| HttpClientError::ProxyConnectionFailed(
                format!("Failed to flush CONNECT: {}", e)
            ))?;

        // Step 4: Parse response (same as HTTP proxy)
        let cloned_stream = proxy_conn.clone_stream();
        let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
            cloned_stream,
            SimpleHttpBody
        );

        let status_code = match reader.next() {
            Some(Ok(IncomingResponseParts::Intro(status, _, _))) => {
                status.into_usize() as u16
            }
            _ => {
                return Err(HttpClientError::ProxyConnectionFailed(
                    "Invalid CONNECT response".to_string()
                ));
            }
        };

        // Drain headers
        while let Some(Ok(part)) = reader.next() {
            match part {
                IncomingResponseParts::Headers(_) => continue,
                IncomingResponseParts::NoBody | IncomingResponseParts::SKIP => break,
                _ => break,
            }
        }

        // Check status
        match status_code {
            200 => Ok(proxy_conn),
            407 => Err(HttpClientError::ProxyAuthenticationFailed(
                "HTTPS proxy authentication required".to_string()
            )),
            code => Err(HttpClientError::ProxyTunnelFailed {
                status: code,
                message: format!("HTTPS CONNECT failed with status {}", code),
            }),
        }
    }

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    pub fn connect_through_https_proxy(
        &self,
        _proxy_config: &ProxyConfig,
        _target_host: &str,
        _target_port: u16,
        _timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }
}
```

**Implementation notes:**
- Same logic as HTTP proxy but starts with TLS connection
- CONNECT sent over encrypted connection to proxy
- Returned connection still wrapped in proxy TLS
- Caller must establish second TLS for target

---

## Phase 8: Unified Proxy Connection Method

### 8.1 Add create_connection_with_proxy Method

```rust
impl<R: DnsResolver> HttpConnectionPool<R> {
    /// Create connection through proxy (if configured)
    ///
    /// WHY: Unified entry point for proxy-aware connections
    /// WHAT: Check proxy config, establish tunnel, optionally upgrade to TLS for target
    /// HOW: Check NO_PROXY → CONNECT tunnel → conditional target TLS upgrade
    pub fn create_connection_with_proxy(
        &self,
        url: &ParsedUrl,
        proxy: Option<&ProxyConfig>,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        let host = url.host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host".to_string()))?;
        let port = url.port_or_default();

        // Check if proxy should be used
        if let Some(proxy_cfg) = proxy {
            // Check NO_PROXY bypass list
            if ProxyConfig::should_bypass(host) {
                // Direct connection (bypass proxy)
                return self.create_http_connection(url, timeout);
            }

            // Establish tunnel through proxy
            let tunnel_conn = match proxy_cfg.protocol {
                ProxyProtocol::Http => {
                    self.connect_through_http_proxy(proxy_cfg, host, port, timeout)?
                }
                #[cfg(any(
                    feature = "ssl-rustls",
                    feature = "ssl-openssl",
                    feature = "ssl-native-tls"
                ))]
                ProxyProtocol::Https => {
                    self.connect_through_https_proxy(proxy_cfg, host, port, timeout)?
                }
                #[cfg(not(any(
                    feature = "ssl-rustls",
                    feature = "ssl-openssl",
                    feature = "ssl-native-tls"
                )))]
                ProxyProtocol::Https => {
                    return Err(HttpClientError::NotSupported);
                }
                #[cfg(feature = "socks5")]
                ProxyProtocol::Socks5 => {
                    self.connect_through_socks5_proxy(proxy_cfg, host, port, timeout)?
                }
            };

            // If target is HTTPS, upgrade tunnel to TLS
            if url.scheme().is_https() {
                return Self::upgrade_tunnel_to_target_tls(tunnel_conn, host);
            }

            return Ok(tunnel_conn);
        }

        // No proxy - direct connection
        self.create_http_connection(url, timeout)
    }

    /// Upgrade tunneled connection to TLS for target server
    ///
    /// WHY: After CONNECT tunnel, need TLS handshake with target (not proxy)
    /// WHAT: Extract stream, perform TLS handshake with target host
    /// HOW: Take stream → extract raw → TLS upgrade → wrap back
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    fn upgrade_tunnel_to_target_tls(
        conn: HttpClientConnection,
        target_host: &str,
    ) -> Result<HttpClientConnection, HttpClientError> {
        let host = conn.host.clone();
        let port = conn.port;

        // Extract underlying stream
        let shared_stream = conn.take_stream();
        let raw_stream = shared_stream.into_inner();

        // Convert RawStream back to Connection for TLS upgrade
        let tcp_conn = raw_stream.into_tcp_stream()
            .map_err(|e| HttpClientError::TlsHandshakeFailed(
                format!("Failed to extract TCP stream: {}", e)
            ))?;

        // Perform TLS handshake with target server
        let connector = SSLConnector::new();
        let (tls_stream, _) = connector.from_tcp_stream(target_host.to_string(), tcp_conn)
            .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?;

        // Wrap back in SharedByteBufferStream
        let stream = SharedByteBufferStream::rwrite(
            RawStream::from_client_tls(tls_stream)
                .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?
        );

        Ok(HttpClientConnection {
            stream,
            host,
            port,
        })
    }

    #[cfg(not(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    )))]
    fn upgrade_tunnel_to_target_tls(
        _conn: HttpClientConnection,
        _target_host: &str,
    ) -> Result<HttpClientConnection, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }
}
```

**Implementation notes:**
- Single entry point for all proxy types
- Checks NO_PROXY before using proxy
- Handles HTTP, HTTPS, and SOCKS5 proxies
- Automatically upgrades to target TLS if needed
- Falls back to direct connection if no proxy

---

## Phase 9: SOCKS5 Support (Feature-Gated, Optional)

### 9.1 SOCKS5 Implementation (Brief)

```rust
#[cfg(feature = "socks5")]
impl<R: DnsResolver> HttpConnectionPool<R> {
    pub fn connect_through_socks5_proxy(
        &self,
        proxy_config: &ProxyConfig,
        target_host: &str,
        target_port: u16,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        // Similar pattern: connect_http_host_port + SOCKS5 handshake
        // Implementation details in separate section if needed
        todo!("SOCKS5 implementation - can be added later if required")
    }
}
```

**Implementation notes:**
- Entire SOCKS5 section is optional
- Can be implemented later if needed
- Uses same pattern: connect + handshake + return connection

---

## Phase 10: Public API Integration

### 10.1 Modify ClientConfig

```rust
// In api.rs - add to ClientConfig

pub struct ClientConfig {
    // ... existing fields ...

    /// Proxy configuration
    pub proxy: Option<ProxyConfig>,

    /// Auto-detect proxy from environment
    pub proxy_from_env: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            proxy: None,
            proxy_from_env: false,
        }
    }
}
```

### 10.2 Add Builder Methods

```rust
// In api.rs - add to SimpleHttpClient

impl SimpleHttpClient {
    /// Set proxy for all requests
    pub fn proxy(mut self, proxy_url: &str) -> Result<Self, HttpClientError> {
        let config = ProxyConfig::parse(proxy_url)?;
        self.config.proxy = Some(config);
        Ok(self)
    }

    /// Set proxy with authentication
    pub fn proxy_auth(mut self, username: &str, password: &str) -> Self {
        if let Some(ref mut proxy) = self.config.proxy {
            proxy.auth = Some(ProxyAuth::new(username, password));
        }
        self
    }

    /// Auto-detect proxy from environment variables
    pub fn proxy_from_env(mut self) -> Self {
        self.config.proxy_from_env = true;
        self
    }
}
```

### 10.3 Update Connection Creation

```rust
// In api.rs or tasks - wherever connection is created

// Determine effective proxy config
let effective_proxy = if let Some(override_proxy) = request_proxy_override {
    override_proxy // Per-request override
} else if client.config.proxy_from_env {
    ProxyConfig::from_env(url.scheme()) // Environment detection
} else {
    client.config.proxy.as_ref() // Client default
};

// Create connection with proxy
let connection = pool.create_connection_with_proxy(url, effective_proxy, timeout)?;
```

---

## Success Criteria

- [ ] ProxyConfig/ProxyAuth/ProxyProtocol structs implemented
- [ ] URL parsing handles http://, https://, socks5:// (feature-gated)
- [ ] Environment detection (HTTP_PROXY, HTTPS_PROXY, NO_PROXY) works
- [ ] HttpClientConnection::connect_http_host_port() works
- [ ] HttpClientConnection::connect_https_host_port() works
- [ ] HttpConnectionPool::connect_through_http_proxy() works
- [ ] HttpConnectionPool::connect_through_https_proxy() works
- [ ] HttpConnectionPool::create_connection_with_proxy() works
- [ ] Proxy authentication (Basic) works
- [ ] NO_PROXY bypass works (wildcard, exact, suffix)
- [ ] Target TLS upgrade after tunnel works
- [ ] SimpleHttpClient builder methods work (.proxy(), .proxy_auth(), .proxy_from_env())
- [ ] Per-request proxy override works
- [ ] All error variants work correctly
- [ ] All unit tests pass (15+ tests)
- [ ] Code passes `cargo fmt` and `cargo clippy`
- [ ] No unwrap/expect in production code

## Verification Commands

```bash
# Format check
cargo fmt --check

# Clippy check
cargo clippy --package foundation_core -- -D warnings

# Unit tests
cargo test --package ewe_platform_tests -- proxy

# With SOCKS5 feature
cargo test --package ewe_platform_tests --features socks5 -- proxy

# Build checks
cargo build --package foundation_core
cargo build --package foundation_core --features socks5

# Check for incomplete code
grep -rn "TODO\|FIXME\|unimplemented!\|todo!" backends/foundation_core/src/wire/simple_http/client/proxy.rs || echo "✓ No stubs"
```

---

*Created: 2026-02-28*
*Last Updated: 2026-03-03 (Complete rewrite using correct architecture)*
