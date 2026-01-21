---
description: Create an HTTP 1.1 client using existing simple_http module structures with iterator-based patterns and valtron executors
status: pending
priority: high
created: 2026-01-18
author: "Main Agent"
metadata:
  version: "3.0"
  last_updated: 2026-01-19
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
  - valtron-utilities
  - tls-verification
  - foundation
  - compression
  - connection
  - proxy-support
  - request-response
  - auth-helpers
  - task-iterator
  - public-api
  - cookie-jar
  - middleware
  - websocket
---

# HTTP 1.1 Client - Requirements

## Overview

Create an HTTP 1.1 client using the existing `simple_http` module structures, leveraging the iterator-based patterns and executor system from `valtron` without using async/await primitives, tokio, or any async runtime. The client will use valtron's `single` and `multi` executor modules to execute tasks in an async-like manner through the `TaskIterator` pattern.

## Features

This specification is broken into 13 features for reduced context size. Features are ordered by dependency.

### Core Features (Required)

| # | Feature | Description | Dependencies |
|---|---------|-------------|--------------|
| 0 | [valtron-utilities](./features/valtron-utilities/feature.md) | Reusable ExecutionAction types, unified executor, state machine helpers | None |
| 1 | [tls-verification](./features/tls-verification/feature.md) | Verify and fix TLS backends (rustls, openssl, native-tls) | valtron-utilities |
| 2 | [foundation](./features/foundation/feature.md) | Error types and DNS resolution | tls-verification |
| 3 | [compression](./features/compression/feature.md) | gzip, deflate, brotli support | foundation |
| 4 | [connection](./features/connection/feature.md) | URL parsing, TCP, TLS | foundation |
| 5 | [proxy-support](./features/proxy-support/feature.md) | HTTP/HTTPS/SOCKS5 proxy | connection |
| 6 | [request-response](./features/request-response/feature.md) | Request builder, response types | connection |
| 7 | [auth-helpers](./features/auth-helpers/feature.md) | Basic, Bearer, Digest auth | request-response |
| 8 | [task-iterator](./features/task-iterator/feature.md) | TaskIterator, ExecutionAction, executors | request-response, valtron-utilities |
| 9 | [public-api](./features/public-api/feature.md) | User-facing API, SimpleHttpClient, integration | task-iterator |

### Extended Features (Optional)

| # | Feature | Description | Dependencies |
|---|---------|-------------|--------------|
| 10 | [cookie-jar](./features/cookie-jar/feature.md) | Automatic cookie handling | public-api |
| 11 | [middleware](./features/middleware/feature.md) | Request/response interceptors | public-api |
| 12 | [websocket](./features/websocket/feature.md) | WebSocket client and server | connection, public-api |

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

The client provides a clean, simple API that hides internal complexity:

```rust
let client = SimpleHttpClient::new();
let response = client.get("https://example.com").send()?;
```

See [public-api feature](./features/public-api/feature.md) for detailed API design and examples.

## File Structure

```
backends/foundation_core/src/
├── valtron/executors/          (valtron-utilities additions)
│   ├── actions.rs              (NEW - Reusable ExecutionAction types)
│   ├── unified.rs              (NEW - Feature-gated unified executor)
│   ├── state_machine.rs        (NEW - State machine helpers)
│   └── wrappers.rs             (NEW - Retry/timeout wrappers)
├── wire/simple_http/
│   ├── mod.rs                  (modify - add pub mod client)
│   └── client/                 (NEW - HTTP client implementation)
│       ├── mod.rs              (module entry, re-exports)
│       ├── errors.rs           (HttpClientError, DnsError)
│       ├── dns.rs              (DnsResolver trait + implementations)
│       ├── compression.rs      (Compression/decompression)
│       ├── connection.rs       (ParsedUrl, HttpClientConnection)
│       ├── proxy.rs            (Proxy configuration and tunneling)
│       ├── request.rs          (ClientRequestBuilder, PreparedRequest)
│       ├── auth.rs             (Authentication helpers)
│       ├── intro.rs            (ResponseIntro wrapper)
│       ├── actions.rs          (ExecutionAction implementations)
│       ├── task.rs             (HttpRequestTask - internal TaskIterator)
│       ├── executor.rs         (Feature-gated single/multi selection)
│       ├── api.rs              (ClientRequest - user-facing)
│       ├── client.rs           (SimpleHttpClient - main entry point)
│       ├── pool.rs             (ConnectionPool - optional)
│       ├── cookie.rs           (Cookie, CookieJar)
│       ├── middleware.rs       (Middleware trait and chain)
│       ├── extensions.rs       (Request extensions)
│       └── websocket/          (WebSocket support)
│           ├── mod.rs          (re-exports)
│           ├── frame.rs        (WebSocketFrame, Opcode)
│           ├── message.rs      (WebSocketMessage)
│           ├── client.rs       (WebSocketClient, handshake)
│           ├── server.rs       (WebSocketUpgrade)
│           ├── connection.rs   (WebSocketConnection)
│           └── error.rs        (WebSocketError)
```

## Success Criteria (Overall)

### Core Functionality
- [ ] All feature success criteria met
- [ ] Plain HTTP requests work end-to-end
- [ ] HTTPS requests work (with TLS feature)
- [ ] DNS resolution with caching works
- [ ] Redirect following works (configurable)
- [ ] Connection pooling works (when enabled)
- [ ] Works with single-threaded executor
- [ ] Works with multi-threaded executor (feature-gated)

### Extended Functionality
- [ ] Compression (gzip, deflate, brotli) works
- [ ] Proxy support (HTTP, HTTPS, SOCKS5) works
- [ ] Authentication helpers work
- [ ] Cookie jar works
- [ ] Middleware system works
- [ ] WebSocket client works
- [ ] WebSocket server works

### Quality
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

## Agent Rules Reference

**MANDATORY**: All agents working on this specification MUST load the rules listed below.

### Location Headers
- **Rules Location**: `.agents/rules/`
- **Stacks Location**: `.agents/stacks/`
- **Skills Location**: `.agents/skills/`

### Mandatory Rules for All Agents

Load these rules from `.agents/rules/`:

| Rule | File | Purpose |
|------|------|---------|
| 01 | `.agents/rules/01-rule-naming-and-structure.md` | File naming conventions |
| 02 | `.agents/rules/02-rules-directory-policy.md` | Directory policies |
| 03 | `.agents/rules/03-dangerous-operations-safety.md` | Dangerous operations safety |
| 04 | `.agents/rules/04-work-commit-and-push-rules.md` | Work commit and push rules |

### Role-Specific Rules

Load additional rules from `.agents/rules/` based on your role:

| Agent Type | Additional Rules to Load |
|------------|--------------------------|
| **Review Agent** | `.agents/rules/06-specifications-and-requirements.md` |
| **Implementation Agent** | `.agents/rules/13-implementation-agent-guide.md`, stack file |
| **Verification Agent** | `.agents/rules/08-verification-workflow-complete-guide.md`, stack file |
| **Documentation Agent** | `.agents/rules/06-specifications-and-requirements.md` |

### Stack Files

Load from `.agents/stacks/`:
- **Language**: Rust → `.agents/stacks/rust.md`

### Skills Referenced
- None

---
*Created: 2026-01-18*
*Last Updated: 2026-01-21 (Added Agent Rules Reference for self-contained specification)*
