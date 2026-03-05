# Server-Sent Events (SSE) Feature Architecture

## Document Overview

This document provides a comprehensive architectural analysis for implementing Server-Sent Events (SSE / EventSource) support in the `foundation_core` wire module. It is based on thorough exploration of the existing codebase infrastructure, particularly the `simple_http`, `http_stream`, `retries`, and `valtron` modules.

**Status**: Draft - Updated for TaskIterator-First Design
**Last Updated**: 2026-03-04
**Version**: 2.0

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Existing Infrastructure Analysis](#existing-infrastructure-analysis)
3. [SSE Protocol Overview](#sse-protocol-overview)
4. [Integration Strategy](#integration-strategy)
5. [Parser Design](#parser-design)
6. [Client Implementation](#client-implementation)
7. [Server Implementation](#server-implementation)
8. [Reconnection Strategy](#reconnection-strategy)
9. [TaskIterator Integration](#taskiterator-integration)
10. [Technical Design Details](#technical-design-details)

---

## 1. Executive Summary

### Key Findings

The `foundation_core` wire module provides **exceptional infrastructure** for SSE implementation:

1. **Existing event_source module skeleton**:
   - Module already present at `wire/event_source/`
   - Currently empty but ready for implementation
   - Proper WASM/non-WASM split architecture

2. **Robust HTTP streaming infrastructure**:
   - `http_stream::ReconnectingStream` - Automatic reconnection with exponential backoff
   - `simple_http::HttpResponseReader` - Streaming response parsing
   - `simple_http::SimpleIncomingRequestBuilder` - HTTP request building
   - `simple_http::HttpClientConnection` - HTTP/1.1 with TLS support

3. **Production-ready retry mechanisms**:
   - `retries::ExponentialBackoffDecider` - Smart backoff with jitter
   - `retries::RetryDecider` trait - Pluggable retry strategies
   - `retries::RetryState` - State tracking for attempts

4. **TaskIterator pattern for non-blocking I/O**:
   - `valtron::TaskIterator` trait - State machine execution
   - `valtron::executors::*` - Unified executor infrastructure
   - `valtron::delayed_iterators::SleepIterator` - Delayed execution

5. **Stream abstractions**:
   - `io::ioutils::SharedByteBufferStream` - Thread-safe buffered I/O
   - `netcap::RawStream` - Unified TCP/TLS stream
   - `netcap::Connection` - Low-level connection wrapper

### Infrastructure Completeness: ~80%

**What exists**:
- ✅ HTTP request/response infrastructure
- ✅ Streaming response parsing
- ✅ Automatic reconnection with backoff
- ✅ TLS support
- ✅ TaskIterator framework
- ✅ Module skeleton (event_source/)

**What needs implementation**:
- SSE protocol parser (field parsing, line handling) - internal plain iterator
- Event types (Event, SseEvent)
- EventSourceTask (TaskIterator - PRIMARY API)
- ReconnectingEventSourceTask (TaskIterator with reconnection)
- EventWriter server-side
- Last-Event-ID tracking
- Blocking wrapper (convenience API only)

**Estimated effort**: 2-3 weeks for complete implementation

### Recommended Approach: TaskIterator with DrivenSendTaskIterator Wrappers

**CRITICAL ARCHITECTURAL PRINCIPLE:** All SSE client implementations MUST use TaskIterator as the core pattern with `DrivenSendTaskIterator` for blocking wrappers. The TaskIterator is a pure state machine with NO loops - execution driving happens in the wrapper.

**Design Hierarchy:**
1. **TaskIterator (Core)** - Pure state machine, ONE step per `next()` call, NO loops
2. **Plain Iterator (Internal)** - Used internally for parsing (SseParser), wrapped by TaskIterator
3. **DrivenSendTaskIterator (Wrapper)** - Handles execution driving via `run_until_next_state()`
4. **Blocking Iterator API (Convenience)** - Wraps DrivenSendTaskIterator, extracts Ready values

**Phase 1: Core SSE Protocol with TaskIterator (1-2 weeks)**
- Implement `EventSourceTask` following valtron TaskIterator pattern
- State machine: Init → Connecting → Reading → Closed
- SSE parser is internal (plain iterator), wrapped by TaskIterator
- Blocking wrapper uses `DrivenSendTaskIterator` - execution driving in wrapper, not TaskIterator

**Phase 2: ReconnectingEventSourceTask (3-5 days)**
- Wrap `EventSourceTask` with `ReconnectingEventSourceTask`
- Use existing `ExponentialBackoffDecider` for backoff
- Auto-send `Last-Event-ID` on reconnect
- TaskIterator-based reconnection state machine

**Phase 3: Advanced Features (3-5 days)**
- Event filtering with callbacks
- Full valtron executor integration
- Performance optimizations

**Phase 3: Advanced Features (3-5 days)**
- Event filtering with callbacks
- Full valtron executor integration
- Performance optimizations

---

## 2. Existing Infrastructure Analysis

### 2.1 Wire Module Structure

```
wire/
├── mod.rs                  # Module exports
├── event_source/           # SSE module (currently skeleton)
│   ├── mod.rs              # Platform-specific exports
│   ├── core.rs             # Empty (ready for Event types)
│   ├── no_wasm.rs          # Empty (ready for native impl)
│   └── wasm.rs             # Empty (ready for WASM impl)
├── http_stream/            # Reconnecting stream utilities
│   └── mod.rs              # ReconnectingStream, ConnectionState
├── simple_http/            # HTTP/1.1 implementation
│   ├── mod.rs
│   ├── errors.rs           # HttpClientError, HttpReaderError
│   ├── impls.rs            # Core types (8000+ lines)
│   ├── url/                # URL parsing (Uri, Scheme, etc.)
│   └── client/             # HTTP client implementation
│       ├── connection.rs   # HttpClientConnection, TLS
│       ├── pool.rs         # Connection pooling
│       ├── dns.rs          # DNS resolution
│       ├── request.rs      # Request building
│       ├── tasks/          # TaskIterator-based execution
│       ├── proxy.rs        # Proxy support
│       ├── middleware.rs   # Middleware pipeline
│       ├── cookie.rs       # Cookie handling
│       └── compression.rs  # Response decompression
```

### 2.2 Core Infrastructure Components

#### 2.2.1 HTTP Response Streaming

**HttpResponseReader** (`simple_http/impls.rs:3423`)

```rust
pub struct HttpResponseReader<F: BodyExtractor, T: std::io::Read + 'static> {
    reader: SharedByteBufferStream<T>,
    bodies: F,
    // Internal state for parsing
}

// Iterator over response parts
impl<F, T> Iterator for HttpResponseReader<F, T> {
    type Item = Result<IncomingResponseParts, HttpReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Streams response: Intro → Headers → Body chunks
    }
}

pub enum IncomingResponseParts {
    Intro(Status, Proto, String),
    Headers(SimpleHeaders),
    Body(BodyPart),
    Completed(ResponseCompleted),
}
```

**Key characteristics**:
- Streams response in parts (intro, headers, body chunks)
- Works with any `Read` stream (TCP, TLS, etc.)
- Buffered via `SharedByteBufferStream`
- Can be used to stream SSE events line-by-line

**SSE Usage Pattern**:
```rust
// 1. Make SSE request
let request = SimpleIncomingRequestBuilder::get("/events")
    .header(SimpleHeader::ACCEPT, "text/event-stream")
    .header(SimpleHeader::CACHE_CONTROL, "no-cache")
    .build()?;

// 2. Get streaming response
let response_reader = HttpResponseReader::new(connection.clone_stream(), SimpleHttpBody);

// 3. Parse SSE events from body chunks
for part in response_reader {
    match part? {
        IncomingResponseParts::Body(chunk) => {
            // Parse SSE events from chunk
            let events = sse_parser.parse(&chunk);
        }
        _ => {}
    }
}
```

#### 2.2.2 Reconnecting Stream Infrastructure

**ReconnectingStream** (`http_stream/mod.rs:36`)

```rust
pub struct ReconnectingStream {
    max_retries: u32,
    state: Arc<Mutex<ConnectionState>>,
    connection_timeout: Duration,
    decider: Box<dyn CReconnectionDecider>,
}

#[derive(Clone, Debug)]
pub enum ConnectionState {
    Todo(ClientEndpoint),           // Initial connection
    Redo(ClientEndpoint, RetryState), // Retry connection
    Reconnect(RetryState, Option<SleepIterator<ClientEndpoint>>), // Waiting to reconnect
    Established(ClientEndpoint),    // Connected
    Exhausted(ClientEndpoint),      // All retries exhausted
}

pub enum ReconnectionStatus {
    NoMoreWaiting,
    Ready(Box<RawStream>),
    Waiting(Duration),
}

impl Iterator for ReconnectingStream {
    type Item = Result<ReconnectionStatus, ReconnectionError>;

    fn next(&mut self) -> Option<Self::Item> {
        // State machine: Todo → Redo → Reconnect → Established
        // Returns Ready(stream) when connected
        // Returns Waiting(duration) when backing off
        // Returns None when exhausted
    }
}
```

**Features**:
- Automatic exponential backoff with jitter
- Configurable retry limits
- Connection timeout support
- Thread-safe state (Arc<Mutex<>>)
- Pluggable retry deciders

**SSE Usage Pattern (TaskIterator-First)**:

```rust
// Phase 1: EventSourceTask (TaskIterator)
let mut task = EventSourceTask::connect("https://api.example.com/events")?;

loop {
    match task.next() {
        Some(TaskStatus::Ready(event)) => {
            handle_event(event?);
        }
        Some(TaskStatus::Pending(progress)) => {
            // Do other work while waiting
        }
        Some(TaskStatus::Delayed(duration)) => {
            println!("Reconnecting in {:?}", duration);
        }
        None => break,
    }
}

// Phase 2: ReconnectingEventSourceTask
let mut task = ReconnectingEventSourceTask::connect(url)?
    .with_max_retries(10);

loop {
    match task.next() {
        Some(TaskStatus::Ready(event)) => {
            handle_event(event?);
        }
        Some(TaskStatus::Delayed(duration)) => {
            // Backing off before reconnect
        }
        None => break, // Max retries exceeded
    }
}

// Blocking wrapper (convenience only)
let event_source = EventSource::connect(url)?;
for event in event_source {
    handle_event(event?);
}
```

#### 2.2.3 Retry Infrastructure

**ExponentialBackoffDecider** (`retries/exponential.rs:6`)

```rust
#[derive(Clone, Debug)]
pub struct ExponentialBackoffDecider {
    pub factor: u32,           // Exponential factor (default: 3)
    pub jitter: f32,           // Jitter amount 0.0-1.0 (default: 0.6)
    pub min_duration: Duration, // Min backoff (default: 100ms)
    pub max_duration: Duration, // Max backoff (default: Duration::MAX)
    pub rng: RefCell<fastrand::Rng>, // Fast RNG for jitter
}

impl RetryDecider for ExponentialBackoffDecider {
    fn decide(&self, state: RetryState) -> Option<RetryState> {
        // Returns None if max retries reached
        // Otherwise calculates: min_duration * (factor ^ attempt) ± jitter
        // Clamps to [min_duration, max_duration]
    }
}
```

**RetryState** (`retries/core.rs:10`)

```rust
#[derive(Clone, Debug)]
pub struct RetryState {
    pub wait: Option<Duration>,  // How long to wait before retry
    pub total_allowed: u32,       // Maximum retry attempts
    pub attempt: u32,             // Current attempt number
}
```

**Backoff calculation**:
```
wait_time = min_duration * (factor ^ attempt)
with_jitter = wait_time * (1.0 ± jitter)
final = clamp(with_jitter, min_duration, max_duration)

Example (factor=3, min=100ms, jitter=0.6):
Attempt 0: 100ms * (3^1) = 300ms ± 60% = 120-480ms
Attempt 1: 100ms * (3^2) = 900ms ± 60% = 360-1440ms
Attempt 2: 100ms * (3^3) = 2700ms ± 60% = 1080-4320ms
```

**SSE Integration**:
- SSE spec recommends client-controlled reconnection
- Server can suggest retry via `retry:` field
- Use ExponentialBackoffDecider as default
- Allow override with server's `retry:` value

#### 2.2.4 Stream Abstractions

**SharedByteBufferStream** (`io/ioutils/mod.rs`)

```rust
pub struct SharedByteBufferStream<T> {
    inner: Arc<Mutex<BufferedStream<T>>>,
}

impl<T: Read> Read for SharedByteBufferStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.lock().unwrap().read(buf)
    }
}

impl<T: Write> Write for SharedByteBufferStream<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().flush()
    }
}

impl<T> Clone for SharedByteBufferStream<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}
```

**Key features**:
- Thread-safe (Arc<Mutex<>>)
- Cloneable stream handles
- Buffered I/O
- Works with any Read/Write stream

**RawStream** (`netcap/mod.rs`)

```rust
pub enum RawStream {
    AsPlain(TcpStream),
    AsClientTls(ClientSSLStream),
    AsServerTls(ServerSSLStream),
}

impl Read for RawStream { /* delegates to inner */ }
impl Write for RawStream { /* delegates to inner */ }
```

**SSE Usage**:
- Use `SharedByteBufferStream<RawStream>` for SSE connections
- Provides buffered, thread-safe, cloneable access
- Supports both HTTP and HTTPS (TLS) transparently

#### 2.2.5 HTTP Request Building

**SimpleIncomingRequestBuilder** (`simple_http/impls.rs:1729`)

```rust
pub struct SimpleIncomingRequestBuilder {
    proto: Option<Proto>,
    req_uri: Option<Uri>,
    url: Option<SimpleUrl>,
    body: Option<SendSafeBody>,
    method: Option<SimpleMethod>,
    headers: Option<SimpleHeaders>,
    extensions: Option<ClientExtensions>,
}

impl SimpleIncomingRequestBuilder {
    pub fn get<S: Into<String>>(path: S) -> Self;
    pub fn post<S: Into<String>>(path: S) -> Self;
    pub fn with_uri(self, uri: Uri) -> Self;
    pub fn header<H: Into<SimpleHeader>, V: Into<String>>(self, name: H, value: V) -> Self;
    pub fn build(self) -> Result<SimpleIncomingRequest, SimpleRequestError>;
}
```

**SSE Request Pattern**:
```rust
let request = SimpleIncomingRequestBuilder::get("/events")
    .header(SimpleHeader::ACCEPT, "text/event-stream")
    .header(SimpleHeader::CACHE_CONTROL, "no-cache")
    .header(SimpleHeader::LAST_EVENT_ID, last_id) // Optional
    .build()?;
```

#### 2.2.6 TaskIterator Framework - PRIMARY PATTERN FOR SSE

**TaskIterator** (`valtron/types.rs`)

```rust
pub trait TaskIterator {
    type Pending;   // Progress/waiting state
    type Ready;     // Completed result
    type Spawner: ExecutionAction;  // Sub-task spawner

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
}

pub enum TaskStatus<R, P, S> {
    Init,               // Initial state
    Pending(P),         // In progress
    Ready(R),           // Result available
    Spawn(S),           // Spawn sub-task
    Delayed(Duration),  // Wait and retry
}
```

**SSE State Machine (EventSourceTask)**:

```rust
use crate::valtron::{TaskIterator, TaskStatus, BoxedSendExecutionAction, DrivenRecvIterator};

pub struct EventSourceTask {
    state: Option<EventSourceState>,
}

enum EventSourceState {
    Init(EventSourceConfig),
    Connecting(DrivenRecvIterator<HttpConnectTask>),
    Reading(EventSourceStreamReader),
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventSourcePending {
    Connecting,
    Reading,
}

impl TaskIterator for EventSourceTask {
    type Ready = Result<Event, EventSourceError>;
    type Pending = EventSourcePending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state.take()? {
            EventSourceState::Init(config) => {
                // Spawn connection task
                let (action, recv) = spawn_connect_task(config);
                self.state = Some(EventSourceState::Connecting(recv));
                Some(TaskStatus::Spawn(action))
            }
            EventSourceState::Connecting(mut recv) => {
                match recv.next() {
                    Some(TaskStatus::Ready(Ok(connection))) => {
                        // Connected, start reading SSE events
                        let reader = EventSourceStreamReader::new(connection);
                        self.state = Some(EventSourceState::Reading(reader));
                        Some(TaskStatus::Pending(EventSourcePending::Reading))
                    }
                    Some(TaskStatus::Ready(Err(e))) => {
                        self.state = Some(EventSourceState::Closed);
                        Some(TaskStatus::Ready(Err(e)))
                    }
                    Some(TaskStatus::Pending(_)) => {
                        self.state = Some(EventSourceState::Connecting(recv));
                        Some(TaskStatus::Pending(EventSourcePending::Connecting))
                    }
                    _ => None,
                }
            }
            EventSourceState::Reading(mut reader) => {
                match reader.next_event() {
                    Ok(event) => {
                        self.state = Some(EventSourceState::Reading(reader));
                        Some(TaskStatus::Ready(Ok(event)))
                    }
                    Err(EventSourceError::ConnectionClosed) => {
                        self.state = Some(EventSourceState::Closed);
                        None // Task complete, reconnection handled by wrapper
                    }
                    Err(e) => {
                        self.state = Some(EventSourceState::Reading(reader));
                        Some(TaskStatus::Ready(Err(e)))
                    }
                }
            }
            EventSourceState::Closed => None,
        }
    }
}
```

**Blocking Wrapper with DrivenSendTaskIterator**:

```rust
use crate::valtron::{drive_iterator, DrivenSendTaskIterator};

// Blocking wrapper using DrivenSendTaskIterator
pub struct EventSourceIterator {
    driven: DrivenSendTaskIterator<EventSourceTask>,
}

impl EventSourceIterator {
    pub fn connect(url: impl Into<String>) -> Result<Self, EventSourceError> {
        let task = EventSourceTask::connect(url)?;
        Ok(Self {
            driven: drive_iterator(task),
        })
    }
}

impl Iterator for EventSourceIterator {
    type Item = Result<Event, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        // DrivenSendTaskIterator handles execution driving
        // Calls run_until_next_state() then task.next() ONCE
        // NO LOOPS in TaskIterator - execution driving is external
        match self.driven.next() {
            Some(TaskStatus::Ready(result)) => Some(result),
            Some(TaskStatus::Delayed(_)) => None,  // Reconnection backoff
            Some(TaskStatus::Pending(_)) => self.next(),  // Continue driving
            Some(TaskStatus::Spawn(_)) => self.next(),  // Spawn handled by executor
            Some(TaskStatus::Init) => self.next(),
            None => None,
        }
    }
}
```

**Key Points**:
- TaskIterator remains pure - NO loops in `next()`
- DrivenSendTaskIterator handles execution driving via `run_until_next_state()`
- Blocking Iterator wrapper extracts Ready values from TaskStatus
- Tail recursion for Pending/Spawn/Init - not a loop in TaskIterator

### 2.3 HTTP Headers for SSE

**Headers already defined** (`simple_http/impls.rs:568`)

```rust
pub enum SimpleHeader {
    // Standard headers
    ACCEPT,
    CACHE_CONTROL,
    CONNECTION,
    CONTENT_TYPE,

    // Custom headers (use for Last-Event-ID)
    CUSTOM(String),
}

impl SimpleHeader {
    pub fn custom<S: Into<String>>(value: S) -> Self {
        Self::CUSTOM(value.into())
    }
}
```

**SSE Headers**:
```rust
// Client → Server
SimpleHeader::ACCEPT, "text/event-stream"
SimpleHeader::CACHE_CONTROL, "no-cache"
SimpleHeader::custom("Last-Event-ID"), "42"

// Server → Client
SimpleHeader::CONTENT_TYPE, "text/event-stream"
SimpleHeader::CACHE_CONTROL, "no-cache"
SimpleHeader::CONNECTION, "keep-alive"
```

---

## 3. SSE Protocol Overview

### 3.1 W3C Specification Summary

**Standard**: [HTML Living Standard - Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html)

**Protocol Characteristics**:
- **Unidirectional**: Server → Client only
- **Text-based**: UTF-8 encoded text stream
- **Line-oriented**: Fields separated by newlines
- **Reconnection**: Client automatically reconnects on disconnect
- **Resume**: Last-Event-ID allows resuming from specific point

### 3.2 Message Format

**Basic Structure**:
```
field: value\n
field: value\n
\n
```

**Field Types**:
1. **`event:`** - Event type (default: "message")
2. **`data:`** - Event data (can appear multiple times)
3. **`id:`** - Event ID (for Last-Event-ID tracking)
4. **`retry:`** - Reconnection time in milliseconds
5. **`:`** - Comment line (ignored, used for keep-alive)

**Example**:
```
: This is a comment (keep-alive)

event: user_joined
data: {"user": "alice"}
data: {"timestamp": 1234567890}
id: 42

data: Simple message without event type
id: 43

retry: 5000

: Another keep-alive
```

### 3.3 Parsing Rules

From W3C spec:

1. **Line Endings**: `\n`, `\r`, or `\r\n`
2. **UTF-8 BOM**: `\uFEFF` at stream start is ignored
3. **Field Parsing**:
   - Line starting with `:` → comment (ignore)
   - First `:` separates field name and value
   - Optional single space after `:` is stripped
   - No `:` → treat entire line as field name with empty value
4. **Event Dispatch**:
   - Empty line → dispatch accumulated event
   - Multiple `data:` fields → join with `\n`
   - No `event:` field → type defaults to "message"
5. **ID Field**:
   - If contains null byte (`\0`) → ignore the field
   - Otherwise → store as last event ID
6. **Retry Field**:
   - Must be valid integer
   - Invalid → ignore the field
   - Sets reconnection time in milliseconds

**State Machine**:
```
For each line:
  If line is empty:
    Dispatch event from buffer
    Reset buffer
  Else if line starts with ':':
    Ignore (comment)
  Else:
    Parse field name and value
    Add to buffer

When dispatching:
  event_type = buffer.event OR "message"
  data = buffer.data.join('\n')
  id = buffer.id (if present)
  retry = buffer.retry (if present)
```

### 3.4 HTTP Handshake

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

: Connected

data: First event
id: 43

```

**Key Points**:
- `Accept: text/event-stream` signals SSE request
- `Content-Type: text/event-stream` confirms SSE response
- `Cache-Control: no-cache` prevents caching
- `Connection: keep-alive` keeps connection open
- Stream is **unbounded** - no Content-Length

---

## 4. Integration Strategy

### 4.1 Component Dependencies

```
EventSource (client)
    ├─> SseParser                    (parse SSE messages)
    ├─> SimpleIncomingRequestBuilder (build GET request)
    ├─> HttpClientConnection         (HTTP/1.1 connection)
    ├─> HttpResponseReader           (stream response)
    └─> ReconnectingStream           (auto-reconnect)
            └─> ExponentialBackoffDecider (backoff strategy)

EventWriter (server)
    └─> Write trait                  (format SSE messages)

EventSourceTask (non-blocking)
    ├─> EventSource                  (blocking version)
    └─> TaskIterator                 (non-blocking wrapper)
```

### 4.2 Reusable Components

| Component | Source | Usage in SSE |
|-----------|--------|--------------|
| `SimpleIncomingRequestBuilder` | `simple_http/impls.rs` | Build SSE GET request |
| `HttpResponseReader` | `simple_http/impls.rs` | Stream response chunks |
| `HttpClientConnection` | `simple_http/client/connection.rs` | HTTP/1.1 + TLS |
| `ReconnectingStream` | `http_stream/mod.rs` | Auto-reconnection |
| `ExponentialBackoffDecider` | `retries/exponential.rs` | Backoff strategy |
| `SharedByteBufferStream` | `io/ioutils/mod.rs` | Buffered I/O |
| `TaskIterator` | `valtron/types.rs` | Non-blocking pattern |

### 4.3 What NOT to Re-implement

- ❌ HTTP request building → Use `SimpleIncomingRequestBuilder`
- ❌ HTTP response streaming → Use `HttpResponseReader`
- ❌ Reconnection logic → Use `ReconnectingStream`
- ❌ Backoff strategy → Use `ExponentialBackoffDecider`
- ❌ TLS handshake → Use `HttpClientConnection`
- ❌ DNS resolution → Use existing DNS infrastructure

---

## 5. Parser Design

### 5.1 SseParser Structure

```rust
pub struct SseParser {
    // Buffering
    line_buffer: String,

    // Current event accumulation
    current_id: Option<String>,
    current_event: Option<String>,
    current_data: Vec<String>,
    current_retry: Option<u64>,

    // State
    last_event_id: Option<String>,
    bom_seen: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            current_id: None,
            current_event: None,
            current_data: Vec::new(),
            current_retry: None,
            last_event_id: None,
            bom_seen: false,
        }
    }

    /// Parse incoming chunk, yield complete events
    pub fn parse(&mut self, chunk: &str) -> Vec<Event> {
        let mut events = Vec::new();

        for ch in chunk.chars() {
            // Handle BOM
            if !self.bom_seen && ch == '\u{FEFF}' {
                self.bom_seen = true;
                continue;
            }

            // Accumulate line
            if ch == '\n' || ch == '\r' {
                if !self.line_buffer.is_empty() {
                    self.process_line(&mut events);
                    self.line_buffer.clear();
                }
            } else {
                self.line_buffer.push(ch);
            }
        }

        events
    }

    fn process_line(&mut self, events: &mut Vec<Event>) {
        let line = self.line_buffer.trim_end_matches('\r');

        // Empty line → dispatch event
        if line.is_empty() {
            if let Some(event) = self.dispatch_event() {
                events.push(event);
            }
            return;
        }

        // Comment line
        if line.starts_with(':') {
            events.push(Event::Comment(line[1..].trim_start().to_string()));
            return;
        }

        // Parse field
        if let Some((field, value)) = self.parse_field(line) {
            self.process_field(field, value);
        }
    }

    fn parse_field<'a>(&self, line: &'a str) -> Option<(&'a str, &'a str)> {
        if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = &line[colon_pos + 1..];
            // Strip optional leading space
            let value = value.strip_prefix(' ').unwrap_or(value);
            Some((field, value))
        } else {
            // No colon → field with empty value
            Some((line, ""))
        }
    }

    fn process_field(&mut self, field: &str, value: &str) {
        match field {
            "event" => self.current_event = Some(value.to_string()),
            "data" => self.current_data.push(value.to_string()),
            "id" => {
                // Ignore if contains null byte
                if !value.contains('\0') {
                    self.current_id = Some(value.to_string());
                }
            }
            "retry" => {
                // Parse as integer, ignore if invalid
                if let Ok(ms) = value.parse::<u64>() {
                    self.current_retry = Some(ms);
                }
            }
            _ => {} // Unknown field → ignore
        }
    }

    fn dispatch_event(&mut self) -> Option<Event> {
        // No data → no event
        if self.current_data.is_empty() {
            return None;
        }

        // Join data lines with \n
        let data = self.current_data.join("\n");

        // Update last event ID if present
        if let Some(id) = &self.current_id {
            self.last_event_id = Some(id.clone());
        }

        let event = Event::Message {
            id: self.current_id.clone(),
            event_type: self.current_event.clone(),
            data,
            retry: self.current_retry,
        };

        // Reset buffer (but keep last_event_id)
        self.current_id = None;
        self.current_event = None;
        self.current_data.clear();
        self.current_retry = None;

        Some(event)
    }

    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }
}
```

### 5.2 Event Types

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

impl Event {
    pub fn message(data: impl Into<String>) -> Self {
        Self::Message {
            id: None,
            event_type: None,
            data: data.into(),
            retry: None,
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Message { id, .. } => id.as_deref(),
            _ => None,
        }
    }

    pub fn event_type(&self) -> Option<&str> {
        match self {
            Self::Message { event_type, .. } => event_type.as_deref(),
            _ => None,
        }
    }

    pub fn data(&self) -> Option<&str> {
        match self {
            Self::Message { data, .. } => Some(data.as_str()),
            _ => None,
        }
    }
}
```

### 5.3 Parser Test Vectors

From W3C spec:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_event() {
        let mut parser = SseParser::new();
        let events = parser.parse("data: hello\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data(), Some("hello"));
        assert_eq!(events[0].event_type(), None);
    }

    #[test]
    fn parse_event_with_type() {
        let mut parser = SseParser::new();
        let events = parser.parse("event: test\ndata: hello\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), Some("test"));
        assert_eq!(events[0].data(), Some("hello"));
    }

    #[test]
    fn parse_multiline_data() {
        let mut parser = SseParser::new();
        let events = parser.parse("data: line1\ndata: line2\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data(), Some("line1\nline2"));
    }

    #[test]
    fn parse_with_id() {
        let mut parser = SseParser::new();
        let events = parser.parse("id: 42\ndata: hello\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id(), Some("42"));
        assert_eq!(parser.last_event_id(), Some("42"));
    }

    #[test]
    fn parse_comment() {
        let mut parser = SseParser::new();
        let events = parser.parse(": this is a comment\n\n");

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Event::Comment(_)));
    }

    #[test]
    fn ignore_id_with_null_byte() {
        let mut parser = SseParser::new();
        let events = parser.parse("id: bad\0id\ndata: hello\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id(), None);
    }

    #[test]
    fn parse_retry() {
        let mut parser = SseParser::new();
        let events = parser.parse("retry: 5000\ndata: hello\n\n");

        assert_eq!(events.len(), 1);
        if let Event::Message { retry, .. } = &events[0] {
            assert_eq!(*retry, Some(5000));
        }
    }

    #[test]
    fn handle_different_line_endings() {
        let mut parser = SseParser::new();

        // \n only
        let events1 = parser.parse("data: test1\n\n");
        assert_eq!(events1.len(), 1);

        // \r\n
        let events2 = parser.parse("data: test2\r\n\r\n");
        assert_eq!(events2.len(), 1);

        // \r only
        let events3 = parser.parse("data: test3\r\r");
        assert_eq!(events3.len(), 1);
    }
}
```

---

## 6. Client Implementation

### 6.1 EventSource (Blocking Iterator)

```rust
pub struct EventSource {
    url: String,
    headers: Vec<(String, String)>,
    last_event_id: Option<String>,
    auto_tracking: bool,
    max_retries: u32,
    retry_interval: Option<Duration>,
}

impl EventSource {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
            last_event_id: None,
            auto_tracking: false,
            max_retries: 10,
            retry_interval: None,
        }
    }

    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>
    ) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn with_last_event_id(mut self, id: impl Into<String>) -> Self {
        self.last_event_id = Some(id.into());
        self
    }

    pub fn with_auto_tracking(mut self, enabled: bool) -> Self {
        self.auto_tracking = enabled;
        self
    }

    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    pub fn with_retry_interval(mut self, interval: Duration) -> Self {
        self.retry_interval = Some(interval);
        self
    }

    pub fn connect(self) -> Result<EventSourceStream, EventSourceError> {
        EventSourceStream::connect(self)
    }
}

pub struct EventSourceStream {
    connection: HttpClientConnection,
    response_reader: HttpResponseReader<SimpleHttpBody, RawStream>,
    parser: SseParser,
    config: EventSourceConfig,
}

struct EventSourceConfig {
    url: String,
    headers: Vec<(String, String)>,
    auto_tracking: bool,
}

impl EventSourceStream {
    fn connect(config: EventSource) -> Result<Self, EventSourceError> {
        // Parse URL
        let uri = Uri::parse(&config.url)?;

        // Build request
        let mut request_builder = SimpleIncomingRequestBuilder::get(uri.path_and_query())
            .header(SimpleHeader::HOST, uri.host_with_port())
            .header(SimpleHeader::ACCEPT, "text/event-stream")
            .header(SimpleHeader::CACHE_CONTROL, "no-cache");

        // Add Last-Event-ID if present
        if let Some(last_id) = &config.last_event_id {
            request_builder = request_builder
                .header(SimpleHeader::custom("Last-Event-ID"), last_id);
        }

        // Add custom headers
        for (name, value) in &config.headers {
            request_builder = request_builder
                .header(SimpleHeader::custom(name), value);
        }

        let request = request_builder.build()?;

        // Connect
        let mut connection = HttpClientConnection::connect(&uri, &SystemDnsResolver, None)?;

        // Send request using RenderHttp
        let request_bytes = Http11::request(&request).http_render()?;
        for chunk in request_bytes {
            connection.write_all(&chunk?)?;
        }
        connection.flush()?;

        // Create response reader
        let response_reader = HttpResponseReader::new(
            connection.clone_stream(),
            SimpleHttpBody
        );

        Ok(Self {
            connection,
            response_reader,
            parser: SseParser::new(),
            config: EventSourceConfig {
                url: config.url,
                headers: config.headers,
                auto_tracking: config.auto_tracking,
            },
        })
    }
}

impl Iterator for EventSourceStream {
    type Item = Result<Event, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Read next response part
            match self.response_reader.next() {
                Some(Ok(IncomingResponseParts::Intro(status, _, _))) => {
                    if status != Status::Ok {
                        return Some(Err(EventSourceError::InvalidStatus(status)));
                    }
                }
                Some(Ok(IncomingResponseParts::Headers(headers))) => {
                    // Verify Content-Type
                    if let Some(content_type) = headers.get(&SimpleHeader::CONTENT_TYPE) {
                        if !content_type.iter().any(|v| v.starts_with("text/event-stream")) {
                            return Some(Err(EventSourceError::InvalidContentType));
                        }
                    }
                }
                Some(Ok(IncomingResponseParts::Body(chunk))) => {
                    // Parse SSE events from chunk
                    let chunk_str = match String::from_utf8(chunk.to_vec()) {
                        Ok(s) => s,
                        Err(e) => return Some(Err(EventSourceError::InvalidUtf8(e))),
                    };

                    let events = self.parser.parse(&chunk_str);

                    // Return first event (if any)
                    if let Some(event) = events.into_iter().next() {
                        // Update last event ID if auto-tracking
                        if self.config.auto_tracking {
                            if let Some(id) = event.id() {
                                // Store for next reconnection
                            }
                        }

                        return Some(Ok(event));
                    }
                    // No complete event yet, continue reading
                }
                Some(Ok(IncomingResponseParts::Completed(_))) => {
                    // Stream ended, signal reconnection needed
                    return Some(Ok(Event::Reconnect));
                }
                Some(Err(e)) => {
                    return Some(Err(EventSourceError::Http(e)));
                }
                None => {
                    // Connection closed
                    return Some(Ok(Event::Reconnect));
                }
            }
        }
    }
}
```

### 6.2 Error Handling

```rust
#[derive(Debug)]
pub enum EventSourceError {
    /// Invalid URL
    InvalidUrl(String),

    /// HTTP connection error
    Http(HttpClientError),

    /// Invalid HTTP status code
    InvalidStatus(Status),

    /// Invalid Content-Type (not text/event-stream)
    InvalidContentType,

    /// Invalid UTF-8 in stream
    InvalidUtf8(std::string::FromUtf8Error),

    /// Connection closed
    ConnectionClosed,

    /// Max retries exceeded
    MaxRetriesExceeded,

    /// IO error
    IoError(std::io::Error),
}

impl From<HttpClientError> for EventSourceError {
    fn from(e: HttpClientError) -> Self {
        Self::Http(e)
    }
}

impl From<std::io::Error> for EventSourceError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
```

---

## 7. Server Implementation

### 7.1 EventWriter

```rust
pub struct EventWriter<W: Write> {
    writer: W,
}

impl<W: Write> EventWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn send(&mut self, event: SseEvent) -> Result<(), std::io::Error> {
        // Write id field
        if let Some(id) = &event.id {
            write!(self.writer, "id: {}\n", id)?;
        }

        // Write event field
        if let Some(event_type) = &event.event_type {
            write!(self.writer, "event: {}\n", event_type)?;
        }

        // Write data fields (one per line)
        for line in &event.data {
            write!(self.writer, "data: {}\n", line)?;
        }

        // Write retry field
        if let Some(retry) = event.retry {
            write!(self.writer, "retry: {}\n", retry)?;
        }

        // Write empty line to dispatch event
        write!(self.writer, "\n")?;

        // Flush to ensure immediate delivery
        self.writer.flush()
    }

    pub fn comment(&mut self, text: &str) -> Result<(), std::io::Error> {
        write!(self.writer, ": {}\n\n", text)?;
        self.writer.flush()
    }
}
```

### 7.2 SseEvent Builder

```rust
pub struct SseEvent {
    id: Option<String>,
    event_type: Option<String>,
    data: Vec<String>,
    retry: Option<u64>,
}

impl SseEvent {
    pub fn message(data: impl Into<String>) -> Self {
        let data_str = data.into();
        let lines = data_str.lines().map(|s| s.to_string()).collect();

        Self {
            id: None,
            event_type: None,
            data: lines,
            retry: None,
        }
    }

    pub fn new() -> SseEventBuilder {
        SseEventBuilder::default()
    }

    pub fn retry(milliseconds: u64) -> Self {
        Self {
            id: None,
            event_type: None,
            data: Vec::new(),
            retry: Some(milliseconds),
        }
    }
}

#[derive(Default)]
pub struct SseEventBuilder {
    id: Option<String>,
    event_type: Option<String>,
    data: Vec<String>,
}

impl SseEventBuilder {
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn event(mut self, event_type: impl Into<String>) -> Self {
        self.event_type = Some(event_type.into());
        self
    }

    pub fn data(mut self, data: impl Into<String>) -> Self {
        let data_str = data.into();
        let lines: Vec<String> = data_str.lines().map(|s| s.to_string()).collect();
        self.data.extend(lines);
        self
    }

    pub fn build(self) -> SseEvent {
        SseEvent {
            id: self.id,
            event_type: self.event_type,
            data: self.data,
            retry: None,
        }
    }
}
```

### 7.3 SSE Response Helper

```rust
pub struct SseResponse;

impl SseResponse {
    pub fn new() -> SseResponseBuilder {
        SseResponseBuilder::default()
    }
}

#[derive(Default)]
pub struct SseResponseBuilder {
    headers: Vec<(String, String)>,
}

impl SseResponseBuilder {
    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>
    ) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn build(self) -> SimpleIncomingResponse {
        let mut response = SimpleIncomingResponse::new();
        response.proto = Proto::Http11;
        response.status = Status::Ok;

        // Required SSE headers
        response.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["text/event-stream".to_string()]
        );
        response.headers.insert(
            SimpleHeader::CACHE_CONTROL,
            vec!["no-cache".to_string()]
        );
        response.headers.insert(
            SimpleHeader::CONNECTION,
            vec!["keep-alive".to_string()]
        );

        // Add custom headers
        for (name, value) in self.headers {
            response.headers.insert(
                SimpleHeader::custom(name),
                vec![value]
            );
        }

        response
    }
}
```

---

## 8. Reconnection Strategy

### 8.1 ReconnectingEventSource

```rust
pub struct ReconnectingEventSource {
    config: EventSourceConfig,
    reconnection_stream: ReconnectingStream,
    current_stream: Option<EventSourceStream>,
    parser: SseParser,
}

impl ReconnectingEventSource {
    pub fn new(url: impl Into<String>) -> Result<Self, EventSourceError> {
        let url_string = url.into();
        let endpoint = ClientEndpoint::from_url(&url_string)?;

        Ok(Self {
            config: EventSourceConfig {
                url: url_string,
                headers: Vec::new(),
                auto_tracking: true,
            },
            reconnection_stream: ReconnectingStream::from_endpoint(endpoint),
            current_stream: None,
            parser: SseParser::new(),
        })
    }

    pub fn with_retry_interval(mut self, duration: Duration) -> Self {
        self.reconnection_stream = ReconnectingStream::with_connection_timeout(
            /* endpoint */,
            duration
        );
        self
    }

    pub fn with_max_retries(mut self, max: u32) -> Self {
        // Configure max_retries on reconnection_stream
        self
    }
}

impl Iterator for ReconnectingEventSource {
    type Item = Result<Event, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If we have a current stream, try to read from it
            if let Some(stream) = &mut self.current_stream {
                match stream.next() {
                    Some(Ok(Event::Reconnect)) => {
                        // Connection lost, trigger reconnection
                        self.current_stream = None;
                        // Fall through to reconnection logic
                    }
                    other => return other,
                }
            }

            // Need to (re)connect
            match self.reconnection_stream.next() {
                Some(Ok(ReconnectionStatus::Ready(raw_stream))) => {
                    // Connected! Create new EventSourceStream
                    match self.create_stream(*raw_stream) {
                        Ok(stream) => {
                            self.current_stream = Some(stream);
                            // Continue to read from new stream
                        }
                        Err(e) => {
                            return Some(Err(e));
                        }
                    }
                }
                Some(Ok(ReconnectionStatus::Waiting(duration))) => {
                    // Waiting for backoff
                    return Some(Ok(Event::Reconnect));
                }
                Some(Ok(ReconnectionStatus::NoMoreWaiting)) => {
                    // Ready to reconnect now, loop
                    continue;
                }
                Some(Err(e)) => {
                    return Some(Err(EventSourceError::Reconnection(e)));
                }
                None => {
                    // Reconnection exhausted
                    return None;
                }
            }
        }
    }

    fn create_stream(
        &self,
        raw_stream: RawStream
    ) -> Result<EventSourceStream, EventSourceError> {
        // Build request with Last-Event-ID
        let mut request_builder = SimpleIncomingRequestBuilder::get(/* path */)
            .header(SimpleHeader::ACCEPT, "text/event-stream")
            .header(SimpleHeader::CACHE_CONTROL, "no-cache");

        // Add Last-Event-ID from parser
        if let Some(last_id) = self.parser.last_event_id() {
            request_builder = request_builder
                .header(SimpleHeader::custom("Last-Event-ID"), last_id);
        }

        // Send request and create stream
        // ...
    }
}
```

### 8.2 Server Retry Field Handling

```rust
impl EventSourceStream {
    fn handle_retry_field(&mut self, retry_ms: u64) {
        // Update reconnection_stream with server-suggested retry interval
        // This overrides the exponential backoff temporarily

        let retry_duration = Duration::from_millis(retry_ms);

        // Store for next reconnection
        self.server_retry = Some(retry_duration);
    }
}
```

---

## 9. TaskIterator Integration

### 9.1 EventSourceTask

```rust
pub struct EventSourceTask {
    state: Option<EventSourceState>,
}

enum EventSourceState {
    Init(EventSourceConfig),
    Connecting(HttpConnectTask),
    Reading(EventSourceStream),
    Reconnecting {
        duration: Duration,
        retry_state: RetryState,
    },
    Closed,
}

pub enum EventSourceProgress {
    Connecting,
    Reading,
    Reconnecting(Duration),
}

impl EventSourceTask {
    pub fn connect(url: impl Into<String>) -> Result<Self, EventSourceError> {
        Ok(Self {
            state: Some(EventSourceState::Init(EventSourceConfig {
                url: url.into(),
                headers: Vec::new(),
                last_event_id: None,
            })),
        })
    }
}

impl TaskIterator for EventSourceTask {
    type Ready = Event;
    type Pending = EventSourceProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Event, EventSourceProgress, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            EventSourceState::Init(config) => {
                // Spawn HTTP connection task
                let connect_task = HttpConnectTask::new(&config.url);
                self.state = Some(EventSourceState::Connecting(connect_task));
                Some(TaskStatus::Pending(EventSourceProgress::Connecting))
            }

            EventSourceState::Connecting(mut task) => {
                match task.next() {
                    Some(TaskStatus::Ready(connection)) => {
                        // Connected, create EventSourceStream
                        let stream = EventSourceStream::from_connection(connection);
                        self.state = Some(EventSourceState::Reading(stream));
                        Some(TaskStatus::Pending(EventSourceProgress::Reading))
                    }
                    Some(TaskStatus::Pending(progress)) => {
                        self.state = Some(EventSourceState::Connecting(task));
                        Some(TaskStatus::Pending(EventSourceProgress::Connecting))
                    }
                    Some(TaskStatus::Delayed(duration)) => {
                        self.state = Some(EventSourceState::Connecting(task));
                        Some(TaskStatus::Delayed(duration))
                    }
                    None => {
                        // Connection failed
                        self.state = Some(EventSourceState::Closed);
                        None
                    }
                    _ => {
                        self.state = Some(EventSourceState::Connecting(task));
                        Some(TaskStatus::Pending(EventSourceProgress::Connecting))
                    }
                }
            }

            EventSourceState::Reading(mut stream) => {
                match stream.next() {
                    Some(Ok(event)) => {
                        // Got an event
                        self.state = Some(EventSourceState::Reading(stream));
                        Some(TaskStatus::Ready(event))
                    }
                    Some(Err(e)) => {
                        // Error, trigger reconnection
                        self.state = Some(EventSourceState::Reconnecting {
                            duration: Duration::from_secs(3),
                            retry_state: RetryState::new(0, 10, None),
                        });
                        Some(TaskStatus::Delayed(Duration::from_secs(3)))
                    }
                    None => {
                        // Stream ended
                        self.state = Some(EventSourceState::Closed);
                        None
                    }
                }
            }

            EventSourceState::Reconnecting { duration, retry_state } => {
                // Transition back to Init
                // TODO: carry config through states
                Some(TaskStatus::Delayed(duration))
            }

            EventSourceState::Closed => None,
        }
    }
}
```

---

## 10. Technical Design Details

### 10.1 File Structure

```
backends/foundation_core/src/wire/event_source/
├── mod.rs              # Platform-specific exports
├── core.rs             # Event and SseEvent types
├── parser.rs           # SseParser implementation
├── client.rs           # EventSource and EventSourceStream
├── writer.rs           # EventWriter and SseEventBuilder
├── reconnecting.rs     # ReconnectingEventSource
├── task.rs             # EventSourceTask (TaskIterator)
├── error.rs            # EventSourceError
├── no_wasm.rs          # Native implementation exports
└── wasm.rs             # WASM implementation exports (future)
```

### 10.2 Module Exports

**`mod.rs`**:
```rust
extern crate url;

mod core;
mod parser;
mod client;
mod writer;
mod error;

pub use core::*;
pub use parser::*;
pub use client::*;
pub use writer::*;
pub use error::*;

#[cfg(not(target_arch = "wasm32"))]
mod no_wasm;

#[cfg(not(target_arch = "wasm32"))]
mod reconnecting;

#[cfg(not(target_arch = "wasm32"))]
mod task;

#[cfg(not(target_arch = "wasm32"))]
pub use no_wasm::*;

#[cfg(target_arch = "wasm32"))]
mod wasm;

#[cfg(target_arch = "wasm32"))]
pub use wasm::*;
```

### 10.3 Dependencies

**No new external dependencies needed!**

All required functionality exists:
- HTTP: `simple_http` module
- Reconnection: `http_stream::ReconnectingStream`
- Retry: `retries::ExponentialBackoffDecider`
- TaskIterator: `valtron` module
- I/O: `io::ioutils`
- TLS: `netcap` with ssl features

### 10.4 Testing Strategy

**Unit Tests**:

1. **Parser tests** (`parser.rs`):
   - Parse single event
   - Parse multi-line data
   - Parse all field types (event, data, id, retry)
   - Handle different line endings (\n, \r, \r\n)
   - Handle UTF-8 BOM
   - Handle comments
   - Ignore invalid retry values
   - Ignore IDs with null bytes
   - Test vectors from W3C spec

2. **Event builder tests** (`writer.rs`):
   - Build simple event
   - Build event with all fields
   - Multi-line data formatting
   - Comment formatting

3. **Writer tests** (`writer.rs`):
   - Format events correctly
   - Handle multi-line data
   - Send comments
   - Verify flush after each event

**Integration Tests**:

1. **Client tests**:
   - Connect to SSE endpoint
   - Receive events
   - Parse all event types
   - Handle reconnection
   - Track Last-Event-ID

2. **Server tests**:
   - Send events to client
   - Format events correctly
   - Handle keep-alive comments

3. **Public SSE servers** (network tests, marked `#[ignore]`):
   - Connect to `https://sse.dev/test`
   - Verify event reception
   - Test reconnection handling

### 10.5 Performance Considerations

**Buffering**:
- Use `SharedByteBufferStream` for I/O buffering
- Parser accumulates complete events before returning
- Minimize allocations (reuse buffers where possible)

**String Operations**:
- `line_buffer: String` - reused for each line
- `current_data: Vec<String>` - collected data lines
- Join operation only on event dispatch

**Reconnection**:
- Exponential backoff prevents server overload
- Jitter prevents thundering herd
- Connection reuse via pool (future optimization)

---

## Appendix A: Implementation Checklist

### Phase 1: Core SSE (Blocking)

- [ ] `core.rs` - Event and SseEvent types
- [ ] `parser.rs` - SseParser with W3C spec compliance
- [ ] `client.rs` - EventSource and EventSourceStream
- [ ] `writer.rs` - EventWriter and SseEventBuilder
- [ ] `error.rs` - EventSourceError
- [ ] Unit tests for parser (all test vectors)
- [ ] Unit tests for writer
- [ ] Integration test with simple server

### Phase 2: Reconnection

- [ ] `reconnecting.rs` - ReconnectingEventSource
- [ ] Last-Event-ID header management
- [ ] Server retry field handling
- [ ] Integration test for reconnection

### Phase 3: TaskIterator

- [ ] `task.rs` - EventSourceTask
- [ ] Non-blocking state machine
- [ ] Integration with valtron executor
- [ ] Performance benchmarks

### Phase 4: Documentation

- [ ] API documentation (doc comments)
- [ ] Usage examples
- [ ] Integration guide
- [ ] Update wire/mod.rs exports

---

## Appendix B: References

- [W3C Server-Sent Events Specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)
- [MDN EventSource](https://developer.mozilla.org/en-US/docs/Web/API/EventSource)
- [RFC 2616 - HTTP/1.1](https://tools.ietf.org/html/rfc2616)
- [SSE Test Server](https://sse.dev/test)

---

*Created: 2026-03-03*
*Last Updated: 2026-03-03*
*Version: 1.0*
