---
feature: auth-helpers
description: Builder methods for common authentication schemes (Basic, Bearer, Digest, API Key)
status: pending
priority: medium
depends_on:
  - request-response
estimated_effort: small
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
      - .agents/rules/11-skills-usage.md
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

# Auth Helpers Feature

## Overview

Add convenient builder methods for common HTTP authentication schemes. This feature provides ergonomic APIs for Basic Auth, Bearer Token, Digest Auth, and custom API key headers, reducing boilerplate for authenticated requests.

## Dependencies

This feature depends on:
- `request-response` - Uses PreparedRequest and request builder patterns

This feature is required by:
- `public-api` - Exposes auth methods to users

## Requirements

### Basic Authentication

HTTP Basic Authentication using `Authorization: Basic base64(user:pass)`:

```rust
// Usage
client.get(url).basic_auth("username", "password").send()?;

// Generates header:
// Authorization: Basic dXNlcm5hbWU6cGFzc3dvcmQ=
```

Implementation:

```rust
impl ClientRequestBuilder {
    pub fn basic_auth(mut self, username: &str, password: &str) -> Self {
        let credentials = format!("{}:{}", username, password);
        let encoded = base64::encode(credentials);
        self.header(SimpleHeader::AUTHORIZATION, format!("Basic {}", encoded))
    }

    /// Basic auth with optional password (empty string if None)
    pub fn basic_auth_opt(self, username: &str, password: Option<&str>) -> Self {
        self.basic_auth(username, password.unwrap_or(""))
    }
}
```

### Bearer Token Authentication

OAuth 2.0 Bearer Token using `Authorization: Bearer <token>`:

```rust
// Usage
client.get(url).bearer_token("my-jwt-token").send()?;

// Generates header:
// Authorization: Bearer my-jwt-token
```

Implementation:

```rust
impl ClientRequestBuilder {
    pub fn bearer_token(mut self, token: &str) -> Self {
        self.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
    }

    /// Alias for bearer_token
    pub fn bearer_auth(self, token: &str) -> Self {
        self.bearer_token(token)
    }
}
```

### Digest Authentication (Optional)

HTTP Digest Authentication with challenge-response:

```rust
// Usage - first request gets 401 with challenge
// Second request includes Digest response
client.get(url)
    .digest_auth("username", "password")
    .send()?;

// Generates header (after receiving 401 challenge):
// Authorization: Digest username="user", realm="...", nonce="...", ...
```

**Note**: Digest auth is complex and optional. It requires:
1. Making initial request
2. Parsing WWW-Authenticate header from 401 response
3. Computing MD5/SHA-256 hash of credentials and challenge
4. Sending second request with Digest header

Implementation:

```rust
#[derive(Clone)]
pub struct DigestCredentials {
    username: String,
    password: String,
}

pub struct DigestChallenge {
    realm: String,
    nonce: String,
    opaque: Option<String>,
    algorithm: DigestAlgorithm,
    qop: Option<DigestQop>,
}

pub enum DigestAlgorithm {
    Md5,
    Md5Sess,
    Sha256,
    Sha256Sess,
}

pub enum DigestQop {
    Auth,
    AuthInt,
}

impl DigestChallenge {
    pub fn from_www_authenticate(header: &str) -> Result<Self, HttpClientError>;

    pub fn compute_response(
        &self,
        credentials: &DigestCredentials,
        method: &str,
        uri: &str,
    ) -> String;
}

impl ClientRequestBuilder {
    pub fn digest_auth(mut self, username: &str, password: &str) -> Self {
        self.digest_credentials = Some(DigestCredentials {
            username: username.to_string(),
            password: password.to_string(),
        });
        self
    }
}
```

### API Key Authentication

Custom header for API key authentication:

```rust
// Usage - X-API-Key header
client.get(url).api_key("X-API-Key", "my-api-key").send()?;

// Usage - query parameter (alternative)
client.get(url).query("api_key", "my-api-key").send()?;
```

Implementation:

```rust
impl ClientRequestBuilder {
    pub fn api_key(mut self, header_name: &str, key: &str) -> Self {
        self.header(header_name, key)
    }

    /// Common X-API-Key header
    pub fn x_api_key(self, key: &str) -> Self {
        self.api_key("X-API-Key", key)
    }
}
```

### Custom Authorization Header

For non-standard auth schemes:

```rust
// Usage
client.get(url)
    .header(SimpleHeader::AUTHORIZATION, "CustomScheme token123")
    .send()?;

// Or with helper
client.get(url)
    .authorization("CustomScheme", "token123")
    .send()?;
```

Implementation:

```rust
impl ClientRequestBuilder {
    pub fn authorization(mut self, scheme: &str, credentials: &str) -> Self {
        self.header(SimpleHeader::AUTHORIZATION, format!("{} {}", scheme, credentials))
    }
}
```

## Implementation Details

### File Structure

```
client/
├── auth.rs    (NEW - Authentication helpers)
└── ...
```

### Dependencies

```toml
[dependencies]
base64 = "0.21"  # For Basic auth encoding

# For Digest auth (optional)
md-5 = { version = "0.10", optional = true }
sha2 = { version = "0.10", optional = true }

[features]
digest-auth = ["md-5", "sha2"]
```

### Error Types

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    // ... existing variants ...

    #[from(ignore)]
    AuthenticationFailed(String),

    #[from(ignore)]
    DigestChallengeParseError(String),

    #[from(ignore)]
    UnsupportedDigestAlgorithm(String),
}
```

### Integration with Redirect Handling

Authentication headers should be handled carefully on redirects:

```rust
impl ClientRequestBuilder {
    /// Control whether auth headers are sent on redirects
    pub fn auth_on_redirect(mut self, enable: bool) -> Self {
        self.config.auth_on_redirect = enable;
        self
    }
}
```

By default:
- Auth headers are **removed** on cross-origin redirects
- Auth headers are **preserved** on same-origin redirects

## Success Criteria

- [ ] `auth.rs` exists and compiles
- [ ] `basic_auth()` correctly encodes credentials in Base64
- [ ] `bearer_token()` correctly formats Bearer header
- [ ] `api_key()` works with custom header names
- [ ] `x_api_key()` uses X-API-Key header
- [ ] `authorization()` works with custom schemes
- [ ] Digest auth parses WWW-Authenticate challenges (feature-gated)
- [ ] Digest auth computes correct response hash (feature-gated)
- [ ] Auth headers are removed on cross-origin redirects
- [ ] Auth headers are preserved on same-origin redirects
- [ ] `auth_on_redirect()` config works
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- auth
cargo build --package foundation_core
cargo build --package foundation_core --features digest-auth
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** request-response feature is complete
- **MUST READ** RFC 7617 (Basic Auth) for correct format
- **MUST READ** RFC 6750 (Bearer Token) for correct format
- **MUST READ** RFC 7616 (Digest Auth) if implementing digest

### Implementation Guidelines
- base64 encoding must use standard alphabet (not URL-safe)
- Bearer token should not be modified (preserve exact value)
- Digest auth is optional - feature-gate behind `digest-auth`
- Consider auth header security on redirects
- Use existing SimpleHeader constants where available

### Security Considerations
- Never log authentication credentials
- Warn about sending auth over non-HTTPS
- Clear credentials from memory after use (consider zeroize)

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
