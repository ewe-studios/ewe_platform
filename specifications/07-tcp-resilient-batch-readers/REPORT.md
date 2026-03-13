---
specification: "07-tcp-resilient-batch-readers"
created: 2026-03-14
status: "pending"
completion_percentage: 0
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-03-14
  phase: "specification"
---

# TCP-Resilient Batch Readers - Completion Report

## Status

**Status**: Specification Created, Implementation Pending
**Phase**: Specification
**Created**: 2026-03-14

## Scope

### New Types
1. **`BatchReader<R: Read>`** — Iterator yielding batches or retry signals, using `read()` not `read_exact()`
2. **`FullBodyReader<R: Read>`** — Reads known-size body with WouldBlock/TimedOut resilience
3. **Updated `SimpleHttpBody`** — Tuple struct with max body size + threshold for choosing reader strategy

### Files
- New: `backends/foundation_core/src/io/readers/mod.rs`
- Modified: `backends/foundation_core/src/io/mod.rs`
- Modified: `backends/foundation_core/src/wire/simple_http/impls.rs`

## Tasks

- Total: 17
- Completed: 0
- Remaining: 17

---

_Created: 2026-03-14_
