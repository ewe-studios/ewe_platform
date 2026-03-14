---
specification: "07-tcp-resilient-batch-readers"
created: 2026-03-14
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-03-14
tags:
  - learnings
  - rust
  - tcp
  - io
  - readers
---

# TCP-Resilient Batch Readers - Learnings

## Pre-Implementation Insights

### 1. read_exact() vs read() on TCP Streams

**Finding**: `read_exact()` internally loops calling `read()` and converts `WouldBlock`/`TimedOut` into `UnexpectedEof`. This makes it impossible for callers to distinguish between "no data available yet" (retryable) and "connection closed" (fatal).

**Reference**: See `wire/websocket/frame.rs:188-221` for the canonical correct pattern using `read()`.

### 2. Ok(0) Semantics Differ by Context

**Finding**: `read()` returning `Ok(0)` has different meanings:
- On a file or completed stream: means EOF
- On a TCP stream with no data yet: may mean "nothing available now" (retryable)

**Implication**: The `eof_on_zero_read` configuration flag on `BatchReader` lets callers choose the correct interpretation for their use case.

### 3. Retry Counter Must Reset on Progress

**Finding**: The WebSocket decoder resets state on successful reads. Similarly, the retry counter must reset each time data is successfully read — only consecutive retries without progress should count toward the limit.

### 4. Test Location — ALL Tests in ./tests Crate

**Finding**: ALL tests (unit AND integration) live in `./tests` directory, crate name `ewe_platform_tests`. There are NO in-crate tests in `backends/foundation_core/src/`. The structure is:
- `tests/backends/foundation_core/units/` — fast unit tests (no I/O)
- `tests/backends/foundation_core/integrations/` — integration tests (real servers/streams)
- New unit tests for `io::readers` should go in `tests/backends/foundation_core/units/io/` (does not exist yet, create it)
- Update `tests/backends/foundation_core/units/mod.rs` to include the new module

**Implication**: Run `cargo test -p ewe_platform_tests` to run both unit and integration tests. Do NOT use `cargo test -p foundation_core` — tests are not there.

### 5. Test Patterns

**Finding**: Tests follow specific patterns:
- Use `foundation_core::*` public API imports (not internal paths)
- WHY/WHAT comments before each test
- Arrange/Act/Assert structure
- `#[traced_test]` for tests needing log output
- ONE test at a time (TDD cycle)
- No `assert!(true)` placeholders

---

_Created: 2026-03-14_
_Updated: 2026-03-14 — Added integration test location_
