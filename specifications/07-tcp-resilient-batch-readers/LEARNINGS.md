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

---

_Created: 2026-03-14_
