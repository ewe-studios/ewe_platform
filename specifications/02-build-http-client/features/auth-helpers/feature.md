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
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
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

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this feature MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** in related modules to understand patterns
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific conventions
4. ✅ **Read parent specification** (`../requirements.md`) for high-level context
5. ✅ **Read module documentation** for modules this feature touches
6. ✅ **Check dependencies** by reading other feature files referenced in `depends_on`
7. ✅ **Follow discovered patterns** consistently with existing codebase

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume patterns based on typical practices without checking this codebase
- ❌ Implement without searching for similar features first
- ❌ Apply generic solutions without verifying project conventions
- ❌ Guess at naming conventions, file structures, or patterns
- ❌ Use pretraining knowledge without validating against actual project code

### Retrieval Checklist

Before implementing, answer these questions by reading code:
- [ ] What similar features exist in this project? (use Grep to find)
- [ ] What patterns do they follow? (read their implementations)
- [ ] What naming conventions are used? (observed from existing code)
- [ ] How are errors handled in similar code? (check error patterns)
- [ ] What testing patterns exist? (read existing test files)
- [ ] Are there existing helper functions I can reuse? (search thoroughly)

### Enforcement

- Show your retrieval steps in your work report
- Reference specific files/patterns you discovered
- Explain how your implementation matches existing patterns
- "I assumed..." responses will be rejected - only "I found in [file]..." accepted

---

## 🚀 CRITICAL: Token and Context Optimization

**ALL agents implementing this specification/feature MUST follow token and context optimization protocols.**

### Machine-Optimized Prompts (Rule 14)

**Main Agent MUST**:
1. Generate `machine_prompt.md` from this file when specification/feature finalized
2. Use pipe-delimited compression (58% token reduction)
3. Commit machine_prompt.md alongside human-readable file
4. Regenerate when human file updates
5. Provide machine_prompt.md path to sub-agents

**Sub-Agents MUST**:
- Read `machine_prompt.md` (NOT verbose human files)
- Parse DOCS_TO_READ section for files to load
- 58% token savings

### Context Compaction (Rule 15)

**Sub-Agents MUST** (before starting work):
1. Read machine_prompt.md and PROGRESS.md
2. Generate `COMPACT_CONTEXT.md`:
   - Embed machine_prompt.md content for current task
   - Extract current status from PROGRESS.md
   - List files for current task only (500-800 tokens)
3. CLEAR entire context
4. RELOAD from COMPACT_CONTEXT.md only
5. Proceed with 97% context reduction (180K→5K tokens)

**After PROGRESS.md Updates**:
- Regenerate COMPACT_CONTEXT.md (re-embed machine_prompt content)
- Clear and reload
- Maintain minimal context

**COMPACT_CONTEXT.md Lifecycle**:
- Generated fresh per task
- Contains ONLY current task (no history)
- Deleted when task completes
- Rewritten from scratch for next task

**See**:
- Rule 14: .agents/rules/14-machine-optimized-prompts.md
- Rule 15: .agents/rules/15-instruction-compaction.md

---

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
