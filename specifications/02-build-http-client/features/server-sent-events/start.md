# Server-Sent Events (SSE) Feature - Quick Start Guide

## Overview

Implementation guide for adding Server-Sent Events support to foundation_core. This feature provides both client-side (EventSource) and server-side (EventWriter) SSE capabilities.

**Status**: Pending
**Estimated Effort**: 2-3 weeks
**Infrastructure Completeness**: ~80% (most components already exist)

---

## Quick Reference

### Documentation Files

1. **[feature.md](./feature.md)** - Complete feature specification
   - Requirements and API examples
   - Implementation phases (3 phases)
   - Success criteria
   - Verification commands

2. **[ARCHITECTURE.md](./ARCHITECTURE.md)** - Technical architecture (10,000+ lines)
   - Existing infrastructure analysis
   - SSE protocol details (W3C spec)
   - Parser design with test vectors
   - Client/server implementation examples
   - Integration patterns

3. **This file (start.md)** - Quick start guide

---

## Before You Start

### Required Reading

1. **Read ARCHITECTURE.md sections 1-2** (30 minutes)
   - Executive Summary - understand infrastructure completeness
   - Existing Infrastructure Analysis - know what to reuse

2. **Read W3C SSE Specification** (1 hour)
   - [HTML Living Standard - Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html)
   - Focus on message format and parsing rules

3. **Explore existing code** (1 hour)
   ```bash
   # HTTP streaming pattern
   cat backends/foundation_core/src/wire/simple_http/impls.rs | grep -A 50 "HttpResponseReader"

   # Reconnection pattern
   cat backends/foundation_core/src/wire/http_stream/mod.rs | grep -A 100 "ReconnectingStream"

   # Retry mechanism
   cat backends/foundation_core/src/retries/exponential.rs
   ```

### Key Infrastructure to Understand

| Component | File | What It Does |
|-----------|------|--------------|
| `HttpResponseReader` | `wire/simple_http/impls.rs:3423` | Streams HTTP response chunks |
| `ReconnectingStream` | `wire/http_stream/mod.rs:36` | Auto-reconnection with backoff |
| `ExponentialBackoffDecider` | `retries/exponential.rs:6` | Smart backoff calculation |
| `SimpleIncomingRequestBuilder` | `wire/simple_http/impls.rs:1729` | HTTP request building |
| `TaskIterator` | `valtron/types.rs` | Non-blocking state machine |

---

## Implementation Phases

### Phase 1: Core SSE Protocol (1-2 weeks)

**Goal**: Working SSE client and server with blocking I/O

**Tasks**:

1. **Create types** (`event_source/core.rs`)
   ```rust
   pub enum Event {
       Message { id, event_type, data, retry },
       Comment(String),
       Reconnect,
   }

   pub struct SseEvent {
       id, event_type, data, retry
   }
   ```

2. **Implement parser** (`event_source/parser.rs`)
   - Parse SSE messages following W3C spec
   - Handle all field types: `event:`, `data:`, `id:`, `retry:`, `:`
   - Handle line endings: `\n`, `\r`, `\r\n`
   - See ARCHITECTURE.md section 5 for complete implementation

3. **Implement client** (`event_source/client.rs`)
   ```rust
   pub struct EventSource;
   pub struct EventSourceStream;

   impl Iterator for EventSourceStream {
       type Item = Result<Event, EventSourceError>;
       // Wraps HttpResponseReader + SseParser
   }
   ```

4. **Implement server writer** (`event_source/writer.rs`)
   ```rust
   pub struct EventWriter<W: Write>;

   impl EventWriter {
       pub fn send(&mut self, event: SseEvent);
       pub fn comment(&mut self, text: &str);
   }
   ```

5. **Add error types** (`event_source/error.rs`)
   ```rust
   pub enum EventSourceError {
       InvalidUrl, Http, InvalidStatus,
       InvalidContentType, InvalidUtf8,
       ConnectionClosed, IoError
   }
   ```

6. **Write tests**
   - Parser test vectors (from ARCHITECTURE.md section 5.3)
   - Client connection test
   - Writer formatting test

**Success Criteria**:
- [ ] Parser passes all W3C test vectors
- [ ] Can connect to SSE endpoint (blocking)
- [ ] Can receive and parse events
- [ ] Can send events (server-side)
- [ ] All unit tests pass
- [ ] `cargo fmt` and `cargo clippy` pass

**Files to Create**:
```
event_source/
├── core.rs           (~100 lines)
├── parser.rs         (~200 lines) - see ARCHITECTURE.md section 5.1
├── client.rs         (~150 lines) - see ARCHITECTURE.md section 6.1
├── writer.rs         (~100 lines) - see ARCHITECTURE.md section 7.1
├── error.rs          (~50 lines) - see ARCHITECTURE.md section 6.2
└── mod.rs            (~30 lines)
```

### Phase 2: Reconnection Support (3-5 days)

**Goal**: Automatic reconnection with backoff

**Tasks**:

1. **Create reconnecting client** (`event_source/reconnecting.rs`)
   ```rust
   pub struct ReconnectingEventSource {
       config: EventSourceConfig,
       reconnection_stream: ReconnectingStream, // REUSE existing!
       current_stream: Option<EventSourceStream>,
       parser: SseParser,
   }

   impl Iterator for ReconnectingEventSource {
       // Wraps ReconnectingStream + EventSourceStream
   }
   ```

2. **Add Last-Event-ID tracking**
   - Store last event ID from parser
   - Send `Last-Event-ID` header on reconnect

3. **Add server retry field handling**
   - Parse `retry:` field from server
   - Override backoff with server value

**Success Criteria**:
- [ ] Auto-reconnects on connection loss
- [ ] Sends `Last-Event-ID` header
- [ ] Respects server `retry:` field
- [ ] Exponential backoff works
- [ ] Max retries honored

**Files to Create**:
```
event_source/
└── reconnecting.rs   (~150 lines) - see ARCHITECTURE.md section 8.1
```

### Phase 3: TaskIterator Integration (3-5 days)

**Goal**: Non-blocking event consumption

**Tasks**:

1. **Create task wrapper** (`event_source/task.rs`)
   ```rust
   pub struct EventSourceTask {
       state: EventSourceState,
   }

   enum EventSourceState {
       Init, Connecting, Reading, Reconnecting, Closed
   }

   impl TaskIterator for EventSourceTask {
       type Ready = Event;
       type Pending = EventSourceProgress;
       // State machine transitions
   }
   ```

2. **Integrate with valtron executor**
   - Use existing executor infrastructure
   - Non-blocking state transitions

**Success Criteria**:
- [ ] `EventSourceTask` implements `TaskIterator`
- [ ] Non-blocking connect works
- [ ] Non-blocking event reading works
- [ ] Integrates with valtron executor

**Files to Create**:
```
event_source/
└── task.rs           (~200 lines) - see ARCHITECTURE.md section 9.1
```

---

## Code Patterns to Follow

### 1. Use Existing HTTP Infrastructure

**DO**:
```rust
// Build request with SimpleIncomingRequestBuilder
let request = SimpleIncomingRequestBuilder::get("/events")
    .header(SimpleHeader::ACCEPT, "text/event-stream")
    .header(SimpleHeader::CACHE_CONTROL, "no-cache")
    .build()?;

// Render with RenderHttp
let bytes = Http11::request(&request).http_render()?;

// Stream with HttpResponseReader
let reader = HttpResponseReader::new(connection.clone_stream(), SimpleHttpBody);
for part in reader {
    // Parse SSE events from body chunks
}
```

**DON'T**:
```rust
// ❌ Hand-write HTTP requests
write!(conn, "GET /events HTTP/1.1\r\n")?;
write!(conn, "Accept: text/event-stream\r\n\r\n")?;
```

### 2. Wrap ReconnectingStream

**DO**:
```rust
// Use existing reconnection infrastructure
pub struct ReconnectingEventSource {
    reconnection_stream: ReconnectingStream, // Existing!
    current_stream: Option<EventSourceStream>,
}

impl Iterator for ReconnectingEventSource {
    fn next(&mut self) -> Option<Result<Event>> {
        // Use reconnection_stream.next() for reconnection logic
    }
}
```

**DON'T**:
```rust
// ❌ Reimplement backoff logic
pub struct ReconnectingEventSource {
    attempt: u32,
    backoff_duration: Duration,
    // Manual backoff calculation...
}
```

### 3. Follow Parser Patterns

**DO** (from ARCHITECTURE.md section 5.1):
```rust
pub struct SseParser {
    line_buffer: String,
    current_id: Option<String>,
    current_event: Option<String>,
    current_data: Vec<String>,
    current_retry: Option<u64>,
    last_event_id: Option<String>,
}

impl SseParser {
    pub fn parse(&mut self, chunk: &str) -> Vec<Event> {
        // Parse line by line
        // Accumulate fields
        // Dispatch on empty line
    }
}
```

**DON'T**:
```rust
// ❌ Parse entire messages at once
pub fn parse_sse_message(message: &str) -> Event {
    // Assumes complete messages, doesn't handle streaming
}
```

---

## Testing Strategy

### Unit Tests

**Parser tests** (`event_source/parser.rs`):
```rust
#[test]
fn parse_simple_event() {
    let mut parser = SseParser::new();
    let events = parser.parse("data: hello\n\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data(), Some("hello"));
}

#[test]
fn parse_multiline_data() {
    let mut parser = SseParser::new();
    let events = parser.parse("data: line1\ndata: line2\n\n");
    assert_eq!(events[0].data(), Some("line1\nline2"));
}
```

See ARCHITECTURE.md section 5.3 for complete test vectors.

### Integration Tests

**Client test** (`tests/backends/foundation_core/integration/event_source_tests.rs`):
```rust
#[test]
#[ignore] // Network test
fn test_connect_to_sse_endpoint() {
    let event_source = EventSource::new("https://sse.dev/test")
        .connect()
        .expect("Failed to connect");

    let mut count = 0;
    for event in event_source {
        match event {
            Ok(Event::Message { data, .. }) => {
                println!("Received: {}", data);
                count += 1;
                if count >= 3 {
                    break;
                }
            }
            Err(e) => panic!("Error: {:?}", e),
            _ => {}
        }
    }

    assert!(count >= 3, "Should receive at least 3 events");
}
```

---

## Common Pitfalls

From ARCHITECTURE.md - Notes for Implementation Agents:

1. **Not handling multi-line data** - Each `data:` field is a separate line
2. **Not stripping space after `:`** - Spec says optional space is stripped
3. **Not joining data fields with `\n`** - Multiple `data:` fields join with newline
4. **Forgetting to flush** after each event (server-side)
5. **Not handling different line endings** - `\n`, `\r`, `\r\n`
6. **Not ignoring UTF-8 BOM** at start of stream
7. **Not resetting event state** after dispatch
8. **Not handling `id:` with null byte** - Should be ignored
9. **Not validating `retry:` value** - Must be valid integer
10. **Hand-writing HTTP requests** - Use `SimpleIncomingRequestBuilder`

---

## Verification Commands

```bash
# Format check
cargo fmt -- --check

# Clippy linting
cargo clippy --package foundation_core --features std -- -D warnings

# Unit tests
cargo test --package foundation_core -- event_source

# Integration tests (network required)
cargo test --package foundation_core -- event_source::integration --ignored
```

---

## Module Structure

Final structure:
```
backends/foundation_core/src/wire/event_source/
├── mod.rs              # Platform-specific exports
├── core.rs             # Event and SseEvent types
├── parser.rs           # SseParser implementation
├── client.rs           # EventSource and EventSourceStream
├── writer.rs           # EventWriter and SseEventBuilder
├── reconnecting.rs     # ReconnectingEventSource (Phase 2)
├── task.rs             # EventSourceTask (Phase 3)
├── error.rs            # EventSourceError
├── no_wasm.rs          # Native implementation exports
└── wasm.rs             # WASM implementation exports (future)
```

---

## Getting Help

**Detailed Information**:
- Architecture: See [ARCHITECTURE.md](./ARCHITECTURE.md)
- Requirements: See [feature.md](./feature.md)
- Examples: ARCHITECTURE.md sections 5, 6, 7, 8, 9

**Key Sections**:
- Parser implementation: ARCHITECTURE.md section 5
- Client implementation: ARCHITECTURE.md section 6
- Server implementation: ARCHITECTURE.md section 7
- Reconnection: ARCHITECTURE.md section 8
- TaskIterator: ARCHITECTURE.md section 9

**W3C Spec**:
- [Server-Sent Events Specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)

---

## Success Metrics

**Phase 1 Complete When**:
- ✅ Parser passes all test vectors
- ✅ Can connect and receive events (blocking)
- ✅ Can send events (server-side)
- ✅ All tests pass

**Phase 2 Complete When**:
- ✅ Auto-reconnection works
- ✅ Last-Event-ID tracking works
- ✅ Server retry field honored

**Phase 3 Complete When**:
- ✅ Non-blocking event consumption works
- ✅ Valtron executor integration works

**Feature Complete When**:
- ✅ All 3 phases complete
- ✅ Documentation complete
- ✅ All verification commands pass
- ✅ Integration tests pass

---

*Created: 2026-03-03*
*Quick Start - Begin with ARCHITECTURE.md sections 1-2, then implement Phase 1*
