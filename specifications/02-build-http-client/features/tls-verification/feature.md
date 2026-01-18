---
feature: tls-verification
description: Verify and fix TLS module to ensure all SSL backends work correctly with proper feature gating
status: pending
depends_on: []
estimated_effort: medium
created: 2026-01-18
last_updated: 2026-01-18
---

# TLS Verification Feature

## Overview

Verify and fix the existing TLS/SSL module in `netcap/ssl/` to ensure all three SSL backends (rustls, openssl, native-tls) work correctly with proper feature gating. This is a prerequisite for the HTTP client as it depends on working TLS infrastructure.

## Dependencies

This feature depends on:
- None (this is the first feature to work on)

This feature is required by:
- `connection` - Uses TLS connectors for HTTPS
- All subsequent HTTP client features

## Current State Analysis

### TLS Module Location
- `backends/foundation_core/src/netcap/ssl/mod.rs` - Module entry with feature-gated re-exports
- `backends/foundation_core/src/netcap/ssl/rustls.rs` - Rustls implementation
- `backends/foundation_core/src/netcap/ssl/openssl.rs` - OpenSSL implementation
- `backends/foundation_core/src/netcap/ssl/native_ttls.rs` - Native-TLS implementation

### Feature Configuration (Cargo.toml)
```toml
[features]
ssl-openssl = ["openssl", "zeroize"]
ssl-rustls = ["rustls", "rustls-pemfile", "zeroize"]
ssl-native-tls = ["native-tls", "native-tls/vendored", "zeroize"]
ssl = ["ssl-rustls"]
default = ["standard", "ssl"]
```

### Issues Found

1. **Feature conflict with defaults**
   - Problem: `default` includes `ssl` which includes `ssl-rustls`
   - When user enables `ssl-openssl`, they get BOTH `ssl-rustls` AND `ssl-openssl`
   - The cfg conditions in `mod.rs` require exactly ONE backend:
     ```rust
     #[cfg(all(feature = "ssl-openssl", not(feature="ssl-rustls"), not(feature="ssl-native-tls")))]
     ```
   - Result: Compilation fails with "unresolved imports"

2. **No enforcement of mutual exclusivity**
   - Cargo allows enabling multiple conflicting features
   - The code assumes only one is enabled, causing silent failures

3. **Missing root certificates for client connections**
   - `ssl-rustls` doesn't include `webpki-roots` dependency
   - Client TLS connections may fail without system or bundled root certs

4. **No tests exist**
   - No unit tests for any TLS backend
   - No integration tests for TLS connections
   - No tests verifying feature combinations

5. **Inconsistent connector APIs**
   - Each backend has slightly different connector patterns
   - No unified trait for TLS connectors

## Requirements

### 1. Fix Feature Conflicts

**Option A: Remove ssl from default features** (Recommended)
```toml
[features]
default = ["standard"]  # Remove ssl from default
ssl-openssl = ["openssl", "zeroize"]
ssl-rustls = ["rustls", "rustls-pemfile", "webpki-roots", "zeroize"]
ssl-native-tls = ["native-tls", "native-tls/vendored", "zeroize"]
```

**Option B: Add compile-time error for conflicting features**
```rust
#[cfg(all(feature = "ssl-rustls", feature = "ssl-openssl"))]
compile_error!("Cannot enable both ssl-rustls and ssl-openssl. Choose one.");
```

### 2. Add Root Certificates for Rustls

For client connections, rustls needs root certificates:
```toml
ssl-rustls = ["rustls", "rustls-pemfile", "webpki-roots", "zeroize"]
```

Update `rustls.rs` to use webpki-roots or system certs for client connections.

### 3. Add Unit Tests

Each backend needs tests for:
- Acceptor creation from PEM certificates
- Connector creation
- Client TLS handshake (mocked or real)
- Server TLS accept (mocked or real)

### 4. Add Integration Tests

Test actual HTTPS connections:
```rust
#[test]
#[cfg(feature = "ssl-rustls")]
fn test_rustls_https_connection() {
    // Connect to a real HTTPS endpoint
    // Verify handshake works
}
```

### 5. Verify Each Backend Compiles Correctly

All three must compile:
```bash
cargo check --package foundation_core --no-default-features --features ssl-rustls
cargo check --package foundation_core --no-default-features --features ssl-openssl
cargo check --package foundation_core --no-default-features --features ssl-native-tls
```

## Implementation Details

### Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Fix feature conflicts, add webpki-roots |
| `ssl/mod.rs` | Add compile_error! for conflicting features |
| `ssl/rustls.rs` | Add root certificate support, add tests |
| `ssl/openssl.rs` | Add tests |
| `ssl/native_ttls.rs` | Add tests |

### Feature Conflict Detection (Add to ssl/mod.rs)

```rust
// Ensure only one SSL backend is enabled
#[cfg(all(feature = "ssl-rustls", feature = "ssl-openssl"))]
compile_error!("Cannot enable both `ssl-rustls` and `ssl-openssl`. Choose one TLS backend.");

#[cfg(all(feature = "ssl-rustls", feature = "ssl-native-tls"))]
compile_error!("Cannot enable both `ssl-rustls` and `ssl-native-tls`. Choose one TLS backend.");

#[cfg(all(feature = "ssl-openssl", feature = "ssl-native-tls"))]
compile_error!("Cannot enable both `ssl-openssl` and `ssl-native-tls`. Choose one TLS backend.");
```

### Rustls Root Certificates

```rust
use rustls::RootCertStore;
use webpki_roots::TLS_SERVER_ROOTS;

fn default_client_config() -> Arc<ClientConfig> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(TLS_SERVER_ROOTS.iter().cloned());

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Arc::new(config)
}
```

## Success Criteria

- [ ] `ssl-rustls` compiles with `--no-default-features --features ssl-rustls`
- [ ] `ssl-openssl` compiles with `--no-default-features --features ssl-openssl`
- [ ] `ssl-native-tls` compiles with `--no-default-features --features ssl-native-tls`
- [ ] Conflicting features produce clear compile_error!
- [ ] Rustls includes webpki-roots for client connections
- [ ] Unit tests exist for each backend
- [ ] Integration test connects to HTTPS endpoint (rustls at minimum)
- [ ] All tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
# Check each backend compiles
cargo check --package foundation_core --no-default-features --features ssl-rustls
cargo check --package foundation_core --no-default-features --features ssl-openssl
cargo check --package foundation_core --no-default-features --features ssl-native-tls

# Verify conflicting features fail with clear message
cargo check --package foundation_core --no-default-features --features ssl-rustls,ssl-openssl 2>&1 | grep "compile_error"

# Run tests
cargo test --package foundation_core --no-default-features --features ssl-rustls -- ssl
cargo test --package foundation_core --no-default-features --features ssl-openssl -- ssl
cargo test --package foundation_core --no-default-features --features ssl-native-tls -- ssl

# Formatting and lints
cargo fmt -- --check
cargo clippy --package foundation_core --no-default-features --features ssl-rustls -- -D warnings
```

## Notes for Agents

### Before Starting
- **MUST READ** parent specification's requirements.md
- **MUST READ** all files in `netcap/ssl/` directory
- **MUST READ** `netcap/no_wasm.rs` to understand how SSL types are used

### Implementation Guidelines
- Keep existing API compatibility
- Add tests as you fix issues
- Test each backend individually
- Use `--no-default-features` when testing specific backends

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
