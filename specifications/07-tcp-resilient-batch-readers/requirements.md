---
description: TCP-resilient batch readers that use read() instead of read_exact() to correctly handle WouldBlock/TimedOut on TCP streams
status: pending
priority: high
created: 2026-03-14
author: Main Agent
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: true
metadata:
  version: '1.0'
  last_updated: 2026-03-14
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
  completed: 0
  uncompleted: 17
  total: 17
  completion_percentage: 0
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

### 4. Updated `SimpleHttpBody`

Change from unit struct to tuple struct with required fields:

```rust
pub struct SimpleHttpBody(usize, usize);
```

- First field: `max_body_size` — maximum allowed body size (applies to all body types)
- Second field: `full_body_threshold` — for `LimitedBody`: if `content_length <= threshold` → use `FullBodyReader` → return `SendSafeBody::Bytes`; if `content_length > threshold` → use `BatchStreamReader` → return `SendSafeBody::Stream`

Implements `Default` with sensible defaults:

```rust
impl Default for SimpleHttpBody {
    fn default() -> Self {
        // max_body_size: 1 GB, full_body_threshold: 1 MB
        SimpleHttpBody(1024 * 1024 * 1024, 1024 * 1024)
    }
}
```

- Default `max_body_size` of 1 GB — permissive default matching Apache/RHEL, can be tightened per use case.
- Default `full_body_threshold` of 1 MB — bodies at or below this are read entirely into memory via `FullBodyReader` and returned as `SendSafeBody::Bytes`. Bodies above are streamed via `BatchStreamReader` as `SendSafeBody::Stream`.

Wherever `SimpleHttpBody` is used, callers should be able to configure it (pass their own instance). If not configured, fall back to `SimpleHttpBody::default()`.

## Files to Create/Modify

### New Files
- `backends/foundation_core/src/io/readers/mod.rs` — `BatchReader`, `BatchStreamReader`, `FullBodyReader`, `Data` enum, builder/config types

### Modified Files
- `backends/foundation_core/src/io/mod.rs` — add `pub mod readers;`
- `backends/foundation_core/src/wire/simple_http/impls.rs` — update `SimpleHttpBody` struct and `BodyExtractor` impl

## Tasks

### Module Setup
- [ ] 1. Create `backends/foundation_core/src/io/readers/mod.rs` with module structure
- [ ] 2. Add `pub mod readers;` to `backends/foundation_core/src/io/mod.rs`

### BatchReader Implementation
- [ ] 3. Implement `Data` enum with `Bytes(Vec<u8>)` and `Retry` variants
- [ ] 4. Implement `BatchReader<R>` struct with configuration fields
- [ ] 5. Implement `BatchReader::new()` and builder methods for configuration
- [ ] 6. Implement `Iterator` for `BatchReader<R>` with WouldBlock/TimedOut/retry handling
- [ ] 7. Write tests for `BatchReader` — normal reads, WouldBlock handling, retry limits, EOF behavior

### FullBodyReader Implementation
- [ ] 8. Implement `FullBodyReader::read_full()` function
- [ ] 9. Write tests for `FullBodyReader` — complete reads, partial reads, retry handling, unexpected EOF

### BatchStreamReader Implementation
- [ ] 10. Implement `BatchStreamReader<R>` struct wrapping `BatchReader<R>`
- [ ] 11. Implement `Iterator` for `BatchStreamReader` with internal spin on `Data::Retry`
- [ ] 12. Write tests for `BatchStreamReader` — verify retry absorption, bytes pass-through, error propagation

### SimpleHttpBody Update
- [ ] 13. Change `SimpleHttpBody` from unit struct to `SimpleHttpBody(usize, usize)` with `Default` impl
- [ ] 14. Update `BodyExtractor` impl to use `FullBodyReader` for small bodies and `BatchStreamReader` for large bodies
- [ ] 15. Update `HttpRequestReader::simple_tcp_stream()` and `HttpResponseReader::simple_tcp_stream()` to use `SimpleHttpBody::default()`
- [ ] 16. Update `HTTPStreams::next_request()` / `next_response()` and the `http_helpers` module to use `SimpleHttpBody::default()`
- [ ] 17. Update call sites in `client/connection.rs`, `client/tasks/`, and `websocket/task.rs` to accept an optional `SimpleHttpBody` or fall back to `SimpleHttpBody::default()`

---
*Created: 2026-03-14*
