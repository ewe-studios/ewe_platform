---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/server-sent-events"
this_file: "specifications/02-build-http-client/features/server-sent-events/feature.md"

status: pending
priority: medium
created: 2026-03-03
updated: 2026-03-03

depends_on:
  - connection
  - request-response
  - task-iterator

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0
---

# Server-Sent Events (SSE) Feature

## Overview

Implement complete Server-Sent Events (SSE / EventSource) support following the [W3C Server-Sent Events specification](https://html.spec.whatwg.org/multipage/server-sent-events.html). This feature provides both **client-side** (consuming SSE streams) and **server-side** (producing SSE streams) capabilities, leveraging existing `simple_http`, `http_stream`, and `valtron` infrastructure.

**Key Capabilities**:
- ✅ Client: Connect to SSE endpoints and consume event streams
- ✅ Server: Send SSE events to connected clients
- ✅ Automatic reconnection with Last-Event-ID tracking
- ✅ Event filtering by event type
- ✅ Non-blocking operation via TaskIterator pattern
- ✅ TLS support (`https://` URLs)
- ✅ Custom headers and authentication

## Dependencies

This feature depends on:
- `connection` - HTTP connections with TLS support
- `request-response` - Request building and response parsing
- `task-iterator` - Non-blocking state machine execution

This feature is required by:
- None (end-user feature)

### Existing Infrastructure

Already available in `foundation_core`:
- `wire/http_stream::ReconnectingStream` - Automatic reconnection with backoff
- `wire/simple_http::HttpClientConnection` - HTTP/1.1 with TLS
- `wire/simple_http::SimpleIncomingRequestBuilder` - HTTP request building
- `wire/simple_http::HttpResponseReader` - Streaming response parsing
- `valtron::TaskIterator` - Non-blocking event consumption
- `io::ioutils::SharedByteBufferStream` - Buffered I/O

### New Dependencies Required

None - all dependencies already in `Cargo.toml`

## Requirements

### Client-Side SSE Consumption

Connect to SSE endpoints and consume events:

```rust
use foundation_core::wire::event_source::{EventSource, Event};

// Connect to SSE endpoint
let event_source = EventSource::new("https://api.example.com/events")?
    .with_header("Authorization", "Bearer token123")
    .with_last_event_id("42")  // Resume from last event
    .connect()?;

// Consume events (blocking iterator)
for event in event_source {
    match event? {
        Event::Message { id, event_type, data, retry } => {
            println!("Event: {}", event_type.unwrap_or("message".to_string()));
            println!("ID: {}", id.unwrap_or_default());
            println!("Data: {}", data);

            if let Some(retry_ms) = retry {
                println!("Server suggests retry: {}ms", retry_ms);
            }
        }
        Event::Comment(comment) => {
            // Server sent a comment (keep-alive)
            println!("Comment: {}", comment);
        }
        Event::Reconnect => {
            println!("Reconnecting...");
        }
    }
}
```

### Non-Blocking Client (TaskIterator)

Use TaskIterator for non-blocking consumption:

```rust
use foundation_core::wire::event_source::EventSourceTask;
use foundation_core::valtron::{TaskIterator, TaskStatus};

let task = EventSourceTask::connect("https://api.example.com/events")?;

loop {
    match task.next() {
        Some(TaskStatus::Ready(event)) => {
            println!("Received: {:?}", event);
        }
        Some(TaskStatus::Pending(progress)) => {
            println!("Progress: {:?}", progress);
            // Do other work
        }
        Some(TaskStatus::Delayed(duration)) => {
            println!("Reconnecting in {:?}", duration);
        }
        None => break, // Stream closed
    }
}
```

### Event Filtering

Filter events by type:

```rust
let event_source = EventSource::new(url)?
    .on_event("user_joined", |data| {
        println!("User joined: {}", data);
    })
    .on_event("user_left", |data| {
        println!("User left: {}", data);
    })
    .connect()?;

// Only fires callbacks for filtered event types
for event in event_source {
    event?; // Callbacks already executed
}
```

### Server-Side SSE Production

Send SSE events to clients:

```rust
use foundation_core::wire::event_source::{EventWriter, SseEvent};
use std::io::Write;

fn handle_sse_connection(mut stream: impl Write) -> Result<(), Error> {
    let mut writer = EventWriter::new(&mut stream);

    // Send events
    writer.send(SseEvent::message("Hello, World!"))?;

    writer.send(SseEvent::new()
        .id("123")
        .event("user_joined")
        .data(r#"{"user": "alice"}"#)
        .build())?;

    // Multi-line data
    writer.send(SseEvent::message("Line 1\nLine 2\nLine 3"))?;

    // Set retry interval
    writer.send(SseEvent::retry(5000))?; // 5 seconds

    // Comment (keep-alive)
    writer.comment("Still alive")?;

    Ok(())
}
```

### SSE Response Builder

Helper for building SSE HTTP responses:

```rust
use foundation_core::wire::event_source::SseResponse;
use foundation_core::wire::simple_http::{Status, SimpleIncomingResponse};

fn handle_http_request(request: &IncomingRequest) -> SimpleIncomingResponse {
    if request.uri.path() == "/events" {
        // Build SSE response
        SseResponse::new()
            .with_header("Cache-Control", "no-cache")
            .with_header("X-Custom-Header", "value")
            .build()
    } else {
        // Regular response
        SimpleIncomingResponse::new(Status::Ok)
    }
}
```

### Automatic Reconnection

Client automatically reconnects on disconnect:

```rust
let event_source = EventSource::new(url)?
    .with_retry_interval(Duration::from_secs(3))  // Default: exponential backoff
    .with_max_retries(10)
    .connect()?;

// Automatic reconnection happens transparently
for event in event_source {
    match event? {
        Event::Reconnect => {
            println!("Connection lost, reconnecting...");
        }
        Event::Message { data, .. } => {
            println!("Data: {}", data);
        }
        _ => {}
    }
}
```

### Last-Event-ID Tracking

Client tracks last event ID for resume:

```rust
// Server sends events with IDs
writer.send(SseEvent::new().id("100").data("Event 100"))?;
writer.send(SseEvent::new().id("101").data("Event 101"))?;

// Client disconnects after ID 100

// Client reconnects with Last-Event-ID header
// Server can resume from ID 101
let event_source = EventSource::new(url)?
    .with_last_event_id("100")
    .connect()?;

// Or use auto-tracking
let event_source = EventSource::new(url)?
    .with_auto_tracking(true)  // Automatically sends Last-Event-ID
    .connect()?;
```

## Implementation Phases

### Phase 1: Core SSE Protocol (Blocking)

**Duration**: 1-2 weeks
**Goal**: Working SSE client and server with blocking I/O

**File structure**:
```
backends/foundation_core/src/wire/event_source/
├── mod.rs              # Public API and re-exports
├── core.rs             # Event and SseEvent types
├── parser.rs           # SSE message parser
├── client.rs           # EventSource client (blocking)
├── writer.rs           # EventWriter (server-side)
└── error.rs            # EventSourceError
```

**Tasks**:

1. **SSE Protocol Types** (`core.rs`)
   - Define `Event` enum (Message, Comment, Reconnect)
   - Define `SseEvent` builder for server-side
   - Message field types (id, event, data, retry)

   ```rust
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum Event {
       Message {
           id: Option<String>,
           event_type: Option<String>,
           data: String,
           retry: Option<u64>,
       },
       Comment(String),
       Reconnect,
   }

   pub struct SseEvent {
       id: Option<String>,
       event_type: Option<String>,
       data: Vec<String>,  // Multi-line data
       retry: Option<u64>,
   }

   impl SseEvent {
       pub fn message(data: impl Into<String>) -> Self;
       pub fn new() -> SseEventBuilder;
       pub fn retry(milliseconds: u64) -> Self;
   }

   pub struct SseEventBuilder {
       id: Option<String>,
       event_type: Option<String>,
       data: Vec<String>,
   }

   impl SseEventBuilder {
       pub fn id(mut self, id: impl Into<String>) -> Self;
       pub fn event(mut self, event_type: impl Into<String>) -> Self;
       pub fn data(mut self, data: impl Into<String>) -> Self;
       pub fn build(self) -> SseEvent;
   }
   ```

2. **SSE Message Parser** (`parser.rs`)
   - Implement streaming SSE parser following W3C spec
   - Handle field types: `id:`, `event:`, `data:`, `retry:`, `:` (comment)
   - Handle multi-line data fields
   - UTF-8 BOM handling
   - Line endings: `\n`, `\r`, `\r\n`

   ```rust
   pub struct SseParser {
       buffer: String,
       current_id: Option<String>,
       current_event: Option<String>,
       current_data: Vec<String>,
       current_retry: Option<u64>,
   }

   impl SseParser {
       pub fn new() -> Self;

       /// Parse incoming chunk, yield complete events
       pub fn parse(&mut self, chunk: &str) -> Vec<Event>;

       /// Get the last event ID seen
       pub fn last_event_id(&self) -> Option<&str>;
   }
   ```

3. **SSE Client** (`client.rs`)
   - Build GET request with `Accept: text/event-stream`
   - Add `Last-Event-ID` header if provided
   - Stream response using `HttpResponseReader`
   - Parse events using `SseParser`
   - Track last event ID

   ```rust
   pub struct EventSource {
       url: String,
       headers: Vec<(String, String)>,
       last_event_id: Option<String>,
       auto_tracking: bool,
   }

   impl EventSource {
       pub fn new(url: impl Into<String>) -> Self;
       pub fn with_header(self, name: impl Into<String>, value: impl Into<String>) -> Self;
       pub fn with_last_event_id(self, id: impl Into<String>) -> Self;
       pub fn with_auto_tracking(self, enabled: bool) -> Self;
       pub fn connect(self) -> Result<EventSourceStream, EventSourceError>;
   }

   pub struct EventSourceStream {
       connection: HttpClientConnection,
       parser: SseParser,
       reader: HttpResponseReader<...>,
       auto_tracking: bool,
   }

   impl Iterator for EventSourceStream {
       type Item = Result<Event, EventSourceError>;

       fn next(&mut self) -> Option<Self::Item> {
           // Read from response stream
           // Parse SSE events
           // Track last event ID if auto_tracking
       }
   }
   ```

4. **SSE Server Writer** (`writer.rs`)
   - Format events according to SSE spec
   - Handle multi-line data (prefix each line with `data: `)
   - Flush after each event

   ```rust
   pub struct EventWriter<W: Write> {
       writer: W,
   }

   impl<W: Write> EventWriter<W> {
       pub fn new(writer: W) -> Self;

       pub fn send(&mut self, event: SseEvent) -> Result<(), std::io::Error> {
           // Format event
           if let Some(id) = event.id {
               write!(self.writer, "id: {}\n", id)?;
           }
           if let Some(event_type) = event.event_type {
               write!(self.writer, "event: {}\n", event_type)?;
           }
           for line in event.data {
               write!(self.writer, "data: {}\n", line)?;
           }
           if let Some(retry) = event.retry {
               write!(self.writer, "retry: {}\n", retry)?;
           }
           write!(self.writer, "\n")?;
           self.writer.flush()
       }

       pub fn comment(&mut self, comment: &str) -> Result<(), std::io::Error> {
           write!(self.writer, ": {}\n\n", comment)?;
           self.writer.flush()
       }
   }
   ```

5. **SSE Response Helper**
   - Helper to build SSE HTTP response with correct headers

   ```rust
   pub struct SseResponse;

   impl SseResponse {
       pub fn new() -> SseResponseBuilder;
   }

   pub struct SseResponseBuilder {
       headers: Vec<(String, String)>,
   }

   impl SseResponseBuilder {
       pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self;

       pub fn build(self) -> SimpleIncomingResponse {
           let mut response = SimpleIncomingResponse::new(Status::Ok);
           response.headers.insert(SimpleHeader::CONTENT_TYPE, "text/event-stream");
           response.headers.insert(SimpleHeader::CACHE_CONTROL, "no-cache");
           response.headers.insert(SimpleHeader::CONNECTION, "keep-alive");
           // Add custom headers
           response
       }
   }
   ```

6. **Error Handling** (`error.rs`)
   - Define `EventSourceError` enum
   - Conversions from I/O, HTTP, and parsing errors

   ```rust
   #[derive(Debug)]
   pub enum EventSourceError {
       /// HTTP client error
       Http(HttpClientError),

       /// Invalid SSE format
       ParseError(String),

       /// Connection closed by server
       ConnectionClosed,

       /// Invalid retry value
       InvalidRetry(String),

       /// IO error
       IoError(std::io::Error),
   }
   ```

**Success Criteria**:
- Can connect to SSE endpoint and receive events
- Parser correctly handles all field types
- Parser correctly handles multi-line data
- Server can send events to clients
- Last-Event-ID tracking works
- TLS support works (`https://` URLs)
- All unit tests pass

### Phase 2: Reconnection Support

**Duration**: 1 week
**Goal**: Automatic reconnection with backoff

**File structure** (additions):
```
backends/foundation_core/src/wire/event_source/
└── reconnecting.rs     # Auto-reconnecting EventSource
```

**Tasks**:

1. **Reconnecting Client** (`reconnecting.rs`)
   - Integrate with `http_stream::ReconnectingStream`
   - Exponential backoff using existing `ExponentialBackoffDecider`
   - Respect server `retry:` field
   - Auto-send `Last-Event-ID` header on reconnect

   ```rust
   pub struct ReconnectingEventSource {
       url: String,
       headers: Vec<(String, String)>,
       reconnection_stream: ReconnectingStream,
       parser: SseParser,
       max_retries: u32,
   }

   impl ReconnectingEventSource {
       pub fn new(url: impl Into<String>) -> Self;
       pub fn with_retry_interval(self, duration: Duration) -> Self;
       pub fn with_max_retries(self, max: u32) -> Self;
   }

   impl Iterator for ReconnectingEventSource {
       type Item = Result<Event, EventSourceError>;

       fn next(&mut self) -> Option<Self::Item> {
           // Try to read event
           // On connection error, trigger reconnection_stream
           // On reconnect, send Last-Event-ID header
       }
   }
   ```

**Success Criteria**:
- Auto-reconnects on connection loss
- Respects server `retry:` field
- Sends `Last-Event-ID` on reconnect
- Exponential backoff works correctly
- Max retries honored

### Phase 3: TaskIterator Integration (Non-Blocking)

**Duration**: 1 week
**Goal**: Non-blocking SSE consumption

**File structure** (additions):
```
backends/foundation_core/src/wire/event_source/
└── task.rs             # EventSourceTask
```

**Tasks**:

1. **EventSource Task** (`task.rs`)
   - State machine: Init → Connecting → Reading → Reconnecting
   - Integrate with valtron executor
   - Non-blocking event parsing

   ```rust
   pub struct EventSourceTask {
       state: EventSourceState,
   }

   enum EventSourceState {
       Init(EventSourceConfig),
       Connecting(DrivenRecvIterator<HttpConnectTask>),
       Reading(EventSourceStream),
       Reconnecting(Duration),
       Closed,
   }

   impl TaskIterator for EventSourceTask {
       type Ready = Event;
       type Pending = EventSourcePending;
       type Spawner = BoxedSendExecutionAction;

       fn next(&mut self) -> Option<TaskStatus<Event, EventSourcePending, ...>>;
   }
   ```

**Success Criteria**:
- Non-blocking connect works
- Non-blocking event reading works
- Integrates with valtron executor
- Can handle multiple concurrent connections

### Phase 4: Advanced Features (Optional)

**Duration**: 1 week
**Goal**: Production-ready SSE support

**Tasks**:

1. **Event Filtering**
   - Filter events by type
   - Callback-based API

2. **Compression Support**
   - Handle `Content-Encoding: gzip` in SSE responses
   - Integrate with existing compression infrastructure

3. **Performance Optimizations**
   - Buffer pooling for parser
   - Zero-copy parsing where possible

**Success Criteria**:
- Event filtering works correctly
- Compression support works
- Performance benchmarks pass

## Success Criteria

**Phase 1 (Core - Blocking)**:
- [ ] `event_source/` module exists and compiles
- [ ] `SseParser` correctly parses all SSE field types
- [ ] `SseParser` handles multi-line data correctly
- [ ] `SseParser` handles all line ending types (`\n`, `\r`, `\r\n`)
- [ ] `EventSource` can connect to SSE endpoint
- [ ] `EventSource` correctly tracks Last-Event-ID
- [ ] `EventWriter` formats events correctly
- [ ] `SseResponse` builds correct HTTP response headers
- [ ] Can consume events from public SSE server
- [ ] Can send events to connected clients
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

**Phase 2 (Reconnection)**:
- [ ] Auto-reconnects on connection loss
- [ ] Respects server `retry:` field
- [ ] Sends `Last-Event-ID` on reconnect
- [ ] Exponential backoff works
- [ ] Max retries honored

**Phase 3 (Non-Blocking)**:
- [ ] `EventSourceTask` implements `TaskIterator`
- [ ] Non-blocking connect works
- [ ] Non-blocking event consumption works
- [ ] Integrates with valtron executor
- [ ] Performance is acceptable

**Phase 4 (Advanced - Optional)**:
- [ ] Event filtering works
- [ ] Compression support works
- [ ] Performance benchmarks pass

## Verification Commands

```bash
# Format check
cargo fmt -- --check

# Clippy linting
cargo clippy --package foundation_core --features std -- -D warnings

# Unit tests
cargo test --package foundation_core -- event_source

# Integration test with public SSE server (requires network)
cargo test --package foundation_core -- event_source::integration --ignored
```

## Notes for Implementation Agents

### Critical Pre-Checks

Before starting implementation, **MUST**:
1. Verify dependencies: `connection`, `request-response`, `task-iterator` features complete
2. Read [W3C Server-Sent Events specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)
3. Explore existing code:
   - `wire/http_stream/mod.rs` - ReconnectingStream pattern
   - `wire/simple_http/client/connection.rs` - HTTP connection patterns
   - `wire/simple_http/impls.rs` - SimpleIncomingRequestBuilder
   - `valtron/task.rs` - TaskIterator pattern

### SSE Protocol Rules

**Message Format** (from W3C spec):
```
field: value\n
field: value\n
\n
```

**Field Types**:
- `event: <type>` - Event type (default: "message")
- `data: <text>` - Event data (can appear multiple times)
- `id: <id>` - Event ID (sent back as Last-Event-ID)
- `retry: <milliseconds>` - Reconnection time
- `: <comment>` - Comment (ignored, used for keep-alive)

**Parsing Rules**:
1. Lines ending with `\n`, `\r`, or `\r\n`
2. UTF-8 BOM (`\uFEFF`) at start of stream is ignored
3. Lines starting with `:` are comments
4. Field name and value separated by first `:`
5. Optional single space after `:` is stripped
6. Empty line dispatches event
7. Multiple `data:` fields are joined with `\n`
8. If `id:` contains null byte (`\0`), ignore the field
9. `retry:` must be valid integer, otherwise ignore

**Example**:
```
: This is a comment

event: user_joined
data: {"user": "alice"}
data: {"timestamp": 1234567890}
id: 42

: Keep-alive

data: Simple message without event type
id: 43

retry: 5000
```

### HTTP Headers

**Client Request**:
```http
GET /events HTTP/1.1
Host: example.com
Accept: text/event-stream
Cache-Control: no-cache
Last-Event-ID: 42
```

**Server Response**:
```http
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
```

### Reusable Components

**From existing codebase**:
- `SimpleIncomingRequestBuilder` - Build SSE request
- `HttpResponseReader` - Stream response chunks
- `HttpClientConnection` - HTTP/1.1 with TLS
- `ReconnectingStream` - Auto-reconnection with backoff
- `ExponentialBackoffDecider` - Backoff strategy
- `SharedByteBufferStream` - Buffered I/O
- `TaskIterator` - Non-blocking pattern (Phase 3)

**Do NOT re-implement**:
- HTTP request building (use `SimpleIncomingRequestBuilder`)
- HTTP response streaming (use `HttpResponseReader`)
- Reconnection logic (use `ReconnectingStream`)
- Backoff strategy (use `ExponentialBackoffDecider`)

### Testing Strategy

**Unit Tests**:
1. **Parser tests**
   - Parse single event
   - Parse multi-line data
   - Parse all field types
   - Handle different line endings
   - Handle UTF-8 BOM
   - Handle comments
   - Handle malformed input

2. **Event builder tests**
   - Build simple event
   - Build event with all fields
   - Multi-line data formatting

3. **Writer tests**
   - Format events correctly
   - Handle multi-line data
   - Send comments

**Integration Tests**:
1. Connect to public SSE server (e.g., `https://sse.dev/test`)
2. Verify event reception
3. Test reconnection
4. Test Last-Event-ID handling

**Test Vectors**:
```
Input:
event: test\n
data: hello\n
\n

Expected:
Event::Message {
    id: None,
    event_type: Some("test"),
    data: "hello",
    retry: None,
}

Input:
data: line1\n
data: line2\n
\n

Expected:
Event::Message {
    id: None,
    event_type: None,
    data: "line1\nline2",
    retry: None,
}
```

### Common Pitfalls to Avoid

1. **Not handling multi-line data** (each `data:` field is a separate line)
2. **Not stripping space after `:`** (spec says optional space is stripped)
3. **Not joining data fields with `\n`** (multiple `data:` fields join with newline)
4. **Forgetting to flush** after each event (server-side)
5. **Not handling different line endings** (`\n`, `\r`, `\r\n`)
6. **Not ignoring UTF-8 BOM** at start of stream
7. **Not resetting event state** after dispatch
8. **Not handling `id:` with null byte** (should be ignored)
9. **Not validating `retry:` value** (must be valid integer)
10. **Hand-writing HTTP requests** (use `SimpleIncomingRequestBuilder`)

### Security Considerations

- **Validate URLs**: Reject invalid schemes (only http/https)
- **Limit message size**: Enforce max event size to prevent DoS
- **Limit reconnection attempts**: Respect max_retries
- **Validate retry values**: Reject negative or extremely large values
- **Handle redirects carefully**: SSE should follow redirects (use existing redirect logic)

---

*Created: 2026-03-03*
*Last Updated: 2026-03-03*
