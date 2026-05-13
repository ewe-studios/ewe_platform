---
feature: expect-100-continue
description: Add middleware to handle HTTP/1.1 Expect: 100-continue header for proper client upload flow
status: design
priority: high
depends_on: []
estimated_effort: small
created: 2026-05-13
last_updated: 2026-05-13
author: Claude Code
---

# Feature: Expect: 100-Continue Middleware

## Overview

Implement a middleware that properly handles the HTTP/1.1 `Expect: 100-continue` header. When a client sends this header, the server must respond with `100 Continue` before the client will send the request body. This is a standard HTTP/1.1 feature used by clients (like curl, HTTP libraries) to avoid sending large request bodies to servers that may reject them.

## Background

### What is Expect: 100-continue?

Per [RFC 7231 Section 5.1.1](https://tools.ietf.org/html/rfc7231#section-5.1.1):

1. Client sends request headers with `Expect: 100-continue` and `Content-Length: N`
2. Client waits for server response before sending body
3. Server responds with:
   - `100 Continue` → Client sends body, then server processes complete request
   - `417 Expectation Failed` → Client should not send body
   - Final error (4xx/5xx) → Client should not send body

### Current Behavior

The HTTP framework currently does not handle the `Expect: 100-continue` header. When a client sends this header:
- Server waits for body that never arrives
- Connection may timeout or client may retry
- Upload workflows fail

### Example Client Request Flow

```http
# Step 1: Client sends headers only
POST /upload HTTP/1.1
Host: example.com
Content-Type: application/json
Content-Length: 1000000
Expect: 100-continue

# Step 2: Server responds with 100 Continue
HTTP/1.1 100 Continue

# Step 3: Client sends body
{"large": "payload", ...}

# Step 4: Server processes request and responds
HTTP/1.1 200 OK
Content-Type: application/json

{"status": "uploaded"}
```

## Design

### Middleware Approach

A new middleware `ExpectContinueMiddleware` will:
1. Check for `Expect: 100-continue` header in incoming requests
2. Check for request body indicators (`Content-Length` or `Transfer-Encoding: chunked`)
3. If present, immediately send `100 Continue` response
4. Allow request processing to continue normally

### Flow

```
Client Request (headers only, Expect: 100-continue)
         ↓
Server receives request headers
         ↓
ExpectContinueMiddleware::handle()
         ↓
    Has Expect: 100-continue?
         ↓
    YES                          NO
     ↓                            ↓
Send 100 Continue              MiddlewareResult::Continue
response immediately               ↓
     ↓                        Normal processing
MiddlewareResult::Continue           ↓
     ↓                         Handler receives
Normal processing               complete request
     ↓
Handler receives body
```

## API Design

### ExpectContinueMiddleware

```rust
/// Middleware to handle HTTP/1.1 Expect: 100-continue header.
///
/// When a client sends `Expect: 100-continue`, this middleware immediately
/// responds with `100 Continue` status, signaling the client to send the
/// request body. The request then proceeds through the middleware chain
/// and to the handler normally.
///
/// WHY: Per RFC 7231, servers must respond to Expect: 100-continue with
/// either 100 Continue or 417 Expectation Failed. Without this handling,
/// clients wait indefinitely and uploads fail.
pub struct ExpectContinueMiddleware;

impl ExpectContinueMiddleware {
    /// Create a new ExpectContinueMiddleware.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl RequestMiddleware for ExpectContinueMiddleware {
    fn handle(
        &self,
        ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        // Check for Expect: 100-continue header
        // If present, send 100 Continue response and continue
        // Otherwise, continue normally
    }
}
```

### Optional: Conditional Handler

```rust
/// Configuration for ExpectContinueMiddleware.
pub struct ExpectContinueConfig {
    /// Maximum body size to accept (optional validation)
    pub max_body_size: Option<usize>,
    /// Custom check function for additional validation
    pub validator: Option<Arc<dyn Fn(&SimpleIncomingRequest) -> bool + Send + Sync>>,
}

impl ExpectContinueConfig {
    /// Create config with optional max body size.
    /// Requests with Content-Length larger than this will receive
    /// 417 Expectation Failed instead of 100 Continue.
    pub fn with_max_body_size(max: usize) -> Self { ... }
}
```

## Implementation Details

### Header Detection

```rust
/// Check if request has Expect: 100-continue header.
fn has_expect_continue(req: &SimpleIncomingRequest) -> bool {
    req.headers
        .get(&SimpleHeader::EXPECT)
        .map(|values| {
            values.iter().any(|v| {
                v.trim().eq_ignore_ascii_case("100-continue")
            })
        })
        .unwrap_or(false)
}

/// Check if request appears to have a body.
fn has_request_body(req: &SimpleIncomingRequest) -> bool {
    // Has Content-Length > 0
    if let Some(values) = req.headers.get(&SimpleHeader::CONTENT_LENGTH) {
        if let Some(len) = values.first().and_then(|v| v.parse::<usize>().ok()) {
            if len > 0 {
                return true;
            }
        }
    }

    // Or Transfer-Encoding: chunked
    if let Some(values) = req.headers.get(&SimpleHeader::TRANSFER_ENCODING) {
        if values.iter().any(|v| v.contains("chunked")) {
            return true;
        }
    }

    false
}
```

### Sending 100 Continue

```rust
/// Build and send 100 Continue response.
fn send_100_continue(conn: &mut SharedByteBufferStream<RawStream>) -> Result<(), ...> {
    let response = SimpleOutgoingResponse::builder()
        .with_status(Status::Continue)
        .build()
        .expect("valid 100 response");

    Http11::response(response)
        .http_render_to_writer(conn)
        .map_err(...)?;

    conn.flush().map_err(...)?;
    Ok(())
}
```

### Middleware Implementation

```rust
impl RequestMiddleware for ExpectContinueMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        // Only handle if Expect: 100-continue is present
        if !has_expect_continue(req) {
            return MiddlewareResult::Continue;
        }

        // Only send 100 Continue if there's actually a body expected
        if !has_request_body(req) {
            // No body expected - client is misbehaving
            // Still continue, handler will deal with it
            return MiddlewareResult::Continue;
        }

        // Note: At this point in the middleware chain, we don't have
        // direct access to the connection stream. The middleware
        // returns Continue, and the server must send 100 Continue
        // before reading the body.
        //
        // ALTERNATIVE: Store flag in request extensions for server to handle
        req.extensions.insert("expect_100_continue_handled", true);

        MiddlewareResult::Continue
    }
}
```

## Alternative: Server-Level Handling

Since middleware doesn't have access to the connection stream, we may need server-level handling in `ConnectionHandler`:

### ConnectionHandler Modification

```rust
// In handle_processing(), before running middleware:

// Check for Expect: 100-continue and send immediate response
if should_handle_expect_continue(&req) {
    // Send 100 Continue response
    let continue_resp = SimpleOutgoingResponse::builder()
        .with_status(Status::Continue)
        .build()
        .expect("valid 100 response");

    let _ = Http11::response(continue_resp)
        .http_render_to_writer(&mut self.conn.clone());

    tracing::trace!("Sent 100 Continue response");
}
```

## Files to Modify

### Option A: Middleware Only (Preferred)

**New File:**
- `backends/foundation_http/src/middleware/expect_continue.rs`
  - New middleware implementation

**Modified Files:**
- `backends/foundation_http/src/middleware/mod.rs`
  - Add `pub use expect_continue::ExpectContinueMiddleware;`
  - Add `mod expect_continue;`

### Option B: Server-Level + Middleware

**New File:**
- `backends/foundation_http/src/middleware/expect_continue.rs`
  - Middleware that marks request with extension flag

**Modified Files:**
- `backends/foundation_http/src/middleware/mod.rs`
  - Export new middleware
- `backends/foundation_http/src/server/connection.rs`
  - Add check and 100 Continue response in `handle_processing()`
  - Before middleware chain runs, check for flag and send response

## Usage Example

```rust
use foundation_http::{HttpApp, middleware::ExpectContinueMiddleware};

fn main() {
    let mut app = HttpApp::new();

    // Add Expect-Continue middleware early in chain
    app.middleware(ExpectContinueMiddleware::new());

    // Add other middleware...
    app.middleware(LoggerMiddleware::new());
    app.middleware(BodyLimitMiddleware::new(10 * 1024 * 1024));

    // Routes...
    app.route::<UploadHandler>(SimpleMethod::POST, "/upload");

    app.server("0.0.0.0:8080").serve(shutdown);
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expect_continue_detected() {
        let req = request_with_header(SimpleHeader::EXPECT, "100-continue");
        assert!(has_expect_continue(&req));
    }

    #[test]
    fn test_expect_continue_case_insensitive() {
        let req = request_with_header(SimpleHeader::EXPECT, "100-Continue");
        assert!(has_expect_continue(&req));
    }

    #[test]
    fn test_expect_continue_not_present() {
        let req = SimpleIncomingRequest::default();
        assert!(!has_expect_continue(&req));
    }

    #[test]
    fn test_has_request_body_content_length() {
        let req = request_with_content_length(100);
        assert!(has_request_body(&req));
    }

    #[test]
    fn test_has_request_body_chunked() {
        let req = request_with_transfer_encoding("chunked");
        assert!(has_request_body(&req));
    }
}
```

### Integration Tests

```rust
#[test]
fn test_upload_with_expect_continue() {
    // Start server with middleware
    let server = test_server_with_expect_continue();

    // Simulate curl: curl -X POST -H "Expect: 100-continue" -d "data" ...
    let response = client
        .post("/upload")
        .header("Expect", "100-continue")
        .header("Content-Length", "1000")
        .body("test data")
        .send();

    // Should receive 100 Continue followed by 200 OK
    assert_eq!(response.status(), 200);
}
```

## Compatibility

### HTTP/1.1 Only

The `Expect: 100-continue` header is HTTP/1.1 specific. For HTTP/2, this mechanism doesn't exist (streams solve the same problem differently).

### Client Behavior

Most HTTP clients handle this automatically:
- **curl**: Sends `Expect: 100-continue` for bodies > 1024 bytes by default
- **reqwest**: Handles 100 Continue responses transparently
- **Python urllib3**: Handles automatically
- **Go net/http**: Sends Expect header, handles 100 continue

### Server-Side Opt-Out

Servers can disable by not including the middleware, or clients can send `Expect:` (empty) to disable.

## Success Criteria

- [ ] New `ExpectContinueMiddleware` implemented
- [ ] Middleware detects `Expect: 100-continue` header correctly
- [ ] Server sends `100 Continue` response before body read
- [ ] Upload handlers receive complete request body
- [ ] curl uploads with `-d @largefile` work correctly
- [ ] All existing tests pass
- [ ] New tests for expect-continue flow

## Related

- RFC 7231 Section 5.1.1: Expect Header
- `backends/foundation_http/src/middleware/mod.rs`
- `backends/foundation_http/src/server/connection.rs`
- `backends/foundation_core/src/wire/simple_http/impls.rs` (Status::Continue)
- Feature 04: Middleware Polish (may overlap)

## Notes

### Why Not Just Middleware?

The challenge is that middleware runs after the request headers are parsed but before the body is read. However:
1. The middleware doesn't have direct access to the connection stream
2. The `100 Continue` response must be sent before body read begins

**Solution**: Either:
1. Add server-level handling in `ConnectionHandler::handle_processing()`
2. Or middleware sets a flag that the server checks before reading body

Option 2 (flag-based) is cleaner as it keeps the logic in middleware while the server just handles the response sending.
