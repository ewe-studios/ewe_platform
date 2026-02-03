# Implementation Plan: Public API Feature

## Current Status Assessment

### What's Already Done ✅
- **SimpleHttpClient** (`client.rs`): Fully implemented with builder pattern
  - Configuration management (ClientConfig)
  - All HTTP verb convenience methods (get, post, put, delete, patch, head, options)
  - Builder methods (timeouts, redirects, connection pooling config)
  - Generic DNS resolver support
  - Currently returns `ClientRequestBuilder` (transitional)

- **Internal Machinery** (task-iterator feature): 100% complete
  - `execute_task()` in `executor.rs`
  - `HttpRequestTask` implementing `TaskIterator`
  - `HttpClientAction` for spawnable actions
  - Platform/feature-based executor selection

- **Foundation Types**: All available
  - `ClientRequestBuilder`, `PreparedRequest` (`request.rs`)
  - `ResponseIntro` (`intro.rs`)
  - `DnsResolver` trait with implementations (`dns.rs`)
  - `HttpClientError` (`errors.rs`)
  - `SimpleResponse`, `IncomingResponseParts`, `SimpleHeaders`, `SimpleBody` (from parent `simple_http`)

### What Needs Implementation ⬜

**PRIMARY TASKS**:
1. Create `ClientRequest` API that wraps TaskIterator execution
2. **NEW**: Implement `ConnectionPool` for connection reuse

---

## Connection Pooling Design ✅ RESOLVED

### The Breakthrough Insight

**KEY DISCOVERY**: Pool `SharedByteBufferStream<RawStream>` instead of raw connections!

#### Why This Works Perfectly:

1. **Already Cloneable** - `SharedByteBufferStream` implements `Clone` via `Arc<RwLock<>>`
2. **Already Owns Stream** - It wraps `RawStream` with buffering
3. **Thread-Safe by Design** - Uses `Arc<RwLock<>>` internally
4. **Already Used Everywhere** - `HttpResponseReader` already takes it
5. **No Ownership Issues** - Multiple references can exist via cloning

#### Architecture:

```rust
pub struct ConnectionPool {
    // Pool SharedByteBufferStream<RawStream> keyed by "host:port"
    streams: Arc<Mutex<HashMap<String, VecDeque<PooledStream>>>>,
    max_per_host: usize,
    max_idle_time: Duration,
}

struct PooledStream {
    stream: SharedByteBufferStream<RawStream>,  // ← Pool THIS!
    last_used: Instant,
    host: String,
    port: u16,
}
```

#### Benefits:

✅ **No extraction needed** - `SharedByteBufferStream` is cloneable
✅ **Natural fit** - Already used in response reading pipeline
✅ **Thread-safe** - `Arc<RwLock<>>` handles concurrency
✅ **Minimal changes** - Just clone before giving to reader
✅ **Connection reuse** - Keep stream alive between requests

#### Usage Flow:

```rust
// 1. Check pool first
let shared_stream = if let Some(stream) = pool.checkout(host, port) {
    stream  // Reuse existing
} else {
    let connection = HttpClientConnection::connect(...)?;
    SharedByteBufferStream::rwrite(connection.take_stream())
};

// 2. Clone for reader
let reader = HttpResponseReader::new(shared_stream.clone(), SimpleHttpBody);

// 3. After response complete, return to pool
if config.pool_enabled {
    pool.checkin(host, port, shared_stream);
}
```

---

## Implementation Approach

### 1. Create `pool.rs` - ConnectionPool (NEW)

**File**: `backends/foundation_core/src/wire/simple_http/client/pool.rs` (NEW)

**Purpose**: Manage pooled `SharedByteBufferStream<RawStream>` connections for reuse

**Key Type**:
```rust
pub struct ConnectionPool {
    streams: Arc<Mutex<HashMap<String, VecDeque<PooledStream>>>>,
    max_per_host: usize,
    max_total: usize,
    max_idle_time: Duration,
}

struct PooledStream {
    stream: SharedByteBufferStream<RawStream>,
    last_used: Instant,
    host: String,
    port: u16,
}
```

**Methods to Implement**:
```rust
impl ConnectionPool {
    pub fn new(max_per_host: usize, max_idle_time: Duration) -> Self;
    pub fn checkout(&self, host: &str, port: u16) -> Option<SharedByteBufferStream<RawStream>>;
    pub fn checkin(&self, host: &str, port: u16, stream: SharedByteBufferStream<RawStream>);
    pub fn cleanup_stale(&self);
    pub fn clear(&self);
}
```

**Implementation Strategy**:
- Use `Arc<Mutex<HashMap<...>>>` for thread-safe pooling
- Implement LRU-style eviction when pool is full
- Add staleness checking based on `last_used` timestamp
- Per-host connection limits to prevent overwhelming servers
- Optional: Add connection health check before checkout

### 2. Create `api.rs` - ClientRequest Type

**File**: `backends/foundation_core/src/wire/simple_http/client/api.rs` (NEW)

**Purpose**: User-facing request execution API that hides all TaskIterator complexity

**Key Type**:
```rust
pub struct ClientRequest<R: DnsResolver> {
    prepared_request: PreparedRequest,
    resolver: R,
    config: ClientConfig,
    pool: Option<Arc<ConnectionPool>>,  // NEW: Connection pool
    // Internal state for progressive reading
    task_state: Option<ClientRequestState<R>>,
}

enum ClientRequestState<R: DnsResolver> {
    NotStarted,
    Executing {
        iter: RecvIterator<TaskStatus<ResponseIntro, HttpRequestState, HttpClientAction<R>>>,
        intro: Option<ResponseIntro>,
        headers: Option<SimpleHeaders>,
        stream: Option<SharedByteBufferStream<RawStream>>,  // NEW: For pooling
    },
    Completed,
}
```

**Methods to Implement**:
```rust
impl<R: DnsResolver> ClientRequest<R> {
    // Internal constructor (called by SimpleHttpClient)
    pub(crate) fn new(
        prepared: PreparedRequest,
        resolver: R,
        config: ClientConfig,
        pool: Option<Arc<ConnectionPool>>  // NEW
    ) -> Self;

    // User API methods
    pub fn introduction(&mut self) -> Result<(ResponseIntro, SimpleHeaders), HttpClientError>;
    pub fn body(&mut self) -> Result<SimpleBody, HttpClientError>;
    pub fn send(mut self) -> Result<SimpleResponse<SimpleBody>, HttpClientError>;
    pub fn parts(self) -> impl Iterator<Item = Result<IncomingResponseParts, HttpClientError>>;
    pub fn collect(self) -> Result<Vec<IncomingResponseParts>, HttpClientError>;
}
```

**Implementation Strategy**:
- `introduction()`: Execute HttpRequestTask via `execute_task()`, drive executor until intro/headers received
- `body()`: Continue driving iterator to read body parts
- `send()`: Execute complete request, collect all parts, build SimpleResponse, return stream to pool
- `parts()`: Return iterator adapter that drives TaskIterator
- `collect()`: Drive to completion, collect all IncomingResponseParts

**Pool Integration**:
- Pass pool to HttpRequestTask for checkout/checkin
- Store stream reference in ClientRequestState for later pool return
- On completion/drop, return stream to pool if enabled

### 3. Update `task.rs` - Use Connection Pool

**File**: `backends/foundation_core/src/wire/simple_http/client/task.rs` (MODIFY)

**Changes**:
```rust
pub struct HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    // ... existing fields ...
    pool: Option<Arc<ConnectionPool>>,  // NEW
}

// In Connecting state:
let shared_stream = if let Some(pool) = &self.pool {
    if let Some(stream) = pool.checkout(host, port) {
        tracing::debug!("Reusing pooled connection");
        stream
    } else {
        create_new_connection_and_wrap()
    }
} else {
    create_new_connection_and_wrap()
};

// Store stream for later return to pool
self.shared_stream = Some(shared_stream.clone());

// Create reader
let reader = HttpResponseReader::new(shared_stream, SimpleHttpBody);
```

### 4. Update `client.rs` - Return ClientRequest & Create Pool

**File**: `backends/foundation_core/src/wire/simple_http/client/client.rs` (MODIFY)

**Changes**:
```rust
pub struct SimpleHttpClient<R: DnsResolver = SystemDnsResolver> {
    resolver: R,
    config: ClientConfig,
    pool: Option<Arc<ConnectionPool>>,  // NEW
}

impl SimpleHttpClient<SystemDnsResolver> {
    pub fn new() -> Self {
        let config = ClientConfig::default();
        let pool = if config.pool_enabled {
            Some(Arc::new(ConnectionPool::new(
                config.pool_max_connections,
                Duration::from_secs(60),  // Default idle timeout
            )))
        } else {
            None
        };

        Self { resolver: SystemDnsResolver, config, pool }
    }
}

// Change return type from ClientRequestBuilder to ClientRequest
pub fn get(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
    let builder = ClientRequestBuilder::new("GET", url)?;
    let prepared = builder.build()?;
    Ok(ClientRequest::new(
        prepared,
        self.resolver.clone(),
        self.config.clone(),
        self.pool.clone(),  // NEW
    ))
}

// Similar for post, put, delete, patch, head, options

pub fn request(&self, builder: ClientRequestBuilder) -> Result<ClientRequest<R>, HttpClientError> {
    let prepared = builder.build()?;
    Ok(ClientRequest::new(
        prepared,
        self.resolver.clone(),
        self.config.clone(),
        self.pool.clone(),  // NEW
    ))
}
```

### 5. Update `mod.rs` - Export New Types

**File**: `backends/foundation_core/src/wire/simple_http/client/mod.rs` (MODIFY)

**Changes**:
```rust
mod api;   // NEW
mod pool;  // NEW
mod actions;
// ... rest

pub use api::ClientRequest;      // NEW
pub use pool::ConnectionPool;    // NEW
pub use client::{ClientConfig, SimpleHttpClient};
// ... rest
```

### 6. Add Integration Tests

**File**: `backends/foundation_core/src/wire/simple_http/client/tests.rs` or new test module

**Test Cases**:
- ✅ `test_client_get_introduction()` - Get intro and headers
- ✅ `test_client_get_body()` - Read body after introduction
- ✅ `test_client_get_send()` - Full request with send()
- ✅ `test_client_get_parts()` - Iterate over parts
- ✅ `test_client_post_with_body()` - POST request
- ✅ `test_client_https()` - HTTPS request (with TLS feature)
- ✅ `test_client_redirects()` - Redirect following
- ✅ `test_client_custom_resolver()` - With MockDnsResolver
- ✅ `test_client_timeout_config()` - Timeout configuration
- ✅ `test_client_max_redirects()` - Redirect limit
- ✅ **NEW** `test_pool_connection_reuse()` - Verify pooling works
- ✅ **NEW** `test_pool_max_per_host()` - Verify per-host limits
- ✅ **NEW** `test_pool_stale_cleanup()` - Verify idle timeout
- ✅ **NEW** `test_pool_disabled()` - Verify no pooling when disabled

---

## Critical Files

### Files to Create:
1. `backends/foundation_core/src/wire/simple_http/client/pool.rs` - **NEW**
2. `backends/foundation_core/src/wire/simple_http/client/api.rs` - **NEW**

### Files to Modify:
1. `backends/foundation_core/src/wire/simple_http/client/task.rs` - Add pool parameter
2. `backends/foundation_core/src/wire/simple_http/client/client.rs` - Update return types, add pool
3. `backends/foundation_core/src/wire/simple_http/client/mod.rs` - Add api and pool module exports
4. `backends/foundation_core/src/wire/simple_http/client/tests.rs` - Add integration tests

### Files to Read (for patterns):
1. `backends/foundation_core/src/wire/simple_http/client/executor.rs` - execute_task() usage
2. `backends/foundation_core/src/valtron/executors/unified.rs` - ReadyValues pattern
3. `backends/foundation_core/src/wire/simple_http/impls.rs` - SimpleResponse, IncomingResponseParts
4. `backends/foundation_core/src/io/ioutils/mod.rs` - SharedByteBufferStream implementation

---

## Implementation Steps

### Phase 1: Create ConnectionPool (pool.rs)
1. Define `ConnectionPool` struct with Arc<Mutex<HashMap>>
2. Define internal `PooledStream` struct
3. Implement constructor with max_per_host and max_idle_time
4. Implement `checkout()` method:
   - Check HashMap for host:port key
   - Validate stream isn't stale
   - Return None if not found or stale
5. Implement `checkin()` method:
   - Create key from host:port
   - Add to HashMap queue
   - Enforce per-host limit (LRU eviction)
6. Implement `cleanup_stale()` method:
   - Iterate all pools
   - Remove entries older than max_idle_time
7. Implement `clear()` method for testing
8. Add unit tests

### Phase 2: Create ClientRequest API (api.rs)
1. Define `ClientRequest` struct with generic DNS resolver
2. Define internal `ClientRequestState` enum for state management
3. Implement constructor (`new()` - pub(crate)) with pool parameter
4. Implement `introduction()` method:
   - Create `HttpRequestTask` from PreparedRequest
   - Pass pool to task
   - Call `execute_task()` to spawn
   - Drive executor (platform-aware)
   - Collect intro and headers
   - Store state for subsequent calls
5. Implement `body()` method:
   - Continue from stored state
   - Drive executor until body received
   - Return SimpleBody
6. Implement `send()` method:
   - Execute complete request
   - Collect all parts
   - Build SimpleResponse<SimpleBody>
   - Return stream to pool if enabled
7. Implement `parts()` method:
   - Return iterator adapter
   - Drive TaskIterator on each next()
8. Implement `collect()` method:
   - Drive to completion
   - Collect Vec<IncomingResponseParts>
9. Implement Drop trait to return stream to pool

### Phase 3: Update HttpRequestTask (task.rs)
1. Add `pool: Option<Arc<ConnectionPool>>` field
2. Add `shared_stream: Option<SharedByteBufferStream<RawStream>>` field
3. Update constructor to accept pool parameter
4. In Connecting state:
   - Try pool.checkout() first
   - If found, use pooled stream
   - If not found, create new connection
   - Wrap RawStream in SharedByteBufferStream
   - Store stream reference for later return
5. After response complete:
   - Return stream to pool if enabled

### Phase 4: Update SimpleHttpClient (client.rs)
1. Add `pool: Option<Arc<ConnectionPool>>` field
2. In `new()`, create pool if config.pool_enabled
3. Update all HTTP verb methods (get, post, etc):
   - Change return type to `Result<ClientRequest<R>, HttpClientError>`
   - Build PreparedRequest from ClientRequestBuilder
   - Construct and return ClientRequest with pool
4. Update `request()` method similarly
5. Remove "TRANSITIONAL" comment

### Phase 5: Update Module Exports (mod.rs)
1. Add `mod api;` declaration
2. Add `mod pool;` declaration
3. Add `pub use api::ClientRequest;` export
4. Add `pub use pool::ConnectionPool;` export

### Phase 6: Add Tests
1. Add unit tests for ConnectionPool
2. Add integration tests for ClientRequest API
3. Add tests for pooling behavior
4. Test both single and multi executor modes (conditional compilation)
5. Test with mock resolver for determinism
6. Test error cases (DNS failure, connection failure, timeout)
7. Test pool limits and cleanup

### Phase 7: Verify and Document
1. Run all verification commands
2. Update this PLAN.md with results
3. Generate VERIFICATION.md

---

## Success Criteria Checklist

From feature requirements:

### Core API:
- [ ] `ClientRequest.introduction()` returns ResponseIntro and SimpleHeaders
- [ ] `ClientRequest.body()` returns SimpleBody
- [ ] `ClientRequest.send()` returns SimpleResponse<SimpleBody>
- [ ] `ClientRequest.parts()` returns iterator over IncomingResponseParts
- [ ] `SimpleHttpClient::new()` creates default client ✅ (already works)
- [ ] `SimpleHttpClient::with_resolver()` accepts custom resolver ✅ (already works)
- [ ] All convenience methods (get, post, etc.) work (need return type update)
- [ ] Builder methods (config, timeout, etc.) work ✅ (already work)

### Connection Pooling:
- [ ] `ConnectionPool` struct implemented and exported
- [ ] Pool checkout returns existing connections
- [ ] Pool checkin stores connections for reuse
- [ ] Per-host connection limits enforced
- [ ] Stale connection cleanup works
- [ ] Pool can be disabled via config
- [ ] Pool is thread-safe (Arc<Mutex<>>)
- [ ] Pools `SharedByteBufferStream<RawStream>` (not raw RawStream)

### Request Execution:
- [ ] Redirect following works (via HttpRequestTask - already implemented)
- [ ] `pub mod client` added to `simple_http/mod.rs` ✅ (check parent module)
- [ ] Feature flag `multi` added to Cargo.toml ✅ (check if exists)
- [ ] Plain HTTP requests work end-to-end
- [ ] HTTPS requests work (with TLS feature)

### Quality:
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Pool tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

---

## Key Design Decisions

### 1. Connection Pooling Strategy ✅ FINAL
**Decision**: Pool `SharedByteBufferStream<RawStream>` instead of raw connections

**Rationale**:
- `SharedByteBufferStream` is already cloneable (Arc<RwLock<>>)
- Already used throughout the response reading pipeline
- Thread-safe by design
- No ownership extraction needed
- Natural fit with existing architecture

**Alternatives Considered**:
- ❌ Pool `RawStream` directly - Ownership issues
- ❌ Pool `HttpClientConnection` - Lifetime issues
- ✅ Pool `SharedByteBufferStream<RawStream>` - Perfect fit!

### 2. Progressive Reading Support
`ClientRequest` supports both progressive reading (`introduction()` then `body()`) and one-shot reading (`send()`). This requires internal state management.

**Choice**: Use `Option<ClientRequestState>` to track execution state
- `NotStarted`: Initial state
- `Executing`: TaskIterator running, may have partial results
- `Completed`: Request finished

### 3. Executor Driving Strategy
Must handle both single and multi-threaded executors.

**Choice**: Platform-aware driving
```rust
#[cfg(any(target_arch = "wasm32", not(feature = "multi")))]
{
    use crate::valtron::single;
    single::run_until_complete();  // Drive single executor
}

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
{
    // Multi executor runs automatically, just consume iterator
}
```

### 4. Generic vs Trait Object for DnsResolver
**Choice**: Generic type parameter `<R: DnsResolver>`
- Matches SimpleHttpClient design
- No boxing overhead
- Allows compiler optimization
- Requires `DnsResolver: Clone` (already satisfied)

### 5. Pool Lifecycle Management
**Choice**: Create pool in SimpleHttpClient constructor if enabled
- Pool is `Arc<>` shared between all requests
- Clone pool Arc when creating ClientRequest
- Pool persists for lifetime of SimpleHttpClient
- Thread-safe via Mutex on internal HashMap

---

## Verification Plan

### Build Checks:
```bash
cargo build --package foundation_core
cargo build --package foundation_core --features multi
cargo build --package foundation_core --features ssl-rustls
cargo build --package foundation_core --all-features
```

### Test Checks:
```bash
cargo test --package foundation_core --lib wire::simple_http::client
cargo test --package foundation_core --lib wire::simple_http::client::api
cargo test --package foundation_core --lib wire::simple_http::client::pool
cargo test --package foundation_core -- --test-threads=1  # Integration tests
```

### Quality Checks:
```bash
cargo fmt --check
cargo clippy --package foundation_core --lib -- -D warnings
```

### End-to-End Test:
```rust
let client = SimpleHttpClient::new()
    .enable_pool(10);  // Enable pooling with 10 connections per host

let response = client.get("http://httpbin.org/get")?.send()?;
assert_eq!(response.intro.status, 200);

// Second request should reuse pooled connection
let response2 = client.get("http://httpbin.org/get")?.send()?;
assert_eq!(response2.intro.status, 200);
```

---

## Risk Analysis

### Low Risk ✅
- Creating new `api.rs` file (additive change)
- Creating new `pool.rs` file (additive change)
- Adding tests (no production impact)
- Module exports (straightforward)

### Medium Risk ⚠️
- Changing `SimpleHttpClient` return types (breaking change for existing users)
  - **Mitigation**: This is likely not used yet (transitional comment indicates incomplete work)
- Executor driving logic (platform differences)
  - **Mitigation**: Follow existing patterns from `executor.rs` tests
- Pool thread-safety
  - **Mitigation**: Use Arc<Mutex<>> pattern, add stress tests

### Resolved Risks ✅
- ~~Connection pooling implementation~~ - **RESOLVED**: Pool SharedByteBufferStream
- ~~Connection ownership~~ - **RESOLVED**: SharedByteBufferStream is cloneable
- ~~Thread safety~~ - **RESOLVED**: Arc<RwLock<>> already built-in

---

## Estimated Effort

- **Phase 1** (pool.rs): 2-3 hours - ConnectionPool implementation
- **Phase 2** (api.rs): 3-4 hours - Core ClientRequest implementation
- **Phase 3** (task.rs): 1-2 hours - Pool integration
- **Phase 4** (client.rs): 1 hour - Return type updates + pool creation
- **Phase 5** (mod.rs): 15 minutes - Module exports
- **Phase 6** (tests): 3-4 hours - Comprehensive testing (including pool tests)
- **Phase 7** (verification): 1 hour - Documentation and checks

**Total**: 11-15 hours of implementation work (up from 7-9 hours with pool)

---

## Next Steps

1. ~~**Analyze connection pooling**~~ ✅ DONE
2. ~~**Refine implementation plan**~~ ✅ DONE
3. **Implement Phase 1**: Create pool.rs with ConnectionPool
4. **Implement Phase 2**: Create api.rs with ClientRequest
5. **Implement Phase 3**: Update task.rs to use pool
6. **Implement Phase 4**: Update client.rs return types and add pool
7. **Implement Phase 5**: Update mod.rs exports
8. **Implement Phase 6**: Add comprehensive tests
9. **Verify**: Run all checks and generate VERIFICATION.md
10. **Commit**: Push completed feature
