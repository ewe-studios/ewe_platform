---
feature: connection
description: URL parsing, TCP connection establishment, and TLS upgrade
status: pending
depends_on:
  - foundation
estimated_effort: small
created: 2026-01-18
last_updated: 2026-01-18
---

# Connection Feature

## Overview

Create the connection management layer for the HTTP 1.1 client. This feature handles URL parsing, TCP connection establishment, and TLS upgrade using the existing `netcap` infrastructure.

## Dependencies

This feature depends on:
- `foundation` - Uses DnsResolver for hostname resolution, HttpClientError for errors

This feature is required by:
- `request-response` - Uses ParsedUrl for request building
- `task-iterator` - Uses HttpClientConnection for state machine

## Requirements

### URL Parsing

Create `ParsedUrl` for parsing HTTP/HTTPS URLs:

```rust
pub struct ParsedUrl {
    pub scheme: Scheme,  // Http or Https
    pub host: String,
    pub port: u16,       // 80 for HTTP, 443 for HTTPS by default
    pub path: String,
    pub query: Option<String>,
}

pub enum Scheme {
    Http,
    Https,
}

impl ParsedUrl {
    pub fn parse(url: &str) -> Result<Self, HttpClientError>;
}
```

### Connection Management

1. **HttpClientConnection**
   - Wraps `netcap::Connection`
   - Factory method with generic resolver: `connect<R: DnsResolver>(...)`
   - HTTP vs HTTPS scheme detection
   - Connection timeout support

2. **TLS Upgrade**
   - Feature-gated TLS connector selection
   - Uses existing `netcap` infrastructure
   - SNI support

### Generic Type Pattern

```rust
impl HttpClientConnection {
    pub fn connect<R: DnsResolver>(
        url: &ParsedUrl,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError>;
}
```

### TLS Feature Gates

```rust
#[cfg(feature = "ssl-rustls")]
fn create_tls_connector() -> RustlsConnector { ... }

#[cfg(feature = "ssl-openssl")]
fn create_tls_connector() -> OpensslConnector { ... }

#[cfg(feature = "ssl-native-tls")]
fn create_tls_connector() -> NativeTlsConnector { ... }
```

## Implementation Details

### File Structure

```
client/
├── connection.rs    (NEW - ParsedUrl, HttpClientConnection)
└── ...
```

### Error Types to Add

Add to `HttpClientError` in errors.rs:
```rust
#[from(ignore)]
ConnectionTimeout(String),

#[from(ignore)]
TlsHandshakeFailed(String),

#[from(ignore)]
InvalidScheme(String),

#[from]
IoError(std::io::Error),
```

## Success Criteria

- [ ] `ParsedUrl` correctly parses HTTP URLs
- [ ] `ParsedUrl` correctly parses HTTPS URLs
- [ ] `ParsedUrl` handles default ports (80/443)
- [ ] `ParsedUrl` handles explicit ports
- [ ] `ParsedUrl` handles paths and query strings
- [ ] `HttpClientConnection::connect()` works for HTTP
- [ ] `HttpClientConnection::connect()` works for HTTPS (with TLS feature)
- [ ] Connection timeout works
- [ ] TLS SNI is set correctly
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- connection
cargo build --package foundation_core
cargo build --package foundation_core --features ssl-rustls
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** foundation feature is complete
- **MUST READ** `netcap/connection/mod.rs` for Connection patterns
- **MUST READ** `netcap/ssl/rustls.rs` for TLS connector usage

### Implementation Guidelines
- Reuse existing netcap types (Connection, RawStream, RustlsConnector)
- Use feature gates for TLS backends
- Generic resolver parameter (not boxed)
- Add `#[cfg(not(target_arch = "wasm32"))]` where needed

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
