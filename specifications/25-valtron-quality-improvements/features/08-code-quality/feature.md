---
feature: code-quality
description: Fix WASM variable name, BranchPath naming convention, and prune unused iterator type aliases
status: pending
priority: low
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 3
  total: 3
  completion_percentage: 0
dependencies: []
---

# Feature 08: Code Quality & Cleanup

## Problem

Three low-severity code quality issues:

### 1. WASM `drive_future_stream` References Wrong Variable (LOW)

**Location:** `drivers.rs:394`

```rust
drive_non_send_iterator(crate::valtron::from_stream(future))
//                                                    ^^^^^^ should be `stream`
```

The parameter is `stream` but the body uses `future`. This is a compile error on wasm32
targets — likely dead code since it's behind `#[cfg(target_arch = "wasm32")]`.

### 2. `BranchPath::SKIP` Uses ALL_CAPS Convention (LOW)

**Location:** `branches.rs:75`

Rust convention reserves ALL_CAPS for constants, not enum variants. Should be `Skip`.

### 3. Excessive Iterator Type Alias Proliferation (LOW)

**Location:** `iterators.rs:13-24, 46-56, 159-170`

~30 type aliases for every primitive type (`CloneableI8Iterator`, `SendU16Iterator`,
`CloneableSendI32Iterator`, etc.). Most are unlikely to be used externally.

## Approach

1. Fix the variable name in the WASM path.
2. Rename the enum variant following Rust conventions.
3. Audit usage of type aliases and remove unused ones, keeping only the generic base types
   and commonly-used concrete ones.

## Tasks

- [ ] TASK-08-01: Fix variable name `future` → `stream` in wasm `drive_future_stream` (`drivers.rs:394`)
- [ ] TASK-08-02: Rename `BranchPath::SKIP` to `BranchPath::Skip` (`branches.rs:75`); update all references
- [ ] TASK-08-03: Audit iterator type aliases in `iterators.rs`; remove unused aliases, keep generic base types and commonly-used concrete ones
