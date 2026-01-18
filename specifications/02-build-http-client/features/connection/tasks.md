---
feature: connection
completed: 0
uncompleted: 4
last_updated: 2026-01-18
tools:
  - Rust
  - cargo
---

# Connection - Tasks

## Task List

### URL Parsing
- [ ] Create `client/connection.rs` - ParsedUrl struct with parse() method

### Connection Management
- [ ] Implement `HttpClientConnection` with generic resolver support
- [ ] Implement TLS upgrade using feature-gated ssl modules

### Testing
- [ ] Write unit tests for URL parsing and connection management

## Implementation Order

1. **ParsedUrl** - URL parsing (no external dependencies)
2. **HttpClientConnection** - TCP connection (depends on ParsedUrl, dns.rs)
3. **TLS upgrade** - HTTPS support (depends on HttpClientConnection)
4. **Tests** - After implementations work

## Notes

### URL Parsing
```rust
pub struct ParsedUrl {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub query: Option<String>,
}

impl ParsedUrl {
    pub fn parse(url: &str) -> Result<Self, HttpClientError> {
        // Handle http:// and https://
        // Default ports: 80 for HTTP, 443 for HTTPS
        // Parse path and query string
    }
}
```

### Connection Pattern
```rust
impl HttpClientConnection {
    pub fn connect<R: DnsResolver>(
        url: &ParsedUrl,
        resolver: &R,
        timeout: Option<Duration>,
    ) -> Result<Self, HttpClientError> {
        let addrs = resolver.resolve(&url.host, url.port)?;
        let conn = Connection::connect_tcp(addrs[0], timeout)?;

        match url.scheme {
            Scheme::Http => Ok(Self { stream: conn.into_plain() }),
            Scheme::Https => {
                let tls = create_tls_connector()?;
                let stream = tls.connect(&url.host, conn)?;
                Ok(Self { stream })
            }
        }
    }
}
```

### Existing Types to Reuse
- `netcap::Connection` - Raw TCP connection
- `netcap::RawStream` - Buffered stream wrapper
- `netcap::RustlsConnector` - TLS connector (feature-gated)

---
*Last Updated: 2026-01-18*
