---
description: Create an HTTP 1.1 client using existing simple_http module structures
  with iterator-based patterns and valtron executors
status: in-progress
priority: high
created: 2026-01-18
author: Main Agent
metadata:
  version: '3.0'
  last_updated: 2026-01-25
  estimated_effort: large
  tags:
  - http-client
  - networking
  - rust
  - iterator-patterns
  - valtron-executors
  tools:
  - Rust
  - cargo
builds_on:
  - ../04-condvar-primitives
related_specs:
  - ../03-wasm-friendly-sync-primitives
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
tasks:
  completed: 9
  uncompleted: 145
  total: 154
  completion_percentage: 6
files_required:
  main_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/05-coding-practice-agent-orchestration.md
      - .agents/rules/06-specifications-and-requirements.md
    files:
      - ./requirements.md
      - ./LEARNINGS.md
      - ./PROGRESS.md
  review_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/06-specifications-and-requirements.md
    files:
      - ./requirements.md
      - .agents/stacks/rust.md
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md
      - ./features/*/feature.md
  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md
  documentation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/06-specifications-and-requirements.md
    files:
      - ./requirements.md
---

# HTTP 1.1 Client - Requirements

## Overview

Create an HTTP 1.1 client using the existing `simple_http` module structures, leveraging the iterator-based patterns and executor system from `valtron` without using async/await primitives, tokio, or any async runtime. The client will use valtron's `single` and `multi` executor modules to execute tasks in an async-like manner through the `TaskIterator` pattern.

## Known Issues

### Pre-existing foundation_wasm Compilation Errors

**CRITICAL**: The workspace has ~110 compilation errors in `foundation_wasm` package due to incorrect `SpinMutex` API usage. This affects:
- ❌ Cannot run `cargo test --workspace`
- ❌ Cannot run workspace-level `cargo fmt --all --check`
- ❌ Cannot run workspace-level `cargo clippy --workspace`
- ✅ **Workaround**: Test features in isolation using `cargo test --package foundation_core`

**Root Cause**: `SpinMutex::lock()` returns `Result<Guard, PoisonError>` but code calls it without unwrapping in frames.rs, intervals.rs, schedule.rs, registry.rs.

**Decision**: This is a pre-existing issue outside the scope of this specification. All verification commands in this spec use `--package foundation_core` to work around these errors.

**Impact on This Spec**:
- All HTTP client code will be in `foundation_core` package (not `foundation_wasm`)
- Verification will use package-level commands: `cargo test --package foundation_core`
- Pure no_std builds cannot be verified until `foundation_wasm` is fixed
- This does NOT block HTTP client implementation

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


---

## Tasks

# HTTP 1.1 Client - Feature Progress

## Feature Priority Order

Complete features in order due to dependencies:

### Core Features (Required)

0. [ ] **valtron-utilities** - Reusable ExecutionAction types, unified executor, state machine helpers, Future adapter (no_std compatible)
   - Status: pending
   - Tasks: 24
   - Dependencies: None
   - See: [features/valtron-utilities/](./features/valtron-utilities/)

1. [ ] **tls-verification** - Verify and fix TLS backends (rustls, openssl, native-tls)
   - Status: pending
   - Tasks: 8
   - Dependencies: valtron-utilities
   - See: [features/tls-verification/](./features/tls-verification/)

2. [x] **foundation** - Error types and DNS resolution
   - Status: completed
   - Tasks: 9 (9 completed)
   - Dependencies: tls-verification
   - See: [features/foundation/](./features/foundation/)

3. [ ] **compression** - gzip, deflate, brotli support
   - Status: pending
   - Tasks: 9
   - Dependencies: foundation
   - See: [features/compression/](./features/compression/)

4. [ ] **connection** - URL parsing, TCP connections, TLS upgrade
   - Status: pending
   - Tasks: 4
   - Dependencies: foundation
   - See: [features/connection/](./features/connection/)

5. [ ] **proxy-support** - HTTP/HTTPS/SOCKS5 proxy
   - Status: pending
   - Tasks: 14
   - Dependencies: connection
   - See: [features/proxy-support/](./features/proxy-support/)

6. [ ] **request-response** - Request builder, response types
   - Status: pending
   - Tasks: 4
   - Dependencies: connection
   - See: [features/request-response/](./features/request-response/)

7. [ ] **auth-helpers** - Basic, Bearer, Digest auth
   - Status: pending
   - Tasks: 10
   - Dependencies: request-response
   - See: [features/auth-helpers/](./features/auth-helpers/)

8. [ ] **task-iterator** - Internal TaskIterator, ExecutionAction, executors
   - Status: pending
   - Tasks: 8
   - Dependencies: request-response, valtron-utilities
   - See: [features/task-iterator/](./features/task-iterator/)

9. [ ] **public-api** - User-facing API, SimpleHttpClient, integration
   - Status: pending
   - Tasks: 6
   - Dependencies: task-iterator
   - See: [features/public-api/](./features/public-api/)

### Extended Features (Optional)

10. [ ] **cookie-jar** - Automatic cookie handling
    - Status: pending
    - Tasks: 15
    - Dependencies: public-api
    - See: [features/cookie-jar/](./features/cookie-jar/)

11. [ ] **middleware** - Request/response interceptors
    - Status: pending
    - Tasks: 14
    - Dependencies: public-api
    - See: [features/middleware/](./features/middleware/)

12. [ ] **websocket** - WebSocket client and server
    - Status: pending
    - Tasks: 20
    - Dependencies: connection, public-api
    - See: [features/websocket/](./features/websocket/)

## Total Tasks Across Features

| Feature | Tasks | Status |
|---------|-------|--------|
| valtron-utilities | 24 | pending |
| tls-verification | 8 | pending |
| foundation | 9 | completed |
| compression | 9 | pending |
| connection | 4 | pending |
| proxy-support | 14 | pending |
| request-response | 4 | pending |
| auth-helpers | 10 | pending |
| task-iterator | 8 | pending |
| public-api | 6 | pending |
| cookie-jar | 15 | pending |
| middleware | 14 | pending |
| websocket | 20 | pending |
| **Total** | **143** | **6% complete** |

## Notes

- Each feature has its own `feature.md` with integrated tasks
- Complete features in order - later features depend on earlier ones
- **valtron-utilities MUST be done first** (foundational patterns)
- **tls-verification** should be done early to ensure TLS works
- Extended features (cookie-jar, middleware, websocket) can be done in any order after public-api
- Mark feature complete in this file only when ALL its tasks are done
- Verification files (PROGRESS.md, REPORT.md, etc.) are at this level, not in features

## Additional Tasks (Not Part of HTTP Client)

### foundation_core::synca Tests

User requested additional test coverage for synca synchronization primitives alongside HTTP client work.

**Status**: pending
**Priority**: medium
**Location**: `backends/foundation_core/src/synca/`

#### synca::event (LockSignal) Tests
- [ ] Test lock() behavior (free, locked, released states)
- [ ] Test try_lock() return values
- [ ] Test signal_one() single thread wake
- [ ] Test signal_all() multiple thread wake
- [ ] Test wait() blocking and unblocking
- [ ] Test lock_and_wait() combined operation
- [ ] Test probe() and probe_locked() state queries
- [ ] Multi-threaded race condition scenarios
- [ ] Edge cases (signal without lock, multiple signals)

#### synca::sleepers Tests
- [ ] Test DurationWaker creation and timing
- [ ] Test Sleepers insertion with sorted order
- [ ] Test Sleepers removal and cleanup
- [ ] Test wake operations at correct times
- [ ] Test Timing trait implementation
- [ ] Concurrent access with multiple threads
- [ ] Edge cases (empty list, single sleeper, many sleepers)

#### synca::entrylist Tests
- [ ] Test EntryList creation and initialization
- [ ] Test entry insertion and structure maintenance
- [ ] Test entry removal and return values
- [ ] Test iteration order and completeness
- [ ] Test entry lookup (find, missing entries)
- [ ] Test clear/cleanup operations
- [ ] Concurrent access with RwLock
- [ ] Edge cases (empty operations, single entry, many entries)

**Verification**:
- [ ] All tests pass: `cargo test --package foundation_core`
- [ ] Tests work with std feature
- [ ] No warnings during test compilation

---
*Last Updated: 2026-01-19*

---

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

---

## File Organization Reminder

ONLY these files allowed:
1. requirements.md - Requirements with tasks
2. LEARNINGS.md - All learnings
3. REPORT.md - All reports
4. VERIFICATION.md - Verification
5. PROGRESS.md - Current status (delete at 100%)
6. fundamentals/, features/, templates/ (optional)

FORBIDDEN: Separate learning/report/verification files

Consolidation: All learnings → LEARNINGS.md, All reports → REPORT.md

See Rule 06 "File Organization" for complete policy.
