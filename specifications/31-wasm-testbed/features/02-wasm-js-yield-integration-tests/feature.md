---
feature: "wasm32 JS Yield Integration Tests"
description: "wasm-pack integration tests verifying full stack: valtron executor → JS yield → Promise resolution → executor re-entry"
status: "draft"
priority: "high"
depends_on: ["04-js-yield-controller", "05-run-until-yield-handling"]
estimated_effort: "medium"
created: 2026-05-22
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# wasm32 JS Yield Integration Tests Feature

## Overview

Create wasm-pack integration tests that verify the full stack: valtron executor → JS yield → Promise resolution → executor re-entry → task completion.

## Problem

We need integration tests that verify the full stack: valtron executor → JS yield → Promise resolution → executor re-entry → task completion.

## Solution

wasm-pack tests (`wasm32-unknown-unknown` target):

```
#[wasm_bindgen_test]
async fn js_yield_returns_immediately()
  → SpinWaiter::wait(100ms) returns in < 5ms

#[wasm_bindgen_test]
async fn js_yield_callback_reschedules_executor()
  → Executor makes progress on re-entry after setTimeout fires

#[wasm_bindgen_test]
async fn concurrent_d1_queries_make_progress()
  → Multiple D1 queries all complete

#[wasm_bindgen_test]
async fn no_deadlock_with_js_yield()
  → Task depending on JS Promise completes within timeout
```

## Architecture

```mermaid
flowchart TD
    A[wasm_bindgen_test] --> B{Test case}
    B --> C[js_yield_returns_immediately]
    B --> D[js_yield_callback_reschedules_executor]
    B --> E[concurrent_d1_queries_make_progress]
    B --> F[no_deadlock_with_js_yield]
    C --> G[Measure wait time < 5ms]
    D --> H[Verify progress after setTimeout]
    E --> I[Run parallel D1 queries]
    F --> J[Set timeout guard, verify completion]
```

## Implementation Phases

1. Create `foundation_core/tests/valtron/wasm_js_yield_integration.rs`
2. Add `#[wasm_bindgen_test]` test for immediate return from `SpinWaiter::wait()`
3. Add test for executor re-entry after setTimeout fires
4. Add test for concurrent D1 queries making progress
5. Add test for no deadlock with JS yield enabled

## Tests

1. `js_yield_returns_immediately` — `SpinWaiter::wait(100ms)` returns in < 5ms
2. `js_yield_callback_reschedules_executor` — Executor makes progress on re-entry after setTimeout fires
3. `concurrent_d1_queries_make_progress` — Multiple D1 queries all complete
4. `no_deadlock_with_js_yield` — Task depending on JS Promise completes within timeout

## Success Criteria

- All 4 wasm-pack integration tests pass
- Tests run with `wasm-pack test --node --features js_eventloop_yield`
- No deadlocks occur during any test
- Measured wait times confirm immediate return from JS-aware wait

## Verification Commands

```bash
wasm-pack test --node --features js_eventloop_yield -p foundation_core -- wasm_js_yield_integration
```
