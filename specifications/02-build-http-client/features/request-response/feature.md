---
feature: request-response
description: Request builder (ClientRequestBuilder), response types (ResponseIntro), and prepared request structure
status: pending
priority: high
depends_on:
  - foundation
  - connection
estimated_effort: small
created: 2026-01-18
last_updated: 2026-01-24
author: Main Agent
tasks:
  completed: 0
  uncompleted: 10
  total: 10
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

# Request/Response Feature

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

Create the request building and response type infrastructure for the HTTP 1.1 client. This feature defines the fluent API for building requests and the wrapper types for responses.

## Dependencies

This feature depends on:
- `foundation` - Uses HttpClientError for errors
- `connection` - Uses ParsedUrl for URL handling

This feature is required by:
- `task-iterator` - Uses PreparedRequest and ResponseIntro
- `public-api` - Exposes ClientRequestBuilder and ResponseIntro

## Reusing Existing Types

The client reuses existing types from `simple_http/impls.rs`:

| Existing Type | Usage in Client |
|---------------|-----------------|
| `SimpleResponse<T>` | Final collected response: `SimpleResponse<SimpleBody>` |
| `IncomingResponseParts` | Iterator yields these parts |
| `Status` | HTTP status codes |
| `Proto` | HTTP protocol version |
| `SimpleHeaders` | Header collections |
| `SimpleBody` | Body variants |
| `SimpleMethod` | HTTP methods |
| `SimpleHeader` | Header keys |
| `Http11RequestIterator` | Renders request to bytes |

## Requirements

### ResponseIntro

Wrapper for intro information from `IncomingResponseParts::Intro`:

```rust
pub struct ResponseIntro {
    pub status: Status,
    pub proto: Proto,
    pub reason: Option<String>,
}

impl From<(Status, Proto, Option<String>)> for ResponseIntro {
    fn from((status, proto, reason): (Status, Proto, Option<String>)) -> Self {
        Self { status, proto, reason }
    }
}
```

### PreparedRequest

Internal type representing a fully prepared request ready to send:

```rust
pub struct PreparedRequest {
    pub method: SimpleMethod,
    pub url: ParsedUrl,
    pub headers: SimpleHeaders,
    pub body: SimpleBody,
}

impl PreparedRequest {
    pub fn into_request_iterator(self) -> Http11RequestIterator;
}
```

### ClientRequestBuilder

Fluent API for building requests:

```rust
pub struct ClientRequestBuilder {
    method: SimpleMethod,
    url: ParsedUrl,
    headers: SimpleHeaders,
    body: Option<SimpleBody>,
}

impl ClientRequestBuilder {
    pub fn new(method: SimpleMethod, url: &str) -> Result<Self, HttpClientError>;

    // Header methods
    pub fn header(self, key: SimpleHeader, value: impl Into<String>) -> Self;
    pub fn headers(self, headers: SimpleHeaders) -> Self;

    // Body methods
    pub fn body_text(self, text: impl Into<String>) -> Self;
    pub fn body_bytes(self, bytes: Vec<u8>) -> Self;
    pub fn body_json<T: Serialize>(self, value: &T) -> Result<Self, HttpClientError>;
    pub fn body_form(self, params: &[(String, String)]) -> Self;

    // Build
    pub fn build(self) -> PreparedRequest;
}
```

### Convenience Methods

```rust
impl ClientRequestBuilder {
    pub fn get(url: &str) -> Result<Self, HttpClientError>;
    pub fn post(url: &str) -> Result<Self, HttpClientError>;
    pub fn put(url: &str) -> Result<Self, HttpClientError>;
    pub fn delete(url: &str) -> Result<Self, HttpClientError>;
    pub fn patch(url: &str) -> Result<Self, HttpClientError>;
    pub fn head(url: &str) -> Result<Self, HttpClientError>;
    pub fn options(url: &str) -> Result<Self, HttpClientError>;
}
```

## Implementation Details

### File Structure

```
client/
├── request.rs   (NEW - ClientRequestBuilder, PreparedRequest)
├── intro.rs     (NEW - ResponseIntro)
└── ...
```

## Success Criteria

- [ ] `ResponseIntro` correctly wraps status, proto, reason
- [ ] `ResponseIntro` implements From for tuple conversion
- [ ] `PreparedRequest` holds all request data
- [ ] `PreparedRequest::into_request_iterator()` works
- [ ] `ClientRequestBuilder` fluent API works
- [ ] All convenience methods (get, post, etc.) work
- [ ] Header methods work correctly
- [ ] Body methods (text, bytes, json, form) work
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- request
cargo test --package foundation_core -- intro
cargo build --package foundation_core
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** foundation and connection features are complete
- **MUST READ** `simple_http/impls.rs` for existing HTTP structures
- **MUST READ** `Http11RequestIterator` implementation

### Implementation Guidelines
- Reuse existing types from impls.rs (DO NOT duplicate)
- Use fluent builder pattern
- Keep PreparedRequest as internal (pub(crate))
- ResponseIntro is public (user-facing)

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
