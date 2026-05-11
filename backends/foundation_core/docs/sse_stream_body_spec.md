# SSE Stream Body Integration Spec

## Problem Statement

The current SSE client in `event_source/task.rs` incorrectly bypasses HTTP response parsing. After sending the HTTP request, it immediately wraps the stream in an `SseParser`, causing HTTP response headers to be parsed as SSE events.

### Current Flow (Broken)
```
1. Send HTTP request
2. Clone stream
3. Create SseParser(stream)  <-- WRONG: HTTP headers still in stream
4. SseParser tries to parse "HTTP/1.1 200 OK" as SSE event
```

### Expected Flow
```
1. Send HTTP request
2. Use HttpResponseReader to parse response
3. Verify status code (200 OK)
4. Verify Content-Type (text/event-stream)
5. Extract stream positioned at body
6. Create SseParser(body_stream)
7. Parse SSE events
```

## Solution Design

### 1. Move SseParser to simple_http

Move `SseParser` from `wire/event_source/parser.rs` to `wire/simple_http/sse.rs` and re-export from `event_source` for backward compatibility.

### 2. Add SendSafeBody::SseStream Variant

Add a new variant to `SendSafeBody` that represents an SSE stream:

```rust
pub enum SendSafeBody {
    // ... existing variants ...
    /// SSE event stream body.
    ///
    /// WHY: SSE streams need special parsing per W3C spec.
    /// WHAT: Wraps an SseParser that yields SSE events from the stream.
    SseStream(Option<BoxedSendableIterator<SseEvent, BoxedError>>),
}
```

### 3. Update HttpResponseReader

When `HttpResponseReader` encounters `Content-Type: text/event-stream`:

1. Return headers as `IncomingResponseParts::Headers(headers)`
2. Set state to `HttpReadState::Body(Body::SseBody(headers))`
3. On next iteration, extract body using `SseBodyExtractor` which:
   - Returns the stream positioned after headers
   - Wraps it in an SseParser
   - Returns `SendSafeBody::SseStream(iterator)`

### 4. Update Body Enum

Add `SseBody` variant to the internal `Body` enum:

```rust
enum Body {
    // ... existing variants ...
    SseBody(SimpleHeaders),
}
```

### 5. Update EventSourceTask

Modify `task.rs` to:

1. Use `HttpResponseReader` after sending request
2. Read response parts until headers received
3. Verify status is 200 OK
4. Verify Content-Type is text/event-stream
5. Extract the SseStream body or get the stream for manual SSE parsing
6. Continue with existing event reading logic

### 6. Add SseEvent Type

Create a dedicated `SseEvent` type for SSE events:

```rust
pub struct SseEvent {
    pub id: Option<String>,
    pub event_type: Option<String>,
    pub data: String,
    pub retry: Option<u64>,
}
```

## Files to Modify

1. `wire/simple_http/impls.rs`
   - Add `SendSafeBody::SseStream` variant
   - Update `From<SendSafeBody> for IncomingResponseParts`
   - Add `Body::SseBody` variant
   - Update `HttpResponseReader` to handle SSE bodies

2. `wire/simple_http/sse.rs` (new file)
   - Move `SseParser` from `event_source/parser.rs`
   - Add `SseEvent` type
   - Add `SseBodyExtractor`

3. `wire/simple_http/mod.rs`
   - Add `pub mod sse;`
   - Re-export SSE types

4. `wire/event_source/parser.rs`
   - Re-export from new location or deprecate

5. `wire/event_source/task.rs`
   - Update to use `HttpResponseReader` first
   - Parse HTTP response before creating SseParser

6. `wire/simple_http/client/body_reader.rs`
   - Add handling for `SendSafeBody::SseStream`

## Implementation Notes

### Backward Compatibility

- Keep `event_source::SseParser` as a re-export or type alias
- Existing code using `SseParser` directly should still work
- The fix in `task.rs` is internal - public API unchanged

### Stream Positioning

The key challenge is ensuring the stream is positioned correctly after headers. `HttpResponseReader` already handles this - after yielding `IncomingResponseParts::Headers`, the underlying `SharedByteBufferStream` is positioned at the body.

### Content-Type Detection

`HttpResponseReader` already detects `text/event-stream` (lines 3817-3832 in impls.rs). This detection should be extended to set up `SseBody` state.

## Testing

1. Unit tests for `SseBody` extraction
2. Integration test verifying HTTP headers are not parsed as SSE events
3. Test with various SSE response formats
4. Test error handling for non-200 responses
5. Test error handling for wrong Content-Type
