---
feature: foundation
description: Error types (HttpClientError, DnsError) and DNS resolution with caching support
status: pending
depends_on:
  - tls-verification
estimated_effort: small
created: 2026-01-18
last_updated: 2026-01-18
---

# Foundation Feature

## Overview

Create the foundational layer for the HTTP 1.1 client: error types and DNS resolution. This feature establishes the error handling patterns and pluggable DNS resolution system that all subsequent features will build upon.

## Dependencies

This feature depends on:
- `tls-verification` - Ensures TLS infrastructure works before building on it

This feature is required by:
- `connection` - Uses DnsResolver for hostname resolution
- `task-iterator` - Uses HttpClientError for error handling
- `public-api` - Exposes error types to users

## Requirements

### Error Handling Pattern (MANDATORY)

All error types **MUST** follow this pattern:

```rust
use derive_more::From;

#[derive(From, Debug)]
pub enum HttpClientError {
    #[from(ignore)]
    DnsResolutionFailed(String),

    #[from(ignore)]
    ConnectionFailed(BoxedError),

    // ... other variants with clear descriptions
}

impl std::error::Error for HttpClientError {}

impl core::fmt::Display for HttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DnsResolutionFailed(host) => {
                write!(f, "DNS resolution failed for host: {}", host)
            }
            // ... clear, descriptive messages for each variant
        }
    }
}
```

### DNS Resolution

1. **DnsResolver Trait**
   - Generic trait for pluggable DNS implementations
   - Method: `resolve(&self, host: &str) -> Result<Vec<SocketAddr>, DnsError>`

2. **SystemDnsResolver**
   - Uses `std::net::ToSocketAddrs`
   - Default implementation for production use

3. **CachingDnsResolver<R: DnsResolver>**
   - Wraps any DnsResolver with caching
   - TTL-based expiration support
   - Generic wrapper (not boxed)

4. **MockDnsResolver**
   - For testing purposes
   - Configurable responses

### Generic Type Pattern (MANDATORY)

Use generics instead of boxed types:

```rust
// Preferred
pub fn with_resolver<R: DnsResolver>(resolver: R) -> Self

// Avoid when possible
pub fn with_resolver(resolver: Box<dyn DnsResolver>) -> Self
```

## Implementation Details

### File Structure

```
client/
├── mod.rs       (module entry, re-exports)
├── errors.rs    (HttpClientError, DnsError)
└── dns.rs       (DnsResolver trait + implementations)
```

### DnsError

```rust
#[derive(From, Debug)]
pub enum DnsError {
    #[from(ignore)]
    ResolutionFailed(String),

    #[from(ignore)]
    InvalidHost(String),

    #[from(ignore)]
    NoAddressesFound(String),

    #[from]
    IoError(std::io::Error),
}
```

### HttpClientError (Initial Variants)

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    #[from]
    DnsError(DnsError),

    #[from(ignore)]
    ConnectionFailed(String),

    #[from(ignore)]
    InvalidUrl(String),

    // Additional variants will be added in later features
}
```

### DnsResolver Trait

```rust
pub trait DnsResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError>;
}
```

### CachingDnsResolver

```rust
pub struct CachingDnsResolver<R: DnsResolver> {
    inner: R,
    cache: HashMap<String, CachedEntry>,
    ttl: Duration,
}

struct CachedEntry {
    addresses: Vec<SocketAddr>,
    expires_at: Instant,
}
```

## Success Criteria

- [ ] `client/mod.rs` exists and compiles
- [ ] `DnsError` implements `From`, `Debug`, `Display`, `std::error::Error`
- [ ] `HttpClientError` implements `From`, `Debug`, `Display`, `std::error::Error`
- [ ] `DnsResolver` trait is defined with generic support
- [ ] `SystemDnsResolver` resolves hostnames correctly
- [ ] `CachingDnsResolver<R>` caches results with TTL
- [ ] `MockDnsResolver` works for testing
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- dns
cargo build --package foundation_core
```

## Notes for Agents

### Before Starting
- **MUST READ** parent specification's requirements.md
- **MUST READ** existing `simple_http/errors.rs` for error patterns
- **MUST READ** existing `simple_http/mod.rs` for module structure
- **MUST VERIFY** derive_more is available in dependencies

### Implementation Guidelines
- Use `derive_more::From` for error enums
- Implement `Debug` and `Display` with clear error messages
- Use generic type parameters for CachingDnsResolver
- Add `#[must_use]` on pure functions

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
