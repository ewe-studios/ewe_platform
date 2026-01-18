---
feature: public-api
completed: 0
uncompleted: 6
last_updated: 2026-01-18
tools:
  - Rust
  - cargo
---

# Public API - Tasks

## Task List

### User-Facing API
- [ ] Create `client/api.rs` - ClientRequest with user-friendly methods
- [ ] Implement `.introduction()`, `.body()`, `.send()`, `.parts()`, `.collect()`

### Main Client
- [ ] Create `client/client.rs` - SimpleHttpClient with builder pattern
- [ ] Implement all convenience methods (get, post, put, delete, etc.)

### Connection Pool (Optional)
- [ ] Create `client/pool.rs` - Optional ConnectionPool implementation

### Integration
- [ ] Modify `simple_http/mod.rs` to include `pub mod client` and add feature flag `multi` to Cargo.toml

## Implementation Order

1. **api.rs** - ClientRequest (depends on all internal machinery)
2. **client.rs** - SimpleHttpClient (depends on api.rs)
3. **pool.rs** - Connection pooling (optional, can be added after client works)
4. **Integration** - mod.rs updates and Cargo.toml (final step)

## Notes

### Public Re-exports in client/mod.rs
```rust
// Public types
pub use api::ClientRequest;
pub use client::{SimpleHttpClient, ClientConfig};
pub use dns::{DnsResolver, SystemDnsResolver, CachingDnsResolver};
pub use errors::{HttpClientError, DnsError};
pub use intro::ResponseIntro;
pub use request::ClientRequestBuilder;

// Internal modules (not re-exported)
mod actions;
mod connection;
mod executor;
mod pool;
mod task;
```

### SimpleHttpClient Pattern
```rust
let client = SimpleHttpClient::new()
    .connect_timeout(Duration::from_secs(10))
    .max_redirects(5)
    .enable_pool(20);

let response = client.get("http://example.com")?.send()?;
```

### ClientRequest Usage Pattern
```rust
// Simple usage
let response = client.get("http://example.com")?.send()?;

// Streaming usage
let mut request = client.get("http://example.com")?;
let (intro, headers) = request.introduction()?;
let body = request.body()?;

// Power user
for part in client.get("http://example.com")?.parts() {
    match part? {
        IncomingResponseParts::Intro(status, proto, reason) => { ... }
        IncomingResponseParts::Headers(headers) => { ... }
        IncomingResponseParts::SizedBody(body) => { ... }
        _ => {}
    }
}
```

### simple_http/mod.rs Addition
```rust
pub mod client;
pub use client::{SimpleHttpClient, ClientRequest, HttpClientError};
```

---
*Last Updated: 2026-01-18*
