# Learnings: SSE HTTP Response Fix

This file captures design decisions, patterns discovered, and mistakes to avoid during the implementation of the SSE HTTP response fix.

## Design Decisions

### Decision 1: Add `SendSafeBody::SseStream` variant

**Why:** SSE is fundamentally an HTTP feature - it's an HTTP response with `Content-Type: text/event-stream`. The current implementation treats SSE parsing as separate from HTTP response handling, which causes the bug.

**Implementation:** Added a new variant to `SendSafeBody` enum that wraps an iterator yielding `ParseResult` items. This allows `HttpResponseReader` to return SSE streams as first-class body types.

**Result:** Successfully allows proper HTTP response parsing before SSE event parsing.

### Decision 2: Use `HttpResponseReader` for Response Parsing

**Why:** `HttpResponseReader` already correctly parses HTTP responses including status line, headers, and body. Reusing it ensures consistent HTTP protocol handling.

**Trade-off:** Required refactoring `EventSourceTask` to use `HttpResponseReader` instead of directly creating `SseParser`.

**Result:** Proper separation of concerns - HTTP layer handles HTTP, SSE layer handles SSE events.

### Decision 3: Add `AwaitingHeaders` State

**Why:** Need a state that uses `HttpResponseReader` to parse HTTP response before transitioning to SSE parsing.

**Implementation:** New state in `EventSourceState` enum that holds the connection and response reader.

**Result:** Clean state machine transition: Init → Connecting → AwaitingHeaders → ReadingStream → Closed.

### Decision 4: Add `ReadingStream` State

**Why:** After HTTP headers are parsed, we have an iterator from `SseStream` body, not an `SseParser`. Need a state to hold this iterator.

**Implementation:** New state that holds the `Box<dyn Iterator>` from `SseStream` body.

**Result:** Proper handling of SSE body iterator with proper error propagation.

### Decision 5: Move `SseParser` to `simple_http/sse.rs`

**Why:** SSE is an HTTP protocol extension. Keeping it in `event_source` module was conceptually incorrect.

**Implementation:** Moved `SseParser`, `EventBuilder`, and related types to new `simple_http/sse.rs` file.

**Result:** Better code organization and clearer abstraction boundaries.

## Patterns Discovered

### Pattern 1: HTTP Response Body Extraction Flow

The HTTP response parsing follows a clear pattern:
1. Read intro line (status code)
2. Read headers
3. Determine body type from headers (`Content-Type`, `Transfer-Encoding`, `Content-Length`)
4. Extract body using appropriate extractor
5. Return body as `SendSafeBody` variant

SSE fits this pattern as a body type determined by `Content-Type: text/event-stream`.

### Pattern 2: State Machine for Protocol Layering

When layering protocols (HTTP → SSE), use explicit states:
- `AwaitingHeaders`: HTTP response parsing
- `ReadingStream`: SSE event parsing

This ensures clean separation and proper error handling at each layer.

### Pattern 3: Iterator-Based Body Streaming

For streaming bodies (chunked, SSE), use iterator pattern:
- Body extractor creates iterator from stream
- Iterator yields items as data arrives
- Caller consumes iterator without blocking

This works well with `TaskIterator` pattern for async-like behavior.

### Pattern 4: Test Server Connection Handling

**Critical learning:** When a response includes `Connection: close`, the server must actually close the connection. `TestHttpServer` was not doing this, causing client to hang.

**Fix:** Check for `Connection: close` header in response and break out of request loop.

## Mistakes to Avoid

### Mistake 1: Don't Parse HTTP Headers as SSE Events

**What went wrong:** Original code created `SseParser` immediately after sending request, causing HTTP headers to be parsed as SSE events.

**Correct approach:**
```rust
// 1. Send request
// 2. Create HttpResponseReader
// 3. Parse response (intro, headers)
// 4. Verify status/content-type
// 5. Extract body iterator
// 6. Use iterator for SSE events
```

### Mistake 2: Don't Lose Stream Position

**What went wrong:** Initially tried to clone stream from reader after body extraction, but stream was moved into iterator.

**Correct approach:** Pass iterator through `SseStream` variant and use it directly.

### Mistake 3: Test Server Must Respect Connection Headers

**What went wrong:** `TestHttpServer` didn't close connections when `Connection: close` was in response, causing tests to hang.

**Correct approach:** Server must check for `Connection: close` and actually close the connection.

### Mistake 4: Iterator Type Compatibility

**What went wrong:** Initially tried to use `SseParser` directly in `ReadingStream` state, but `SseStream` body returns `Box<dyn Iterator<Item=Result<ParseResult, BoxedError>>>`.

**Correct approach:** Added `ReadingStream` state that holds the boxed iterator directly.

## Testing Patterns

### Integration Test Pattern

Integration tests should verify:
1. HTTP request sent correctly
2. HTTP response parsed (status, headers)
3. Content-Type verified
4. SSE events parsed correctly
5. HTTP headers NOT parsed as SSE events

Example test structure:
```rust
#[test]
fn test_sse_response_parsing() {
    // 1. Create mock server returning SSE response
    // 2. Create EventSourceTask
    // 3. Poll for events
    // 4. Verify HTTP headers NOT in events
    // 5. Verify actual SSE events received
}
```

### Test Server Patterns

**For SSE testing:**
- Use `SseTestServer` for streaming SSE responses
- Use `TestHttpServer` for request-response testing with proper `Connection: close` handling

**Key point:** Server must close connection when `Connection: close` is in response, or client will hang.

## Performance Considerations

- **Zero Copy:** `SseParser` uses `SharedByteBufferStream` for efficient buffering
- **No Extra Allocations:** Reuse existing buffer from `HttpResponseReader`
- **Streaming:** Events parsed as they arrive, no buffering of entire response
- **Iterator Overhead:** Minimal overhead from boxed iterator in `ReadingStream`

## Security Considerations

- **Content-Type Verification:** Prevents content sniffing attacks by verifying server returns expected MIME type
- **Status Code Verification:** Ensures successful response before SSE parsing
- **Header Size Limits:** Uses `HttpResponseReader` limits to prevent DoS

## Code Organization Lessons

1. **Protocol Extensions Belong in Base Protocol Module:** SSE is HTTP → `simple_http/sse.rs`, not `event_source`.

2. **State Machines Should Reflect Protocol Layers:** Each protocol layer gets its own state(s).

3. **Test Infrastructure Must Match Production:** Test servers must implement proper HTTP semantics.

---

_Last Updated: 2026-05-12_
