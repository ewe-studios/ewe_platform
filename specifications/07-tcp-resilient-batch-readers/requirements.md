---
description: TCP-resilient batch readers that use read() instead of read_exact() to correctly handle WouldBlock/TimedOut on TCP streams
status: completed
priority: high
created: 2026-03-14
author: Main Agent
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: true
metadata:
  version: '1.2'
  last_updated: 2026-03-14 — Added EofReader for TCP-resilient EOF reading, used by Body::FullBody
  estimated_effort: medium
  tags:
  - tcp
  - io
  - readers
  - resilience
  - streaming
  stack_files:
  - .agents/stacks/rust.md
  skills: []
  tools:
  - Rust
  - cargo
builds_on: []
related_specs:
- 02-build-http-client
has_features: false
has_fundamentals: false
tasks:
  completed: 23
  uncompleted: 0
  total: 23
  completion_percentage: 100
---

# TCP-Resilient Batch Readers - Requirements

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this specification MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** to understand project patterns and conventions
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific patterns
4. ✅ **Read module documentation** for modules you'll modify
5. ✅ **Follow discovered patterns** - do NOT invent new patterns without justification
6. ✅ **Verify all assumptions** by reading actual code

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume typical patterns without checking the codebase
- ❌ Implement without searching for similar code first
- ❌ Apply generic best practices without verifying project conventions
- ❌ Guess file structures, naming conventions, or API patterns
- ❌ Use pretraining knowledge without verification against project code

## Problem Statement

`Body::LimitedBody` in `simple_http/impls.rs:4989` uses `read_exact()` to read HTTP response bodies of known size. On TCP streams, `read_exact()` converts `WouldBlock` and `TimedOut` errors into `UnexpectedEof`, making it impossible for callers to distinguish "no data available yet" from "connection closed." This causes spurious failures on slow or congested connections.

The WebSocket frame decoder (`wire/websocket/frame.rs:188-221`) already demonstrates the correct pattern: using `read()` instead of `read_exact()` and propagating `WouldBlock`/`TimedOut` directly so callers can retry.

## Solution

Create reusable reader types in `foundation_core::io::readers` that encapsulate the TCP-resilient read pattern, then update `SimpleHttpBody` to use them.

## Key Existing Code References

| Reference | Path | Purpose |
|-----------|------|---------|
| WebSocket frame decode | `backends/foundation_core/src/wire/websocket/frame.rs:188-221` | Canonical WouldBlock/TimedOut handling pattern |
| SimpleHttpBody | `backends/foundation_core/src/wire/simple_http/impls.rs:4964-5014` | Current brittle implementation to replace |
| Read extensions | `backends/foundation_core/src/io/stream_ext.rs` | Existing Read extension pattern |
| SharedByteBufferStream | `backends/foundation_core/src/io/ioutils/mod.rs:938` | Stream wrapper used by extractors |
| Iterator aliases | `backends/foundation_core/src/valtron/iterators.rs` | Project iterator type patterns |
| HttpReaderError | `backends/foundation_core/src/wire/simple_http/errors.rs:365` | Error type for HTTP readers |

## New Types

### 1. `BatchReader<R: Read>` — Iterator over read batches

An iterator that wraps a `Read` source and yields `Result<Data, Error>`:

```rust
pub enum Data {
    /// A batch of bytes successfully read from the source
    Bytes(Vec<u8>),
    /// Yielded on WouldBlock, TimedOut, or Ok(0)-when-not-EOF
    /// Signals the caller should retry (poll again later)
    Retry,
}
```

**Configuration:**
- `batch_size: usize` — size of read buffer (default: 512)
- `eof_on_zero_read: bool` — if `true`, `read()` returning 0 means EOF (iterator ends); if `false`, 0 is retryable (yields `Data::Retry`)
- `max_consecutive_retries: usize` — maximum retries without progress before erroring (resets on each successful read)

**Behavior:**
- Uses `read()`, never `read_exact()`
- On `WouldBlock`/`TimedOut` → yields `Data::Retry`, increments retry counter
- On `Ok(0)` when `eof_on_zero_read` is false → yields `Data::Retry`
- On `Ok(0)` when `eof_on_zero_read` is true → returns `None` (iterator done)
- On `Ok(n)` → yields `Data::Bytes(buf[..n].to_vec())`, resets retry counter
- On other errors → yields `Err(error)`
- When retry counter exceeds `max_consecutive_retries` → yields `Err`

### 2. `FullBodyReader<R: Read>` — Read known-size body with retry resilience

Reads a body of known total size using `read()` with retry handling:

```rust
pub fn read_full(reader: &mut R, total_size: usize, max_retries: usize) -> Result<Vec<u8>, Error>
```

**Behavior:**
- Allocates `Vec<u8>` of `total_size`
- Loops calling `read()` on remaining slice until full
- On `WouldBlock`/`TimedOut` → increments retry counter, continues
- On `Ok(n > 0)` → advances position, resets retry counter
- On `Ok(0)` → returns error (unexpected EOF — we know the expected size)
- On retry counter exceeded → returns error
- On other errors → returns error

### 3. `BatchStreamReader<R: Read>` — Adapter from BatchReader to `Iterator<Item = Result<Vec<u8>, E>>`

Wraps a `BatchReader` and implements `Iterator<Item = Result<Vec<u8>, BoxedError>> + Send`. Internally spins on `Data::Retry` results, only yielding when it gets actual bytes or an error.

```rust
pub struct BatchStreamReader<R: Read> {
    inner: BatchReader<R>,
}
```

**Trait impls:**
- `Iterator` with `Item = std::result::Result<Vec<u8>, BoxedError>`
- `Send` — satisfied when `R: Send`

**Behavior:**
- `next()` calls `self.inner.next()` in a loop:
  - On `Some(Ok(Data::Bytes(bytes)))` → returns `Some(Ok(bytes))`
  - On `Some(Ok(Data::Retry))` → continues the loop (spins)
  - On `Some(Err(e))` → returns `Some(Err(e))` (boxed)
  - On `None` → returns `None` (EOF)
- The retry budget is still enforced by the inner `BatchReader` — when `max_consecutive_retries` is exceeded, `BatchReader` yields `Err`, which `BatchStreamReader` propagates

**Why this exists:** `SendSafeBody::Stream` needs an iterator of `Result<Vec<u8>, BoxedError>`. `BatchReader` yields `Result<Data, Error>` where `Data` is an enum with `Bytes` and `Retry` variants — the stream consumer shouldn't need to know about retries.

### 4. `EofReader` — Read until EOF with retry resilience

Reads an unknown-size body (until EOF) using `read()` with retry handling:

```rust
pub fn read_to_end<R: Read>(
    reader: &mut R,
    batch_size: usize,
    max_retries: usize,
    max_size: Option<usize>,
) -> Result<Vec<u8>, io::Error>
```

**Behavior:**
- Uses `BatchReader` internally with `eof_on_zero_read=true`
- Loops calling `read()` until EOF, accumulating bytes into a `Vec<u8>`
- On `WouldBlock`/`TimedOut` → increments retry counter, continues
- On `Ok(n > 0)` → accumulates bytes, resets retry counter
- On `Ok(0)` → returns accumulated bytes (normal EOF)
- On retry counter exceeded → returns error
- On `max_size` exceeded → returns `InvalidInput` error

**Why this exists:** `Body::FullBody` reads until EOF without a known size. Using `EofReader` provides TCP-resilient behavior consistent with `FullBodyReader` and `BatchStreamReader`.

### 5. Updated `SimpleHttpBody`

Change from unit struct to tuple struct with configurable fields:

```rust
pub struct SimpleHttpBody(pub Option<u64>, pub u64, pub usize, pub usize);
```

- First field: `max_body_size` — optional maximum allowed body size (`None` disables checking, useful for clients downloading large payloads)
- Second field: `full_body_threshold` — for `LimitedBody`: if `content_length <= threshold` → use `FullBodyReader` → return `SendSafeBody::Bytes`; if `content_length > threshold` → use `BatchStreamReader` → return `SendSafeBody::Stream`
- Third field: `batch_size` — read buffer size for `BatchReader` (default: 8192)
- Fourth field: `max_retries` — max consecutive retries for WouldBlock/TimedOut (default: 100)

Implements `Default` with sensible defaults:

```rust
impl Default for SimpleHttpBody {
    fn default() -> Self {
        // max_body_size: 1 GB (Some), full_body_threshold: 512 KB, batch_size: 8192, max_retries: 100
        SimpleHttpBody(Some(1024 * 1024 * 1024), 512 * 1024, 8192, 100)
    }
}
```

- Default `max_body_size` of 1 GB — permissive default matching Apache/RHEL, can be tightened per use case or disabled via `None`.
- Default `full_body_threshold` of 512 KB — bodies at or below this are read entirely into memory via `FullBodyReader` and returned as `SendSafeBody::Bytes`. Bodies above are streamed via `BatchStreamReader` as `SendSafeBody::Stream`.
- Default `batch_size` of 8192 bytes — read buffer size for batched streaming reads.
- Default `max_retries` of 100 — maximum consecutive WouldBlock/TimedOut retries before erroring.

#### Builder Methods

Both `HttpRequestReader<SimpleHttpBody, T>` and `HttpResponseReader<SimpleHttpBody, T>` provide builder-style setters:

```rust
impl<T: std::io::Read + Send + 'static> HttpRequestReader<SimpleHttpBody, T> {
    #[must_use]
    pub fn with_max_body_size(mut self, max_body_size: Option<u64>) -> Self

    #[must_use]
    pub fn with_full_body_threshold(mut self, threshold: u64) -> Self

    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self

    #[must_use]
    pub fn with_max_retries(mut self, max_retries: usize) -> Self
}
```

Same methods are available on `HttpResponseReader<SimpleHttpBody, T>`.

#### Usage Examples

**Server-side (strict limits):**
```rust
let reader = HttpRequestReader::new(stream)
    .with_max_body_size(Some(10 * 1024 * 1024)) // 10 MB max
    .with_full_body_threshold(256 * 1024)        // 256 KB threshold
    .with_batch_size(8192)
    .with_max_retries(100);
```

**Client-side (unlimited downloads):**
```rust
let reader = HttpResponseReader::new(stream)
    .with_max_body_size(None)  // No body size limit for downloads
    .with_full_body_threshold(512 * 1024)
    .with_batch_size(16384)    // Larger batches for throughput
    .with_max_retries(200);    // More retries for slow connections
```

Wherever `SimpleHttpBody` is used, callers should be able to configure it (pass their own instance). If not configured, fall back to `SimpleHttpBody::default()`.

## Files to Create/Modify

### New Files
- `backends/foundation_core/src/io/readers/mod.rs` — `BatchReader`, `BatchStreamReader`, `FullBodyReader`, `EofReader`, `Data` enum, builder/config types

### Modified Files
- `backends/foundation_core/src/io/mod.rs` — add `pub mod readers;`
- `backends/foundation_core/src/wire/simple_http/impls.rs` — update `SimpleHttpBody` struct and `BodyExtractor` impl

## Tasks

### Module Setup
- [x] 1. Create `backends/foundation_core/src/io/readers/mod.rs` with module structure
- [x] 2. Add `pub mod readers;` to `backends/foundation_core/src/io/mod.rs`

### BatchReader Implementation
- [x] 3. Implement `Data` enum with `Bytes(Vec<u8>)` and `Retry` variants
- [x] 4. Implement `BatchReader<R>` struct with configuration fields
- [x] 5. Implement `BatchReader::new()` and builder methods for configuration
- [x] 6. Implement `Iterator` for `BatchReader<R>` with WouldBlock/TimedOut/retry handling
- [x] 7. Write tests for `BatchReader` — normal reads, WouldBlock handling, retry limits, EOF behavior

### FullBodyReader Implementation
- [x] 8. Implement `FullBodyReader::read_full()` function
- [x] 9. Write tests for `FullBodyReader` — complete reads, partial reads, retry handling, unexpected EOF

### BatchStreamReader Implementation
- [x] 10. Implement `BatchStreamReader<R>` struct wrapping `BatchReader<R>`
- [x] 11. Implement `Iterator` for `BatchStreamReader` with internal spin on `Data::Retry`
- [x] 12. Write tests for `BatchStreamReader` — verify retry absorption, bytes pass-through, error propagation

### EofReader Implementation
- [x] 13. Implement `EofReader::read_to_end()` function for EOF-based reading
- [x] 14. Write tests for `EofReader` — complete reads, partial reads, retry handling, max size enforcement, empty sources

### SimpleHttpBody Update
- [x] 15. Change `SimpleHttpBody` from unit struct to `SimpleHttpBody(Option<u64>, u64, usize, usize)` with `Default` impl and builder methods on reader types
- [x] 16. Update `BodyExtractor` impl to use `FullBodyReader` for small bodies, `BatchStreamReader` for large bodies, and `EofReader` for EOF-based bodies
- [x] 17. Update `HttpRequestReader::simple_tcp_stream()` and `HttpResponseReader::simple_tcp_stream()` to use `SimpleHttpBody::default()`
- [x] 18. Update `HTTPStreams::next_request()` / `next_response()` and the `http_helpers` module to use `SimpleHttpBody::default()`
- [x] 19. Update call sites in `client/connection.rs`, `client/tasks/`, and `websocket/task.rs` to accept an optional `SimpleHttpBody` or fall back to `SimpleHttpBody::default()`

### Validation (Mandatory)
- [x] 20. Run `cargo check` on `foundation_core` — must compile with zero errors
- [x] 21. Run all existing unit tests (`cargo test -p foundation_core`) — all must pass, no regressions
- [x] 22. Run all existing integration tests — all must pass, no regressions
- [x] 23. Run `cargo clippy -p foundation_core` — no new warnings introduced

## Validation Requirements

> **MANDATORY**: Every task above must be validated against these criteria before it can be marked complete.

### Per-Task Validation
After completing each task:
1. `cargo check -p foundation_core` must succeed (no compile errors)
2. `cargo test -p foundation_core` must pass (no test regressions)
3. `cargo clippy -p foundation_core` must produce no new warnings

### Final Validation
Before the specification can be marked complete:
1. **All existing unit tests pass** — `cargo test -p foundation_core` with zero failures
2. **All existing integration tests pass** — no regressions in any dependent crate
3. **Clippy clean** — `cargo clippy -p foundation_core` with no new warnings
4. **No behavioral regressions** — `SimpleHttpBody::default()` must produce identical behavior to the old unit struct for all existing call sites (same max body size semantics, same error types)

### Regression Test Checklist
- [ ] WebSocket frame decoding still works (existing tests)
- [ ] HTTP request/response parsing still works (existing tests)
- [ ] Chunked transfer encoding still works (existing tests)
- [ ] Line-feed body reading still works (existing tests)
- [ ] Client connection handling still works (existing tests)
- [ ] All `SimpleHttpBody` call sites compile and behave identically to pre-change

---
*Created: 2026-03-14*
*Updated: 2026-03-14 — Added mandatory validation requirements*
