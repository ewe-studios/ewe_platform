---
feature: public-api
description: User-facing API (ClientRequest, SimpleHttpClient), optional connection pooling, and module integration
status: pending
priority: high
depends_on:
  - foundation
  - connection
  - request-response
  - task-iterator
estimated_effort: medium
created: 2026-01-18
last_updated: 2026-01-24
author: Main Agent
tasks:
  completed: 0
  uncompleted: 17
  total: 17
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
      - ./templates/
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

# Public API Feature

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

## Overview

Create the user-facing API for the HTTP 1.1 client. This feature implements the clean public API that hides all TaskIterator complexity, the main `SimpleHttpClient` entry point, optional connection pooling, and final module integration.

## Dependencies

This feature depends on:
- `foundation` - DnsResolver, errors
- `connection` - HttpClientConnection, ParsedUrl
- `request-response` - ClientRequestBuilder, ResponseIntro
- `task-iterator` - execute_task(), HttpRequestTask

This feature is required by:
- None (final feature)

## Design Principle: Hide Internal Complexity

The internal `TaskIterator` machinery **MUST** be hidden from the user. Users interact with a clean, simple API.

## Requirements

### User-Facing API

```rust
// Create client
let http_client = SimpleHttpClient::new();

// Option 1: Get just the intro and headers
let mut request = http_client.get("http://google.com");
let (intro, headers) = request.introduction()?;
let body = request.body()?;

// Option 2: Get everything at once
let response = http_client.get("http://google.com").send()?;

// Option 3: Iterate over parts (power user)
for part in http_client.get("http://google.com").parts() { ... }
```

### ClientRequest

```rust
pub struct ClientRequest {
    // Internal: prepared request, client config, etc.
}

impl ClientRequest {
    pub fn introduction(&mut self) -> Result<(ResponseIntro, SimpleHeaders), HttpClientError>;
    pub fn body(&mut self) -> Result<SimpleBody, HttpClientError>;
    pub fn send(self) -> Result<SimpleResponse<SimpleBody>, HttpClientError>;
    pub fn parts(self) -> impl Iterator<Item = Result<IncomingResponseParts, HttpClientError>>;
    pub fn collect(self) -> Result<Vec<IncomingResponseParts>, HttpClientError>;
}
```

### SimpleHttpClient

```rust
pub struct SimpleHttpClient<R: DnsResolver = SystemDnsResolver> {
    resolver: R,
    config: ClientConfig,
    pool: Option<ConnectionPool>,
}

pub struct ClientConfig {
    pub connect_timeout: Option<Duration>,
    pub read_timeout: Option<Duration>,
    pub write_timeout: Option<Duration>,
    pub max_redirects: u8,
    pub default_headers: SimpleHeaders,
    pub pool_enabled: bool,
    pub pool_max_connections: usize,
}

impl SimpleHttpClient {
    pub fn new() -> Self;
}

impl<R: DnsResolver> SimpleHttpClient<R> {
    pub fn with_resolver(resolver: R) -> Self;
    pub fn config(self, config: ClientConfig) -> Self;
    pub fn connect_timeout(self, timeout: Duration) -> Self;
    pub fn max_redirects(self, max: u8) -> Self;
    pub fn enable_pool(self, max_connections: usize) -> Self;

    pub fn get(&self, url: &str) -> Result<ClientRequest, HttpClientError>;
    pub fn post(&self, url: &str) -> Result<ClientRequest, HttpClientError>;
    pub fn put(&self, url: &str) -> Result<ClientRequest, HttpClientError>;
    pub fn delete(&self, url: &str) -> Result<ClientRequest, HttpClientError>;
    pub fn patch(&self, url: &str) -> Result<ClientRequest, HttpClientError>;
    pub fn head(&self, url: &str) -> Result<ClientRequest, HttpClientError>;
    pub fn options(&self, url: &str) -> Result<ClientRequest, HttpClientError>;

    pub fn request(&self, builder: ClientRequestBuilder) -> ClientRequest;
}
```

### Method Mapping to Internal Logic

| User Method | Internal Behavior |
|-------------|-------------------|
| `request.introduction()` | Executes TaskIterator until Intro+Headers received |
| `request.body()` | Continues TaskIterator to read body |
| `request.send()` | Executes full TaskIterator, returns complete response |
| `request.parts()` | Returns iterator wrapper that drives TaskIterator |

### ConnectionPool (Optional)

```rust
pub struct ConnectionPool {
    connections: HashMap<PoolKey, Vec<PooledConnection>>,
    max_connections: usize,
}
```

### Feature Flags

Add to `Cargo.toml`:

```toml
[features]
default = []
multi = []
ssl-rustls = ["rustls", "webpki-roots"]
ssl-openssl = ["openssl"]
ssl-native-tls = ["native-tls"]
```

## Implementation Details

### File Structure

```
client/
├── api.rs       (NEW - ClientRequest)
├── client.rs    (NEW - SimpleHttpClient)
├── pool.rs      (NEW - ConnectionPool, optional)
└── ...
```

### Public vs Internal

| File | Visibility | Purpose |
|------|------------|---------|
| `client.rs` | **Public** | `SimpleHttpClient` |
| `api.rs` | **Public** | `ClientRequest` |
| `request.rs` | **Public** | `ClientRequestBuilder` |
| `intro.rs` | **Public** | `ResponseIntro` |
| `errors.rs` | **Public** | Error types |
| `dns.rs` | **Public** | `DnsResolver` trait |
| `task.rs` | Internal | `HttpRequestTask` |
| `actions.rs` | Internal | `ExecutionAction` implementations |
| `executor.rs` | Internal | Executor selection |
| `connection.rs` | Internal | Connection management |
| `pool.rs` | Internal | Connection pooling |

## Success Criteria

- [ ] `ClientRequest.introduction()` returns ResponseIntro and SimpleHeaders
- [ ] `ClientRequest.body()` returns SimpleBody
- [ ] `ClientRequest.send()` returns SimpleResponse<SimpleBody>
- [ ] `ClientRequest.parts()` returns iterator over IncomingResponseParts
- [ ] `SimpleHttpClient::new()` creates default client
- [ ] `SimpleHttpClient::with_resolver()` accepts custom resolver
- [ ] All convenience methods (get, post, etc.) work
- [ ] Builder methods (config, timeout, etc.) work
- [ ] Connection pooling works when enabled
- [ ] Redirect following works (configurable)
- [ ] `pub mod client` added to `simple_http/mod.rs`
- [ ] Feature flag `multi` added to Cargo.toml
- [ ] Plain HTTP requests work end-to-end
- [ ] HTTPS requests work (with TLS feature)
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core
cargo build --package foundation_core
cargo build --package foundation_core --features multi
cargo build --package foundation_core --features ssl-rustls
cargo build --package foundation_core --all-features
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** all previous features are complete
- **MUST READ** existing `simple_http/mod.rs` for module pattern
- **MUST READ** `simple_http/impls.rs` for types to reuse

### Implementation Guidelines
- Public API should be clean and simple
- Hide all TaskIterator complexity
- Reuse existing types (SimpleResponse, IncomingResponseParts, etc.)
- Use generic type parameters for DnsResolver
- Connection pooling is optional (configurable)

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
