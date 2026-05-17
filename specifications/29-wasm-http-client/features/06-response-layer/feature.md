---
feature: "Response Layer"
description: "Response struct wrapping parsed result data from ABI (status/headers/body MemoryId/URL/ExternalPointer), body consumption methods (text/bytes/json), error_for_status"
status: "pending"
priority: "high"
depends_on: ["abi-http-bridge", "body-types"]
estimated_effort: "medium"
created: 2026-05-18
last_updated: 2026-05-18
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---

# Response Layer

## Overview

This feature implements the `Response` struct — the representation of an HTTP response. Unlike reqwest which wraps `http::Response<web_sys::Response>`, our Response holds the parsed data extracted from the ABI result MemoryId: status code, headers, body bytes, URL, and an ExternalPointer to the JS Response object for potential streaming.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | Response types, body consumption, error checking | `.agents/skills/rust-clean-code/skill.md` |

## Architecture (COMPREHENSIVE)

### File Structure

- `backends/foundation_wasm/src/http/response.rs` — Response struct and all consumption methods

### Response Struct

```rust
pub struct Response {
    /// HTTP status code
    status: StatusCode,

    /// Response headers
    headers: HeaderMap,

    /// Response body (already read from shared memory)
    body: Body,

    /// Final URL (after redirects)
    url: alloc::string::String,

    /// ExternalPointer to the JS Response object.
    /// Kept for potential streaming (bytes_stream) and cleanup.
    /// None if the body has already been consumed as text/bytes.
    js_response: Option<ExternalPointer>,

    /// AbortGuard to keep the AbortController alive until Response is dropped.
    _abort: AbortGuard,
}
```

**Key difference from reqwest**: The body is already materialized (read from shared memory by the fetch executor). In reqwest, the body is lazily consumed from the `web_sys::Response`. Here, the JS side already called `response.arrayBuffer()` and wrote the bytes to shared memory. The `Body` field contains those bytes directly.

### Body Consumption Methods

Since the body is already in memory, these methods are simpler than reqwest's async versions:

```rust
impl Response {
    /// Consumes the response and returns the body as text.
    /// Returns an error if the body is not valid UTF-8.
    pub fn text(self) -> HttpResult<String> {
        let body = self.body;
        match body.as_bytes() {
            Some(bytes) => {
                String::from_utf8(bytes.to_vec())
                    .map_err(|e| HttpError::Decode(e.to_string()))
            }
            None => Err(HttpError::Decode("multipart body cannot be converted to text".into())),
        }
    }

    /// Consumes the response and returns the body as bytes.
    pub fn bytes(self) -> HttpResult<Bytes> {
        let body = self.body;
        match body.as_bytes() {
            Some(bytes) => Ok(Bytes::from(bytes.to_vec())),
            None => Err(HttpError::Decode("multipart body cannot be converted to bytes".into())),
        }
    }

    /// Consumes the response and deserializes the body as JSON.
    #[cfg(feature = "http-json")]
    pub fn json<T: DeserializeOwned>(self) -> HttpResult<T> {
        let bytes = self.bytes()?;
        serde_json::from_slice(bytes.as_slice())
            .map_err(|e| HttpError::Decode(e.to_string()))
    }
}
```

**Note**: These are synchronous methods since the data is already in memory. No async needed.

### Status and Headers Access

```rust
impl Response {
    /// Returns the HTTP status code.
    pub fn status(&self) -> StatusCode;

    /// Returns a reference to the response headers.
    pub fn headers(&self) -> &HeaderMap;

    /// Returns the Content-Length header value, if present.
    pub fn content_length(&self) -> Option<u64>;

    /// Returns the final URL (after redirects).
    pub fn url(&self) -> &str;

    /// Returns true if the status is in the 2xx range.
    pub fn is_success(&self) -> bool;

    /// Returns true if the status is in the 4xx or 5xx range.
    pub fn is_client_error(&self) -> bool;
    pub fn is_server_error(&self) -> bool;

    /// Returns an error if the status is 4xx or 5xx.
    /// Consumes the response on error, returns self on success.
    pub fn error_for_status(self) -> HttpResult<Self>;

    /// Returns an error if the status is 4xx or 5xx, without consuming.
    pub fn error_for_status_ref(&self) -> HttpResult<&Self>;
}
```

### error_for_status Implementation

```rust
impl Response {
    pub fn error_for_status(self) -> HttpResult<Self> {
        if self.is_success() {
            Ok(self)
        } else {
            let status = self.status;
            Err(HttpError::RequestFailed(
                alloc::format!("HTTP status {} for {}", status.as_u16(), self.url)
            ))
        }
    }
}
```

### Parsing from ABI Result MemoryId

The bridge module parses the response from the result MemoryId:

```rust
fn parse_response_from_memory(result_mem_id: MemoryId, abort: AbortGuard) -> HttpResult<Response> {
    // Read the result buffer from shared memory
    let alloc = internal_api::get_memory(result_mem_id);
    let data = alloc.buffer();

    // Parse using the existing FromBinary / ReturnValueParserIter infrastructure:
    // Expected layout:
    //   [Begin marker][status(u16)][headers_json(str)][body_mem_id(u64)]
    //   [url(str)][response_uid(u64)][End marker]

    let mut parser = ReturnValueParserIter::new(data);

    // Parse status
    let status_u16 = parser.next_u16()?;
    let status = StatusCode::from_u16(status_u16)
        .map_err(|_| HttpError::Decode(alloc::format!("Invalid status code: {}", status_u16)))?;

    // Parse headers JSON
    let headers_json = parser.next_string()?;
    let headers = parse_headers_json(&headers_json)?;

    // Parse body MemoryId
    let body_mem_id = parser.next_memory_id()?;
    let body = read_body_from_memory(body_mem_id)?;

    // Parse URL
    let url = parser.next_string()?;

    // Parse Response ExternalPointer UID (keep for streaming)
    let response_uid = parser.next_u64()?;
    let js_response = Some(ExternalPointer::from_u64(response_uid));

    Ok(Response {
        status,
        headers,
        body,
        url,
        js_response,
        _abort: abort,
    })
}
```

### Component Details

#### Header JSON Parsing

```rust
fn parse_headers_json(json: &str) -> HttpResult<HeaderMap> {
    // Parse JSON array of [name, value] pairs:
    // '[["content-type","text/html"],["content-length","1234"]]'
    let pairs: Vec<(String, String)> = serde_json::from_str(json)
        .map_err(|e| HttpError::Decode(alloc::format!("Invalid headers JSON: {}", e)))?;

    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| HttpError::Decode(alloc::format!("Invalid header name: {}", name)))?;
        let value = HeaderValue::from_str(&value)
            .map_err(|_| HttpError::Decode(alloc::format!("Invalid header value: {}", value)))?;
        headers.insert(name, value);
    }
    Ok(headers)
}
```

### Drop Behavior

When the Response is dropped:
1. The `AbortGuard` drops, which calls `abort()` on the AbortController — but since the response is already received, this is a no-op (the fetch completed).
2. The `js_response` ExternalPointer should be released — call the JS-side `releaseResponse()` via ABI to free the object heap reference.

```rust
impl Drop for Response {
    fn drop(&mut self) {
        // Release the JS Response object if we still hold a reference
        if let Some(uid) = self.js_response {
            host_runtime::web::release_response(uid);
        }
        // AbortGuard drops naturally (no-op after completion)
    }
}
```

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| Body pre-consumed (ArrayBuffer) | Simpler Rust side, one-shot transfer | Lazy consumption — requires streaming infrastructure |
| Response stored as ExternalPointer | Needed for streaming (feature 07) | Don't store it — can't support streaming later |
| Synchronous text/bytes/json methods | Data already in memory | Async methods — unnecessary overhead |
| `serde_json` for header parsing | Already available if json feature enabled | Manual JSON parsing — unnecessary complexity |

## Implementation

### Files to Create/Modify

- `backends/foundation_wasm/src/http/response.rs` — New file: Response struct and methods
- `backends/foundation_wasm/src/http/bridge.rs` — Add `parse_response_from_memory()` function

### Tasks

- [ ] T1: Create `Response` struct with all fields
- [ ] T2: Implement `status()`, `headers()`, `url()`, `content_length()`
- [ ] T3: Implement `is_success()`, `is_client_error()`, `is_server_error()`
- [ ] T4: Implement `error_for_status()` and `error_for_status_ref()`
- [ ] T5: Implement `text()` body consumption
- [ ] T6: Implement `bytes()` body consumption
- [ ] T7: Implement `json()` body consumption (feature-gated: `http-json`)
- [ ] T8: Implement `Drop` to release JS Response ExternalPointer
- [ ] T9: Implement `parse_response_from_memory()` in bridge.rs
- [ ] T10: Export from `http/mod.rs`

## Testing

### Test Cases

1. **Status access**: Response with status 200 returns correct StatusCode
2. **Headers access**: Response with Content-Type header returns correct value
3. **text()**: UTF-8 body correctly decoded to String
4. **text() error**: Invalid UTF-8 body returns Decode error
5. **bytes()**: Body returned as Bytes with correct length
6. **error_for_status**: 200 returns Ok(self), 404 returns Err
7. **error_for_status_ref**: Same without consuming
8. **URL tracking**: Response.url() returns final URL after redirects
9. **content_length**: Parses Content-Length header correctly

## Success Criteria

- [ ] All tasks completed
- [ ] Response correctly parses from ABI result MemoryId
- [ ] Body consumption methods work correctly
- [ ] error_for_status correctly identifies 4xx/5xx
- [ ] Drop correctly releases JS Response ExternalPointer
- [ ] No regressions on native target

---

_Created: 2026-05-18_
