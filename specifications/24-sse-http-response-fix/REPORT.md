# REPORT: SSE HTTP Response Fix

## Summary

Fixed the SSE (Server-Sent Events) HTTP response handling to properly parse HTTP response headers before SSE events. The issue was that `EventSourceTask` was bypassing HTTP response parsing and wrapping the raw stream in an `SseParser`, causing HTTP response headers to be incorrectly parsed as SSE events.

## Status

**Status:** Complete

**Completion:** 100%

## Implementation Details

### Problem

The `EventSourceTask` in `wire/event_source/task.rs` incorrectly bypassed HTTP response parsing. After sending an HTTP request, it immediately wrapped the stream in an `SseParser`, causing HTTP response headers (`HTTP/1.1 200 OK`, `Content-Type: text/event-stream`, etc.) to be incorrectly parsed as SSE events.

**Example of the bug:**
```
Read total bytes 17 from reader: "HTTP/1.1 200 OK\r\n"
Read total bytes 33 from reader: "Content-Type: text/event-stream\r\n"
Read total bytes 19 from reader: "Connection: close\r\n"
Read total bytes 2 from reader: "\r\n"
Read total bytes 21 from reader: "data: {\"test\": true}\n"
```

These HTTP headers were being parsed as SSE field lines, which is incorrect.

### Solution

Added proper HTTP response handling with a new state machine state:

1. **New `AwaitingHeaders` state**: After sending the HTTP request, the task enters this state which uses `HttpResponseReader` to parse the HTTP response headers.

2. **HTTP Response Validation**: Before creating the `SseParser`, the code now:
   - Reads HTTP response status (verifies 200 OK)
   - Reads HTTP response headers (verifies `Content-Type: text/event-stream`)
   - Extracts the SSE body as a stream positioned after headers

3. **New `ReadingStream` state**: After HTTP headers are validated, the task transitions to this state which uses the SSE iterator extracted from the response body.

4. **Added `SendSafeBody::SseStream` variant**: A new body type for SSE streams that wraps an iterator yielding `ParseResult` items.

5. **Added `Body::SseBody` variant**: Internal state for HTTP response reader to track SSE body type.

6. **Moved `SseParser` to `simple_http/sse.rs`**: SSE is an HTTP protocol extension, so the parser belongs in the HTTP module.

### Files Modified

| File | Changes |
|------|---------|
| `backends/foundation_core/src/wire/simple_http/sse.rs` (NEW) | Moved `SseParser` from `event_source/parser.rs` with 13 unit tests |
| `backends/foundation_core/src/wire/simple_http/mod.rs` | Added `pub mod sse;` export |
| `backends/foundation_core/src/wire/simple_http/impls.rs` | Added `SendSafeBody::SseStream`, `Body::SseBody`, `SimpleSseIterator`, updated body extraction |
| `backends/foundation_core/src/wire/simple_http/client/body_reader.rs` | Added handling for `SseStream` variant |
| `backends/foundation_core/src/wire/event_source/parser.rs` | Re-export from new location for backward compatibility |
| `backends/foundation_core/src/wire/event_source/task.rs` | Added `AwaitingHeaders` and `ReadingStream` states with HTTP response parsing |
| `backends/foundation_testing/src/http/server.rs` | Fixed `TestHttpServer` to close connection when `Connection: close` header is present |

### Test Results

**Unit Tests:**
- `wire::simple_http::sse`: 13 tests passing
- `event_source`: 35 tests passing

**Integration Tests:**
- `test_reconnecting_task_initial_connection_sends_post_with_body`: **PASS**
- `test_reconnecting_task_reconnects_with_headers_but_not_body`: **PASS**

## Challenges and Solutions

### Challenge 1: Stream Position After HTTP Parsing

**Problem:** After `HttpResponseReader` parses the HTTP response headers, the stream position needs to be at the body for `SseParser` to read SSE events correctly.

**Solution:** The `SseBody` body type in `HttpResponseReader` returns a `SimpleSseIterator` that wraps the stream. The iterator is passed through `SendSafeBody::SseStream` and extracted in `task.rs`, ensuring the stream is properly positioned.

### Challenge 2: `Connection: close` Handling

**Problem:** The `TestHttpServer` wasn't closing connections when `Connection: close` header was present, causing the client to hang waiting for more data.

**Solution:** Updated `TestHttpServer::handle_connection` to check for `Connection: close` header and break out of the request loop, effectively closing the connection.

### Challenge 3: Backward Compatibility

**Problem:** Existing code might use `SseParser` directly from `event_source::parser`.

**Solution:** Added re-export in `event_source/parser.rs` so existing imports continue to work.

## Lessons Learned

1. **HTTP Protocol Layering is Critical:** SSE is an HTTP protocol extension. The HTTP response must be fully parsed (status, headers) before SSE event parsing begins.

2. **State Machine Design:** Adding an intermediate `AwaitingHeaders` state allows proper separation between HTTP response parsing and SSE event parsing.

3. **Test Server Behavior:** Test servers must properly implement HTTP semantics like `Connection: close` for realistic testing.

4. **Stream Ownership:** When using iterators that wrap streams, careful attention must be paid to stream ownership and positioning.

## Verification Commands

```bash
# SSE unit tests
cargo test --package foundation_core --lib -- simple_http::sse

# Event source tests  
cargo test --package foundation_core --features multi --profile uat -- event_source::

# Integration tests
cargo test --package foundation_core --features multi --profile uat -- reconnecting_integration_tests
```

## References

- [Feature Specification](./features/00-sse-body-integration/feature.md)
- [Learnings](./LEARNINGS.md)
- [W3C Server-Sent Events Specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)

---

_Created: 2026-05-11_
_Last Updated: 2026-05-12_
