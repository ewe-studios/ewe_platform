---
feature: proxy-support
completed: 0
uncompleted: 14
last_updated: 2026-01-19
tools:
  - Rust
  - cargo
---

# Proxy Support - Tasks

## Task List

### Module Setup
- [ ] Create `client/proxy.rs` - Proxy configuration and connection module
- [ ] Add optional socks dependency to Cargo.toml
- [ ] Add socks5 feature flag

### Core Types
- [ ] Define `ProxyProtocol` enum (Http, Https, Socks5)
- [ ] Define `ProxyConfig` struct
- [ ] Define `ProxyAuth` struct
- [ ] Implement `ProxyConfig::parse()` URL parser

### Environment Detection
- [ ] Implement `ProxyConfig::from_env()` for HTTP_PROXY/HTTPS_PROXY
- [ ] Implement `ProxyConfig::should_bypass()` for NO_PROXY
- [ ] Support case-insensitive environment variables

### HTTP Proxy (CONNECT)
- [ ] Implement TCP connection to HTTP proxy
- [ ] Implement CONNECT request generation
- [ ] Implement CONNECT response parsing
- [ ] Implement Proxy-Authorization header for auth
- [ ] Implement TLS handshake through tunnel

### HTTPS Proxy
- [ ] Implement TLS connection to HTTPS proxy
- [ ] Implement CONNECT over TLS tunnel
- [ ] Implement nested TLS for target connection

### SOCKS5 Proxy (Feature-Gated)
- [ ] Implement SOCKS5 greeting handshake
- [ ] Implement SOCKS5 authentication
- [ ] Implement SOCKS5 connect request
- [ ] Implement connection through SOCKS5 tunnel

### Client Integration
- [ ] Add `proxy()` method to SimpleHttpClient builder
- [ ] Add `proxy_auth()` method to SimpleHttpClient builder
- [ ] Add `proxy_from_env()` method
- [ ] Add `proxy()` per-request override
- [ ] Add `no_proxy()` per-request bypass

### Error Handling
- [ ] Add `ProxyConnectionFailed` error variant
- [ ] Add `ProxyAuthenticationFailed` error variant
- [ ] Add `ProxyTunnelFailed` error variant
- [ ] Add `InvalidProxyUrl` error variant
- [ ] Add `Socks5Error` error variant

## Implementation Order

1. **Cargo.toml** - Add optional dependencies
2. **proxy.rs** - Core types (ProxyProtocol, ProxyConfig, ProxyAuth)
3. **proxy.rs** - URL parsing for proxy config
4. **proxy.rs** - Environment variable detection
5. **proxy.rs** - HTTP CONNECT tunneling
6. **proxy.rs** - HTTPS proxy with TLS
7. **proxy.rs** - SOCKS5 (feature-gated)
8. **errors.rs** - Add proxy error variants
9. **Integration** - Add methods to client and request builders

## Notes

### Proxy URL Formats
```
http://proxy:8080
http://user:pass@proxy:8080
https://proxy:8443
socks5://proxy:1080
socks5://user:pass@proxy:1080
```

### CONNECT Request Pattern
```
CONNECT target.example.com:443 HTTP/1.1
Host: target.example.com:443
Proxy-Authorization: Basic dXNlcjpwYXNz

```

### Environment Variables
```bash
HTTP_PROXY=http://proxy:8080
HTTPS_PROXY=https://secure-proxy:8443
NO_PROXY=localhost,127.0.0.1,.internal.corp
```

### Feature Gate Pattern
```rust
#[cfg(feature = "socks5")]
pub fn connect_socks5(...) -> Result<Connection, HttpClientError> {
    // SOCKS5 implementation
}
```

---
*Last Updated: 2026-01-19*
