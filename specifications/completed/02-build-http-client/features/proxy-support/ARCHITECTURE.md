# Proxy Support Implementation - REVISED Architecture Plan

**Created**: 2026-03-03
**Status**: Planning - Architecture Review Complete

---

## Architecture Analysis

### What We Have:
1. **HttpConnectionPool** - Manages connections with checkout/checkin
2. **HttpClientConnection** - Wraps SharedByteBufferStream<RawStream>
3. **SharedByteBufferStream** - Cloneable, movable stream wrapper
4. **HttpResponseReader** - Iterator-based HTTP parser
5. **Connection::with_timeout()** - Low-level TCP with timeout
6. **SSLConnector** - TLS upgrade via from_tcp_stream()
7. **TaskIterator pattern** - For async-like state machines (if needed)

### What We Need:
1. **Extend HttpConnectionPool** - Add `connect_http_proxy()` and `connect_https_proxy()` methods
2. **Extend HttpClientConnection** - Add `connect_from_host_port()` methods for HTTP/HTTPS
3. **Proxy CONNECT** - Use HttpResponseReader to parse proxy responses
4. **No TaskIterator needed** - Proxy connection is synchronous blocking (happens before request)

---

## Implementation Strategy

### Phase 1: Extend HttpClientConnection

Add methods that connect by host:port instead of Uri:

```rust
impl HttpClientConnection {
    /// Connect to HTTP endpoint by host and port
    ///
    /// WHY: Proxy needs to connect to proxy server, not final target URL
    /// WHAT: Direct TCP connection without URL parsing
    /// HOW: Use Connection::with_timeout, wrap in SharedByteBufferStream
    pub fn connect_http_host_port<R: DnsResolver>(
        host: &str,
        port: u16,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        // DNS resolution
        let addrs = resolver.resolve(host, port)?;

        // Try each address
        for addr in addrs {
            let conn_result = if let Some(timeout_duration) = timeout {
                Connection::with_timeout(addr, timeout_duration)
            } else {
                Connection::without_timeout(addr)
            };

            if let Ok(connection) = conn_result {
                let stream = SharedByteBufferStream::rwrite(
                    RawStream::from_connection(connection)?
                );
                return Ok(HttpClientConnection {
                    stream,
                    host: host.to_string(),
                    port
                });
            }
        }

        Err(HttpClientError::ConnectionFailed(...))
    }

    /// Connect to HTTPS endpoint by host and port
    ///
    /// WHY: Proxy needs TLS connection to HTTPS proxy
    /// WHAT: TCP + TLS handshake
    /// HOW: TCP connection + SSLConnector upgrade
    pub fn connect_https_host_port<R: DnsResolver>(
        host: &str,
        port: u16,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        // DNS + TCP (same as HTTP)
        let addrs = resolver.resolve(host, port)?;

        for addr in addrs {
            let conn_result = if let Some(timeout_duration) = timeout {
                Connection::with_timeout(addr, timeout_duration)
            } else {
                Connection::without_timeout(addr)
            };

            if let Ok(connection) = conn_result {
                // TLS upgrade
                let connector = SSLConnector::new();
                let (tls_stream, _) = connector.from_tcp_stream(host.to_string(), connection)?;

                let stream = SharedByteBufferStream::rwrite(
                    RawStream::from_client_tls(tls_stream)?
                );
                return Ok(HttpClientConnection {
                    stream,
                    host: host.to_string(),
                    port
                });
            }
        }

        Err(HttpClientError::ConnectionFailed(...))
    }
}
```

### Phase 2: Extend HttpConnectionPool

Add proxy connection methods that use the pool:

```rust
impl<R: DnsResolver> HttpConnectionPool<R> {
    /// Establish HTTP CONNECT tunnel through proxy
    ///
    /// WHY: HTTPS needs tunneling through HTTP proxy
    /// WHAT: Connect to proxy, send CONNECT, parse response, return tunneled connection
    /// HOW: Use HttpClientConnection::connect_http_host_port + HttpResponseReader
    pub fn connect_through_http_proxy(
        &self,
        proxy_config: &ProxyConfig,
        target_host: &str,
        target_port: u16,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        use std::io::Write;

        // Step 1: Connect to proxy (uses pool/resolver)
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

        if let Some(ref auth) = proxy_config.auth {
            request.push_str(&format!(
                "Proxy-Authorization: Basic {}\r\n",
                auth.to_basic_auth()
            ));
        }
        request.push_str("\r\n");

        // Step 3: Send CONNECT request
        proxy_conn.stream_mut().write_all(request.as_bytes())?;
        proxy_conn.stream_mut().flush()?;

        // Step 4: Parse response with HttpResponseReader
        let cloned_stream = proxy_conn.clone_stream();
        let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
            cloned_stream,
            SimpleHttpBody
        );

        // Step 5: Get status from intro
        let status_code = match reader.next() {
            Some(Ok(IncomingResponseParts::Intro(status, _, _))) => {
                status.into_usize() as u16
            }
            _ => return Err(HttpClientError::ProxyConnectionFailed("Invalid CONNECT response".into())),
        };

        // Step 6: Drain headers
        while let Some(Ok(part)) = reader.next() {
            match part {
                IncomingResponseParts::Headers(_) => continue,
                IncomingResponseParts::NoBody | IncomingResponseParts::SKIP => break,
                _ => break,
            }
        }

        // Step 7: Check status
        match status_code {
            200 => {
                // Tunnel established! Return connection ready for TLS upgrade
                Ok(proxy_conn)
            }
            407 => Err(HttpClientError::ProxyAuthenticationFailed("...".into())),
            code => Err(HttpClientError::ProxyTunnelFailed { status: code, message: "...".into() }),
        }
    }

    /// Establish tunnel through HTTPS proxy (double TLS)
    ///
    /// WHY: HTTPS proxies require TLS to proxy, then CONNECT
    /// WHAT: TLS to proxy + CONNECT + return connection for target TLS
    /// HOW: connect_https_host_port + CONNECT over TLS + HttpResponseReader
    pub fn connect_through_https_proxy(
        &self,
        proxy_config: &ProxyConfig,
        target_host: &str,
        target_port: u16,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        // Similar to HTTP proxy but start with TLS connection
        let mut proxy_conn = HttpClientConnection::connect_https_host_port(
            &proxy_config.host,
            proxy_config.port,
            &*self.resolver,
            timeout,
        )?;

        // ... same CONNECT logic as above ...

        // Return connection (still has proxy TLS, caller adds target TLS)
        Ok(proxy_conn)
    }

    /// Create connection through proxy (if configured)
    ///
    /// WHY: Unified entry point for proxy-aware connections
    /// WHAT: Check proxy config, establish tunnel, optionally upgrade to TLS for target
    /// HOW: Proxy CONNECT + conditional TLS upgrade
    pub fn create_connection_with_proxy(
        &self,
        url: &Uri,
        proxy: Option<&ProxyConfig>,
        timeout: Option<Duration>,
    ) -> Result<HttpClientConnection, HttpClientError> {
        let host = url.host_str().ok_or(...)?;
        let port = url.port_or_default();

        // Check NO_PROXY bypass
        if let Some(proxy_cfg) = proxy {
            if ProxyConfig::should_bypass(host) {
                // Direct connection (no proxy)
                return self.create_http_connection(url, timeout);
            }

            // Establish tunnel through proxy
            let tunnel_conn = match proxy_cfg.protocol {
                ProxyProtocol::Http => {
                    self.connect_through_http_proxy(proxy_cfg, host, port, timeout)?
                }
                ProxyProtocol::Https => {
                    self.connect_through_https_proxy(proxy_cfg, host, port, timeout)?
                }
                #[cfg(feature = "socks5")]
                ProxyProtocol::Socks5 => {
                    self.connect_through_socks5_proxy(proxy_cfg, host, port, timeout)?
                }
            };

            // If target is HTTPS, upgrade tunnel to TLS
            if url.scheme().is_https() {
                return Self::upgrade_connection_to_tls(tunnel_conn, host);
            }

            return Ok(tunnel_conn);
        }

        // No proxy - direct connection
        self.create_http_connection(url, timeout)
    }

    fn upgrade_connection_to_tls(
        mut conn: HttpClientConnection,
        host: &str,
    ) -> Result<HttpClientConnection, HttpClientError> {
        // Extract underlying Connection
        let raw_stream = conn.take_stream().into_inner();
        let tcp_conn = raw_stream.into_tcp_stream()?;

        // TLS upgrade
        let connector = SSLConnector::new();
        let (tls_stream, _) = connector.from_tcp_stream(host.to_string(), tcp_conn)?;

        let stream = SharedByteBufferStream::rwrite(
            RawStream::from_client_tls(tls_stream)?
        );

        Ok(HttpClientConnection {
            stream,
            host: host.to_string(),
            port: conn.port,
        })
    }
}
```

### Phase 3: Proxy Configuration (Same as before)

ProxyConfig, ProxyAuth, ProxyProtocol - these remain unchanged from original plan.

### Phase 4: Integration Points

1. **Modify existing `create_http_connection()`** - Add proxy parameter
2. **Update all call sites** - Pass proxy config from ClientConfig
3. **Builder API** - Add .proxy(), .proxy_from_env(), .no_proxy()

---

## Key Differences from Original Plan

| Aspect | Original (Wrong) | Revised (Correct) |
|--------|-----------------|-------------------|
| Stream type | RawStream | SharedByteBufferStream<RawStream> |
| Connection | TcpStream::connect | HttpClientConnection methods |
| Pooling | Manual | Use HttpConnectionPool |
| HTTP parsing | Manual read_line | HttpResponseReader iterator |
| TLS upgrade | Manual SSLConnector | Reuse upgrade_to_tls pattern |
| State machine | Not needed | Only if truly necessary |

---

## No TaskIterator Needed

Proxy connection is **synchronous blocking**:
- CONNECT handshake completes before HTTP request starts
- No need for async-like state machine
- Happens once per connection, not per request
- Blocking is acceptable (network I/O already blocks)

If performance becomes issue later, can add TaskIterator wrapper. But start simple.

---

## Implementation Order

1. ✅ ProxyConfig/ProxyAuth/ProxyProtocol structs
2. ✅ URL parsing and environment detection
3. ✅ Error variants
4. ✅ HttpClientConnection::connect_http_host_port()
5. ✅ HttpClientConnection::connect_https_host_port()
6. ✅ HttpConnectionPool::connect_through_http_proxy()
7. ✅ HttpConnectionPool::connect_through_https_proxy()
8. ✅ HttpConnectionPool::create_connection_with_proxy()
9. ✅ SOCKS5 (if needed, feature-gated)
10. ✅ Integration with ClientConfig and builder API
11. ✅ Tests

---

*Created: 2026-03-03*
*Last Updated: 2026-03-03 (Complete architecture rewrite)*
