---
feature: "wasm Credential Store End-to-End Tests"
description: "End-to-end wasm tests proving CredentialStorage::set() and SessionManager::create_session() work with JS yield enabled"
status: "draft"
priority: "high"
depends_on: ["07-cf-login-app-integration", "08-wasm-js-yield-integration-tests"]
estimated_effort: "medium"
created: 2026-05-22
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 2
  total: 2
  completion_percentage: 0%
---

# wasm Credential Store End-to-End Tests Feature

## Overview

Create end-to-end wasm tests that prove the original deadlock path (`CredentialStorage::set()` → `schedule_future()` → `drive_non_send_iterator()` → valtron spin loop) now works correctly with JS yield enabled.

## Problem

The original deadlock path: `CredentialStorage::set()` → `schedule_future()` → `drive_non_send_iterator()` → valtron spin loop → JS Promise never resolves. We need tests that prove this path works with JS yield enabled.

## Solution

```
#[wasm_bindgen_test]
async fn set_and_get_with_js_yield()
  → CredentialStorage::set() + ::get() work (exact deadlocking call path)

#[wasm_bindgen_test]
async fn session_create_with_js_yield()
  → SessionManager::create_session() completes successfully
```

## Architecture

```mermaid
sequenceDiagram
    participant Test
    participant CredentialStorage
    participant StorageProvider
    participant schedule_future
    participant drive_non_send_iterator
    participant run_until
    participant JS Yield

    Test->>CredentialStorage: set(key, value)
    CredentialStorage->>StorageProvider: set(key, value)
    StorageProvider->>schedule_future: schedule
    schedule_future->>drive_non_send_iterator: drive
    drive_non_send_iterator->>run_until: run_until_next_state
    run_until->>run_until: schedule_and_do_work → Pending
    run_until->>JS Yield: yield_for(duration)
    Note over JS Yield: setTimeout registered, returns immediately
    Note over JS Yield: after timeout, Promise resolves
    JS Yield-->>run_until: re-entry
    run_until->>run_until: check tasks → resolved
    run_until-->>drive_non_send_iterator: complete
    drive_non_send_iterator-->>schedule_future: complete
    schedule_future-->>StorageProvider: complete
    StorageProvider-->>CredentialStorage: complete
    CredentialStorage-->>Test: Ok(())
    Test->>CredentialStorage: get(key)
    CredentialStorage-->>Test: value
```

## Implementation Phases

1. Create `foundation_core/tests/valtron/wasm_credential_store_with_yield.rs`
2. Add `set_and_get_with_js_yield` wasm_bindgen_test
3. Add `session_create_with_js_yield` wasm_bindgen_test

## Tests

1. `set_and_get_with_js_yield` — `CredentialStorage::set()` + `::get()` work (exact deadlocking call path)
2. `session_create_with_js_yield` — `SessionManager::create_session()` completes successfully

## Success Criteria

- Both e2e tests pass with `wasm-pack test --node`
- No deadlock occurs during credential store operations
- Session creation completes within timeout
- Tests verify the exact call path that previously deadlocked

## Verification Commands

```bash
wasm-pack test --node --features js_eventloop_yield -p foundation_core -- wasm_credential_store_with_yield
```
