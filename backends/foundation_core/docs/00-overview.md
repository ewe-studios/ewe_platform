# foundation_core — Core platform utilities

## What it is
Shared infrastructure primitives used across all platform crates: async execution
(valtron), sync primitives, error handling, and common types.

## Key modules
- **`valtron/`** — Multi-threaded async executor with work-stealing. Designed for
  serverless/wasm where the sync/async split matters. Features: task scheduling,
  pool management, stream iterators, `Send`/`!Send` task handling.
- **`synca/`** — Sync primitives optimized for wasm (no OS threads). MPMC channels,
  waitgroups, idleman, event handles.
- **`io/`** — Buffer pools, byte streams, memory-mapped I/O helpers.
- **`macros/`** — Platform-agnostic derive macros and procedural utilities.

## Design principles
- **wasm-first**: All primitives work on single-threaded wasm via `SendWrapper`.
  The `multi` feature enables true parallelism on native.
- **zero-copy where possible**: Stream iterators, buffer pools, and byte views
  avoid allocations.
- **explicit send boundaries**: `Send` is never assumed — `SendWrapper` makes
  the single-threaded assumption explicit.
