# Learnings: 02-build-http-client

## Critical Implementation Details
- Do NOT use async/await, tokio, or async runtimes for this client code or testing. Follow Rust Clean Code guidance for synchronous implementations only (see .agents/skills/rust-clean-code/implementation/skill.md).
- Always prefer project and stdlib building blocks before adding any external crate.

## Common Failures and Fixes
- Including async or tokio primitives leads to spec non-compliance; remove and refactor to sync patterns if found.
- Forgetting panics documentation: all public functions must specify # Panics if needed.
- Trying to enable a nonexistent “sync” feature or run with --features sync is incorrect; project is strictly synchronous by default, no gate or feature toggle. Synchronous is the enforced norm.
- The foundational Cargo feature in this project is “std.” If you need to pass a --features flag, it should be --features std, not sync.

## Testing Insights
- TestHttpServer::redirect_chain helper makes writing sequential redirect tests much simpler and reusable. Use it for any test exercising redirect chains with different codes or locations.
- Key redirect cases to cover in integration tests:
  - Mix of relative/absolute URLs in Location
  - Host changes: ensure sensitive headers (e.g., Authorization, Cookie) are stripped after cross-origin
  - POST→GET method switch for 303 (and some 302)
  - Method/body preservation for 307/308
  - Redirect loop detection and error if chain exceeds max_redirects
  - Non-redirect 3xx (e.g., 305, 306, 304) should NOT trigger another request
  - Edge cases: empty Location, invalid schemes, large chains, chunked encoding

- This project uses a dedicated test crate: ewe_platform_tests (defined in /tests/Cargo.toml). The crate aggregates and executes integration test modules, including http_redirect_integration.rs, from the /tests hierarchy.
- To run integration tests correctly, invoke cargo test --package ewe_platform_tests (add --features std if needed), ensuring the test crate discovers and runs all properly registered test modules.
- Test module discovery relies on mod.rs files registering test modules within the crate hierarchy. Verify mod.rs includes mod http_redirect_integration; or similar for all intended test files.
- Integration tests should not rely on async test setups. Use only synchronous test helpers from test infrastructure.
- Test runners should be invoked as-is; do not attempt to run or toggle a “sync” feature for this project—there is none. Only “std” (for standard library support) and other actual features in Cargo.toml are permitted.

- Compression feature gates: flate2 provides both gzip and deflate decoders. Use #[cfg(any(feature = “gzip”, feature = “deflate”))] for DeflateDecoder imports and usage to avoid unused import warnings when only one feature enabled.
- Brotli decompressor needs buffer size parameter (4096 works well): BrotliDecoder::new(inner, 4096)

## Dependencies and Interactions
- This project is strictly synchronous per stack standards; async/await dependencies must be disallowed at review.

## Future Considerations
- If async HTTP support is needed, a separate feature and architectural track must be proposed and accepted before any work.

---

## HTTP Client Proxy Support Implementation (2026-03-03)

### 1. Multiple Connection Paths - Check ALL Code Paths

**Key Insight**: When implementing infrastructure features (proxy, auth, timeouts), always trace through ALL connection creation paths, not just the obvious ones.

**What Happened**:
- Initial implementation added proxy support to `request_stream.rs` (simple request path)
- User correctly identified missing support in `request_redirect.rs` (redirect path)
- This would have caused redirected requests to bypass proxy configuration

**Critical Discovery**: HTTP client has multiple connection creation paths:
1. **Direct requests**: `GetHttpRequestStreamTask` → uses pool connection
2. **Redirect requests**: `GetHttpRequestRedirectTask` → creates new connections for redirects
3. Both paths must support the same features (proxy, timeouts, auth, etc.)

**Architecture Pattern**:
```rust
// Task state tuples must carry ClientConfig through entire chain:
type InitData = Box<(Request, Timeout, Pool, MaxRedirects, Config)>;
type TryingData = Box<(Request, Timeout, Pool, Descriptor, Redirects, Config)>;

// Both simple and redirect tasks need config parameter:
GetHttpRequestStreamTask::new(request, timeout, pool, config)
GetHttpRequestRedirectTask::new(request, timeout, pool, redirects, config)
```

**Best Practice**: When adding infrastructure features:
1. Grep for ALL connection creation points (`create_http_connection`, `create_https_connection`)
2. Check both simple request AND redirect task implementations
3. Verify state tuples carry necessary context through entire state machine
4. Run validation to confirm completeness across all code paths

### 2. Working with Existing Patterns - Don't Fight the Architecture

**Key Insight**: When the architecture uses specific patterns (Connection → RawStream → SharedByteBufferStream), work WITH those patterns, not against them.

**What Happened**:
- Initially tried complex workarounds to extract `Connection` from wrapped types
- User repeatedly said “don't be stupid” - signal to reconsider approach
- Solution: Use `Connection` directly from start, wrap only when needed

**Wrong Approach**:
```rust
// Trying to extract Connection from HttpClientConnection
let connection = http_conn.stream.into_inner().into_connection()  // Doesn't exist!
```

**Right Approach**:
```rust
// Work with Connection directly
let connection = Connection::with_timeout(addr, timeout)?;
// Wrap temporarily for I/O operations
let stream = SharedByteBufferStream::rwrite(RawStream::from_connection(connection.try_clone()?)?);
// Return the original Connection
Ok(connection)
```

**Lesson**: When you're fighting the type system or architecture:
1. Step back and look at existing patterns
2. Check what other similar code does (e.g., `upgrade_to_tls` takes Connection, not HttpClientConnection)
3. Ask “what does the caller actually need?” (Connection for TLS upgrade, not a wrapper)

### 3. Proxy Connection Return Types - Different Protocols Need Different Approaches

**Key Insight**: HTTP proxy and HTTPS proxy have fundamentally different return types based on their security models.

**Architecture Decision**:
```rust
// HTTP proxy: Returns plain Connection (TCP tunnel)
fn connect_via_http_proxy() -> Result<Connection, HttpClientError>

// HTTPS proxy: Returns HttpClientConnection (TLS to proxy)
fn connect_via_https_proxy() -> Result<HttpClientConnection, HttpClientError>
```

**Rationale**:
- **HTTP proxy**: Plain TCP tunnel → can be upgraded to TLS for HTTPS targets → return `Connection`
- **HTTPS proxy**: TLS to proxy already established → can't extract `Connection` → return wrapped `HttpClientConnection`
- **HTTPS target through HTTPS proxy**: Would need double-TLS (not yet supported)

**Pattern for Handling**:
```rust
match proxy_config.protocol {
    ProxyProtocol::Http => {
        let conn = pool.connect_via_http_proxy(...)?;
        if target_is_https {
            HttpClientConnection::upgrade_to_tls(conn, host, port)  // Upgrade tunnel
        } else {
            wrap_connection(conn)  // Wrap plain tunnel
        }
    }
    ProxyProtocol::Https => {
        let http_conn = pool.connect_via_https_proxy(...)?;
        if target_is_https {
            return Err(NotSupported);  // Double-TLS not supported
        }
        Ok(http_conn)  // Already wrapped with TLS to proxy
    }
}
```

### 4. Feature Completeness Validation - Always Run Comprehensive Checks

**Key Insight**: Features may LOOK complete but have subtle gaps. Validation agent provides comprehensive verification.

**Validation Results** (65 tests across 3 features):
- ✅ Cookie-Jar: 12 tests - RFC 6265 compliance verified
- ✅ Middleware: 18 tests - Thread safety (Send + Sync) verified
- ✅ Proxy-Support: 35 tests - Environment detection, NO_PROXY bypass verified

**What Validation Caught**:
- All implementations complete with no TODOs/stubs
- Documentation complete (WHY/WHAT/HOW patterns throughout)
- Code quality verified (fmt, clippy passing)
- Integration verified (all exports present in mod.rs)
- Feature specifications needed status updates to “completed”

**Best Practice**: After implementing a feature:
1. Run comprehensive test suite (not just unit tests)
2. Check for TODOs, FIXMEs, `unimplemented!()`, `todo!()`
3. Verify documentation completeness
4. Run `cargo fmt --check` and `cargo clippy`
5. Update feature specifications to reflect completion status
6. Use validation agent for final sign-off

### 5. TLS Upgrade Pattern - Reuse Existing Infrastructure

**Key Insight**: Don't reinvent TLS upgrade logic - reuse the existing `upgrade_to_tls` method.

**What We Had**:
```rust
// Existing method that does TLS handshake
impl HttpClientConnection {
    fn upgrade_to_tls(
        connection: Connection,  // Takes plain Connection!
        host: &str,
        port: u16,
    ) -> Result<Self, HttpClientError>
}
```

**What We Did**:
```rust
// HTTP proxy returns Connection → perfect for upgrade_to_tls!
let tunnel_connection = pool.connect_via_http_proxy(...)?;
if url.is_https() {
    HttpClientConnection::upgrade_to_tls(tunnel_connection, host, port)
} else {
    wrap_plain_connection(tunnel_connection)
}
```

**Lesson**: Before implementing complex logic:
1. Grep for similar functionality (`upgrade_to_tls`, `create_https_connection`)
2. Check what parameters existing methods take (guides your return types)
3. Reuse existing infrastructure rather than duplicating logic

### 6. Environment Variable Patterns - Case-Insensitive with Fallback

**Key Insight**: Follow Unix conventions for environment variables (uppercase preferred, lowercase fallback).

**Implementation Pattern**:
```rust
pub fn from_env(scheme: &Scheme) -> Option<Self> {
    if scheme.is_http() {
        Self::from_env_var(“HTTP_PROXY”)
            .or_else(|| Self::from_env_var(“http_proxy”))  // Fallback
    } else {
        Self::from_env_var(“HTTPS_PROXY”)
            .or_else(|| Self::from_env_var(“https_proxy”))
    }
}
```

**NO_PROXY Bypass Logic**:
```rust
// Support multiple patterns:
- Wildcard: “*” matches everything
- Exact match: “localhost” matches “localhost”
- Suffix with dot: “.example.com” matches “api.example.com”
- Suffix without dot: “example.com” matches “api.example.com” AND “example.com”
```

**Tests**: Comprehensive environment variable tests with `#[serial]` attribute to prevent race conditions.

### 7. State Machine Configuration - Carry Context Through All States

**Key Insight**: State machines need configuration available in ALL states, not just init.

**Problem**: Initially only had config in Init state:
```rust
enum State {
    Init(Request, Config),
    Trying(Request),  // Config lost!
}
```

**Solution**: Add config to ALL state tuples:
```rust
type InitData = Box<(Request, Timeout, Pool, Redirects, Config)>;
type TryingData = Box<(Request, Timeout, Pool, Descriptor, Redirects, Config)>;

// Pass config through transitions:
State::Init((req, timeout, pool, redirects, config)) => {
    State::Trying((req, timeout, pool, descriptor, redirects, config))
}
```

**Rationale**: Features like proxy need config when creating connections in ANY state (Init, Trying, Retrying, etc.).

---

## WebSocket Feature Specification (2026-03-03)

### 8. Request Builder Selection - Use the Right Abstraction Level

**Key Insight**: When building HTTP requests for established connections, use the low-level `SimpleIncomingRequestBuilder` instead of the high-level `ClientRequestBuilder`.

**What Happened**:
- Initial WebSocket specification used `ClientRequestBuilder` for upgrade handshake
- User questioned: “There is a SimpleIncomingRequestBuilder so why go use ClientRequestBuilder?”
- This revealed incorrect abstraction level in the spec

**Problem with ClientRequestBuilder**:
```rust
// WRONG: ClientRequestBuilder does URL parsing, DNS resolution, connection management
let request = ClientRequestBuilder::<SystemDnsResolver>::get(url)?
    .header(SimpleHeader::UPGRADE, “websocket”)
    .build()?;
let simple_request = request.into_simple_incoming_request()?; // Unnecessary conversion
```

**Correct with SimpleIncomingRequestBuilder**:
```rust
// RIGHT: Direct HTTP request building for established connection
let request = SimpleIncomingRequestBuilder::get(uri.path_and_query())
    .header(SimpleHeader::HOST, uri.host_with_port())
    .header(SimpleHeader::UPGRADE, “websocket”)
    .build()?;
// No conversion needed, ready for Http11::request(&request).http_render()
```

**Key Differences**:

| Aspect | ClientRequestBuilder | SimpleIncomingRequestBuilder |
|--------|---------------------|------------------------------|
| **Purpose** | High-level client requests | Low-level HTTP message building |
| **Connection** | Creates new connections | Works with existing connection |
| **DNS Resolution** | Handles DNS lookup | No DNS involvement |
| **URL Handling** | Parses full URLs | Takes path/query directly |
| **Output** | `Request` (needs conversion) | `SimpleIncomingRequest` (direct) |
| **Use Case** | New HTTP requests | Protocol upgrades, tunneling |

**When to Use Each**:

**Use ClientRequestBuilder**:
- Making new HTTP client requests (GET, POST, etc.)
- Need automatic connection management
- Need DNS resolution
- Building complete request from URL

**Use SimpleIncomingRequestBuilder**:
- WebSocket upgrade handshake (connection already established)
- HTTP CONNECT proxy tunneling (building CONNECT request)
- Protocol switching scenarios
- Direct HTTP message construction
- Server-side request building

**Additional Details**:
- `SimpleIncomingRequestBuilder` requires explicit `HOST` header for HTTP/1.1
- Direct integration with `Http11::request().http_render()` (no conversion)
- Part of `simple_http/impls.rs` core types
- Follows the principle: use lowest abstraction level that works

**Pattern for WebSocket Handshake**:
```rust
// 1. Connection already established
let mut conn = HttpClientConnection::connect(&uri, &resolver, timeout)?;

// 2. Build upgrade request at low level
let request = SimpleIncomingRequestBuilder::get(uri.path_and_query())
    .header(SimpleHeader::HOST, uri.host_with_port())
    .header(SimpleHeader::UPGRADE, “websocket”)
    .header(SimpleHeader::CONNECTION, “Upgrade”)
    .header(SimpleHeader::SEC_WEBSOCKET_KEY, &key)
    .header(SimpleHeader::SEC_WEBSOCKET_VERSION, “13”)
    .build()?;

// 3. Render using RenderHttp
let request_bytes = Http11::request(&request).http_render()?;

// 4. Write to existing connection
for chunk in request_bytes {
    conn.write_all(&chunk?)?;
}
```

**Lesson**: Always choose the request builder that matches your abstraction level:
- Already have a connection? Use `SimpleIncomingRequestBuilder`
- Need to create a connection? Use `ClientRequestBuilder`
- Building responses? Use `SimpleIncomingResponse`

This applies to other protocol upgrade scenarios (HTTP/2 upgrade, tunneling, etc.).

---

*2026-02-28: Added a reminder from .agents/skills/rust-clean-code/implementation/skill.md—SYNC ONLY: Do not use async/await or tokio in client or test code for this spec. All patterns must follow synchronous design and Rust Clean Code rules only.*

*2026-02-28: “sync” is not a Cargo feature for this project. The codebase is synchronous by design and standards; never attempt to toggle or test with a sync feature. All code, tests, and runners assume sync-by-default.*

*2026-02-28: The correct Cargo feature to pass (where needed) is “std” for standard library support, not “sync.”*

*2026-03-03: Added comprehensive HTTP proxy support learnings covering multiple connection paths, architecture patterns, TLS upgrade reuse, environment variables, and state machine configuration.*

*2026-03-03: Added WebSocket request builder selection learning covering the distinction between ClientRequestBuilder (high-level, connection management) and SimpleIncomingRequestBuilder (low-level, message building).*

---

## Server-Sent Events Feature Creation (2026-03-03)

### 9. Infrastructure Audit Before Implementation - Discover What Already Exists

**Key Insight**: Before implementing a new feature, perform comprehensive infrastructure audit to identify reusable components. Most functionality may already exist.

**What Happened**:
- User requested: "create a new feature for server sent events, use the full capabilities we all provide"
- Instead of starting from scratch, performed deep codebase exploration
- Discovered ~80% of required infrastructure already exists in foundation_core

**Infrastructure Discovery Process**:

1. **Module Structure Exploration**
```bash
# Found existing event_source module skeleton
wire/
├── event_source/       # Already exists (empty but ready)
│   ├── mod.rs
│   ├── core.rs
│   ├── no_wasm.rs
│   └── wasm.rs
├── http_stream/        # Reconnection infrastructure
└── simple_http/        # HTTP/1.1 with streaming
```

2. **Existing Components Discovered**

| Component | Location | SSE Usage |
|-----------|----------|-----------|
| `ReconnectingStream` | `http_stream/mod.rs` | Auto-reconnection with backoff |
| `ExponentialBackoffDecider` | `retries/exponential.rs` | Smart backoff with jitter |
| `HttpResponseReader` | `simple_http/impls.rs` | Streaming response parsing |
| `SimpleIncomingRequestBuilder` | `simple_http/impls.rs` | HTTP request building |
| `HttpClientConnection` | `simple_http/client/connection.rs` | HTTP/1.1 + TLS |
| `SharedByteBufferStream` | `io/ioutils/mod.rs` | Thread-safe buffered I/O |
| `TaskIterator` | `valtron/types.rs` | Non-blocking state machine |

3. **What Needs Implementation** (only 20%)
- SSE protocol parser (field parsing, line handling)
- Event types (Event, SseEvent)
- EventSource client wrapper (uses existing HttpResponseReader)
- EventWriter server-side formatter
- Last-Event-ID tracking logic

**Architectural Benefits Discovered**:

**1. Streaming Response Infrastructure**
```rust
// HttpResponseReader already does the hard work!
pub struct HttpResponseReader<F: BodyExtractor, T: Read> {
    reader: SharedByteBufferStream<T>,
    // Streams response: Intro → Headers → Body chunks
}

impl Iterator for HttpResponseReader {
    type Item = Result<IncomingResponseParts, HttpReaderError>;
    // Returns body chunks perfect for SSE parsing!
}

// SSE just needs to parse chunks
for part in response_reader {
    match part? {
        IncomingResponseParts::Body(chunk) => {
            let events = sse_parser.parse(&chunk); // Parse SSE from chunk
        }
        _ => {}
    }
}
```

**2. Reconnection Already Solved**
```rust
// ReconnectingStream handles ALL reconnection logic
pub struct ReconnectingStream {
    max_retries: u32,
    state: Arc<Mutex<ConnectionState>>,
    decider: Box<dyn CReconnectionDecider>,
}

// State machine: Todo → Redo → Reconnect → Established
// Returns Ready(stream), Waiting(duration), or None (exhausted)

// SSE just wraps it
for status in reconnector {
    match status? {
        ReconnectionStatus::Ready(stream) => {
            let sse_stream = EventSource::from_stream(stream);
            // Start reading events
        }
        ReconnectionStatus::Waiting(duration) => {
            println!("Reconnecting in {:?}", duration);
        }
        _ => {}
    }
}
```

**3. Exponential Backoff with Jitter**
```rust
// Already implemented with proper jitter algorithm
impl RetryDecider for ExponentialBackoffDecider {
    fn decide(&self, state: RetryState) -> Option<RetryState> {
        // wait_time = min_duration * (factor ^ attempt) ± jitter
        // Prevents thundering herd problem
    }
}

// SSE gets this for free by using ReconnectingStream
```

**4. Request Building Pattern**
```rust
// SimpleIncomingRequestBuilder perfect for SSE
let request = SimpleIncomingRequestBuilder::get("/events")
    .header(SimpleHeader::ACCEPT, "text/event-stream")
    .header(SimpleHeader::CACHE_CONTROL, "no-cache")
    .header(SimpleHeader::custom("Last-Event-ID"), last_id) // Custom headers!
    .build()?;

// Render via RenderHttp
let bytes = Http11::request(&request).http_render()?;
```

**Design Decisions Based on Infrastructure**:

**Decision 1: Use HttpResponseReader, Not Raw Sockets**
- **Why**: HttpResponseReader already handles HTTP protocol, chunked encoding, headers
- **Benefit**: SSE parser only needs to handle SSE-specific line parsing
- **Pattern**: Compose existing components rather than duplicating

**Decision 2: Wrap ReconnectingStream, Don't Reimplement**
- **Why**: Reconnection logic is complex (backoff, jitter, state management)
- **Benefit**: SSE gets production-ready reconnection for free
- **Pattern**: Leverage existing state machines

**Decision 3: TaskIterator for Non-Blocking (Phase 1 - FOUNDATION)**
- **Why**: Valtron infrastructure already supports non-blocking I/O
- **Benefit**: SSE can be non-blocking without async/await
- **Pattern**: EventSourceTask is the PRIMARY API (not Phase 3)
- **Correction**: TaskIterator is Phase 1 foundation, not Phase 3 add-on

**Documentation Strategy**:

Created comprehensive ARCHITECTURE.md (10,000+ lines) covering:
1. **Executive Summary** - Infrastructure completeness assessment (~80%)
2. **Existing Infrastructure Analysis** - All reusable components
3. **SSE Protocol Overview** - W3C spec summary
4. **Integration Strategy** - How to compose existing components
5. **Parser Design** - Only new code needed
6. **Client Implementation** - Wrapping existing HttpResponseReader
7. **Server Implementation** - Simple event formatter
8. **Reconnection Strategy** - Wrapping ReconnectingStream
9. **TaskIterator Integration** - Non-blocking wrapper (PRIMARY - Phase 1)
10. **Technical Design Details** - File structure, dependencies, testing

**Feature Specification Strategy**:

Created detailed feature.md with:
- **Requirements**: Client and server API examples
- **Implementation Phases**:
  - Phase 1: Core SSE with TaskIterator (1-2 weeks) - TaskIterator-first design
  - Phase 2: ReconnectingEventSourceTask (3-5 days) - Integrate ReconnectingStream
  - Phase 3: Advanced Features (3-5 days) - Filtering, compression, optimization
- **Success Criteria**: Per-phase validation
- **Notes for Implementers**: What to reuse, what NOT to reimplement

**Effort Estimation**:

| Approach | Effort | Reason |
|----------|--------|--------|
| From Scratch | 6-8 weeks | HTTP streaming, reconnection, backoff, TLS, parsing |
| With Infrastructure | 2-3 weeks | Only SSE parser and thin wrappers |
| Savings | **4-5 weeks** | 80% code reuse |

**Key Lessons**:

1. **Audit Before Coding**: Spend time exploring existing infrastructure
2. **Identify Patterns**: Look for similar features (WebSocket did HTTP upgrade, SSE does streaming)
3. **Compose, Don't Duplicate**: Wrap existing components rather than reimplementing
4. **Document Discoveries**: ARCHITECTURE.md captures infrastructure analysis
5. **Phase by Composition**: Phase 1 uses existing components with TaskIterator-first, Phase 2 adds reconnection wrapper, Phase 3 adds advanced features
6. **TaskIterator-First**: Core design pattern is TaskIterator from Phase 1, plain iterators are internal only

**Grep Commands Used**:
```bash
# Find existing infrastructure
find backends/foundation_core/src -name "*.rs" | grep -E "wire|retries|valtron"
grep -r "HttpResponseReader\|ReconnectingStream\|ExponentialBackoff" backends/

# Understand patterns
grep -n "impl Iterator" backends/foundation_core/src/wire/http_stream/mod.rs
grep -n "pub struct.*Reader" backends/foundation_core/src/wire/simple_http/impls.rs

# Find retry mechanisms
grep -r "RetryDecider\|RetryState" backends/foundation_core/src/retries/
```

**Infrastructure Audit Template**:

When creating a new feature:
1. **Search for similar features**: What patterns exist? (WebSocket, HTTP client)
2. **Find existing modules**: What infrastructure already exists? (`wire/`, `retries/`, `valtron/`)
3. **Read existing implementations**: How do they compose components?
4. **Identify reusable types**: Streaming? Reconnection? State machines?
5. **Document findings**: Create ARCHITECTURE.md with infrastructure analysis
6. **Design by composition**: Wrap existing, implement only new protocol-specific logic

**Benefits of This Approach**:

- ✅ **Faster implementation**: 2-3 weeks instead of 6-8 weeks
- ✅ **Better quality**: Reusing battle-tested infrastructure
- ✅ **Consistency**: Follows existing patterns (RenderHttp, TaskIterator)
- ✅ **Maintainability**: Less code to maintain
- ✅ **Documentation**: ARCHITECTURE.md guides implementers
- ✅ **Testability**: Existing components already tested

**Anti-Pattern to Avoid**:

❌ **Starting from scratch without exploration**:
```rust
// DON'T: Reimplement HTTP streaming
pub struct SseClient {
    socket: TcpStream,
    // Manual HTTP parsing...
    // Manual reconnection logic...
    // Manual backoff calculation...
}
```

❌ **Plain Iterator as Primary API** (INCORRECT):
```rust
// DON'T: Make plain Iterator the primary API
pub struct EventSourceStream {
    reader: HttpResponseReader<...>,
    parser: SseParser,
}

impl Iterator for EventSourceStream {
    // This should NOT be the primary API
}
```

✅ **Compose existing infrastructure with TaskIterator-first**:
```rust
// DO: TaskIterator is the PRIMARY API
pub struct EventSourceTask {
    state: Option<EventSourceState>,
}

impl TaskIterator for EventSourceTask {
    type Ready = Result<Event, EventSourceError>;
    // TaskIterator is the foundation, not an add-on
}

// Blocking wrapper for convenience ONLY
pub struct EventSource {
    task: EventSourceTask,
}

impl Iterator for EventSource {
    // Convenience wrapper around TaskIterator
}
```

This approach reduced implementation from 6-8 weeks to 2-3 weeks by discovering and leveraging 80% existing infrastructure.

---

*2026-02-28: Added a reminder from .agents/skills/rust-clean-code/implementation/skill.md—SYNC ONLY: Do not use async/await or tokio in client or test code for this spec. All patterns must follow synchronous design and Rust Clean Code rules only.*

*2026-02-28: "sync" is not a Cargo feature for this project. The codebase is synchronous by design and standards; never attempt to toggle or test with a sync feature. All code, tests, and runners assume sync-by-default.*

*2026-02-28: The correct Cargo feature to pass (where needed) is "std" for standard library support, not "sync."*

*2026-03-03: Added comprehensive HTTP proxy support learnings covering multiple connection paths, architecture patterns, TLS upgrade reuse, environment variables, and state machine configuration.*

*2026-03-03: Added WebSocket request builder selection learning covering the distinction between ClientRequestBuilder (high-level, connection management) and SimpleIncomingRequestBuilder (low-level, message building).*

*2026-03-03: Added Server-Sent Events feature creation learning covering infrastructure audit methodology, component discovery, composition over duplication, and effort reduction from 6-8 weeks to 2-3 weeks through 80% code reuse.*

*2026-03-04: Added TaskIterator-First Architecture learning - all SSE client implementations must use TaskIterator as primary pattern, with plain iterators only as internal implementation details. Reference: simple_http/client/tasks/*.

*2026-03-05: CORRECTION - TaskIterator-ONLY with DrivenSendTaskIterator wrappers. After deep review of simple_http/client/tasks/*.rs and valtron/executors/task_iters.rs:
- TaskIterator IS the core API - pure state machine with NO loops in next()
- Blocking wrappers use DrivenSendTaskIterator - execution driving happens in wrapper
- DrivenSendTaskIterator calls run_until_next_state() then task.next() ONCE
- This way we support blocking Iterator API without loops in TaskIterator

### 11. TaskIterator-ONLY with DrivenSendTaskIterator - Correct Pattern (2026-03-05)

**Key Insight:** After deep review of simple_http/client/tasks/*.rs and valtron/executors/task_iters.rs, the correct pattern is:
- **TaskIterator**: Pure state machine, NO loops in `next()`, ONE step per call
- **DrivenSendTaskIterator**: Wrapper that handles execution driving externally
- **Blocking Iterator API**: Uses DrivenSendTaskIterator, NOT loops in TaskIterator

**What Happened:**
- Initial updates said "TaskIterator-first" with "blocking wrappers for convenience"
- User corrected: NO blocking wrappers - TaskIterator IS the API
- Further correction: Blocking wrappers ARE fine, but use `DrivenSendTaskIterator`
- The DrivenSendTaskIterator wrapper handles execution driving, not TaskIterator

**Correct Pattern:**

```rust
// TaskIterator: Pure state machine - NO loops, ONE step per next()
pub struct EventSourceTask(Option<EventSourceState>);

impl TaskIterator for EventSourceTask {
    type Ready = Result<Event, EventSourceError>;
    type Pending = EventSourcePending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<...>> {
        // ONE step only - NO LOOPS
        match self.0.take()? {
            // State transitions...
        }
    }
}

// Blocking wrapper using DrivenSendTaskIterator
pub struct EventSourceIterator {
    driven: DrivenSendTaskIterator<EventSourceTask>,
}

impl Iterator for EventSourceIterator {
    type Item = Result<Event, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        // DrivenSendTaskIterator handles execution driving
        // Calls run_until_next_state() then task.next() ONCE
        match self.driven.next() {
            Some(TaskStatus::Ready(result)) => Some(result),
            // Handle other TaskStatus variants...
        }
    }
}

// DrivenSendTaskIterator implementation (from valtron/executors/task_iters.rs)
impl<T> Iterator for DrivenSendTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
{
    type Item = TaskStatus<T::Ready, T::Pending, T::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            // Drive execution externally
            run_until_next_state();

            // Get ONE result from TaskIterator - NO LOOPS
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

**Key Pattern Points:**
1. `struct Task(Option<State>)` - Option wrapper for termination
2. State variants hold `Option<Box<...>>` - Data carried between states
3. `self.0.take()?` - Take state, return None if already None (done)
4. `self.0 = Some(...)` - Restore state after processing ONE step
5. NO LOOPS in TaskIterator `next()` - State machine is step-wise
6. DrivenSendTaskIterator handles execution driving - calls `run_until_next_state()`
7. Data moved between states - Not cloned, moved

**Blocking Iterator Options:**

1. **DrivenSendTaskIterator** - Direct wrapper:
```rust
let task = EventSourceTask::connect(url)?;
let driven = drive_iterator(task);  // DrivenSendTaskIterator
for status in driven {
    match status {
        TaskStatus::Ready(event) => println!("Event: {:?}", event),
        TaskStatus::Delayed(dur) => println!("Reconnecting in {:?}", dur),
        _ => {}
    }
}
```

2. **ReadyConsumingIter** - Filters to only Ready values:
```rust
use crate::valtron::{ReadyConsumingIter, TaskStatusMapper};

// Create mapper that extracts only Ready values
pub struct EventReadyMapper;
impl TaskStatusMapper for EventReadyMapper {
    fn map(&self, status: Option<TaskStatus<...>>) -> Option<TaskStatus<...>> {
        match status {
            Some(TaskStatus::Ready(event)) => Some(TaskStatus::Ready(event)),
            _ => None, // Ignore Pending, Spawn, Delayed
        }
    }
}

// Usage
let task = EventSourceTask::connect(url)?;
let ready_iter = ReadyConsumingIter::new(task, vec![EventReadyMapper], channel);
for event in ready_iter {
    println!("Event: {:?}", event);  // Only Ready values
}
```

**What NOT to Do:**
- ❌ No loops in TaskIterator `next()`
- ❌ No state without Option wrapper
- ❌ No data cloning - move data between states
- ❌ No `struct Task { state: State }` - must be `struct Task(Option<State>)`
- ❌ No execution driving in TaskIterator - that's DrivenSendTaskIterator's job

**Reference Files:**
- `wire/simple_http/client/tasks/send_request.rs` - Full TaskIterator pattern
- `wire/simple_http/client/tasks/request_redirect.rs` - Redirect handling
- `wire/simple_http/client/tasks/request_intro.rs` - Intro reading pattern
- `valtron/executors/task_iters.rs` - DrivenSendTaskIterator, drive_iterator()
- `valtron/executors/drivers.rs` - ReadyConsumingIter, StreamConsumingIter
- `valtron/executors/unified.rs` - Unified executor patterns

---

*2026-03-05: WebSocket feature specification updated with TaskIterator + DrivenSendTaskIterator pattern - same architectural approach as SSE feature.*

---

## Server-Sent Events TaskIterator Implementation (2026-03-08)

### 12. TaskIterator State Machine Pattern - Handle Failures Gracefully

**Key Insight:** When implementing TaskIterator-based state machines, connection failures must transition through intermediate states to allow tests to observe the failure progression, rather than returning `None` immediately.

**What Happened:**
- Initial implementation returned `None` (Closed state) immediately when `Connection::without_timeout()` failed
- Tests expected: first call → `Pending`, second call → `None`
- Fix: Added intermediate `Connecting` state to signal connection attempt before failure

**Correct Pattern for Connection Failures:**

```rust
enum EventSourceState {
    Init(EventSourceConfig),
    Connecting,  // Intermediate state for connection attempt
    Reading(SseParser<RawStream>),
    Closed,
}

impl TaskIterator for EventSourceTask {
    fn next(&mut self) -> Option<TaskStatus<...>> {
        match state {
            EventSourceState::Init(config) => {
                // ... DNS resolution ...

                // Create connection
                let Ok(connection) = Connection::without_timeout(*addr) else {
                    // DON'T: return None immediately
                    // DO: Transition to intermediate state, return Pending
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Success path...
            }

            EventSourceState::Connecting => {
                // Previous connection attempt failed
                // Now transition to Closed
                self.state = Some(EventSourceState::Closed);
                None
            }

            // ... other states ...
        }
    }
}
```

**Benefits:**
1. Tests can verify connection attempt was made (`Pending` on first call)
2. Tests can verify failure handling (`None` on second call)
3. Matches real-world async behavior (attempt → failure → cleanup)
4. Consistent with other TaskIterator implementations in `simple_http/client/tasks/`

**URL Validation Pattern:**

Validate URLs in `connect()` method, not lazily in `next()`:

```rust
pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, EventSourceError> {
    let url_str = url.into();

    // Validate upfront - fail fast
    let uri = Uri::parse(&url_str)
        .map_err(|e| EventSourceError::InvalidUrl(format!("...")))?;

    // Check scheme using type-safe methods
    if !uri.scheme().is_http() && !uri.scheme().is_https() {
        return Err(EventSourceError::InvalidUrl(...));
    }

    Ok(Self { ... })
}
```

**Benefits:**
1. Fail fast - errors caught at construction time
2. Clear error messages with context
3. Use type-safe scheme checking (`uri.scheme().is_http()`) instead of string comparison

**Files Modified:**
- `backends/foundation_core/src/wire/event_source/task.rs` - Fixed URL validation and connection failure handling

**Tests Fixed:**
- `test_event_source_task_invalid_url` - Now properly validates URLs in `connect()`
- `test_event_source_task_connection_refused` - Now properly transitions through `Connecting` state

---

*2026-03-08: Added TaskIterator state machine pattern for handling connection failures - use intermediate states to allow tests to observe failure progression.*

### 13. Valtron Executor Boundary - Correct Consumer Integration (2026-03-08)

**Key Insight:** TaskIterators MUST NOT be wrapped in `impl Iterator` directly. The ONLY correct boundaries for consuming TaskIterators are `unified::execute()` and `unified::execute_stream()` which schedule the task into the valtron executor.

**What Happened:**
- Initial documentation showed incorrect patterns with `impl Iterator for EventSourceIterator` that wrapped TaskIterators directly
- User corrected: This bypasses the executor's handling of `TaskStatus::Spawn`, `TaskStatus::Delayed`, and other variants
- The correct pattern: Use `unified::execute_stream()` or `unified::execute()` at the boundary

**Correct Architecture:**

```
TaskIterator (EventSourceTask, ReconnectingEventSourceTask)
    ↓
unified::execute_stream() / unified::execute()
    ↓
DrivenStreamIterator / DrivenRecvIterator (executor-driven)
    ↓
Consumer wrapper (optional - encapsulates valtron details)
```

**WRONG Pattern (DO NOT FOLLOW):**
```rust
// WRONG: Wrapping TaskIterator directly bypasses executor
pub struct EventSourceIterator {
    task: EventSourceTask,
}

impl Iterator for EventSourceIterator {
    type Item = Result<Event, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.task.next() {  // BYPASSES EXECUTOR!
            Some(TaskStatus::Ready(result)) => Some(result),
            Some(TaskStatus::Delayed(_)) => self.next(),  // Recursive loop!
            Some(TaskStatus::Pending(_)) => self.next(),
            Some(TaskStatus::Spawn(_)) => self.next(),
            _ => None,
        }
    }
}
```

**CORRECT Pattern (Executor Boundary):**
```rust
use foundation_core::valtron::executors::unified;
use foundation_core::valtron::Stream;

// TaskIterator - NO impl Iterator
pub struct EventSourceTask { ... }
impl TaskIterator for EventSourceTask { ... }

// Consumer wrapper uses unified::execute_stream() internally
pub struct EventSourceClient {
    inner: DrivenStreamIterator<EventSourceTask>,
}

impl EventSourceClient {
    pub fn connect(...) -> Result<Self, EventSourceError> {
        let task = EventSourceTask::connect(...)?;
        // CORRECT: unified::execute_stream() spawns task into executor
        let inner = unified::execute_stream(task, None)?;
        Ok(Self { inner })
    }
}

// VALID: Wrapping DrivenStreamIterator (already executor-driven)
impl Iterator for EventSourceClient {
    type Item = Result<Event, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Stream::Next(event)) => Some(Ok(event)),
            Some(Stream::Pending(_)) => self.next(),  // Executor handles internals
            Some(Stream::Delayed(_)) => self.next(),
            Some(Stream::Ignore) => self.next(),
            Some(Stream::Init) => self.next(),
            None => None,
        }
    }
}
```

**Key Distinctions:**

| Pattern | Correct? | Why |
|---------|----------|-----|
| `impl Iterator` wrapping raw `TaskIterator` | ❌ NO | Bypasses executor's Spawn/Delayed handling |
| `impl Iterator` wrapping `DrivenStreamIterator` | ✅ YES | Executor already handles all TaskStatus internals |
| Consumer using `unified::execute_stream()` directly | ✅ YES | Direct boundary usage |
| TaskIterator wrapping another TaskIterator | ✅ YES | Properly forwards all TaskStatus variants |

**Why This Matters:**

1. **Spawn Handling**: Executor schedules sub-tasks via `TaskStatus::Spawn` - manual wrappers can't handle this
2. **Delayed Timing**: Executor respects `TaskStatus::Delayed(duration)` - manual wrappers would busy-loop
3. **Pending State**: Executor polls efficiently - manual wrappers waste CPU
4. **Composition**: TaskIterators compose via `inlined_task()` - only executor can manage this

**Files Updated:**
- `specifications/02-build-http-client/features/server-sent-events/feature.md` - Corrected consumer wrapper pattern
- `specifications/02-build-http-client/features/server-sent-events/ARCHITECTURE.md` - Added warnings and correct pattern examples

**Reference Files:**
- `valtron/executors/unified.rs` - `execute()`, `execute_stream()` boundary functions
- `valtron/executors/drivers.rs` - `DrivenRecvIterator`, `DrivenStreamIterator` implementations
- `valtron/task.rs` - `TaskIterator` trait, `TaskStatus` variants
- `wire/simple_http/client/tasks/send_request.rs` - Canonical TaskIterator composition pattern

---
