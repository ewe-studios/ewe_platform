# Implementation Plan: Public API Feature

## Current Status Assessment

### What's Already Done ✅
- **SimpleHttpClient** (`client.rs`): Fully implemented with builder pattern
  - Configuration management (ClientConfig)
  - All HTTP verb convenience methods (get, post, put, delete, patch, head, options)
  - Builder methods (timeouts, redirects, connection pooling config)
  - Generic DNS resolver support
  - **TRANSITIONAL STATE**: Currently returns `ClientRequestBuilder` (passthrough only - doesn't use client context)

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

**PRIMARY TASK**: Create `ClientRequest` API that wraps TaskIterator execution and update SimpleHttpClient methods to return it

The code has "TRANSITIONAL" comments indicating the HTTP verb methods (get, post, etc.) are incomplete and need to be updated to return `ClientRequest` instead of just passthroughs to `ClientRequestBuilder`.

---

## Implementation Approach

### 1. Create `api.rs` - ClientRequest Type

**File**: `backends/foundation_core/src/wire/simple_http/client/api.rs` (NEW)

**Purpose**: User-facing request execution API that hides all TaskIterator complexity

**Key Type**:
```rust
pub struct ClientRequest<R: DnsResolver> {
    prepared_request: PreparedRequest,
    resolver: R,
    config: ClientConfig,
    // Internal state for progressive reading
    task_state: Option<ClientRequestState<R>>,
}

enum ClientRequestState<R: DnsResolver> {
    NotStarted,
    Executing {
        iter: RecvIterator<TaskStatus<ResponseIntro, HttpRequestState, HttpClientAction<R>>>,
        intro: Option<ResponseIntro>,
        headers: Option<SimpleHeaders>,
    },
    Completed,
}
```

**Methods to Implement**:
```rust
impl<R: DnsResolver> ClientRequest<R> {
    // PUBLIC constructor (not pub(crate) - use pub)
    pub fn new(prepared: PreparedRequest, resolver: R, config: ClientConfig) -> Self;

    // User API methods (all pub)
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
- `send()`: Execute complete request, collect all parts, build SimpleResponse
- `parts()`: Return iterator adapter that drives TaskIterator
- `collect()`: Drive to completion, collect all IncomingResponseParts

**Executor Driving**:
- Single mode (WASM/native without multi): Call `single::run_once()` or `run_until_complete()`
- Multi mode (native with multi): Just consume iterator (threads run automatically)
- Use `ReadyValues::new(iter)` to filter for Ready status values

### 2. Update `client.rs` - Complete TRANSITIONAL Methods

**File**: `backends/foundation_core/src/wire/simple_http/client/client.rs` (MODIFY)

**Rationale**: The "TRANSITIONAL" comment at line 239 explicitly states these methods will return `ClientRequest` once api.rs is implemented. They currently just passthrough to `ClientRequestBuilder::get(url)` without using the client's resolver or config.

**Changes**:
```rust
// Update return type and implementation to USE client context
pub fn get(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
    let builder = ClientRequestBuilder::get(url)?;
    // Apply client's default headers if any
    // Build the request
    let prepared = builder.build()?;
    // Create ClientRequest with client's resolver and config
    Ok(ClientRequest::new(prepared, self.resolver.clone(), self.config.clone()))
}

// Similar updates for post, put, delete, patch, head, options

pub fn request(&self, builder: ClientRequestBuilder) -> Result<ClientRequest<R>, HttpClientError> {
    let prepared = builder.build()?;
    Ok(ClientRequest::new(prepared, self.resolver.clone(), self.config.clone()))
}
```

**Remove**:
```rust
// Delete this TRANSITIONAL comment once complete
// TRANSITIONAL: These will return ClientRequest once api.rs is implemented
```

**Requirements**:
- `DnsResolver` must be `Clone` (already is)
- `ClientConfig` must be `Clone` (already is)

### 3. Update `mod.rs` - Export ClientRequest

**File**: `backends/foundation_core/src/wire/simple_http/client/mod.rs` (MODIFY)

**Changes**:
```rust
mod api;  // Add new module
mod actions;
// ... rest

pub use api::ClientRequest;  // Export public type (pub not pub(crate))
pub use client::{ClientConfig, SimpleHttpClient};
// ... rest
```

### 4. Add Integration Tests

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

### 5. Connection Pooling (Optional - Future)

**Status**: Config exists, but actual pool implementation can be deferred

The `ClientConfig` already has `pool_enabled` and `pool_max_connections` fields. Actual `ConnectionPool` implementation can be a separate feature if needed for performance.

**Current Approach**: Document as "not yet implemented" if accessed

---

## Critical Files

### Files to Create:
1. `backends/foundation_core/src/wire/simple_http/client/api.rs` - **NEW** (all types `pub`)

### Files to Modify:
1. `backends/foundation_core/src/wire/simple_http/client/client.rs` - **UPDATE** transitional methods to return `ClientRequest<R>`
2. `backends/foundation_core/src/wire/simple_http/client/mod.rs` - Add `api` module export (`pub use`)
3. `backends/foundation_core/src/wire/simple_http/client/tests.rs` - Add integration tests

### Files to Read (for patterns):
1. `backends/foundation_core/src/wire/simple_http/client/executor.rs` - execute_task() usage
2. `backends/foundation_core/src/valtron/executors/unified.rs` - ReadyValues pattern
3. `backends/foundation_core/src/wire/simple_http/impls.rs` - SimpleResponse, IncomingResponseParts

---

## Implementation Steps

### Phase 1: Create ClientRequest API (api.rs)
1. Define `ClientRequest` struct with generic DNS resolver (all `pub`)
2. Define internal `ClientRequestState` enum for state management
3. Implement `pub fn new()` constructor
4. Implement `pub fn introduction()` method:
   - Create `HttpRequestTask` from PreparedRequest
   - Call `execute_task()` to spawn
   - Drive executor (platform-aware)
   - Collect intro and headers
   - Store state for subsequent calls
5. Implement `pub fn body()` method:
   - Continue from stored state
   - Drive executor until body received
   - Return SimpleBody
6. Implement `pub fn send()` method:
   - Execute complete request
   - Collect all parts
   - Build SimpleResponse<SimpleBody>
7. Implement `pub fn parts()` method:
   - Return iterator adapter
   - Drive TaskIterator on each next()
8. Implement `pub fn collect()` method:
   - Drive to completion
   - Collect Vec<IncomingResponseParts>

### Phase 2: Complete TRANSITIONAL Methods (client.rs)
1. Import `ClientRequest` from api module
2. **Update (not add)** all HTTP verb methods (get, post, etc):
   - Change return type from `Result<ClientRequestBuilder, _>` to `Result<ClientRequest<R>, _>`
   - Build PreparedRequest from ClientRequestBuilder
   - Pass client's resolver and config to ClientRequest::new()
   - Apply default headers from config if set
3. **Update (not add)** `request()` method similarly
4. **Remove** "TRANSITIONAL" comment once complete

### Phase 3: Update Module Exports (mod.rs)
1. Add `mod api;` declaration
2. Add `pub use api::ClientRequest;` export (not pub(crate))

### Phase 4: Add Tests
1. Add integration tests for each user scenario
2. Test both single and multi executor modes (conditional compilation)
3. Test with mock resolver for determinism
4. Test error cases (DNS failure, connection failure, timeout)

### Phase 5: Verify and Document
1. Run all verification commands
2. Update feature PROGRESS.md
3. Generate VERIFICATION.md

---

## Success Criteria Checklist

From feature.md:

- [ ] `ClientRequest.introduction()` returns ResponseIntro and SimpleHeaders
- [ ] `ClientRequest.body()` returns SimpleBody
- [ ] `ClientRequest.send()` returns SimpleResponse<SimpleBody>
- [ ] `ClientRequest.parts()` returns iterator over IncomingResponseParts
- [ ] `SimpleHttpClient::new()` creates default client ✅ (already works)
- [ ] `SimpleHttpClient::with_resolver()` accepts custom resolver ✅ (already works)
- [ ] All convenience methods (get, post, etc.) return ClientRequest (update transitional methods)
- [ ] Builder methods (config, timeout, etc.) work ✅ (already work)
- [ ] Connection pooling config exists ✅ (implementation optional)
- [ ] Redirect following works (via HttpRequestTask - already implemented)
- [ ] Plain HTTP requests work end-to-end
- [ ] HTTPS requests work (with TLS feature)
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

---

## Key Design Decisions

### 1. Update Transitional Methods (Not Add New Ones)
**Decision**: Update existing methods that return `ClientRequestBuilder`
**Rationale**:
- Code has "TRANSITIONAL" comment indicating incomplete implementation
- Current methods are passthroughs that don't use client context
- Intended design is for these to return `ClientRequest`
- Not a breaking change if code isn't being used yet (transitional state)

### 2. All Public Visibility (Not pub(crate))
**Decision**: Use `pub` for all types and methods in api.rs
**Rationale**: User-facing API should be fully public

### 3. Progressive Reading Support
`ClientRequest` supports both progressive reading (`introduction()` then `body()`) and one-shot reading (`send()`). This requires internal state management.

**Choice**: Use `Option<ClientRequestState>` to track execution state
- `NotStarted`: Initial state
- `Executing`: TaskIterator running, may have partial results
- `Completed`: Request finished

### 4. Executor Driving Strategy
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

### 5. Generic vs Trait Object for DnsResolver
**Choice**: Generic type parameter `<R: DnsResolver>`
- Matches SimpleHttpClient design
- No boxing overhead
- Allows compiler optimization
- Requires `DnsResolver: Clone` (already satisfied)

### 6. Connection Pooling
**Choice**: Defer actual pool implementation
- Config fields exist and documented
- Can be added later without breaking API
- Focus on core request/response flow first

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
cargo test --package foundation_core -- --test-threads=1  # Integration tests
```

### Quality Checks:
```bash
cargo fmt --check
cargo clippy --package foundation_core --lib -- -D warnings
```

### End-to-End Test:
```rust
let client = SimpleHttpClient::new();
let response = client.get("http://httpbin.org/get")?.send()?;
assert_eq!(response.intro.status, 200);
```

---

## Risk Analysis

### Low Risk ✅
- Creating new `api.rs` file (additive change)
- Adding tests (no production impact)
- Module exports (straightforward)

### Medium Risk ⚠️
- Updating `SimpleHttpClient` return types (breaking change)
  - **Mitigation**: TRANSITIONAL comment indicates this is expected/incomplete work
  - Methods currently don't use client context (just passthroughs)
  - Likely not in use yet since marked transitional
- Executor driving logic (platform differences)
  - **Mitigation**: Follow existing patterns from `executor.rs` tests

---

## Estimated Effort

- **Phase 1** (api.rs): 3-4 hours - Core implementation
- **Phase 2** (client.rs): 1 hour - Complete transitional methods
- **Phase 3** (mod.rs): 15 minutes - Module exports
- **Phase 4** (tests): 2-3 hours - Comprehensive testing
- **Phase 5** (verification): 1 hour - Documentation and checks

**Total**: 7-9 hours of implementation work

---

## Next Steps

1. **Get approval** for this plan
2. **Implement Phase 1**: Create api.rs with ClientRequest (all pub)
3. **Implement Phase 2**: Complete transitional methods in client.rs
4. **Implement Phase 3**: Update mod.rs exports
5. **Implement Phase 4**: Add comprehensive tests
6. **Verify**: Run all checks and generate VERIFICATION.md
7. **Commit**: Push completed feature
