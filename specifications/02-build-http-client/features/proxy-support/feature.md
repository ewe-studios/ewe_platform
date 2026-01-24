---
feature: proxy-support
description: HTTP/HTTPS/SOCKS5 proxy support with environment variable detection
status: pending
priority: medium
depends_on:
  - connection
estimated_effort: medium
created: 2026-01-19
last_updated: 2026-01-24
author: Main Agent
tasks:
  completed: 0
  uncompleted: 13
  total: 13
  completion_percentage: 0
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
---

# Proxy Support Feature

## Overview

Add comprehensive proxy support for the HTTP client, including HTTP CONNECT tunneling, HTTPS proxies, and SOCKS5 proxies. This feature enables transparent proxy usage with automatic environment variable detection and per-request overrides.

## Dependencies

This feature depends on:
- `connection` - Uses HttpClientConnection for TCP/TLS connections

This feature is required by:
- `public-api` - Exposes proxy configuration to users

## Requirements

### Proxy Types

Support three proxy protocols:

```rust
pub enum ProxyProtocol {
    /// HTTP proxy (CONNECT method for HTTPS targets)
    Http,

    /// HTTPS proxy (TLS to proxy, then CONNECT tunnel)
    Https,

    /// SOCKS5 proxy (feature-gated)
    Socks5,
}

pub struct ProxyConfig {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub auth: Option<ProxyAuth>,
}

pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}
```

### HTTP Proxy (CONNECT Tunneling)

For HTTPS targets through HTTP proxy:

```rust
// 1. Connect to proxy via TCP
// 2. Send CONNECT request
// 3. Receive 200 Connection Established
// 4. Perform TLS handshake through tunnel
// 5. Send actual HTTP request

// CONNECT request format:
// CONNECT target.example.com:443 HTTP/1.1
// Host: target.example.com:443
// Proxy-Authorization: Basic dXNlcjpwYXNz  (if auth)
```

### HTTPS Proxy

TLS connection to proxy, then CONNECT tunnel:

```rust
// 1. Connect to proxy via TCP
// 2. Perform TLS handshake with proxy
// 3. Send CONNECT request over TLS
// 4. Receive 200 Connection Established
// 5. Perform TLS handshake for target through tunnel
// 6. Send actual HTTP request
```

### SOCKS5 Proxy (Feature-Gated)

SOCKS5 protocol support:

```rust
// Feature-gated: requires `socks5` feature
#[cfg(feature = "socks5")]
pub struct Socks5Connector {
    proxy: ProxyConfig,
}

// SOCKS5 handshake:
// 1. Send greeting with auth methods
// 2. Receive server's chosen method
// 3. Authenticate if required
// 4. Send connect request
// 5. Receive connection response
// 6. Proceed with TLS/HTTP
```

### Environment Variable Detection

Auto-detect proxies from environment:

```rust
impl ProxyConfig {
    /// Detect proxy from environment variables
    pub fn from_env(scheme: &Scheme) -> Option<Self> {
        match scheme {
            Scheme::Http => Self::from_env_var("HTTP_PROXY")
                .or_else(|| Self::from_env_var("http_proxy")),
            Scheme::Https => Self::from_env_var("HTTPS_PROXY")
                .or_else(|| Self::from_env_var("https_proxy")),
        }
    }

    /// Check if host should bypass proxy
    pub fn should_bypass(host: &str) -> bool {
        let no_proxy = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();

        for pattern in no_proxy.split(',') {
            let pattern = pattern.trim();
            if pattern == "*" {
                return true;
            }
            if host == pattern || host.ends_with(&format!(".{}", pattern)) {
                return true;
            }
        }
        false
    }

    fn from_env_var(var: &str) -> Option<Self> {
        std::env::var(var).ok().and_then(|url| Self::parse(&url).ok())
    }
}
```

### Configuration API

```rust
// Client-level proxy
let client = SimpleHttpClient::new()
    .proxy("http://proxy.example.com:8080")
    .proxy_auth("user", "password");

// Environment detection
let client = SimpleHttpClient::new()
    .proxy_from_env();

// Per-request override
let response = client.get(url)
    .proxy("http://other-proxy:8080")
    .send()?;

// Bypass proxy for specific request
let response = client.get(url)
    .no_proxy()
    .send()?;

// SOCKS5 (feature-gated)
let client = SimpleHttpClient::new()
    .proxy("socks5://proxy:1080");
```

### Error Handling

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    // ... existing variants ...

    #[from(ignore)]
    ProxyConnectionFailed(String),

    #[from(ignore)]
    ProxyAuthenticationFailed(String),

    #[from(ignore)]
    ProxyTunnelFailed { status: u16, message: String },

    #[from(ignore)]
    InvalidProxyUrl(String),

    #[from(ignore)]
    Socks5Error(String),
}
```

## Implementation Details

### File Structure

```
client/
├── proxy.rs    (NEW - Proxy configuration and connection)
└── ...
```

### Proxy URL Parsing

```rust
impl ProxyConfig {
    pub fn parse(url: &str) -> Result<Self, HttpClientError> {
        // Formats:
        // http://proxy:8080
        // http://user:pass@proxy:8080
        // https://proxy:8443
        // socks5://proxy:1080
        // socks5://user:pass@proxy:1080
    }
}
```

### HTTP CONNECT Implementation

```rust
pub struct ProxyTunnel {
    connection: Connection,
    config: ProxyConfig,
}

impl ProxyTunnel {
    pub fn connect_http_proxy(
        proxy: &ProxyConfig,
        target_host: &str,
        target_port: u16,
    ) -> Result<Connection, HttpClientError> {
        // 1. TCP connect to proxy
        let mut conn = TcpStream::connect((proxy.host.as_str(), proxy.port))?;

        // 2. Send CONNECT request
        let connect_req = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n{}\r\n",
            target_host, target_port,
            target_host, target_port,
            proxy.auth.as_ref().map(|a| format!(
                "Proxy-Authorization: Basic {}\r\n",
                base64::encode(format!("{}:{}", a.username, a.password))
            )).unwrap_or_default()
        );
        conn.write_all(connect_req.as_bytes())?;

        // 3. Read response
        let response = read_connect_response(&mut conn)?;
        if response.status != 200 {
            return Err(HttpClientError::ProxyTunnelFailed {
                status: response.status,
                message: response.reason,
            });
        }

        Ok(Connection::from_tcp(conn))
    }
}
```

### Feature Gates

```toml
[dependencies]
# SOCKS5 support (optional)
socks = { version = "0.3", optional = true }

[features]
socks5 = ["socks"]
```

## Success Criteria

- [ ] `proxy.rs` exists and compiles
- [ ] `ProxyConfig` correctly parses proxy URLs
- [ ] HTTP proxy with CONNECT tunneling works
- [ ] HTTPS proxy with TLS + CONNECT works
- [ ] Proxy authentication (Basic) works
- [ ] Environment variable detection works (HTTP_PROXY, HTTPS_PROXY)
- [ ] NO_PROXY bypass list works
- [ ] Per-request proxy override works
- [ ] `no_proxy()` bypasses client proxy for request
- [ ] SOCKS5 proxy works (feature-gated)
- [ ] Error handling covers all failure modes
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- proxy
cargo build --package foundation_core
cargo build --package foundation_core --features socks5
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** connection feature is complete
- **MUST READ** RFC 7231 Section 4.3.6 (CONNECT method)
- **MUST READ** existing netcap connection handling

### Implementation Guidelines
- HTTP proxy requires CONNECT method for HTTPS targets
- HTTPS proxy needs double TLS (to proxy, then to target)
- SOCKS5 must be feature-gated (different protocol)
- Environment variables have case-insensitive fallbacks
- NO_PROXY supports wildcards and domain matching
- Proxy auth uses same Base64 as Basic auth

### Security Considerations
- Proxy credentials should be handled securely
- Consider proxy credential exposure in logs
- Validate proxy certificates for HTTPS proxies

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
