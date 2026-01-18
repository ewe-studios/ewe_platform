---
feature: foundation
completed: 0
uncompleted: 7
last_updated: 2026-01-18
tools:
  - Rust
  - cargo
---

# Foundation - Tasks

## Task List

### Module Setup
- [ ] Create `client/mod.rs` - Module entry point with re-exports

### Error Types
- [ ] Create `client/errors.rs` - DnsError with derive_more::From, Debug, Display
- [ ] Add HttpClientError (initial variants) with derive_more::From, Debug, Display

### DNS Resolution
- [ ] Create `client/dns.rs` - DnsResolver trait with generic type support
- [ ] Implement `SystemDnsResolver` using `std::net::ToSocketAddrs`
- [ ] Implement `CachingDnsResolver<R: DnsResolver>` with TTL support
- [ ] Implement `MockDnsResolver` for testing

## Implementation Order

1. **mod.rs** - Empty module structure first
2. **errors.rs** - Error types (no dependencies)
3. **dns.rs** - DnsResolver trait, then implementations

## Notes

### Error Pattern
```rust
#[derive(From, Debug)]
pub enum ErrorType {
    #[from(ignore)]
    VariantName(InnerType),
}

impl std::error::Error for ErrorType {}
impl Display for ErrorType { /* clear messages */ }
```

### Generic DNS Pattern
```rust
pub trait DnsResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError>;
}

pub struct CachingDnsResolver<R: DnsResolver> {
    inner: R,
    cache: HashMap<String, CachedEntry>,
    ttl: Duration,
}
```

### Re-exports in mod.rs
```rust
mod dns;
mod errors;

pub use dns::{DnsResolver, SystemDnsResolver, CachingDnsResolver, MockDnsResolver};
pub use errors::{DnsError, HttpClientError};
```

---
*Last Updated: 2026-01-18*
