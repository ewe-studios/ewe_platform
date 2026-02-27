---
feature: request-response
description: Request builder (ClientRequestBuilder), response types (ResponseIntro), and prepared request structure
status: pending
depends_on:
  - foundation
  - connection
estimated_effort: small
created: 2026-01-18
last_updated: 2026-01-18
---

# Request/Response Feature

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
