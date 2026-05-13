---
name: "Migrate foundation_core Tests"
description: "Move all single-crate tests from tests/backends/foundation_core/ to backends/foundation_core/tests/"
status: "pending"
priority: "high"
dependencies: []
estimated_effort: "medium"
---

# Feature: Migrate foundation_core Tests

## Overview

Move approximately 20 test files from `tests/backends/foundation_core/` into `backends/foundation_core/tests/`. These tests use `foundation_core` as the primary crate under test and `foundation_testing` as a test helper.

## Why This Is Safe

`foundation_core` already has `foundation_testing` as a dev-dependency (line 67 in Cargo.toml). Dev-dependencies only activate when testing the declaring crate, preventing circular dependency issues.

## Source Files to Migrate

### Event Source Tests
| File | Uses foundation_testing? | Notes |
|------|-------------------------|-------|
| `event_source/mod.rs` | N/A | Module declarations |
| `event_source/parser_tests.rs` | Yes (`SharedBuffer`) | Parser unit tests |
| `event_source/consumer_integration_tests.rs` | Yes (`TestHttpServer`) | SseStream consumer tests |
| `event_source/task_integration_tests.rs` | Yes (`TestHttpServer`) | EventSourceTask tests |
| `event_source/reconnecting_task_integration_tests.rs` | Yes (`TestHttpServer`) | Reconnection tests |

### WebSocket Tests
| File | Uses foundation_testing? | Notes |
|------|-------------------------|-------|
| `websocket/mod.rs` | N/A | Module declarations |
| `websocket/echo_tests.rs` | Yes (`WebSocketEchoServer`) | WebSocket echo tests |
| `websocket/reconnection_tests.rs` | No | Reconnection task tests |
| `websocket/server_tests.rs` | No | Server-side tests |
| `websocket/subprotocol_tests.rs` | Yes (`WebSocketEchoServer`) | Subprotocol negotiation |

### HTTP Tests
| File | Uses foundation_testing? | Notes |
|------|-------------------------|-------|
| `simple_http/mod.rs` | N/A | Module declarations |
| `simple_http/compliance_tests.rs` | No | HTTP reader compliance |
| `simple_http/http_redirect_integration.rs` | Yes (`TestHttpServer`) | Redirect chain tests |
| `simple_http/http_redirect_limit_tests.rs` | No | Redirect limit tests |

### Wire/Network Tests
| File | Uses foundation_testing? | Notes |
|------|-------------------------|-------|
| `wire/mod.rs` | N/A | Module declarations |
| `wire/http_client_body_reading.rs` | Yes (`TestHttpServer`) | Body reading tests |
| `wire/http_external_validation.rs` | Likely | Currently commented out |
| `wire/http_server_integration.rs` | Likely | Currently commented out |
| `wire/http_task_iterator_integration.rs` | Likely | Currently commented out |
| `wire/tls_communication.rs` | Likely | Currently commented out |
| `wire/tls_integration.rs` | Likely | Currently commented out |
| `wire/tls_local_server.rs` | Likely | Currently commented out |

## Destination Structure

```
backends/foundation_core/tests/
├── mod.rs                              # Test module declarations
├── event_source/
│   ├── mod.rs
│   ├── parser_tests.rs
│   ├── consumer_integration_tests.rs
│   ├── task_integration_tests.rs
│   └── reconnecting_task_integration_tests.rs
├── websocket/
│   ├── mod.rs
│   ├── echo_tests.rs
│   ├── reconnection_tests.rs
│   ├── server_tests.rs
│   └── subprotocol_tests.rs
├── simple_http/
│   ├── mod.rs
│   ├── compliance_tests.rs
│   ├── http_redirect_integration.rs
│   └── http_redirect_limit_tests.rs
└── wire/
    ├── mod.rs
    ├── http_client_body_reading.rs
    └── [commented files - optional]
```

## Required Changes

### 1. Move Files
Copy all files maintaining directory structure.

### 2. Update Imports (if needed)
Most imports should work as-is since:
- `foundation_core::*` imports remain valid
- `foundation_testing::*` is available via dev-dependency

### 3. Update backends/foundation_core/tests/mod.rs
Add new module declarations:
```rust
// Test modules for foundation_core
mod event_source;
mod websocket;
mod simple_http;
mod wire;
```

### 4. Remove from tests/backends/foundation_core/mod.rs
Remove migrated module declarations.

## Verification Steps

1. Run `cargo test -p foundation_core` to verify tests compile and pass
2. Check no circular dependency errors
3. Verify foundation_testing dev-dependency is available

## Rollback Plan

If issues occur:
1. Revert changes to `backends/foundation_core/tests/mod.rs`
2. Delete migrated files
3. Restore original files in `tests/backends/foundation_core/`
