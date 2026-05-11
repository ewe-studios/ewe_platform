# Learnings: SSE HTTP Response Fix

This file captures design decisions, patterns discovered, and mistakes to avoid during the implementation of the SSE HTTP response fix.

## Design Decisions

### Decision 1: Add `SendSafeBody::SseStream` variant

**Why:** SSE is fundamentally an HTTP feature - it's an HTTP response with `Content-Type: text/event-stream`. The current implementation treats SSE parsing as separate from HTTP response handling, which causes the bug.

**Implementation:** Add a new variant to `SendSafeBody` enum that wraps an SSE stream iterator. This allows `HttpResponseReader` to return SSE streams as first-class body types.

### Decision 2: Use `HttpResponseReader` for Response Parsing

**Why:** `HttpResponseReader` already correctly parses HTTP responses including status line, headers, and body. Reusing it ensures consistent HTTP protocol handling.

**Trade-off:** Requires refactoring `EventSourceTask` to use `HttpResponseReader` instead of directly creating `SseParser`.

### Decision 3: Keep `SseParser` API Backward Compatible

**Why:** Existing code may use `SseParser` directly. Breaking changes would require widespread refactoring.

**Implementation:** Move `SseParser` to `simple_http/sse.rs` and re-export from `event_source/parser.rs` as a type alias or deprecated wrapper.

## Patterns Discovered

### Pattern 1: Body Extraction Flow

The HTTP response parsing follows a clear pattern:
1. Read intro line (status code)
2. Read headers
3. Determine body type from headers (`Content-Type`, `Transfer-Encoding`, `Content-Length`)
4. Extract body using appropriate extractor
5. Return body as `SendSafeBody` variant

SSE fits this pattern as a body type determined by `Content-Type: text/event-stream`.

### Pattern 2: Stream Positioning

`HttpResponseReader` uses `SharedByteBufferStream` which maintains internal buffer state. After reading headers, the stream is positioned at the body, ready for SSE parsing.

Key insight: We need to pass the `SharedByteBufferStream` (not the raw `RawStream`) to `SseParser` to preserve buffer state.

## Mistakes to Avoid

### Mistake 1: Don't Parse HTTP Headers as SSE Events

The current bug - don't create `SseParser` until after HTTP response headers have been parsed.

**Correct approach:**
```rust
// 1. Send request
// 2. Create HttpResponseReader
// 3. Parse response (intro, headers)
// 4. Verify status/content-type
// 5. Extract body stream
// 6. Create SseParser from body stream
```

### Mistake 2: Don't Lose Buffer State

`SharedByteBufferStream` maintains read-ahead buffer. Creating `SseParser` from raw stream would lose buffered data.

**Correct approach:** Pass `SharedByteBufferStream<RawStream>` to `SseParser`, not `RawStream`.

### Mistake 3: Don't Forget Content-Type Verification

SSE endpoints should return `Content-Type: text/event-stream`. Verify this before SSE parsing.

**Correct approach:** Check `Content-Type` header matches expected value, return error if mismatch.

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

## Performance Considerations

- `HttpResponseReader` adds minimal overhead (already in use elsewhere)
- `SendSafeBody::SseStream` variant adds one enum variant (minimal memory impact)
- Stream cloning is already happening, no additional overhead

## Security Considerations

- Verify `Content-Type` prevents content sniffing attacks
- Status code verification prevents parsing error responses as SSE
- Header size limits prevent DoS via large headers

---

_Last Updated: 2026-05-11_
