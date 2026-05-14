---
feature: expect-100-continue
description: Server-level handling of HTTP/1.1 Expect: 100-continue header + MiddlewareResult::InterimResponse for non-short-circuiting responses
status: completed
priority: high
depends_on: ["12-body-reader-streaming"]
estimated_effort: small
created: 2026-05-13
last_updated: 2026-05-14
author: Claude Code
---

# Feature: Expect: 100-Continue Handling

## Overview

When a client sends `Expect: 100-continue`, the server responds with `HTTP/1.1 100 Continue` before reading the request body. This is handled at two levels:

1. **Server-level fallback** in `ConnectionHandler::handle_processing()` — detects the header and writes 100 Continue directly to the stream before middleware runs. This is the primary path because 100 Continue is an interim response, not a final one.
2. **`MiddlewareResult::InterimResponse`** — a new middleware result variant that renders a response immediately but **does not short-circuit** the request. The middleware chain and handler continue to run after the interim response is sent.

## Why Streaming Bodies Make This Simpler

The original design assumed the body was read eagerly during request parsing, requiring middleware to have connection-stream access to send 100 Continue. With the streaming body architecture (Feature 12), the body is a lazy `SendSafeBody::Stream` that is **not consumed** until the handler pulls it. This means:

1. `read_next_request` parses headers only → body stored as `SendSafeBody::Stream` (lazy, not pulled)
2. `handle_processing` runs — middleware, routing, handler — all before body is consumed
3. We can detect `Expect: 100-continue` in `req.headers` at the top of `handle_processing` and send `100 Continue` directly to `self.conn`
4. The client receives 100 Continue and starts sending the body into the TCP buffer
5. When the handler eventually calls `try_collect_bytes` / `collect_bytes` etc., the body stream pulls data that is now buffered

## Design

### Server-Level Detection (Primary Path)

In `ConnectionHandler::handle_processing()`, **before** the middleware chain runs:

```rust
// Check for Expect: 100-continue and send response if body expected
if Self::has_expect_continue(&req) && Self::has_request_body(&req) {
    let _ = respond::continue_100(&mut self.conn.clone());
}
```

### MiddlewareResult::InterimResponse (General Pattern)

New middleware result variant that renders a response immediately but continues processing:

```rust
pub enum MiddlewareResult {
    /// Continue to next middleware / handler.
    Continue,
    /// Short-circuit: render this response and skip the handler.
    Response(SimpleOutgoingResponse),
    /// Interim response: render immediately but DO NOT short-circuit.
    /// The middleware chain continues, and the handler still runs.
    /// Useful for 100-Continue, progress trailers, etc.
    InterimResponse(SimpleOutgoingResponse),
}
```

Handler in `handle_processing()`:

```rust
match mw.handle(&bag, &mut req) {
    MiddlewareResult::Continue => {}
    MiddlewareResult::Response(resp) => {
        middleware_response = Some(resp);
        break; // short-circuit
    }
    MiddlewareResult::InterimResponse(resp) => {
        let _ = Http11::response(resp).http_render_to_writer(&mut self.conn.clone());
        // Continue middleware chain — no break
    }
}
```

### Response Helper

In `serve/respond.rs`:

```rust
/// Write a 100 Continue interim response.
pub fn continue_100(conn: &mut impl std::io::Write) -> Result<(), ErrorTrace<ServeError>> { ... }
```

### Detection Functions

```rust
fn has_expect_continue(req: &SimpleIncomingRequest) -> bool { ... }
fn has_request_body(req: &SimpleIncomingRequest) -> bool { ... }
```

### Flow

```
Client sends headers (Expect: 100-continue)
         ↓
read_next_request → headers parsed, body = lazy Stream (NOT pulled)
         ↓
handle_processing() — detect Expect header
         ↓
send HTTP/1.1 100 Continue\r\n\r\n to conn
         ↓
Middleware chain → (any InterimResponse renders but continues) → Router dispatch → Serve::serve
         ↓
Handler pulls body stream (data is now buffered)
         ↓
Complete response
```

## Implementation Details

### Files Modified

| File | Changes |
|------|---------|
| `backends/foundation_http/src/server/connection.rs` | Add expect-continue detection before middleware; handle `InterimResponse` in middleware loop |
| `backends/foundation_http/src/serve/mod.rs` | Add `continue_100()` helper; add `InterimResponse` to `MiddlewareResult` |
| `backends/foundation_http/src/middleware/mod.rs` | Export updated `MiddlewareResult` |

### Edge Cases

1. **Expect header without body**: No 100 Continue sent — client is misbehaving, but we don't need to do anything special
2. **Expect header on GET/HEAD**: No body expected — skip, no 100 Continue needed
3. **100 Continue render failure**: Log trace, proceed normally — the handler will still try to read the body and may timeout
4. **Non-HTTP/1.1 clients**: The Expect header is HTTP/1.1 only, but if an HTTP/1.0 client sends it, we still handle it safely (rendering 100 Continue is harmless)

## Success Criteria

- [x] `Expect: 100-continue` header detected in parsed request headers
- [x] `100 Continue` response sent before middleware/handler runs
- [x] `MiddlewareResult::InterimResponse` variant added — renders but continues
- [x] Middleware loop handles `InterimResponse` correctly (no short-circuit)
- [x] Body stream receives client data after 100 Continue
- [x] curl uploads with `-d @largefile` work correctly
- [x] Requests without Expect header are unaffected
- [x] All existing tests pass

## Related

- RFC 7231 Section 5.1.1: Expect Header
- Feature 12: Body Reader Streaming (lazy body enables this design)
- `SimpleHeader::EXPECT` in simple_http
- `Status::Continue` (100) in simple_http
