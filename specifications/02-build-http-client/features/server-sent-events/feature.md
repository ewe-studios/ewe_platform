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
  uncompleted: 7
  total: 7
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

### Architectural Principle: TaskIterator-Only Design

**CRITICAL:** All SSE client implementations MUST use the TaskIterator pattern EXCLUSIVELY. There is NO blocking Iterator wrapper. The TaskIterator IS the API.

**Key Pattern Points:**
1. **State carries ALL data** - State enum variants hold `Option<Box<...>>` with all data needed
2. **No loops in `next()`** - Each call to `next()` does ONE step and transitions state
3. **`Option<State>` wrapper** - Task wraps state in `Option` so it can be set to `None` when done
4. **Data moved between states** - Data is extracted from one state variant and moved to the next
5. **No blocking wrappers** - Consumers use TaskIterator directly with valtron executor

**Reference Pattern from `send_request.rs`:**
```rust
pub struct SendRequestTask<R>(Option<SendRequestState<R>>)
where
    R: DnsResolver + Send + 'static;

pub enum SendRequestState<R> {
    Init(Option<Box<SendRequest<R>>>),
    Connecting(DrivenRecvIterator<GetHttpRequestRedirectTask<R>>),
    Reading(DrivenRecvIterator<GetRequestIntroTask>),
    SkipReading(Box<Option<RequestIntro>>),
    Done,
}

impl<R> TaskIterator for SendRequestTask<R> {
    type Ready = RequestIntro;
    type Pending = HttpRequestPending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {  // Take state, return None if already None
            SendRequestState::Init(mut inner) => match inner.take() {
                // Extract data, transition state, return TaskStatus
                // NO LOOPS - each call does ONE step
            }
            SendRequestState::Connecting(mut recv_iter) => {
                let next_value = recv_iter.next();  // One step
                self.0 = Some(SendRequestState::Connecting(recv_iter));  // Restore state
                // Process one value, transition
            }
            // ... each state handles ONE step
        }
    }
}
```

### Client-Side SSE Consumption (TaskIterator - ONLY API)

The ONLY API for SSE consumption uses TaskIterator with valtron executor:

```rust
use foundation_core::wire::event_source::EventSourceTask;
use foundation_core::valtron::{TaskIterator, TaskStatus};

// Create SSE task - this is the complete API
let mut task = EventSourceTask::connect("https://api.example.com/events")?
    .with_header("Authorization", "Bearer token123")
    .with_last_event_id("42"); // Resume from last event

// Use with valtron executor
loop {
    match task.next() {
        Some(TaskStatus::Ready(result)) => {
            match result {
                Ok(Event::Message { id, event_type, data, retry }) => {
                    println!("Event: {}", event_type.unwrap_or("message"));
                    println!("Data: {}", data);
                }
                Ok(Event::Comment(comment)) => {
                    println!("Keep-alive: {}", comment);
                }
                Err(e) => eprintln!("SSE Error: {:?}", e),
            }
        }
        Some(TaskStatus::Pending(progress)) => {
            // Task is working - do other work or wait
        }
        Some(TaskStatus::Delayed(duration)) => {
            // Reconnection backoff - executor handles delay
        }
        Some(TaskStatus::Spawn(action)) => {
            // Executor spawns sub-task (e.g., connection task)
            executor.spawn(action)?;
        }
        None => {
            // Task complete (stream closed or error)
            break;
        }
    }
}
```

**NO BLOCKING WRAPPER:** The TaskIterator is the API. Consumers integrate with valtron executor.

### Event Filtering

Filter events by type via TaskIterator callbacks:

```rust
use foundation_core::wire::event_source::EventSourceTask;
use foundation_core::valtron::TaskStatus;

// Create task with event filtering callbacks
let mut task = EventSourceTask::connect(url)?
    .on_event("user_joined", |data| {
        println!("User joined: {}", data);
    })
    .on_event("user_left", |data| {
        println!("User left: {}", data);
    });

// TaskIterator yields events - callbacks execute during parsing
loop {
    match task.next() {
        Some(TaskStatus::Ready(result)) => {
            match result {
                // Filtered events already processed by callbacks
                Ok(Event::Message { event_type, data, .. }) => {
                    // Callback already executed for filtered types
                    println!("Received {}: {}", event_type.unwrap_or("message"), data);
                }
                _ => {}
            }
        }
        _ => {}
    }
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

Client automatically reconnects on disconnect via TaskIterator state machine:

```rust
use foundation_core::wire::event_source::ReconnectingEventSourceTask;
use foundation_core::valtron::TaskStatus;
use std::time::Duration;

let mut task = ReconnectingEventSourceTask::connect(url)?
    .with_max_retries(10);

// TaskIterator handles reconnection state machine
loop {
    match task.next() {
        Some(TaskStatus::Ready(result)) => {
            match result {
                Ok(Event::Message { data, .. }) => {
                    println!("Data: {}", data);
                }
                Ok(Event::Reconnect) => {
                    println!("Connection lost, reconnecting...");
                }
                Err(e) => eprintln!("SSE Error: {:?}", e),
            }
        }
        Some(TaskStatus::Delayed(duration)) => {
            // Executor handles backoff delay
            println!("Reconnecting in {:?}", duration);
        }
        Some(TaskStatus::Pending(progress)) => {
            // Task working on reconnection
        }
        None => {
            // Max retries exceeded or stream closed permanently
            break;
        }
    }
}
```

**NO WRAPPING:** `ReconnectingEventSourceTask` IS the API - no blocking wrapper.

### Last-Event-ID Tracking

Client tracks last event ID for resume via TaskIterator:

```rust
use foundation_core::wire::event_source::EventSourceTask;

// Server sends events with IDs
writer.send(SseEvent::new().id("100").data("Event 100"))?;
writer.send(SseEvent::new().id("101").data("Event 101"))?;

// Client disconnects after ID 100

// Client reconnects with Last-Event-ID header
// Server can resume from ID 101
let mut task = EventSourceTask::connect(url)?
    .with_last_event_id("100");

// Or use auto-tracking
let mut task = EventSourceTask::connect(url)?
    .with_auto_tracking(true);  // Automatically sends Last-Event-ID on reconnect

// Task tracks last event ID internally
// On reconnect, automatically includes Last-Event-ID header
```

## Implementation Phases

**ARCHITECTURAL PRINCIPLE:** TaskIterator-ONLY for core logic. Blocking wrappers use `DrivenSendTaskIterator` to handle execution externally - NO loops in TaskIterator `next()`.

**Key Pattern Points** (from `simple_http/client/tasks/*.rs` and `valtron/executors/task_iters.rs`):
1. **TaskIterator: Pure state machine** - NO loops in `next()`, ONE step per call
2. **State carries ALL data** - State enum variants hold `Option<Box<...>>`
3. **`Option<State>` wrapper** - Task wraps state in `Option` for termination
4. **Data moved between states** - Extracted from one variant, moved to next
5. **Blocking wrappers use DrivenSendTaskIterator** - Wrapper handles execution driving, not TaskIterator

**DrivenSendTaskIterator Pattern** (blocking wrapper that extracts Ready values):
```rust
// Wrapper that drives execution and provides Iterator interface
pub struct DrivenSendTaskIterator<T>(Option<T>)
where
    T: TaskIterator + Send + 'static;

impl<T> Iterator for DrivenSendTaskIterator<T> {
    type Item = TaskStatus<T::Ready, T::Pending, T::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            // Drive execution externally (no loop in TaskIterator!)
            run_until_next_state();

            // Get ONE result from TaskIterator
            let next_value = task_iterator.next();

            // Restore task for next call
            if next_value.is_some() {
                self.0.replace(task_iterator);
            }
            next_value
        } else {
            None
        }
    }
}
```

**Ready-only Iterator Mapper** (extracts only Ready values, ignores Pending/Spawn/Delayed):
```rust
// Mapper that filters TaskStatus to extract only Ready values
pub struct ReadyMapper<Done, Pending, Action>;

impl TaskStatusMapper for ReadyMapper {
    type Done = Done;
    type Pending = Pending;
    type Action = Action;

    fn map(&self, status: Option<TaskStatus<Done, Pending, Action>>) -> Option<TaskStatus<...>> {
        match status {
            Some(TaskStatus::Ready(ready)) => Some(TaskStatus::Ready(ready)),
            Some(TaskStatus::Pending(_)) => None, // Ignore pending
            Some(TaskStatus::Spawn(_)) => None,   // Spawn handled by executor
            Some(TaskStatus::Delayed(_)) => None, // Ignore delayed
            Some(TaskStatus::Init) => None,
            None => None,
        }
    }
}

// Usage: Create blocking iterator that yields only Ready values
let task = EventSourceTask::connect(url)?;
let driven = drive_iterator(task);  // DrivenSendTaskIterator
let ready_only = driven.map(ReadyMapper);  // Only yields Event results
```

### Phase 1: Core SSE Protocol with TaskIterator

**Duration**: 1-2 weeks
**Goal**: Working SSE client and server using TaskIterator pattern

**File structure**:
```
backends/foundation_core/src/wire/event_source/
├── mod.rs              # Public API and re-exports
├── core.rs             # Event and SseEvent types
├── parser.rs           # SSE message parser (internal plain iterator)
├── task.rs             # EventSourceTask (TaskIterator - core API)
├── iter.rs             # Blocking iterators (DrivenSendTaskIterator wrappers)
├── writer.rs           # EventWriter (server-side)
└── error.rs            # EventSourceError
```

**NOTE**: Blocking wrappers in `iter.rs` use `DrivenSendTaskIterator` - TaskIterator remains pure (no loops).

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
   - **Note**: This is an internal plain iterator, wrapped by TaskIterator

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

3. **EventSource Task** (`task.rs`) - **Core TaskIterator**
   - Follow pattern from `simple_http/client/tasks/send_request.rs`
   - State enum with `Option<Box<...>>` for data carrying
   - State machine: Init → Connecting → Reading → Closed
   - NO loops in `next()` - ONE step per call

   ```rust
   use crate::valtron::{TaskIterator, TaskStatus, BoxedSendExecutionAction, DrivenRecvIterator};

   // Task wraps state in Option for termination
   pub struct EventSourceTask(Option<EventSourceState>);

   // State carries ALL data - follows send_request.rs pattern
   enum EventSourceState {
       Init(Option<Box<EventSourceConfig>>),
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
           // Pattern: take state, process ONE step, restore state
           match self.0.take()? {
               EventSourceState::Init(mut config_opt) => match config_opt.take() {
                   Some(config) => {
                       // Spawn connection sub-task
                       let (action, recv) = spawn_connect_task(&config);
                       self.0 = Some(EventSourceState::Connecting(recv));
                       Some(TaskStatus::Spawn(action))
                   }
                   None => {
                       self.0 = Some(EventSourceState::Closed);
                       None
                   }
               }
               EventSourceState::Connecting(mut recv) => {
                   // ONE step: call next() on receiver
                   let result = recv.next();
                   self.0 = Some(EventSourceState::Connecting(recv));
                   match result {
                       Some(TaskStatus::Ready(Ok(conn))) => {
                           let reader = EventSourceStreamReader::new(conn);
                           self.0 = Some(EventSourceState::Reading(reader));
                           Some(TaskStatus::Pending(EventSourcePending::Reading))
                       }
                       // ... handle other cases - ONE step only
                   }
               }
               EventSourceState::Reading(mut reader) => {
                   // ONE step: read ONE event from stream
                   match reader.next_event() {
                       Ok(event) => {
                           self.0 = Some(EventSourceState::Reading(reader));
                           Some(TaskStatus::Ready(Ok(event)))
                       }
                       Err(EventSourceError::ConnectionClosed) => {
                           self.0 = Some(EventSourceState::Closed);
                           None  // Task done
                       }
                       Err(e) => {
                           self.0 = Some(EventSourceState::Reading(reader));
                           Some(TaskStatus::Ready(Err(e)))
                       }
                   }
               }
               EventSourceState::Closed => None,
           }
       }
   }
   ```

4. **Blocking Iterator Wrapper** (`iter.rs`) - **Uses DrivenSendTaskIterator**
   - Wraps `EventSourceTask` with `DrivenSendTaskIterator`
   - Provides simple `Iterator` interface for blocking use cases
   - NO loops in TaskIterator - execution driving handled by wrapper

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
           // TaskIterator.next() is called ONCE per Iterator.next()
           // NO LOOPS in either iterator
           match self.driven.next() {
               Some(TaskStatus::Ready(result)) => Some(result),
               Some(TaskStatus::Delayed(_)) => {
                   // Reconnection backoff - could sleep or return None
                   None
               }
               Some(TaskStatus::Pending(_)) => {
                   // Task working - continue driving
                   self.next()  // Tail recursion, not a loop in TaskIterator
               }
               Some(TaskStatus::Spawn(_)) => {
                   // Spawn handled by executor automatically
                   self.next()
               }
               Some(TaskStatus::Init) => self.next(),
               None => None,
           }
       }
   }
   ```

   **Alternative: Ready-only mapper** (filters to only Ready values):
   ```rust
   use crate::valtron::{TaskStatusMapper, ReadyConsumingIter};

   // Mapper that extracts only Ready values
   pub struct EventReadyMapper;

   impl TaskStatusMapper<Result<Event, EventSourceError>, EventSourcePending, BoxedSendExecutionAction>
       for EventReadyMapper
   {
       fn map(
           &self,
           status: Option<TaskStatus<Result<Event, EventSourceError>, EventSourcePending, BoxedSendExecutionAction>>,
       ) -> Option<TaskStatus<Result<Event, EventSourceError>, EventSourcePending, BoxedSendExecutionAction>>
       {
           match status {
               Some(TaskStatus::Ready(event_result)) => Some(TaskStatus::Ready(event_result)),
               // Ignore Pending, Spawn, Delayed - let executor handle them
               _ => None,
           }
       }
   }

   // Usage with ReadyConsumingIter - only yields Ready values
   let task = EventSourceTask::connect(url)?;
   let mapper = EventReadyMapper;
   let ready_iter = ReadyConsumingIter::new(task, vec![mapper], channel);
   ```

5. **EventSource Stream Reader** (internal to `task.rs`)
   - Wraps `HttpResponseReader` with SSE parsing
   - Reads ONE event per `next_event()` call - NO LOOPS
   - Uses `SseParser` internally

   ```rust
   struct EventSourceStreamReader {
       response_reader: HttpResponseReader<SimpleHttpBody, RawStream>,
       parser: SseParser,
       pending_events: Vec<Event>,
   }

   impl EventSourceStreamReader {
       fn new(connection: HttpClientConnection) -> Self;
       fn next_event(&mut self) -> Result<Event, EventSourceError>;  // ONE event, NO LOOPS
   }
   ```

6. **SSE Server Writer** (`writer.rs`)
   - Format events according to SSE spec
   - Handle multi-line data (prefix each line with `data: `)
   - Flush after each event

   ```rust
   pub struct EventWriter<W: Write> {
       writer: W,
   }

   impl<W: Write> EventWriter<W> {
       pub fn new(writer: W) -> Self;
       pub fn send(&mut self, event: SseEvent) -> Result<(), std::io::Error>;
       pub fn comment(&mut self, comment: &str) -> Result<(), std::io::Error>;
   }
   ```

6. **SSE Response Helper**
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
       pub fn with_header(self, name: impl Into<String>, value: impl Into<String>) -> Self;
       pub fn build(self) -> SimpleIncomingResponse;
   }
   ```

7. **Error Handling** (`error.rs`)
   - Define `EventSourceError` enum
   - Conversions from I/O, HTTP, and parsing errors

   ```rust
   #[derive(Debug)]
   pub enum EventSourceError {
       Http(HttpClientError),
       ParseError(String),
       ConnectionClosed,
       InvalidRetry(String),
       ReadError(std::io::Error),
   }
   ```

**Success Criteria**:
- [ ] `EventSourceTask` implements `TaskIterator` correctly
- [ ] State machine follows valtron pattern (Init → Connecting → Reading → Closed)
- [ ] State carries ALL data between transitions (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` - each call does ONE step
- [ ] `TaskStatus` variants used correctly (Ready, Pending, Delayed, Spawn)
- [ ] `SseParser` correctly parses all SSE field types
- [ ] `SseParser` handles multi-line data correctly
- [ ] `SseParser` handles all line ending types (`\n`, `\r`, `\r\n`)
- [ ] `EventWriter` formats events correctly
- [ ] `SseResponse` builds correct HTTP response headers
- [ ] `DrivenSendTaskIterator` wrapper works for blocking use cases
- [ ] `EventSourceIterator` (blocking wrapper) yields events correctly
- [ ] Can consume events from public SSE server via TaskIterator
- [ ] Can consume events from public SSE server via blocking Iterator
- [ ] Can send events to connected clients
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

### Phase 2: ReconnectingEventSourceTask with Backoff

**Duration**: 1 week
**Goal**: Automatic reconnection with backoff using TaskIterator

**File structure** (additions):
```
backends/foundation_core/src/wire/event_source/
└── reconnecting_task.rs  # ReconnectingEventSourceTask (TaskIterator)
```

**Tasks**:

1. **ReconnectingEventSourceTask** (`reconnecting_task.rs`)
   - Wrap `EventSourceTask` with reconnection logic
   - Use existing `http_stream::ReconnectingStream` pattern
   - Exponential backoff using existing `ExponentialBackoffDecider`
   - Respect server `retry:` field
   - Auto-send `Last-Event-ID` header on reconnect
   - Follow valtron TaskIterator pattern

   ```rust
   use crate::valtron::{TaskIterator, TaskStatus, BoxedSendExecutionAction};

   pub struct ReconnectingEventSourceTask {
       state: ReconnectingEventSourceState,
       max_retries: u32,
       backoff: ExponentialBackoffDecider,
   }

   enum ReconnectingEventSourceState {
       Init(EventSourceConfig),
       Connecting(EventSourceTask),
       Reading(EventSourceTask),
       Reconnecting(Duration),
       Exhausted,
   }

   impl TaskIterator for ReconnectingEventSourceTask {
       type Ready = Result<Event, EventSourceError>;
       type Pending = EventSourcePending;
       type Spawner = BoxedSendExecutionAction;

       fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
           // Handle reconnection state machine
           // On connection loss: transition to Reconnecting state
           // On reconnect: send Last-Event-ID header
           // Respect backoff duration
       }
   }
   ```

**NOTE**: NO blocking wrapper - TaskIterator IS the API.

**Success Criteria**:
- [ ] `ReconnectingEventSourceTask` implements `TaskIterator` correctly
- [ ] State carries ALL data (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` - each call does ONE step
- [ ] Auto-reconnects on connection loss via TaskIterator state machine
- [ ] Respects server `retry:` field
- [ ] Sends `Last-Event-ID` on reconnect
- [ ] Exponential backoff works via `ExponentialBackoffDecider`
- [ ] Max retries honored

### Phase 3: Advanced Features and Executor Integration

**Duration**: 1 week
**Goal**: Production-ready SSE support with full valtron executor integration

**File structure** (additions):
```
backends/foundation_core/src/wire/event_source/
└── filtering.rs        # Event filtering with callbacks
```

**Tasks**:

1. **Event Filtering**
   - Filter events by type with callback support
   - Integrate with `EventSourceTask` and `ReconnectingEventSourceTask`

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
- Full integration with valtron executors

## Success Criteria

**Phase 1 (Core with TaskIterator)**:
- [ ] `event_source/` module exists and compiles
- [ ] `EventSourceTask` implements `TaskIterator` trait correctly
- [ ] State machine follows valtron pattern (Init → Connecting → Reading → Closed)
- [ ] `TaskStatus` variants used correctly (Ready, Pending, Delayed, Spawn)
- [ ] `SseParser` correctly parses all SSE field types
- [ ] `SseParser` handles multi-line data correctly
- [ ] `SseParser` handles all line ending types (`\n`, `\r`, `\r\n`)
- [ ] `EventWriter` formats events correctly
- [ ] `SseResponse` builds correct HTTP response headers
- [ ] Can consume events from public SSE server via TaskIterator
- [ ] Can send events to connected clients
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

**Phase 2 (Reconnection with TaskIterator)**:
- [ ] `ReconnectingEventSourceTask` implements `TaskIterator` trait
- [ ] State carries ALL data (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` - each call does ONE step
- [ ] Auto-reconnects on connection loss via TaskIterator state machine
- [ ] Respects server `retry:` field
- [ ] Sends `Last-Event-ID` on reconnect
- [ ] Exponential backoff works via `ExponentialBackoffDecider`
- [ ] Max retries honored

**Phase 3 (Advanced Features)**:
- [ ] Event filtering works with TaskIterator integration
- [ ] Compression support works
- [ ] Full integration with valtron executors
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

### CRITICAL: TaskIterator-ONLY with DrivenSendTaskIterator Wrappers

**MANDATORY:** All SSE client implementations MUST follow the TaskIterator-only design pattern with `DrivenSendTaskIterator` for blocking wrappers:

1. **TaskIterator: Pure state machine** - NO loops in `next()`, ONE step per call
2. **State carries ALL data** - Use `Option<State>` wrapper, state variants hold `Option<Box<...>>`
3. **Blocking wrappers use DrivenSendTaskIterator** - Execution driving happens in wrapper, NOT in TaskIterator
4. **Data moved between states** - Extract from one variant, move to next

**DO NOT:**
- Use loops inside `next()` in TaskIterator - state machine must be step-wise
- Store state without `Option` wrapper (task must be able to terminate)
- Put execution driving logic in TaskIterator - that's what DrivenSendTaskIterator is for
- Ignore valtron state machine patterns

**DO:**
- Follow `send_request.rs` pattern exactly for TaskIterator
- Wrap task state in `Option<...>`
- Carry data in state variants using `Option<Box<...>>`
- Return `None` from `next()` when task is done
- Use `TaskStatus` variants correctly (Ready, Pending, Delayed, Spawn)
- Use `DrivenSendTaskIterator` for blocking Iterator wrappers
- Use `TaskStatusMapper` to filter/transform TaskStatus values

### Pattern Summary

```
TaskIterator (EventSourceTask)
    ↓ (pure state machine, ONE step per next())
DrivenSendTaskIterator (wraps TaskIterator)
    ↓ (calls run_until_next_state() then task.next())
Iterator::next() -> TaskStatus<Ready, Pending, Spawner>
    ↓ (optional: map/filter with TaskStatusMapper)
Ready values only (for blocking Iterator API)
```

### TaskIterator Pattern Reference

Study these existing implementations:
- `wire/simple_http/client/tasks/send_request.rs` - `SendRequestTask` state machine
- `wire/simple_http/client/tasks/request_stream.rs` - `GetHttpRequestStreamTask`
- `wire/simple_http/client/tasks/state.rs` - State machine patterns
- `valtron/executors/task_iters.rs` - `DrivenSendTaskIterator` implementation
- `valtron/executors/drivers.rs` - `ReadyConsumingIter` for Ready-only extraction

**Key Pattern:**
```rust
pub struct EventSourceTask {
    state: EventSourceState,  // Option<State> for termination
}

enum EventSourceState {
    Init(Config),
    Connecting(DrivenRecvIterator<...>),
    Reading(Stream),
    Reconnecting(Duration),
    Closed,
}

impl TaskIterator for EventSourceTask {
    type Ready = Result<Event, EventSourceError>;
    type Pending = EventSourcePending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state.take()? {
            EventSourceState::Init(...) => {
                // Transition to Connecting
                // Return TaskStatus::Pending or TaskStatus::Spawn
            }
            EventSourceState::Connecting(...) => {
                // Handle connection result
                // Transition to Reading
            }
            EventSourceState::Reading(...) => {
                // Read events, yield via TaskStatus::Ready
            }
            EventSourceState::Closed => None,
        }
    }
}
```

### Critical Pre-Checks

Before starting implementation, **MUST**:
1. Verify dependencies: `connection`, `request-response`, `task-iterator` features complete
2. Read [W3C Server-Sent Events specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)
3. Explore existing code:
   - `wire/simple_http/client/tasks/` - TaskIterator patterns
   - `valtron/task.rs` - TaskIterator trait and TaskStatus
   - `wire/http_stream/mod.rs` - ReconnectingStream pattern
   - `wire/simple_http/impls.rs` - SimpleIncomingRequestBuilder, HttpResponseReader

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
- `ReconnectingStream` - Auto-reconnection with backoff (pattern for Phase 2)
- `ExponentialBackoffDecider` - Backoff strategy
- `SharedByteBufferStream` - Buffered I/O
- `TaskIterator` - Non-blocking pattern (PRIMARY - Phase 1)
- `DrivenRecvIterator` - Driven receiver iterator for spawned tasks
- `InlineSendAction` - Task spawning actions

**Do NOT re-implement**:
- HTTP request building (use `SimpleIncomingRequestBuilder`)
- HTTP response streaming (use `HttpResponseReader`)
- Reconnection logic (use `ReconnectingStream` pattern)
- Backoff strategy (use `ExponentialBackoffDecider`)
- TaskIterator patterns (follow existing `*-Task` implementations)

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
