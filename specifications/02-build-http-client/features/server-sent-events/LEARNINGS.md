# Learnings: Server-Sent Events Feature

## Tracing and Observability Requirements (2026-03-08)

### Comprehensive Tracing Added to Specification

**Key Insight:** Production systems need proper observability. The `tracing` crate provides structured logging that enables debugging, monitoring, and incident response.

**Decision:** All SSE components and `simple_http` client code MUST use `tracing` for logging.

**Tracing Levels:**

| Level | Usage | Examples |
|-------|-------|----------|
| `trace!` | Very detailed, noisy debugging | Raw byte reads, individual field parsing |
| `debug!` | Diagnostic information | State transitions, connection events |
| `info!` | Normal operational messages | Connection established, reconnection attempt |
| `warn!` | Unexpected but recoverable | Retry attempt, slow response |
| `error!` | Error conditions | Connection failure, parse error, max retries |

**Instrumentation Pattern:**
```rust
use tracing::{debug, error, info, instrument, trace};

#[instrument(skip(self, resolver), fields(url = %url))]
pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, EventSourceError> {
    info!("Connecting to SSE endpoint");
    // ...
}
```

**Testing with tracing_test:**

All tests MUST use `#[traced_test]` attribute to capture and display logs:

```rust
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_event_source_task_dns_failure() {
    // Test code...
    // Logs automatically appear in test output:
    // [DEBUG] Resolving DNS
    // [ERROR] DNS resolution failed host="invalid.invalid"
}
```

**Benefits:**
1. Logs appear in test output without manual setup
2. Helps identify issues quickly during development
3. Documents expected behavior through log assertions
4. No RUST_LOG interference with `no-env-filter` feature

**Files to Instrument:**
- `event_source/task.rs` - EventSourceTask
- `event_source/reconnecting_task.rs` - ReconnectingEventSourceTask
- `event_source/parser.rs` - SseParser
- `event_source/writer.rs` - EventWriter
- `event_source/response.rs` - SseResponse
- `simple_http/client/*.rs` - HttpClientConnection, DnsResolver

**Checklist:**
- [ ] Add `use tracing::{...}` to all modules
- [ ] Add `#[instrument]` to all public methods
- [ ] Add logging at state transitions (debug!)
- [ ] Add logging at errors (error!)
- [ ] Add logging at operational events (info!)
- [ ] Add `#[traced_test]` to all tests
- [ ] Verify logs appear in test output

---

## Specification Completeness Update (2026-03-08)

### Gap Analysis and Fixes

**Key Insight:** During specification review, several gaps were identified and fixed:

1. **DNS Failure Observability** - Original spec had 4 states (Init → Connecting → Reading → Closed), but DNS failures went directly to Closed without observation. Fixed by adding 5th state:
   ```
   Init → Resolving → Connecting → Reading → Closed
   ```
   Now both DNS failures and connection failures transition through intermediate states that return `TaskStatus::Pending` before `Closed`, allowing tests and consumers to observe the failure progression.

2. **Last-Event-ID Tracking** - Original design had hidden parser state with `last_event_id()` getter. Fixed by introducing `ParseResult`:
   ```rust
   pub struct ParseResult {
       pub event: Event,
       pub last_known_id: Option<String>,
   }
   ```
   This makes the last known ID explicit in the return value - no hidden state, no stale getters.

3. **Idle Timeout Missing** - No mechanism to detect dead connections. Phase 3 requires:
   - `with_idle_timeout(Duration)` configuration
   - Track `last_activity` timestamp in `Reading` state
   - Return `TaskStatus::Delayed(timeout)` when idle period exceeded

4. **Max Reconnect Duration Missing** - Only max retries was tracked. Phase 3 requires:
   - `with_max_reconnect_duration(Duration)` configuration
   - Track `start_time` of first connection attempt
   - Transition to `Exhausted` when duration exceeded

5. **Parser Documentation Mismatch** - Spec showed character-by-character parsing, but implementation uses simpler line-based `read_line()`. Updated spec to match implementation.

### Mermaid Diagrams Added

Added visual architecture diagrams to specification:
- EventSourceTask state machine (5 states)
- ReconnectingEventSourceTask state machine
- Last-Event-ID flow sequence diagram
- Complete SSE connection flow chart

### ParseResult Design Decision

**Problem:** How to propagate `last_known_id` from parser to reconnection logic without hidden state.

**Rejected Approaches:**

| Approach | Why Rejected |
|----------|--------------|
| Hidden state + `last_event_id()` getter | Caller must remember to call getter; state can be stale; parser and task can have divergent IDs |
| Return tuple `(Event, Option<String>)` | Unnamed fields less clear; harder to extend |

**Chosen: `ParseResult` struct**
```rust
pub struct ParseResult {
    pub event: Event,
    pub last_known_id: Option<String>,
}
```

**Benefits:**
- Explicit - ID is part of return type, can't be forgotten
- Named fields - clear semantics
- Extensible - can add more fields later without breaking API
- Immutable - passed by value, no mutation concerns

**Flow:**
```
SseParser::parse_next() → ParseResult
    ↓
EventSourceTask reads last_known_id
    ↓
ReconnectingEventSourceTask caches ID
    ↓
On reconnect: apply cached ID via with_last_event_id()
```

---

## EventSourceTask Compilation Fixes (2026-03-08)

### Fixed Connection API Usage in TaskIterator

**Key Insight**: When implementing TaskIterator with network connections, use the correct connection APIs (`Connection::without_timeout` + `RawStream::from_connection`) instead of non-existent methods (`ClientEndpoint::from_socket_addr`).

**Problem**:
```rust
// WRONG: ClientEndpoint::from_socket_addr doesn't exist
let endpoint = crate::netcap::ClientEndpoint::from_socket_addr(*addr);
match RawStream::from_endpoint(&endpoint) { ... }
```

**Solution**:
```rust
// RIGHT: Use Connection directly, then convert to RawStream
let connection = crate::netcap::Connection::without_timeout(*addr)?;
let mut raw_stream = crate::netcap::RawStream::from_connection(connection)?;
// Write to stream, then wrap with SharedByteBufferStream
let buffer = SharedByteBufferStream::rwrite(raw_stream);
let parser = SseParser::new(buffer);
```

**Pattern from `simple_http/client/connection.rs`**:
1. Resolve DNS to get socket addresses
2. Create `Connection` with `Connection::with_timeout()` or `Connection::without_timeout()`
3. Convert to `RawStream` with `RawStream::from_connection()`
4. Optionally wrap with `SharedByteBufferStream` for buffered I/O

**Code Quality Fixes Applied**:
1. Added `#[must_use]` to `SseParser::new`
2. Fixed doc markdown (`EventBuilder` → [`EventBuilder`])
3. Simplified if-let chains: `if let Event::Message { id: Some(id), .. }` instead of nested patterns
4. Used `write!` instead of `format!` for building HTTP request strings (avoids extra allocations)
5. Added `#![allow(clippy::single_match_else)]` and `#![allow(clippy::manual_let_else)]` for TaskIterator style consistency
6. Removed unused `PhantomData` import and field

**Result**: All 22 tests passing, no clippy warnings in event_source module.

---

## Test Migration (2026-03-07)

### Moved Inline Tests to Dedicated Test Crate

**Key Insight**: Tests should live in the dedicated test crate (`ewe_platform_tests`) rather than as inline tests in source files. This provides better separation of concerns and faster compilation times for library code.

**What Was Done**:
1. Created `writer_tests.rs` and `response_tests.rs` in `tests/backends/foundation_core/units/event_source/`
2. Created `mod.rs` in `tests/backends/foundation_core/units/event_source/` to register all test modules
3. Updated `tests/backends/foundation_core/units/mod.rs` to include `pub mod event_source;`
4. Removed inline `#[cfg(test)] mod tests` blocks from all source files:
   - `backends/foundation_core/src/wire/event_source/core.rs`
   - `backends/foundation_core/src/wire/event_source/error.rs`
   - `backends/foundation_core/src/wire/event_source/parser.rs`
   - `backends/foundation_core/src/wire/event_source/writer.rs`
   - `backends/foundation_core/src/wire/event_source/response.rs`

**Result**: All 22 tests now run from `ewe_platform_tests` crate, 0 inline tests in source files.

**Test Files Structure**:
```
tests/backends/foundation_core/units/event_source/
├── mod.rs              # Module registration
├── core_tests.rs       # SseEvent and SseEventBuilder tests
├── error_tests.rs      # EventSourceError Display tests
├── parser_tests.rs     # SseParser protocol tests
├── response_tests.rs   # SseResponse builder tests
└── writer_tests.rs     # EventWriter formatting tests
```

---

## Implementation Progress (2026-03-05)

### Phase 1: Core SSE Protocol - COMPLETED ✅

#### Completed Components

**1. Core Types (`core.rs`)**
- `Event` enum: Message, Comment, Reconnect variants
- `SseEvent` struct with builder pattern
- `SseEventBuilder` with fluent API (id, event, data, build)
- Convenience constructors: `message()`, `retry()`, `new()`
- Getter methods: `id()`, `event_type()`, `data_lines()`, `retry_ms()`
- 4 passing tests

**2. SSE Parser (`parser.rs`)**
- `SseParser` with stateful parsing
- Handles all SSE field types: `id:`, `event:`, `data:`, `retry:`, `:` (comment)
- Multi-line data support (joins with `\n`)
- Multiple line ending support (`\n`, `\r\n`)
- Last-Event-ID tracking via `last_event_id()`
- Null byte handling in id field (ignored per spec)
- Invalid retry handling (ignored per spec)
- 7 passing tests

**3. Error Types (`error.rs`)**
- `EventSourceError` enum covering all SSE error cases
- Display and Error trait implementations
- From<std::io::Error> conversion
- 1 passing test

**4. Event Writer (`writer.rs`)**
- `EventWriter<W>` generic over Write
- `send()` method for full events
- `comment()` for keep-alive messages
- `message()` convenience method
- Proper flushing after each event
- 5 passing tests

**5. SSE Response Helper (`response.rs`)**
- `SseResponse` builder for SSE HTTP responses
- Default headers: Content-Type, Cache-Control, Connection
- Custom header support via `with_header()`
- Integration with `SimpleOutgoingResponseBuilder`
- 3 passing tests

**Total: 20 passing tests**

---

## Key Implementation Details

### TDD Workflow
Following strict TDD with ONE test at a time:
1. Write test → Verify fails → Implement → Verify passes → Next test
2. All tests have WHY/WHAT documentation
3. All code passes `cargo fmt` and has no new clippy warnings

### SSE Protocol Rules Implemented
1. **Field parsing**: `id:`, `event:`, `data:`, `retry:`, `:` (comment)
2. **Line endings**: `\n`, `\r`, `\r\n` all supported
3. **Multi-line data**: Multiple `data:` fields joined with `\n`
4. **Leading space stripping**: Optional space after `:` is stripped
5. **Null byte in ID**: Field ignored if value contains `\0`
6. **Invalid retry**: Non-integer retry values ignored
7. **Empty line**: Dispatches accumulated event

### Code Quality
- All public functions have documentation with WHY/WHAT
- Follows project synchronous-only patterns (no async/await)
- Uses project building blocks (no external dependencies added)
- All tests are independent and meaningful

---

## Remaining Phase 1 Work

1. **EventSourceTask** - TaskIterator implementation for client-side consumption
2. **EventSourceStreamReader** - Wrap HttpResponseReader with SSE parsing
3. **Integration tests** - Test with real SSE streams

---

*Created: 2026-03-05*
*Last Updated: 2026-03-07*

---

## Phase 1 Completion - Clippy Fixes (2026-03-07)

### Documentation and Linting Fixes

**Key Insight**: Proper documentation with backticks and `#[must_use]` attributes improves code quality and API usability.

**Fixes Applied**:
1. Added backticks around type names in documentation (e.g., `[`Event`]`, `[`SseEvent`]`)
2. Wrapped URLs in angle brackets (e.g., `<https://...>`)
3. Added `#[must_use]` attributes to builder methods and constructors
4. Added `# Errors` sections to functions returning `Result`
5. Added `# Panics` section to `SseResponse::build()`
6. Used direct format string interpolation (e.g., `write!(f, "{msg}")` instead of `write!(f, "{}", msg)`)
7. Changed `send(event: SseEvent)` to `send(&event: &SseEvent)` to avoid unnecessary clone
8. Added `#[allow(clippy::new_ret_no_self)]` for builder pattern (`SseEvent::new()` returns `SseEventBuilder`)
9. Added `#[allow(clippy::large_enum_variant)]` for `HttpConnectState` (internal enum)
10. Fixed match arm to use explicit `()` pattern: `TaskStatus::Pending(())`

**Result**: All 22 tests passing, no clippy warnings in event_source module, formatting clean.

---

## SseParser Iterator Refactor (2026-03-07)

### Refactored `SseParser` to Implement `Iterator` Trait

**Key Insight**: Using `Iterator` trait directly is more idiomatic Rust than returning `Vec<Event>` from `parse()`. Eliminates intermediate allocations and simplifies `EventSourceStreamReader`.

**Changes Made**:
1. Added `event_queue: Vec<Event>` field to `SseParser`
2. Renamed `parse(&mut self, chunk: &str) -> Vec<Event>` to `feed(&mut self, chunk: &str)`
3. Implemented `Iterator for SseParser` with `next()` pulling from queue
4. Removed `event_queue` from `EventSourceStreamReader` - now delegates directly to parser
5. Updated all parser tests to use `feed()` + `collect()` pattern

**Before**:
```rust
let events = parser.parse("data: hello\n\n");  // Returns Vec<Event>
```

**After**:
```rust
parser.feed("data: hello\n\n");
let event = parser.next();  // Iterator yields one Event at a time
// Or: let events: Vec<Event> = parser.collect();
```

**Benefits**:
- More idiomatic Rust (uses standard Iterator trait)
- Eliminates intermediate `Vec` allocation in `EventSourceStreamReader`
- Cleaner separation: `feed()` inputs data, `Iterator` outputs events
- Tests can still `collect()` into `Vec` for assertions
