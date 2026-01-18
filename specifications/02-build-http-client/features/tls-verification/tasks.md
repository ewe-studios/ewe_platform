---
feature: tls-verification
completed: 0
uncompleted: 8
last_updated: 2026-01-18
tools:
  - Rust
  - cargo
---

# TLS Verification - Tasks

## Task List

### Fix Feature Conflicts
- [ ] Update `Cargo.toml` - Remove `ssl` from default features OR add webpki-roots to ssl-rustls
- [ ] Add `compile_error!` macros to `ssl/mod.rs` for conflicting feature combinations

### Add Root Certificates for Rustls
- [ ] Add `webpki-roots` dependency for `ssl-rustls` feature in Cargo.toml
- [ ] Update `rustls.rs` to create default client config with webpki-roots

### Add Tests
- [ ] Add unit tests for `ssl/rustls.rs` (acceptor, connector creation)
- [ ] Add unit tests for `ssl/openssl.rs` (acceptor, connector creation)
- [ ] Add unit tests for `ssl/native_ttls.rs` (acceptor, connector creation)
- [ ] Add integration test for HTTPS connection (at least for rustls)

## Implementation Order

1. **Fix feature conflicts** - Cargo.toml and compile_error! macros
2. **Add webpki-roots** - For rustls client connections
3. **Add tests** - Unit tests first, then integration tests

## Notes

### Feature Conflict Fix
```toml
# Option 1: Remove ssl from default (recommended)
default = ["standard"]

# Option 2: Keep ssl default but add webpki-roots
ssl-rustls = ["rustls", "rustls-pemfile", "webpki-roots", "zeroize"]
```

### Compile Error Pattern
```rust
#[cfg(all(feature = "ssl-rustls", feature = "ssl-openssl"))]
compile_error!("Cannot enable both `ssl-rustls` and `ssl-openssl`. Choose one.");
```

### Testing Commands
```bash
# Test each backend individually
cargo test --package foundation_core --no-default-features --features ssl-rustls -- ssl
cargo test --package foundation_core --no-default-features --features ssl-openssl -- ssl
cargo test --package foundation_core --no-default-features --features ssl-native-tls -- ssl
```

### HTTPS Integration Test Pattern
```rust
#[test]
#[cfg(feature = "ssl-rustls")]
fn test_rustls_https_get() {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    // Connect to example.com:443
    // Perform TLS handshake
    // Send HTTP GET request
    // Verify response
}
```

---
*Last Updated: 2026-01-18*
