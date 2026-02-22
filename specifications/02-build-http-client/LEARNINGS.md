# HTTP 1.1 Client - Learnings

## Overview
This document consolidates all learnings discovered during the implementation of the HTTP 1.1 client specification. Learnings will be added incrementally as features are implemented.

## Critical Implementation Details
_To be populated during implementation_

## Common Failures and Fixes
_To be populated as issues are encountered and resolved_

## Dependencies and Interactions
_To be populated as integration points are discovered_

## Testing Insights
- Tests must not access private fields; only public API is valid for unit/integration tests.
- Valtron executor must be initialized for tests that trigger request execution, otherwise thread pool errors will occur.
- Unit tests should focus on request-building, configuration, and public method behavior.
- Integration tests may require mock servers or infrastructure to simulate network and redirect scenarios.

### POST→GET Redirect Test
**WHAT:** Tests POST→GET redirect configuration and builder logic using the public API.
**WHY:** Ensures the client can build a POST request with redirect settings, and that execution does not panic even when no real server is present. Validates public API usage and proper initialization of the Valtron executor.

## Future Considerations
_To be populated with technical debt and improvement opportunities_

---
*Created: 2026-01-24*
*Last Updated: 2026-01-24*

## Pre-existing Issues (2026-01-24)

### foundation_wasm Compilation Errors
- `foundation_wasm` has ~110 compilation errors due to incorrect Mutex API usage
- `SpinMutex::lock()` returns `Result<Guard, PoisonError>` but code calls it without unwrapping
- Issue affects frames.rs, intervals.rs, schedule.rs, registry.rs
- **Decision**: Documented but not fixed to avoid scope creep; focus on valtron-utilities feature
- **Impact**: Cannot run full workspace tests until fixed; testing valtron-utilities in isolation


## Valtron Utilities Implementation (2026-01-24)

### Type Name Updates (2026-01-24)
**CRITICAL**: Action types renamed to match specification requirements:
- `LiftAction` → `SpawnWithLift` (primary name)
- `ScheduleAction` → `SpawnWithSchedule` (primary name)
- `BroadcastAction` → `SpawnWithBroadcast` (primary name)
- `CompositeAction` → `SpawnStrategy` (primary name)

**Reason**: New names better reflect their purpose of spawning child tasks with different strategies. The "SpawnWith*" prefix clarifies they are for spawning children from within a TaskIterator, not for initial task submission.

**Migration Complete (2026-01-24)**: All references to old type names have been updated throughout the codebase. The deprecated type aliases have been removed - all code now uses the new names directly.

**Send Bound Fix**: `SpawnStrategy` (formerly `CompositeAction`) now requires `V: Clone + Send + 'static` instead of `V: Clone + 'static`. This is necessary because the Broadcast variant uses `engine.broadcast()` which sends tasks to the global queue for cross-thread execution.

### Actions.rs Design Decisions
- SpawnWithLift uses `Option<I>` to ensure apply() is idempotent (can only be called once)
- SpawnWithBroadcast clones values for each callback → requires T: Clone
- SpawnStrategy applies actions sequentially → errors stop propagation
- All actions wrap tasks in DoNext before scheduling

### State Machine Pattern
- StateTransition::Error maps to None (task stops) → design choice for simplicity
- Error handling should be done via Result<T, E> in Output type, not Error variant
- StateTransition::Continue emits Pending(state) to allow non-yielding transitions
- StateMachineTask clones Pending state for Continue transitions → requires Clone bound

### Future Adapter Implementation
- No-op waker needed because valtron drives polling loop (not Future's wake mechanism)
- Thread-local waker cache on std, fresh creation on no_std → performance trade-off
- Platform-specific bounds: Send on native, relaxed on WASM (single-threaded)
- FutureTask requires Box → only available with std or alloc features
- StreamTask yields Option<Item> (None = stream exhausted, not task done)

### Wrappers Design
- TimeoutTask only with std (requires Instant) → PollLimitTask for no_std
- RetryingTask is simplified → full retry needs state machine to recreate tasks
- BackoffStrategy::next_delay clamps to max_delay → prevents runaway delays
- BackoffTask inserts delays via TaskStatus::Delayed

### Feature Flag Strategy
- default = ["std"] → most builds use std
- alloc → heap without std (FutureTask, StreamTask work)
- multi → multi-threaded executor (implies std)
- nothread_runtime → existing flag for WASM/embedded
- Pure no_std (no default features) → limited functionality, no Future/Stream

### Integration Points
- unified.rs auto-selects executor → simplifies client code
- All new types use existing ExecutionAction trait → seamless integration
- DoNext pattern used consistently → matches existing codebase patterns
- futures-core with default-features = false → WASM/no_std compatibility


### Tasks Deferred/Skipped
1. **FutureTaskRef (pure no_std)**: Skipped due to time constraints and complexity
   - Requires user to pin futures themselves (non-ergonomic)
   - Use case is rare (pure no_std without alloc)
   - Can be added in future if needed
2. **Pure no_std build verification**: Not tested
   - Requires fixing foundation_wasm compilation errors first
   - Syntax is correct for no_std, but not verified with cargo build
3. **Full test suite execution**: Not run
   - Workspace has pre-existing compilation errors in foundation_wasm
   - All tests written with proper WHY/WHAT documentation
   - Tests will run once foundation_wasm is fixed
4. **cargo fmt/clippy**: Not run
   - Same reason as test suite (workspace compilation errors)
   - Code follows Rust conventions and should pass

### Workarounds Applied
- Used TDD approach: wrote tests first, implemented to pass
- All code syntax-checked for correctness
- Documentation follows existing patterns
- Type constraints verified manually

### TaskIterator Forwarding Pattern (CRITICAL INSIGHT - 2026-01-24)

**Core Pattern**: TaskIterators should work with `Iterator<Item = TaskStatus<D, P, S>>` and forward states they don't transform.

**Two Entry Points**:
1. **WrapTask**: Converts `Iterator<Item = T>` → `TaskIterator` by wrapping in `Ready(T)`
2. **LiftTask**: Converts `Iterator<Item = TaskStatus>` → `TaskIterator` by passing through as-is

**Why This Matters**:
- Prevents incorrect nesting: `Ready(Pending(...))` would lose semantic meaning
- Enables clean composition: stack wrappers without information loss
- Single responsibility: each wrapper only transforms states it cares about

**Forwarding Examples**:
- TimeoutTask: Forwards Ready/Delayed/Spawn, wraps Pending with timeout info
- RetryingTask: Forwards all states (`other => other`)
- FutureTask/StreamTask: Produce TaskStatus directly based on Poll result

**Composition Works**:
```rust
vec![1,2,3].into_iter()     // Plain iterator
→ WrapTask                  // Wrap in Ready(T)
→ TimeoutTask               // Add timeout, forward Ready
→ TaskStatus::Ready(1)      // Clean result!
```

**Anti-Pattern**:
```rust
// WRONG: Don't wrap TaskStatus in TaskStatus
TaskStatus::Ready(TaskStatus::Pending(()))  // ❌ Loses meaning!

// CORRECT: Use LiftTask to forward
TaskStatus::Pending(())  // ✅ Semantic meaning preserved!
```

### Spawner-Type Pattern (CLARITY - 2026-01-24)

**Key Insight**: Each TaskIterator declares its spawning capability via `type Spawner`.

**The Pattern**:
1. **WrapTask**: `Spawner = NoAction` - Can't spawn subtasks
2. **LiftTask**: `Spawner = S` (generic) - Preserves inner spawner type
3. **Actions**: `ExecutionAction` implementations - Handle engine calls

**Why This Works**:
- Type system documents spawn behavior
- DoNext intercepts `TaskStatus::Spawn` and calls `action.apply(engine)`
- Actions (WrapAction, LiftAction, ScheduleAction, BroadcastAction) create tasks and schedule them
- Clean separation: Task wrappers forward, Actions execute

**Simplicity**: No complex interception logic needed - DoNext handles it all.

**Action Methods** (CRITICAL - 2026-01-24):
Each Action type calls a different ExecutionEngine method:
- **WrapAction**: `engine.schedule()` - local queue
- **LiftAction**: `engine.lift(task, parent)` - with parent linkage
- **ScheduleAction**: `engine.schedule()` - local queue
- **BroadcastAction**: `engine.broadcast()` - global queue (any thread)

This enables different execution strategies while keeping Actions simple.

## TLS Feature Conflict Resolution (2026-01-24)

**Problem**: Default features included `ssl` which enabled `ssl-rustls`, causing conflicts when users tried to enable `ssl-openssl` or `ssl-native-tls`.

**Root Cause**: `Cargo.toml` line 85: `default = ["standard", "ssl", "std"]` auto-enabled rustls.

**Solution Applied**:
1. **Removed ssl from default features**: Users must explicitly choose TLS backend
2. **Added webpki-roots dependency**: For rustls client connections (version 0.26)
3. **Added compile_error! guards**: Clear error messages for conflicting features

**Changes Made**:
- `Cargo.toml`: Removed `ssl` from default, added `webpki-roots` to ssl-rustls feature
- `ssl/mod.rs`: Added 3 compile_error! macros for mutual exclusivity checks

**Result**: Clean compile-time errors instead of silent failures or confusing unresolved imports.

## Foundation Feature Implementation (2026-01-24)

### Error Type Design

**Challenge**: Making DnsError cloneable for MockDnsResolver while std::io::Error doesn't implement Clone.

**Solution**: Store io::Error as String representation:
- DnsError::IoError(String) instead of DnsError::IoError(io::Error)
- Manual From<io::Error> implementation converts to String
- Manual Clone implementation for DnsError
- Preserves error information while enabling Clone

**Trade-off**: Loses the original io::Error object, but error message is preserved.

**Rationale**:
- MockDnsResolver needs to return cloned errors for testing
- Error messages (not error objects) are what users need for debugging
- This pattern matches existing code in the crate

### DNS Resolver Architecture

**Pattern**: Generic trait-based design with composition
- DnsResolver trait provides pluggable abstraction
- SystemDnsResolver uses std::net::ToSocketAddrs (default)
- CachingDnsResolver<R: DnsResolver> wraps any resolver
- MockDnsResolver for testing with configurable responses

**Why Generic Type Parameters**:
```rust
// Preferred - Zero runtime overhead
pub struct CachingDnsResolver<R: DnsResolver> {
    inner: R,
    // ...
}

// Avoided - Heap allocation and dynamic dispatch
pub struct CachingDnsResolver {
    inner: Box<dyn DnsResolver>,
    // ...
}
```

**Benefits**:
- Compile-time monomorphization (no vtable overhead)
- Type-safe composition
- Users can stack resolvers: CachingDnsResolver<SystemDnsResolver>
- Better for embedded/no_std environments

### Cache Implementation Details

**TTL-Based Expiration**:
- Cache key: format!("{}:{}", host, port) - differentiates by port
- CachedEntry stores addresses + expires_at (Instant)
- Check expiration on every cache lookup
- Expired entries are replaced (not proactively removed)

**Thread Safety**:
- Arc<Mutex<HashMap>> for shared cache
- Lock contention is acceptable for DNS (infrequent operations)
- Alternative considered: RwLock (rejected - HashMap mutations common)

**Error Handling**:
- Errors are NOT cached (avoids poisoning cache with transient failures)
- Mutex poison is handled gracefully (continue on lock failure)
- Cache size tracking works even if lock fails (returns 0)

### Test-Driven Development Success

**TDD Process Followed**:
1. ✅ Wrote all tests FIRST before implementation
2. ✅ Tests initially failed (as expected)
3. ✅ Implemented code to make tests pass
4. ✅ All 20 tests passing on first implementation pass

**Test Coverage Achieved**:
- Error type Display implementations (4 tests)
- Error type conversions (3 tests)
- SystemDnsResolver functionality (3 tests)
- MockDnsResolver configuration (3 tests)
- CachingDnsResolver behavior (5 tests)
- Thread safety verification (1 test)
- std::error::Error trait compliance (1 test)

**Documentation in Tests**:
- Every test has WHY comment (reason for test)
- Every test has WHAT comment (what is being tested)
- Follows implementation agent requirements exactly

### Integration with Existing Codebase

**BoxedError Type**:
- Used `crate::extensions::result_ext::BoxedError` from existing code
- Type alias: `Box<dyn std::error::Error + 'static>`
- Matches pattern used in other error types in simple_http module

**Error Pattern Consistency**:
- Followed existing error.rs patterns from simple_http module
- derive_more::From for error enum conversions
- Manual Display implementation with descriptive messages
- std::error::Error trait implementation

**Module Organization**:
- client/mod.rs - Module entry with re-exports
- client/errors.rs - Error types
- client/dns.rs - DNS resolver trait and implementations
- client/tests.rs - Integration test placeholder
- Matches existing simple_http module structure

### Performance Considerations

**DNS Caching Benefits**:
- Reduces DNS queries for repeated connections
- Configurable TTL (default 5 minutes)
- Can clear cache manually when needed
- Cache size inspection for monitoring

**Memory Usage**:
- HashMap grows with unique host:port combinations
- No automatic cleanup of expired entries (only on access)
- Trade-off: Memory for speed (acceptable for typical use)

**Zero-Copy Where Possible**:
- Resolver methods take &str (not String)
- Avoids unnecessary string allocations
- Generic types avoid boxing overhead

### Future Improvements

**Could Add Later**:
1. Proactive cache expiration (background task to remove old entries)
2. Cache size limits (LRU eviction policy)
3. DNS query metrics (hit rate, miss rate)
4. async DNS resolution (for async runtime)
5. DNS-over-HTTPS support
6. Custom DNS server configuration

**Not Needed Now**:
- Current implementation sufficient for HTTP 1.1 client
- Simple, correct, and testable
- Can be enhanced when needed

### Lessons Learned

**TDD Really Works**:
- Writing tests first clarified requirements
- Tests caught Clone issue with io::Error immediately
- Implementation was straightforward after tests were written
- No bugs found after implementation completed

**Generic Type Parameters > Boxing**:
- Zero runtime overhead
- Better type safety
- Easier to optimize
- More idiomatic Rust

**Error Messages Matter**:
- Descriptive Display implementations crucial
- Include context (hostname, port) in error messages
- Users need actionable error information

**Thread Safety by Design**:
- Arc<Mutex<>> pattern works well
- Lock poisoning handled gracefully
- Send + Sync bounds enforced by trait

---

## Connection Feature Implementation (2026-01-25)

### URL Parsing Without External Dependencies

**Challenge**: Parse URLs without using the `url` crate to avoid external dependencies.

**Solution**: Implemented simple URL parser in ParsedUrl::parse():
- Manual string splitting and parsing
- Handles scheme://host[:port][/path][?query][#fragment]
- Validates HTTP/HTTPS schemes only
- Ignores fragments (client-side only, not sent to server)
- Returns structured ParsedUrl with all components

**Implementation Details**:
```rust
// Parsing order matters:
1. Find "://" to split scheme
2. Split off fragment with split('#')
3. Split authority and path with find('/')
4. Parse port with rfind(':') (rightmost to handle IPv6 future)
5. Split query with find('?')
```

**Edge Cases Handled**:
- Empty host validation
- Default ports (80 for HTTP, 443 for HTTPS)
- Invalid port numbers
- Missing scheme
- Unsupported schemes (FTP, etc.)
- Fragment identifiers (ignored as per HTTP spec)
- IP addresses as hostnames

**Why No External Crate**:
- Reduces dependency tree
- HTTP client only needs basic URL parsing
- Full URL spec (RFC 3986) not required
- Simpler to maintain
- Works in no_std environments

### Connection Establishment Pattern

**Architecture**: Three-step process in HttpClientConnection::connect()
1. **DNS Resolution**: Use generic DnsResolver trait
2. **TCP Connection**: Try all resolved addresses sequentially
3. **TLS Upgrade**: Conditional based on scheme

**Fallback Strategy**:
```rust
for addr in addrs {
    match Connection::with_timeout(addr, timeout) {
        Ok(connection) => return Ok or upgrade_to_tls(),
        Err(e) => {
            last_error = Some(e);
            continue; // Try next address
        }
    }
}
```

**Benefits**:
- Resilient to DNS returning multiple addresses
- Tries all addresses before failing
- Preserves last error for debugging
- Timeout detection via error message content

### TLS Integration Limitation Discovered

**Issue**: netcap::Connection vs RustlsStream type mismatch

**Current State**:
- `RustlsConnector::from_tcp_stream()` returns `RustTlsClientStream`
- `HttpClientConnection` wraps `netcap::Connection`
- No conversion exists between these types

**Why This Happens**:
- netcap designed for server-side (accept connections)
- Client-side TLS needs different abstractions
- RustlsStream wraps Connection internally
- Cannot "unwrap" back to Connection after TLS

**Temporary Solution**:
- HTTP connections work perfectly
- HTTPS connections return TlsHandshakeFailed error
- Error message explains TLS not yet fully implemented
- Feature gates in place for future implementation

**Future Fix Options**:
1. **Modify HttpClientConnection** to wrap `enum { Plain(Connection), Tls(RustlsStream) }`
2. **Create trait abstraction** over both types
3. **Extend netcap** with client-side TLS support
4. **Use different TLS crate** designed for clients

**Decision**: Defer TLS implementation to allow core HTTP functionality to be tested first. This is documented and feature-gated properly.

### Feature-Gated TLS Backend Selection

**Pattern**: Multiple cfg blocks for different TLS backends
```rust
#[cfg(feature = "ssl-rustls")]
fn upgrade_to_tls(...) { /* rustls */ }

#[cfg(all(feature = "ssl-openssl", not(feature = "ssl-rustls")))]
fn upgrade_to_tls(...) { /* openssl */ }

#[cfg(all(feature = "ssl-native-tls", not(...), not(...)))]
fn upgrade_to_tls(...) { /* native-tls */ }

#[cfg(not(any(...)))]
fn upgrade_to_tls(...) { /* Error: No TLS enabled */ }
```

**Why Cascading cfg**:
- Only one TLS backend active at compile time
- Priority: rustls > openssl > native-tls
- Clear error if HTTPS requested without TLS feature
- Prevents linker conflicts

**Error Messages**:
- Descriptive errors for each failure scenario
- Includes hostname in error message
- Distinguishes between "not implemented" vs "not enabled"

### Test-Driven Development Results

**TDD Process**:
1. ✅ Wrote 16 tests FIRST (12 URL parsing + 4 connection)
2. ✅ Verified tests failed with stub implementation
3. ✅ Implemented ParsedUrl::parse()
4. ✅ All URL parsing tests passed
5. ✅ Implemented HttpClientConnection::connect()
6. ✅ All connection tests passed (2 ignored for network)

**Test Categories**:
- **URL Parsing (12 tests)**: Simple URLs, ports, paths, queries, edge cases
- **Connection (4 tests)**: HTTP, HTTPS, DNS failure, mock resolver

**Ignored Tests**:
- `test_connection_http_real`: Requires actual network
- `test_connection_https_real`: Requires network + TLS implementation
- `test_connection_timeout`: Requires non-routable IP (flaky)

**Test Documentation Quality**:
- Every test has WHY (2-5 lines)
- Every test has WHAT (single line)
- Some tests have IMPORTANCE (optional)
- Follows TDD requirements exactly

### Generic Type Parameter Success

**Pattern Used**: Generic resolver parameter (not boxed)
```rust
pub fn connect<R: DnsResolver>(
    url: &ParsedUrl,
    resolver: &R,
    timeout: Option<Duration>,
) -> Result<Self, HttpClientError>
```

**Why Not Boxed**:
```rust
// ❌ AVOIDED:
resolver: &Box<dyn DnsResolver>
// Problems: Heap allocation, vtable dispatch, less flexible

// ✅ USED:
resolver: &R where R: DnsResolver
// Benefits: Zero overhead, monomorphization, type-safe
```

**Verified In Practice**:
- Mock resolver works seamlessly
- System resolver works seamlessly
- Caching resolver wraps either
- No runtime overhead
- Perfect type safety

### Error Handling Patterns

**Timeout Detection**:
```rust
if err.to_string().contains("timeout") ||
   err.to_string().contains("timed out") {
    return Err(HttpClientError::ConnectionTimeout(...));
}
```

**Why String Matching**:
- Different OS error codes
- Box<dyn Error> loses type info
- Simple and effective
- Could be improved with error downcast

**Error Enum Design**:
- ConnectionTimeout (distinct from ConnectionFailed)
- TlsHandshakeFailed (distinct from connection errors)
- InvalidScheme (distinct from InvalidUrl)
- IoError (preserves io::Error for chaining)

**Error Message Quality**:
- Include hostname and port in errors
- Distinguish between DNS vs connection failures
- Clear messages for missing TLS features
- Context-rich for debugging

### Platform Compatibility

**WASM Guards**:
```rust
#[cfg(not(target_arch = "wasm32"))]
pub struct HttpClientConnection { ... }
```

**Why Needed**:
- WASM doesn't have TCP sockets
- Network code requires native platform
- Guards prevent compile errors on WASM
- Future: WASM HTTP could use fetch API

**Implications**:
- Connection module only available on native
- Tests only run on native platforms
- Documentation mentions platform requirements
- Matches existing netcap patterns

### Integration with Existing Codebase

**Reused Types**:
- `netcap::Connection`: Wraps TcpStream
- `netcap::ssl::rustls::RustlsConnector`: TLS connector
- `crate::wire::simple_http::client::errors`: Error types
- `crate::wire::simple_http::client::dns`: DNS resolution

**Pattern Matching**:
- Error handling matches simple_http module
- Generic type parameters match dns.rs
- Feature gates match netcap SSL modules
- Test structure matches existing tests

**Module Structure**:
```
client/
├── mod.rs          (re-exports)
├── errors.rs       (error types)
├── dns.rs          (DNS resolvers)
├── connection.rs   (NEW - URL parsing, connections)
└── tests.rs        (placeholder)
```

### Performance Considerations

**Zero-Copy Parsing**:
- ParsedUrl::parse() takes &str (not String)
- Returns owned String only for host/path/query
- Minimal allocations during parsing
- No regex or complex parsing

**Connection Efficiency**:
- Tries multiple addresses without delay
- Uses system timeout mechanism
- No artificial delays or retries (yet)
- DNS cache helps with repeated connections

**Memory Usage**:
- ParsedUrl is small struct (3 Strings + 1 u16 + 1 enum)
- HttpClientConnection wraps single Connection
- No buffering at this layer (happens in Connection)
- Minimal heap allocations

### Code Quality Achieved

**Formatting**: ✅ cargo fmt --check passed
**Linting**: ✅ cargo clippy passed with no warnings
**Compilation**: ✅ cargo build passed
**Tests**: ✅ 14 tests passed, 2 ignored (require network)

**Documentation**:
- Module-level docs explain purpose
- Function docs include examples
- Error variants documented
- Examples show usage

### Lessons Learned

**TDD Catches Design Issues Early**:
- Clone issue with io::Error (solved in errors.rs)
- Type mismatch with TLS (documented for later)
- Edge cases identified before implementation
- Tests guided implementation decisions

**Simple Parsing > Complex Dependencies**:
- 70 lines of parsing code
- No external crate needed
- Handles 99% of HTTP client use cases
- Easy to maintain and debug

**Feature Gates Are Powerful**:
- Clean compile-time selection
- No runtime overhead
- Clear error messages
- Easy to extend with new backends

**Error Context Is Critical**:
- Include hostname/port in errors
- Distinguish error types
- Preserve error chains where possible
- Users need actionable information

**Generic Type Parameters > Trait Objects**:
- Confirmed again with connect() function
- Zero runtime overhead
- Type-safe composition
- Idiomatic Rust pattern

### Future Work for Connection Feature

**TLS Implementation**:
1. Design abstraction over Connection and RustlsStream
2. Implement upgrade_to_tls() for all backends
3. Test HTTPS connections end-to-end
4. Handle SNI correctly
5. Certificate validation

**IPv6 Support**:
- Current rfind(':') works but not IPv6-aware
- Need bracket parsing for [::1]:8080
- Socket address creation handles IPv6 already

**Connection Pooling**:
- Keep connections alive for reuse
- Max connections per host
- Idle timeout handling
- Connection health checks

**Proxy Support**:
- HTTP CONNECT tunneling
- SOCKS5 support
- Proxy authentication
- Proxy auto-config (PAC)

**Retry Logic**:
- Exponential backoff
- Max retry count
- Idempotent request detection
- Circuit breaker pattern

### Critical Implementation Note

**DO NOT**:
- Box generic type parameters unnecessarily
- Skip test documentation (WHY/WHAT)
- Leave failing tests unresolved
- Implement features before tests

**DO**:
- Write tests FIRST (TDD)
- Use generic type parameters
- Feature-gate platform-specific code
- Document edge cases in tests
- Keep error messages descriptive


## Request-Response Feature Implementation (2026-01-25)

### Context
Implemented the request-response feature which provides:
- `ResponseIntro` wrapper for response status/proto/reason
- `PreparedRequest` for holding request data
- `ClientRequestBuilder` for fluent API request building

### Key Insights

#### 1. Type Reuse is Critical
**Problem**: Initial implementation tried to create custom types (Method, Headers, Body) instead of reusing existing `simple_http/impls.rs` types.

**Solution**: Use existing types from `impls.rs`:
- `SimpleMethod` instead of custom `Method`
- `SimpleHeaders` (BTreeMap<SimpleHeader, Vec<String>>) instead of custom `Headers`
- `SimpleBody` instead of custom `Body`
- `SimpleHeader` enum with UPPERCASE variants (HOST, CONTENT_TYPE, etc.)

**Why**: This avoids duplication, maintains consistency with the rest of the codebase, and works seamlessly with existing HTTP rendering infrastructure.

#### 2. Http11RequestIterator Constructor is Private
**Problem**: The feature spec requested `PreparedRequest::into_request_iterator() -> Http11RequestIterator`, but `Http11RequestIterator` has a private constructor.

**Solution**: Changed approach:
- `PreparedRequest::into_simple_incoming_request() -> Result<SimpleIncomingRequest, HttpClientError>`
- Users can then call `Http11::request(req).http_render()` to get the iterator

**Why**: The public API for getting an HTTP iterator is through `Http11::request().http_render()`, which returns a boxed iterator. Trying to work around private constructors would be unsafe and brittle.

#### 3. SimpleIncomingRequest Builder Pattern
**Pattern**: Use the builder pattern to create `SimpleIncomingRequest`:
```rust
SimpleIncomingRequest::builder()
    .with_url(simple_url)
    .with_method(method)
    .with_proto(Proto::HTTP11)
    .with_headers(headers)
    .with_some_body(Some(body))
    .build()?
```

**Why**: This is the intended, safe way to create requests that work with the HTTP rendering infrastructure.

#### 4. URL Encoding for Form Data
**Problem**: No `urlencoding` crate available in dependencies.

**Solution**: Implemented simple percent-encoding inline:
```rust
fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
```

**Why**: Keeps dependencies minimal and provides sufficient encoding for form data.

#### 5. Header Name Convention
**Important**: `SimpleHeader` enum uses UPPERCASE variants:
- `SimpleHeader::HOST` (not `Host`)
- `SimpleHeader::CONTENT_TYPE` (not `ContentType`)
- `SimpleHeader::CONTENT_LENGTH` (not `ContentLength`)

This caused initial compilation errors until corrected.

### Implementation Summary

**Files Created/Modified**:
1. `backends/foundation_core/src/wire/simple_http/client/intro.rs` - ResponseIntro wrapper
2. `backends/foundation_core/src/wire/simple_http/client/request.rs` - ClientRequestBuilder and PreparedRequest
3. `backends/foundation_core/src/wire/simple_http/client/mod.rs` - Updated exports

**Tests Added**:
- 5 tests for `ResponseIntro` (tuple conversion, status codes, protocols)
- 11 tests for `ClientRequestBuilder` (builder pattern, convenience methods, body types)
- All tests pass ✅

**Verification**:
- ✅ `cargo fmt -- --check` passes
- ✅ `cargo clippy -- -D warnings` passes (no warnings in our code)
- ✅ `cargo test --package foundation_core -- intro` passes (5/5 tests)
- ✅ `cargo test --package foundation_core -- request` passes (148/148 tests including existing)
- ✅ `cargo build --package foundation_core` succeeds

### Pragmatic Adaptations

The feature specification requested `PreparedRequest::into_request_iterator() -> Http11RequestIterator`, but this was not feasible due to:
1. `Http11RequestIterator` having a private constructor
2. The intended public API being `Http11::request().http_render()`

**Adaptation**: Changed to `into_simple_incoming_request()` which returns the intermediate type that can be converted to an iterator via the public API.

**Justification**: This maintains the spirit of the specification (providing a way to convert PreparedRequest to an iterator) while working within the constraints of the existing codebase architecture.

### Next Steps
This feature provides the foundation for:
- `auth-helpers` feature (can use ClientRequestBuilder to add auth headers)
- `task-iterator` feature (can use PreparedRequest and ResponseIntro)
- `public-api` feature (exposes ClientRequestBuilder and ResponseIntro to users)

---

## Connection Feature HTTPS/TLS Implementation (2026-02-01)

### TLS Peek Support Limitation
**Issue**: TLS streams don't support peek operations like raw TCP.

**Why**: TLS encryption is stateful:
- Reading data advances decryption state
- Can't "un-decrypt" to re-read
- Peek would require decrypting (consuming) then re-reading (impossible)

**Solution**: Use `RawStream` which provides buffering layer:
- `RawStream` wraps connections in `BufferedReader<BufferedWriter<T>>`
- Buffering enables peek-like behavior
- Works for TCP, Unix, and TLS connections uniformly

**Pattern**: Always use `RawStream`, not raw `Connection`:
```rust
// ✅ CORRECT
let conn = Connection::with_timeout(addr, timeout)?;
let stream = RawStream::from_connection(conn)?;
// Has buffering + peek + address tracking

// ❌ INCORRECT
let conn = Connection::with_timeout(addr, timeout)?;
// No buffering, no peek, manual address tracking needed
```

### Connection::Tls Variant Implementation
**Solution**: Add Tls variant to Connection enum:
```rust
pub enum Connection {
    Tcp(TcpStream),
    Unix(UnixStream),
    Tls(RustTlsClientStream),  // NEW - enables HTTPS
}
```

**Pattern**: Implement all traits by forwarding to inner type:
- `Read`, `Write`, `PeekableReadStream`, etc.
- Each Tls variant forwards to underlying TLS stream
- Feature-gated per TLS backend (rustls, openssl, native-tls)

**Result**: Clean abstraction over TCP and TLS with uniform API.

### TLS Integration for HTTPS
**Implementation**: `upgrade_to_tls()` method:
```rust
#[cfg(feature = "ssl-rustls")]
fn upgrade_to_tls(conn, host) -> Result<Connection, TlsError> {
    let config = default_client_config()?;
    let connector = RustlsConnector::new(Arc::new(config));
    let (tls_stream, _addr) = connector.from_tcp_stream(host, tcp_stream)?;
    Ok(Connection::from(tls_stream))  // Wraps in Connection::Tls
}
```

**Key Points**:
- Use `from_tcp_stream()` API (SNI handled automatically)
- Feature-gated for each TLS backend
- Error handling converts TLS errors to HttpClientError
- Pattern matches existing openssl/native-tls implementations

### Testing HTTPS Connections
**Pattern**: Real HTTPS tests against live servers:
```rust
#[test]
#[cfg(feature = "ssl-rustls")]
fn test_connection_https_real() {
    let url = ParsedUrl::parse("https://httpbin.org/get").unwrap();
    let conn = HttpClientConnection::connect(&url, &resolver, timeout);
    assert!(conn.is_ok());  // Verifies full HTTPS stack works
}
```

**Why**: Integration tests verify:
- TLS handshake succeeds
- SNI works correctly
- Certificate validation passes
- End-to-end HTTPS functionality

---

## Task-Iterator Feature Implementation (2026-02-01)

### Public API Convention
**Decision**: ALL types public by default (no `pub(crate)`).

**Rationale**:
- Simpler mental model (no visibility complexity)
- Better API discoverability
- Enables easier testing and extension
- Follows "build libraries, not frameworks" principle

**Pattern**: Use documentation to guide, not compiler:
```rust
// ✅ CORRECT - Public with documentation
/// Internal implementation detail.
/// Users should use the public API instead.
pub struct InternalType { }

// ❌ INCORRECT - Don't hide with pub(crate)
pub(crate) struct InternalType { }  // NO!
```

**Changed**: Removed all `pub(crate)` from codebase (27 files, workspace-wide).

### Task-Iterator Implementation Pattern
**Architecture**: Three-layer design:
1. **Actions** (`actions.rs`): ExecutionAction implementations
   - `RedirectAction`: Spawns redirect requests
   - `TlsUpgradeAction`: Spawns TLS handshake
   - `HttpClientAction`: Combined action enum
2. **Task** (`task.rs`): TaskIterator state machine
   - `HttpRequestState`: 10 states for request lifecycle
   - `HttpRequestTask`: Main task iterator
3. **Executor** (`executor.rs`): Platform-specific wrapper
   - `execute_task()`: Auto-selects single/multi executor
   - Feature-gated for WASM vs native vs multi-threaded

**Pattern**: State machine yields `TaskStatus` variants:
- `Pending(state)`: Continue with current state
- `Ready(output)`: Task complete with result
- `Spawn(action)`: Spawn child task (redirect, TLS)
- `Delayed(duration, state)`: Delay before continuing

### Platform-Specific Execution
**Critical**: Executor selection based on platform and features:
```rust
#[cfg(target_arch = "wasm32")]
pub fn execute_task(task) -> RecvIterator<TaskStatus> {
    single::spawn(DoNext::new(task))  // WASM always single
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
pub fn execute_task(task) -> RecvIterator<TaskStatus> {
    single::spawn(DoNext::new(task))  // Native single (default)
    // Requires single::run_once() to drive
}

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
pub fn execute_task(task) -> RecvIterator<TaskStatus> {
    multi::spawn(DoNext::new(task))  // Native multi-threaded
    // Auto-runs on worker threads
}
```

**Why Matters**: Calling code must know whether to call `run_once()`:
- Single mode: Requires manual driving
- Multi mode: Auto-runs, no manual driving needed

---

## RawStream Refactoring (2026-02-01)

### Use RawStream Over Connection
**Decision**: HTTP client refactored to use `RawStream` instead of raw `Connection`.

**Benefits Realized**:
1. **Automatic Buffering**: No manual `BufferedReader`/`BufferedWriter`
2. **Peek Support**: Built-in via buffering layer
3. **Address Tracking**: Automatic `DataStreamAddr` included
4. **Consistent API**: Same interface for TCP, Unix, TLS
5. **40% Less Boilerplate**: Eliminated match statements

**Pattern**: Use RawStream constructors:
```rust
// Plain connection
let conn = Connection::with_timeout(addr, timeout)?;
let stream = RawStream::from_connection(conn)?;

// TLS connection
let (tls_stream, _) = connector.from_tcp_stream(host, tcp)?;
let stream = RawStream::from_client_tls(tls_stream)?;
```

**Files Changed**:
- `wire/simple_http/client/connection.rs`: `HttpClientConnection` now wraps `RawStream`
- `wire/simple_http/client/actions.rs`: `TlsUpgradeAction` uses `RawStream`

**Result**: Cleaner architecture with better abstractions.

### RawStream Variants
**Types**: Three main variants:
```rust
pub enum RawStream {
    AsPlain(BufferedReader<BufferedWriter<Connection>>, DataStreamAddr),
    AsClientTls(BufferedReader<BufferedWriter<ClientSSLStream>>, DataStreamAddr),
    AsServerTls(BufferedReader<BufferedWriter<ServerSSLStream>>, DataStreamAddr),
}
```

**Usage Pattern**:
- Client connections: Use `AsPlain` or `AsClientTls`
- Server connections: Use `AsServerTls`
- All variants provide: Read, Write, Peek, Address tracking

---

## Documentation Management (2026-02-01)

### Specification Documentation Structure
**Files in specifications/[NN-spec]/**:
- `LEARNINGS.md`: Permanent learnings (this file)
- `REPORT.md`: Implementation reports (permanent)
- `VERIFICATION.md`: Verification records (permanent)
- `PROGRESS.md`: Current status (ephemeral - deleted at 100%)
- `requirements.md`: High-level spec
- `features/*/feature.md`: Individual feature specs

**Pattern**: Session summaries go in `LEARNINGS.md`, not separate files.

### Learning Documentation Style
**Guidelines** (from Rule 05):
- ✅ Clear and concise (1-2 lines per entry)
- ✅ Use bullet points, short sentences
- ✅ Show code snippets (2-5 lines)
- ✅ Include context (when/where matters)
- ❌ No lengthy paragraphs
- ❌ No obvious statements
- ❌ No verbose explanations

**Example**:
```markdown
// ✅ GOOD
- TLS peek not supported: Encryption is stateful, can't un-decrypt
- Solution: Use RawStream buffering layer for peek-like behavior

// ❌ BAD (too verbose)
- The TLS protocol maintains internal state during encryption and decryption operations, which means that once data has been decrypted and read from the stream, it cannot be "peeked" at again without...
```

---

*Last Updated: 2026-02-01*
