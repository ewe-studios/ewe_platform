---
feature: "Valtron JS Event Loop-Aware Yield on wasm32"
description: "Fix NotifyQueue contract and add JS-aware yielding so valtron executor yields to JS event loop on wasm32"
status: "done"
priority: "high"
depends_on: ["specifications/03-wasm-friendly-sync-primitives", "specifications/28-cloudflare-workers-readiness"]
related_to: ["specifications/28-cloudflare-workers-readiness/features/11-valtron-async-bridge"]
estimated_effort: "medium"
created: 2026-05-22
author: "Main Agent"
has_features: true
tasks:
  completed: 2
  uncompleted: 0
  total: 2
  completion_percentage: 100%
---

# Valtron JS Event Loop-Aware Yield on wasm32

## Overview

On wasm32 (`wasm32-unknown-unknown`), the valtron executor's single-threaded event loop can deadlock when waiting for JS Promises to resolve. Two root causes exist:

1. **NotifyQueue indefinite loop**: `NotifyQueue::wait_for_item` uses `loop {}` that only exits when an item is available or the queue is closed. The CondVar timeout is effectively ignored — on timeout it resets and loops forever. This prevents the executor from ever yielding control.

2. **No JS-aware spin waiting**: `SpinWaiter::wait()` burns CPU via `hint::spin_loop()` (a NOP on wasm32). When the executor spins waiting for a JS Promise, the JS event loop never gets control so the Promise never resolves.

This specification fixes the NotifyQueue contract first (Feature 1 — prerequisite), then adds JS-aware yielding through callback injection so the executor can yield to the JS event loop and resume via `setTimeout`.

## Feature Index

| Feature # | Name | Description | Status | Priority | Dependencies |
|-----------|------|-------------|--------|----------|--------------|
| 01 | [notify-queue-contract-fix](./features/01-notify-queue-contract-fix/feature.md) | Add NotificationItem enum, max_spins, fix indefinite loop | done | critical | none |
| 02 | [js-eventloop-yield-end-to-end](./features/02-js-aware-spinwaiter/feature.md) | Cooperative spin mutex, YieldSignal, JSThreadYielder (wasmbindgen + foundation_wasm), executor stop logic | done | high | 01 |
| 07 | [cf-login-app-integration](./features/07-cf-login-app-integration/feature.md) | Enable JS yield in cf-login-app example | cancelled | medium | 02 |
| 08 | wasm-js-yield-integration-tests | wasm-pack integration tests for full JS yield stack | cancelled | high | 02 |
| 09 | [wasm-credential-store-e2e-tests](./features/09-wasm-credential-store-e2e-tests/feature.md) | End-to-end tests for credential store with JS yield | cancelled | high | 07, 08 |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        CF Worker fetch()                     │
│                                                              │
│  get_or_init_app() ────────────────────────────────────────┐ │
│    ├─ init_valtron()                                       │ │
│    │   (JSThreadYielder auto-active via feature gate)      │ │
│    ├─ D1WasmStorage::new()                                 │ │
│    └─ MigrationRunner::run_async()                         │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  route handler (e.g., handle_post_login)                    │
│    ├─ user_exists_async() ──→ D1WasmStorage::query_async()  │
│    │                              │                         │
│    │                    JsFuture::from(promise).await        │
│    │                         │                             │
│    │              (proper .await, yields to JS)              │
│    │                                                          │
│    └─ SessionManager (async path via WasmSessionManager)     │
│         └─ storage.execute_async()                           │
│              └─ JsFuture::from(promise).await                │
│                   │                                          │
│         (proper .await, yields to JS)                        │
│                                                              │
│  For sync paths (if any still exist):                       │
│    └─ CredentialStorage::set()                               │
│         └─ StorageProvider::set()                            │
│              └─ schedule_future()                            │
│                   └─ drive_non_send_iterator()               │
│                        └─ run_until_next_state()             │
│                             └─ run_until()                   │
│                                  ├─ schedule_and_do_work()   │
│                                  │     → Pending             │
│                                  └─ yielder.yield_for(dur)   │
│                                       │                      │
│                                  JS: setTimeout, breaks loop  │
│                                  Native: SpinWaiter spins     │
└─────────────────────────────────────────────────────────────┘
```

## Spec-Wide Success Criteria

1. All existing valtron tests pass unchanged on native
2. `js_eventloop_yield` feature has no effect on native builds
3. `NotifyQueue::wait_for_item` returns `NotificationItem::None` after max_spins (not infinite loop)
4. On wasm32 with feature enabled, `SpinWaiter::wait()` returns immediately (< 5ms for any duration)
5. On wasm32 with feature enabled, D1-backed operations complete within timeout (no deadlock)
6. `cf-login-app` full flow (register → login → dashboard → logout) works on miniflare

## Feature Dependencies

```
01-notify-queue-contract-fix (critical, base)
    |
    v
02-js-eventloop-yield-end-to-end (done)
```

---

_Created: 2026-05-22_
_Structure: Feature-based (has_features: true)_
