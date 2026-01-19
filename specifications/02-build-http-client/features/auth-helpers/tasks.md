---
feature: auth-helpers
completed: 0
uncompleted: 10
last_updated: 2026-01-19
tools:
  - Rust
  - cargo
---

# Auth Helpers - Tasks

## Task List

### Module Setup
- [ ] Create `client/auth.rs` - Authentication helpers module
- [ ] Add base64 dependency to Cargo.toml
- [ ] Add optional md-5 and sha2 dependencies for digest auth

### Basic Auth
- [ ] Implement `basic_auth(username, password)` method
- [ ] Implement `basic_auth_opt(username, Option<password>)` method
- [ ] Add unit tests for Base64 encoding

### Bearer Token
- [ ] Implement `bearer_token(token)` method
- [ ] Implement `bearer_auth(token)` alias

### API Key
- [ ] Implement `api_key(header_name, key)` method
- [ ] Implement `x_api_key(key)` convenience method

### Custom Authorization
- [ ] Implement `authorization(scheme, credentials)` method

### Digest Auth (Optional)
- [ ] Define `DigestCredentials` struct
- [ ] Define `DigestChallenge` struct with parsing
- [ ] Define `DigestAlgorithm` enum (MD5, SHA-256)
- [ ] Define `DigestQop` enum (auth, auth-int)
- [ ] Implement `from_www_authenticate()` parser
- [ ] Implement `compute_response()` hash computation
- [ ] Implement `digest_auth(username, password)` method

### Redirect Security
- [ ] Implement `auth_on_redirect(bool)` config
- [ ] Strip auth headers on cross-origin redirects
- [ ] Preserve auth headers on same-origin redirects

### Error Handling
- [ ] Add `AuthenticationFailed` error variant
- [ ] Add `DigestChallengeParseError` error variant
- [ ] Add `UnsupportedDigestAlgorithm` error variant

## Implementation Order

1. **Cargo.toml** - Add base64 dependency
2. **auth.rs** - Basic auth with Base64 encoding
3. **auth.rs** - Bearer token authentication
4. **auth.rs** - API key methods
5. **auth.rs** - Custom authorization helper
6. **request.rs** - Add auth methods to ClientRequestBuilder
7. **Digest auth** (optional) - Feature-gated digest implementation
8. **Redirect handling** - Auth header security on redirects

## Notes

### Basic Auth Pattern
```rust
impl ClientRequestBuilder {
    pub fn basic_auth(mut self, username: &str, password: &str) -> Self {
        let credentials = format!("{}:{}", username, password);
        let encoded = base64::encode(credentials);
        self.header(SimpleHeader::AUTHORIZATION, format!("Basic {}", encoded))
    }
}
```

### Bearer Token Pattern
```rust
impl ClientRequestBuilder {
    pub fn bearer_token(mut self, token: &str) -> Self {
        self.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
    }
}
```

### Redirect Security Pattern
```rust
fn should_preserve_auth_header(original: &ParsedUrl, redirect: &ParsedUrl) -> bool {
    // Same-origin check
    original.host == redirect.host
        && original.port == redirect.port
        && original.scheme == redirect.scheme
}
```

### Digest WWW-Authenticate Format
```
WWW-Authenticate: Digest realm="example",
                         nonce="abc123",
                         algorithm=SHA-256,
                         qop="auth"
```

---
*Last Updated: 2026-01-19*
