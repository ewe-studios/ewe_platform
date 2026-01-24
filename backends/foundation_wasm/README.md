# Foundation WASM

A WASM-compatible runtime implementation providing JavaScript interoperability and memory management for `foundation_nostd`.

## Overview

`foundation_wasm` implements the nostd interface runtime for WebAssembly (WASM) and JavaScript interoperability. It provides:

- Memory allocation and management for WASM environments
- JavaScript API bindings for WASM modules
- Animation frame and interval callback handling
- Scheduling and registry systems for async operations
- Type-safe parameter encoding/decoding between WASM and JS

## ChangeLogs

### Version 0.0.2 (2026-01-24)

#### Breaking Changes
- **Migrated to `foundation_nostd::comp` compatibility module**
  - All synchronization primitives (`Mutex`, `RwLock`, `CondVar`) now use the `foundation_nostd::comp` module instead of `foundation_nostd::primitives` directly
  - This provides a unified API that automatically selects between `std` and `no_std` implementations
  - All `.lock()` calls now properly handle the `LockResult` return type using `.unwrap_or_else(foundation_nostd::comp::PoisonError::into_inner)`
  - Applied Clippy's suggestions to simplify closure syntax for better performance and readability (74 instances)

#### Files Updated
- `schedule.rs`: Updated `Mutex` imports and lock handling
- `registry.rs`: Updated `Mutex` imports and lock handling
- `intervals.rs`: Updated `Mutex` imports and lock handling
- `frames.rs`: Updated `Mutex` imports and lock handling
- `wrapped.rs`: Updated `WrappedItem` to use comp module
- `mem.rs`: Updated `MemoryAllocation` lock handling (~15 methods)
- `jsapi.rs`: Updated all static Mutex operations (~50+ lock calls)

#### Benefits
- **Unified API**: Single compatibility layer for all sync primitives
- **Future-proof**: Insulated from underlying implementation changes
- **Better abstraction**: Cleaner separation between std/nostd implementations
- **Consistent**: All mutex usage goes through the same interface

#### Tests
- All 41 unit tests passing
- No behavioral changes to existing functionality
- Lock poisoning handled gracefully in all contexts

---
