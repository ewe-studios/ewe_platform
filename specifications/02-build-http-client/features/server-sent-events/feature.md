---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/server-sent-events"
this_file: "specifications/02-build-http-client/features/server-sent-events/feature.md"

status: in-progress
priority: medium
created: 2026-03-03
updated: 2026-03-08

depends_on:
  - connection
  - request-response
  - task-iterator

tasks:
  completed: 8
  uncompleted: 1
  total: 9
  completion_percentage: 89
---

# Server-Sent Events (SSE) Feature

## Overview

Implement complete Server-Sent Events (SSE / EventSource) support following the [W3C Server-Sent Events specification](https://html.spec.whatwg.org/multipage/server-sent-events.html). This feature provides both **client-side** (consuming SSE streams) and **server-side** (producing SSE streams) capabilities, leveraging existing `simple_http` and `valtron` infrastructure.

**Key Capabilities**:
- ✅ Client: Connect to SSE endpoints and consume event streams
- ✅ Server: Send SSE events to connected clients
- ✅ Automatic reconnection with Last-Event-ID tracking
- ✅ Non-blocking operation via TaskIterator pattern
- ✅ TLS support (`https://` URLs)
- ✅ Custom headers and authentication

## Dependencies

This feature depends on:
- `connection` - HTTP connections with TLS support
- `request-response` - Request building and response parsing
- `task-iterator` - Non-blocking state machine execution
- `retries` - ExponentialBackoffDecider for reconnection

This feature is required by:
- None (end-user feature)

### Existing Infrastructure

Already available in `foundation_core`:
- `wire/simple_http::HttpClientConnection` - HTTP/1.1 with TLS
- `wire/simple_http::HttpResponseReader` - Streaming response parsing
- `valtron::TaskIterator` - Non-blocking event consumption
- `valtron::executors::unified::execute()` - Boundary: TaskIterator → normal Iterator
- `valtron::executors::unified::execute_stream()` - Boundary: TaskIterator → Stream Iterator
- `valtron::InlineSendAction` / `inlined_task()` - Sub-task composition within TaskIterator
- `valtron::DrivenRecvIterator` - Receiver for spawned sub-task results
- `retries::ExponentialBackoffDecider` - Backoff strategy
- `io::ioutils::SharedByteBufferStream` - Buffered I/O

### New Dependencies Required

None - all dependencies already in `Cargo.toml`

## Requirements

### Architectural Principle: TaskIterator Design

**CRITICAL:** All SSE client task implementations MUST implement `TaskIterator`, NOT `Iterator`.

TaskIterators are the core execution unit in valtron. They yield `TaskStatus` variants that the executor handles:
- `TaskStatus::Ready(value)` — task produced a value
- `TaskStatus::Pending(state)` — task is waiting (I/O, computation)
- `TaskStatus::Delayed(duration)` — task wants to be woken after a delay
- `TaskStatus::Spawn(action)` — task requests the executor to spawn a sub-task
- `TaskStatus::Init` — initialization signal

**Key Rules:**
1. **TaskIterators implement `TaskIterator`, never `Iterator`** — wrapping a TaskIterator in an Iterator loses the ability to properly handle Spawn, Delayed, etc.
2. **Composing TaskIterators uses `inlined_task()` + `TaskStatus::Spawn`** — spawn sub-tasks via the executor, receive results via `DrivenRecvIterator`
3. **The boundary to `Iterator` is `unified::execute()` / `unified::execute_stream()`** — these schedule the task into the executor engine and return a driven iterator
4. **State carries ALL data** — state enum variants hold the data needed for each phase
5. **`Option<State>` wrapper** — task wraps state in `Option` for termination

### Sub-Task Composition Pattern

From `send_request.rs` — the correct way to compose TaskIterators:

```rust
// 1. Create sub-task and get (action, receiver) pair
let (action, receiver) = inlined_task(
    InlineSendActionBehaviour::LiftWithParent,
    Vec::new(),  // mappers
    ChildTask::new(...),
    Duration::from_millis(100),
);

// 2. Store receiver in parent state
self.state = Some(ParentState::WaitingForChild(receiver));

// 3. Return Spawn to executor — executor handles scheduling
Some(TaskStatus::Spawn(action.into_box_send_execution_action()))

// 4. On next call, poll receiver
ParentState::WaitingForChild(mut recv) => {
    let result = recv.next();  // DrivenRecvIterator polls child
    // Process result, transition state
}
```

**DO NOT** wrap TaskIterators in `impl Iterator` — this bypasses the executor's Spawn/Delayed handling.

### Executor Boundary (Where TaskIterator Becomes Iterator)

Users consume SSE events by scheduling the task into the valtron executor. The executor handles all internal TaskStatus mechanics (Spawn, Delayed, etc.) and presents a simpler interface.

**Recommended: `execute_stream()` — simplified Stream interface**

For users who don't need to handle `TaskStatus` variants directly, `execute_stream()` returns a `DrivenStreamIterator` that encapsulates away the internal mechanics and presents a simpler `Stream` type:

```rust
use foundation_core::valtron::executors::unified;
use foundation_core::valtron::Stream;
use foundation_core::wire::event_source::EventSourceTask;

let task = EventSourceTask::connect(resolver, "https://api.example.com/events")?
    .with_header(SimpleHeader::custom("Authorization"), "Bearer token123");

// execute_stream() hides TaskStatus internals, returns Stream<Ready, Pending>
let mut stream = unified::execute_stream(task, None)?;

for item in stream {
    match item {
        Stream::Next(Event::Message { data, .. }) => println!("{data}"),
        Stream::Pending(_) => { /* executor is working */ }
        Stream::Delayed(_) => { /* backoff wait */ }
        _ => {}
    }
}
```

**Advanced: `execute()` — full TaskStatus control**

For advanced consumers that need to handle `TaskStatus::Spawn` or other variants:

```rust
use foundation_core::valtron::executors::unified;

let task = EventSourceTask::connect(resolver, url)?;

// execute() returns DrivenRecvIterator with full TaskStatus visibility
let mut iter = unified::execute(task, None)?;

for status in iter {
    match status {
        TaskStatus::Ready(Event::Message { data, .. }) => println!("{data}"),
        TaskStatus::Pending(_) => { /* executor is working */ }
        TaskStatus::Delayed(d) => { /* backoff wait */ }
        TaskStatus::Spawn(_) => { /* sub-task spawned */ }
        _ => {}
    }
}
```

**Key point:** `unified::execute()` and `unified::execute_stream()` are the ONLY correct boundaries for converting a TaskIterator into a consumable Iterator. Do NOT manually wrap TaskIterators in `impl Iterator`.

### Server-Side SSE Production

Send SSE events to clients:

```rust
use foundation_core::wire::event_source::{EventWriter, SseEvent};

fn handle_sse_connection(mut stream: impl Write) -> Result<(), Error> {
    let mut writer = EventWriter::new(&mut stream);

    writer.send(SseEvent::message("Hello, World!"))?;

    writer.send(SseEvent::new()
        .id("123")
        .event("user_joined")
        .data(r#"{"user": "alice"}"#)
        .build())?;

    writer.comment("Still alive")?;

    Ok(())
}
```

### Automatic Reconnection

`ReconnectingEventSourceTask` wraps `EventSourceTask` with reconnection logic.
It is itself a `TaskIterator` (NOT an Iterator) and properly forwards all TaskStatus variants.

```rust
use foundation_core::wire::event_source::ReconnectingEventSourceTask;
use foundation_core::valtron::executors::unified;
use foundation_core::valtron::Stream;

let task = ReconnectingEventSourceTask::connect(resolver, url)?
    .with_max_retries(10)
    .with_last_event_id("42");

// Use execute_stream() for simplified consumption
let mut stream = unified::execute_stream(task, None)?;

for item in stream {
    match item {
        Stream::Next(Event::Message { data, .. }) => {
            println!("Data: {data}");
        }
        Stream::Delayed(duration) => {
            // Executor handles backoff delay
        }
        Stream::Pending(ReconnectingProgress::Reconnecting) => {
            println!("Reconnecting...");
        }
        _ => {}
    }
}
```

## Implementation Phases

### Phase 1: Core SSE Protocol ✅ COMPLETE

**File structure**:
```
backends/foundation_core/src/wire/event_source/
├── mod.rs              # Public API and re-exports
├── core.rs             # Event and SseEvent types
├── parser.rs           # SSE message parser (internal, wraps SharedByteBufferStream)
├── task.rs             # EventSourceTask (TaskIterator)
├── writer.rs           # EventWriter (server-side)
├── response.rs         # SseResponse builder
└── error.rs            # EventSourceError
```

**Completed Tasks**:

1. **SSE Protocol Types** (`core.rs`) ✅
   - `Event` enum (Message, Comment, Reconnect)
   - `SseEvent` builder for server-side
   - `SseEventBuilder` with fluent API

2. **SSE Message Parser** (`parser.rs`) ✅
   - `SseParser` wrapping `SharedByteBufferStream`
   - `Iterator<Item = Result<Event, EventSourceError>>` — properly returns errors
   - All field types: id, event, data, retry, comments
   - Multi-line data, line ending handling, Last-Event-ID tracking

3. **EventSource Task** (`task.rs`) ✅ — **Core TaskIterator**
   - Implements `TaskIterator` (NOT Iterator)
   - State machine: Init → Connecting → Reading → Closed
   - DNS resolution, HTTP request building, SSE parsing

4. **SSE Server Writer** (`writer.rs`) ✅
5. **SSE Response Helper** (`response.rs`) ✅
6. **Error Handling** (`error.rs`) ✅
7. **Test Migration** ✅ — All tests in dedicated test crate

**Total: 33 passing tests (unit + integration)**

### Phase 2: ReconnectingEventSourceTask ✅ COMPLETE

**File**: `reconnecting_task.rs`

1. **ReconnectingEventSourceTask** ✅
   - Implements `TaskIterator` — properly forwards Ready, Pending, Delayed, Spawn, Init
   - State machine: Connected → Waiting → Reconnecting → Connected (loop) → Exhausted
   - Exponential backoff via `ExponentialBackoffDecider`
   - Last-Event-ID tracking across reconnections
   - Respects server `retry:` field
   - Max retries with configurable limit
   - `TaskStatus::Delayed(duration)` for backoff signaling

**Total: 44 passing tests (unit + integration)**

### Phase 3: Simplified Consumer APIs + Advanced Features (Future)

1. **Simplified Consumer Wrappers**
   - Convenience functions that properly use `unified::execute()` / `unified::execute_stream()` internally
   - Hide executor setup from callers while correctly spawning tasks into the execution engine
   - Wrappers encapsulate valtron internals (`DrivenStreamIterator`, `TaskStatus`, etc.) behind simple APIs
   - These wrappers use `unified::execute_stream()` at the boundary — the iterator they return is valid because it wraps the executor-driven iterator, not a raw TaskIterator

   **Correct Consumer Wrapper Pattern:**
   ```rust
   /// Simplified SSE client that encapsulates valtron executor details.
   ///
   /// This wrapper:
   /// - Internally uses unified::execute_stream() to spawn the task correctly
   /// - Returns a wrapped iterator that hides DrivenStreamIterator from users
   /// - Presents a simple Iterator<Item = Result<Event, EventSourceError>> interface
   pub struct EventSourceClient {
       inner: DrivenStreamIterator<EventSourceTask<...>>,
   }

   impl EventSourceClient {
       /// Connect to an SSE endpoint and return a client that yields events.
       /// Internally spawns the task into the valtron executor via execute_stream().
       pub fn connect(
           resolver: impl DnsResolver + Clone + Send + 'static,
           url: impl Into<String>,
       ) -> Result<Self, EventSourceError> {
           let task = EventSourceTask::connect(resolver, url)?;
           let inner = unified::execute_stream(task, None)
               .map_err(|e| EventSourceError::Http(e.to_string()))?;
           Ok(Self { inner })
       }

       /// Connect with automatic reconnection.
       pub fn connect_reconnecting(
           resolver: impl DnsResolver + Clone + Send + 'static,
           url: impl Into<String>,
           max_retries: u32,
       ) -> Result<Self, EventSourceError> {
           let task = ReconnectingEventSourceTask::connect(resolver, url)?
               .with_max_retries(max_retries);
           let inner = unified::execute_stream(task, None)
               .map_err(|e| EventSourceError::Http(e.to_string()))?;
           Ok(Self { inner })
       }
   }

   impl Iterator for EventSourceClient {
       type Item = Result<Event, EventSourceError>;

       fn next(&mut self) -> Option<Self::Item> {
           // This is VALID: we wrap DrivenStreamIterator (already executor-driven)
           // The valtron executor handles all TaskStatus internals behind the scenes
           match self.inner.next() {
               Some(Stream::Next(event)) => Some(Ok(event)),
               Some(Stream::Pending(_)) => self.next(), // executor working, continue
               Some(Stream::Delayed(_)) => self.next(), // backoff, continue
               Some(Stream::Ignore) => self.next(), // spawn/init, continue
               Some(Stream::Init) => self.next(),
               None => None,
           }
       }
   }

   // Consumer usage — simple, no valtron knowledge needed:
   let client = EventSourceClient::connect(resolver, url)?;
   for event_result in client {
       match event_result? {
           Event::Message { data, .. } => println!("{data}"),
           _ => {}
       }
   }
   ```

   **Key Distinction:**
   - **WRONG:** `impl Iterator` that wraps a raw `TaskIterator` directly — bypasses executor
   - **CORRECT:** `impl Iterator` that wraps `DrivenStreamIterator` (from `unified::execute_stream()`) — executor handles all internals

2. **Compression Support** — handle `Content-Encoding: gzip` in SSE responses
3. **Performance Optimizations** — buffer pooling, zero-copy parsing

## Success Criteria

**Phase 1 (Core with TaskIterator)** ✅:
- [x] `event_source/` module exists and compiles
- [x] `EventSourceTask` implements `TaskIterator` correctly
- [x] State machine follows valtron pattern
- [x] `SseParser` correctly parses all SSE field types
- [x] `SseParser::Iterator` returns `Result<Event, EventSourceError>` (not swallowing errors)
- [x] `EventWriter` formats events correctly
- [x] `SseResponse` builds correct HTTP response headers
- [x] All unit tests pass
- [x] Code passes `cargo fmt` and `cargo clippy`

**Phase 2 (Reconnection with TaskIterator)** ✅:
- [x] `ReconnectingEventSourceTask` implements `TaskIterator` correctly
- [x] Properly forwards all `TaskStatus` variants from inner task
- [x] Auto-reconnects on connection loss
- [x] Respects server `retry:` field
- [x] Sends `Last-Event-ID` on reconnect
- [x] Exponential backoff works
- [x] Max retries honored
- [x] Retry state resets on successful event receipt

**Phase 3 (Advanced Features)**:
- [ ] Event filtering works
- [ ] Compression support works

## Verification Commands

```bash
# Format check
cargo fmt -- --check

# Clippy linting
cargo clippy --package foundation_core -- -W clippy::all

# All event_source tests
cargo test --manifest-path tests/Cargo.toml -- event_source
```

## Notes for Implementation Agents

### CRITICAL: TaskIterator Rules

1. **SSE tasks implement `TaskIterator`, NEVER `Iterator`**
   - `Iterator` cannot forward `Spawn`, `Delayed`, or other executor signals
   - The executor needs these signals to schedule sub-tasks and manage timing

2. **The boundary to `Iterator` is `unified::execute()` / `unified::execute_stream()`**
   - These functions schedule the TaskIterator into the executor engine
   - They return `DrivenRecvIterator` / `DrivenStreamIterator` which implement `Iterator`
   - This is the ONLY correct way to get an Iterator from a TaskIterator

3. **Sub-task composition uses `inlined_task()` + `TaskStatus::Spawn`**
   - Creates `(InlineSendAction, RecvIterator)` pair
   - Parent yields `Spawn(action)` → executor schedules child
   - Parent stores receiver, polls via `receiver.next()` on subsequent calls
   - See `send_request.rs` lines 129-149 for the canonical pattern

4. **Wrapping a TaskIterator inside another TaskIterator is correct**
   - `ReconnectingEventSourceTask` wraps `EventSourceTask`
   - The wrapper forwards all TaskStatus variants properly
   - This maintains the executor's ability to handle Spawn/Delayed

5. **Consumer wrappers CAN implement `Iterator` at the extreme boundaries**
   - Wrappers that encapsulate valtron details are encouraged
   - The wrapper MUST use `unified::execute_stream()` or `unified::execute()` internally
   - The wrapper wraps the `DrivenStreamIterator` / `DrivenRecvIterator`, NOT the raw TaskIterator
   - This is correct because the executor already handles all TaskStatus internals

### SSE Protocol Rules

**Message Format** (from W3C spec):
```
field: value\n
field: value\n
\n
```

**Field Types**:
- `event: <type>` — Event type (default: "message")
- `data: <text>` — Event data (can appear multiple times, joined with `\n`)
- `id: <id>` — Event ID (sent back as Last-Event-ID)
- `retry: <milliseconds>` — Reconnection time
- `: <comment>` — Comment (ignored, used for keep-alive)

**Parsing Rules**:
1. Lines ending with `\n`, `\r`, or `\r\n`
2. Lines starting with `:` are comments
3. Field name and value separated by first `:`
4. Optional single space after `:` is stripped
5. Empty line dispatches event
6. If `id:` contains null byte (`\0`), ignore the field
7. `retry:` must be valid integer, otherwise ignore

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

---

*Created: 2026-03-03*
*Last Updated: 2026-03-08*
