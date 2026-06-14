---
feature: "Integration Tests"
description: "End-to-end tests: spawn dev service, verify proxy forwarding, verify file-change-triggered rebuild, verify SSE reload"
status: "pending"
priority: "medium"
depends_on: ["08-dev-service"]
estimated_effort: "medium"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: Integration Tests

## Problem

Need to verify the entire dev server works end-to-end without tokio/axum/hyper.

## Solution

### Test Categories

1. **Unit Tests** (per component):
   - HTTP request parser: parse valid requests, handle malformed input
   - HTTP response writer: correct headers, body, status codes
   - FileChange categorization: correct extension mapping
   - VecStringExt: vec<&str> → Vec<String>

2. **Integration Tests**:
   - Proxy forwarding: start proxy, send request to source, verify it reaches destination
   - File watching: create temp dir, touch file, verify WatchEvent delivered
   - SSE stream: connect to SSE endpoint, verify event format

3. **End-to-End Test**:
   - Create minimal test project (hello-world binary)
   - Start DevService pointing at it
   - Verify proxy serves the binary's HTTP responses
   - Modify a Rust file, verify cargo rebuild triggers
   - Verify binary restarts with new code

### Test Infrastructure

```rust
// tests/integration.rs

#[test]
fn test_proxy_forwards_http1() {
    // 1. Start a simple HTTP server on port X (using std::net::TcpListener)
    // 2. Start ProxyTask forwarding from port Y to port X
    // 3. Send HTTP request to port Y
    // 4. Verify response matches what server on port X returns
}

#[test]
fn test_file_watcher_detects_change() {
    // 1. Create temp directory
    // 2. Create FileWatcherTask watching temp dir
    // 3. Write a file
    // 4. Verify FileChange event appears in queue
}

#[test]
fn test_sse_event_format() {
    // 1. Create SseStream
    // 2. Trigger a FileChange event
    // 3. Verify output matches SSE format:
    //    "event: reload\r\ndata: ready\r\ncomment: ...\r\n\r\n"
}

#[test]
fn test_cargo_builder_skips_check() {
    // 1. Create ProjectBuilderTask with CargoBuilder (skip_check = true)
    // 2. Push FileChange::Rust event
    // 3. Verify cargo build runs but cargo check does not
}

#[test]
fn test_binary_runner_kills_old_process() {
    // 1. Create BinaryRunnerTask
    // 2. Signal build complete
    // 3. Verify old process is killed before new one spawns
}
```

### Cross-Platform Compilation

```bash
# Native compilation
cargo check -p foundation_toolings

# wasm32 (should compile with stub/no-op components)
cargo check -p foundation_toolings --target wasm32-unknown-unknown
```

### Task Breakdown

1. [ ] Write unit tests for HTTP parser
2. [ ] Write unit tests for HTTP response writer
3. [ ] Write unit tests for FileChange categorization
4. [ ] Write integration test for proxy forwarding
5. [ ] Write integration test for file watching
6. [ ] Write integration test for SSE event format
7. [ ] Write integration test for cargo builder (skip_check)
8. [ ] Write integration test for binary runner lifecycle
9. [ ] Verify cross-platform compilation (native + wasm32)
10. [ ] `cargo test -p foundation_toolings` passes

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/tests/integration.rs` | Create |

---

_Created: 2026-06-01_
