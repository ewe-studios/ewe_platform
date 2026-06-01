---
description: "Add optional ValtronSingleton with OnceLock<PoolGuard> to single-threaded valtron executor, providing a convenient initialization + guard API similar to CfHttpAppSingleton."
status: "in_progress"
priority: "medium"
created: 2026-06-01
updated: 2026-06-01
author: "Main Agent"
metadata:
  version: "2.0"
  estimated_effort: "small"
  tags:
    - valtron
    - singleton
    - single-threaded
    - once-lock
    - convenience-api
has_features: false
has_fundamentals: false
builds_on: []
related_specs:
  - "specifications/32-cf-serve-app"
---

# Valtron Executor — Optional Singleton for Single-Threaded Mode

## Problem

### Single Executor

Callers in CF Worker / Web WASM contexts want a simple "init once, get guard" API — similar to `CfHttpAppSingleton::get_or_init()` — rather than having to call `initialize_pool()` and then `PoolGuard::default()` separately.

The existing free functions (`initialize_pool`, `spawn`, `run_until_complete`, etc.) work fine and continue to exist. This is a convenience wrapper.

## Approach

A simple `OnceLock<PoolGuard>` behind a zero-sized namespace struct:

```rust
thread_local! {
    static VALTRON_SINGLETON: OnceLock<PoolGuard> = OnceLock::new();
}

pub struct ValtronSingleton;

impl ValtronSingleton {
    /// Initialize the executor and return a PoolGuard.
    ///
    /// On first call: calls `initialize_pool(seed)`, stores a PoolGuard in
    /// the OnceLock, runs the setup closure. On subsequent calls: returns
    /// a cloned guard (PoolGuard is Copy/Clone since it's zero-sized in single mode).
    pub fn get_or_init<F: FnOnce(&PoolGuard)>(seed: u64, setup: F) -> PoolGuard { ... }

    /// Get an existing guard. Panics if not initialized.
    pub fn guard() -> PoolGuard { ... }

    /// Check if initialized.
    pub fn is_initialized() -> bool { ... }

    /// Reset for testing. cfg-gated.
    #[cfg(test)]
    pub fn reset() { ... }
}
```

### Storage mechanism

Uses the existing `thread_local!` with `OnceCell<LocalThreadExecutor<...>>` that already exists in the module. The singleton just adds a second `thread_local! OnceLock<PoolGuard>` for guard ownership.

The free functions (`initialize_pool`, `spawn`, `run_until_complete`, `run_until`, `run_once`) continue to exist unchanged — they use the existing `thread_local!` directly.

### Method internals

- `get_or_init`: calls `initialize_pool(seed)` (which sets up the `thread_local!` executor), creates `PoolGuard::default()`, stores it in the OnceLock, runs `setup(&guard)`, returns the guard.
- `guard`: calls `PoolGuard::default()` — the real work is in the thread-local executor which must have been initialized.
- `is_initialized`: checks if the thread-local executor has been initialized.

### Backward compatibility

All existing free functions remain. `ValtronSingleton` is optional — callers who want explicit lifecycle control use the free functions directly, callers who want convenience use the singleton.
