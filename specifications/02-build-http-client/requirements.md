---
description: Create an HTTP 1.1 client using existing simple_http module structures with iterator-based patterns and valtron executors
status: pending
priority: high
created: 2026-01-18
author: "Main Agent"
metadata:
  version: "2.0"
  last_updated: 2026-01-18
  estimated_effort: "large"
  tags:
    - http-client
    - networking
    - rust
    - iterator-patterns
    - valtron-executors
builds_on: []
related_specs: []
has_features: true
features:
  - tls-verification
  - foundation
  - connection
  - request-response
  - task-iterator
  - public-api
---

# HTTP 1.1 Client - Requirements

## Overview

Create an HTTP 1.1 client using the existing `simple_http` module structures, leveraging the iterator-based patterns and executor system from `valtron` without using async/await primitives, tokio, or any async runtime. The client will use valtron's `single` and `multi` executor modules to execute tasks in an async-like manner through the `TaskIterator` pattern.

## Features

This specification is broken into 6 features for reduced context size:

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| [tls-verification](./features/tls-verification/feature.md) | Verify and fix TLS backends (rustls, openssl, native-tls) | None |
| [foundation](./features/foundation/feature.md) | Error types and DNS resolution | tls-verification |
| [connection](./features/connection/feature.md) | URL parsing, TCP, TLS | foundation |
| [request-response](./features/request-response/feature.md) | Request builder, response types | connection |
| [task-iterator](./features/task-iterator/feature.md) | TaskIterator, ExecutionAction, executors | request-response |
| [public-api](./features/public-api/feature.md) | User-facing API, SimpleHttpClient, integration | task-iterator |

**Agents MUST read the relevant feature.md and tasks.md for detailed requirements.**

## Requirements Conversation Summary

### User's Initial Request

Build an HTTP 1.1 client that:
- Uses existing structures from `simple_http` module
- Leverages iterator patterns from `valtron` for non-blocking streaming
- Uses valtron's `single` and `multi` executor modules for async-like execution
- Avoids async/await - purely synchronous with iterator-based streaming
- Supports pluggable DNS resolution
- Has configurable connection pooling
- Has configurable redirect handling
- Reuses TLS infrastructure from `netcap`

### Clarifying Questions Asked

1. **Connection Pooling**: Optional/configurable - support both modes
2. **Redirect Handling**: Configurable - user sets max redirects or disables
3. **DNS Resolution**: Pluggable - custom resolver trait with implementations
4. **Code Location**: `wire/simple_http/client/` submodule
5. **Error Handling**: Custom errors using `derive_more::From`, `Debug`, `Display`
6. **Type Flexibility**: Generic types (e.g., `<T: StreamIterator>`)
7. **Execution Model**: Use valtron's `multi` and `single` modules via `TaskIterator`

### Final Requirements Agreement

- HTTP 1.1 client using existing `simple_http` structures
- **Clean user-facing API** that hides TaskIterator complexity
- Iterator-based patterns via `TaskIterator` trait internally
- Use valtron's `single::spawn()` and `multi::spawn()` for execution
- **Feature-gated executor selection**: `multi` feature for multi-threaded
- Custom error types with `derive_more::From`, `Debug`, `Display`
- Generic type parameters (not boxed) for flexibility
- Pluggable DNS resolution via trait
- Optional connection pooling (configurable)
- Configurable redirect following
- TLS support via existing `netcap` infrastructure

## User-Facing API (High-Level)

```rust
// Create client
let http_client = SimpleHttpClient::new();

// Simple usage
let response = http_client.get("http://google.com").send()?;

// Streaming usage
let mut request = http_client.get("http://google.com");
let (intro, headers) = request.introduction()?;
let body = request.body()?;

// Power user: iterate over parts
for part in http_client.get("http://google.com").parts() { ... }
```

See [public-api feature](./features/public-api/feature.md) for detailed API design.

## File Structure

```
backends/foundation_core/src/wire/
├── simple_http/
│   ├── mod.rs           (modify - add pub mod client)
│   └── client/          (NEW)
│       ├── mod.rs       (module entry, re-exports)
│       ├── errors.rs    (HttpClientError, DnsError)
│       ├── dns.rs       (DnsResolver trait + implementations)
│       ├── connection.rs (ParsedUrl, HttpClientConnection)
│       ├── request.rs   (ClientRequestBuilder, PreparedRequest)
│       ├── intro.rs     (ResponseIntro wrapper)
│       ├── actions.rs   (ExecutionAction implementations)
│       ├── task.rs      (HttpRequestTask - internal TaskIterator)
│       ├── executor.rs  (Feature-gated single/multi selection)
│       ├── api.rs       (ClientRequest - user-facing)
│       ├── client.rs    (SimpleHttpClient - main entry point)
│       └── pool.rs      (ConnectionPool - optional)
```

## Success Criteria (Overall)

- [ ] All feature success criteria met
- [ ] Plain HTTP requests work end-to-end
- [ ] HTTPS requests work (with TLS feature)
- [ ] DNS resolution with caching works
- [ ] Redirect following works (configurable)
- [ ] Connection pooling works (when enabled)
- [ ] Works with single-threaded executor
- [ ] Works with multi-threaded executor (feature-gated)
- [ ] All tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`
- [ ] Compiles with all TLS feature combinations

## Module Documentation References

### simple_http/client (NEW)
- **Documentation**: `documentation/simple_http_client/doc.md` (to be created)
- **Purpose**: HTTP 1.1 client implementation

### Existing Modules (READ FIRST)
- `wire/simple_http/impls.rs` - HTTP structures to reuse
- `netcap/connection/mod.rs` - Connection type
- `netcap/ssl/rustls.rs` - TLS connector
- `valtron/executors/task.rs` - TaskIterator trait
- `valtron/executors/single/mod.rs` - Single-threaded executor
- `valtron/executors/multi/mod.rs` - Multi-threaded executor

## Verification Scripts

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core
cargo build --package foundation_core
cargo build --package foundation_core --features multi
cargo build --package foundation_core --features ssl-rustls
cargo build --package foundation_core --all-features
```

## Important Notes for Agents

### Before Starting Any Feature
- **MUST READ** this requirements.md first
- **MUST READ** the specific feature's `feature.md` and `tasks.md`
- **MUST VERIFY** dependent features are complete
- **MUST READ** referenced module documentation

### Implementation Guidelines
- Follow existing code patterns in the codebase
- Use `derive_more::From` for error enums
- Use generic type parameters instead of boxed types
- Implement `TaskIterator` for async-like execution
- Add `#[cfg(not(target_arch = "wasm32"))]` where appropriate
- Ensure TLS code is behind feature gates

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
