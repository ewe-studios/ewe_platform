---
feature: "foundation_conditional"
description: "Replace crates/trace with block-level conditional compilation macros (conditional::debug!, conditional::trace!) that gate entire code blocks on feature flags without repeating #[cfg()] per line"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# Feature: foundation_conditional

## Overview

Replace `crates/trace` (`ewe_trace`) with a proper block-level conditional compilation crate. The current `ewe_trace` wraps `tracing::info!`, `tracing::debug!`, etc. behind feature flags — but macros only work for single statements. Sometimes you need to gate an entire block of code (multiple lines, variable bindings, function calls) on a feature flag without repeating `#[cfg(feature = "X")]` on every line.

## Why not just use #[cfg()]

`#[cfg()]` works fine for functions, structs, and single expressions. But for inline blocks inside a function body, you have to wrap each statement individually:

```rust
#[cfg(feature = "debug_trace")]
let start = Instant::now();
#[cfg(feature = "debug_trace")]
tracing::debug!("processing file: {:?}", path);
#[cfg(feature = "debug_trace")]
let result = expensive_validation(&path);
#[cfg(feature = "debug_trace")]
tracing::debug!("validation took: {:?}", start.elapsed());
```

This is verbose, error-prone (easy to forget a `#[cfg]`), and harder to read.

## Design: Block-level macros

The `conditional` module provides macros that expand to the block when the feature is enabled, and to nothing when it's not:

```rust
use foundation_conditional::debug;

// Gates the entire block on the "debug_trace" feature
debug! {
    let start = Instant::now();
    tracing::debug!("processing file: {:?}", path);
    let result = expensive_validation(&path);
    tracing::debug!("validation took: {:?}", start.elapsed());
}
```

When `debug_trace` is not enabled, the macro expands to `{}` — the code inside is never compiled, never type-checked, never linked. Zero cost.

## Available macros

| Macro | Feature flag | Use case |
|---|---|---|
| `debug! { ... }` | `debug_trace` | Debug-only instrumentation, performance timing |
| `trace! { ... }` | `debug_trace` | Same as debug, semantically for trace-level |
| `test_util! { ... }` | `test_utils` | Test-only helper code in non-test functions |
| `cfg!(name => { ... })` | any feature name | General-purpose with explicit feature name |

## Completed Tasks

1. ✅ Created `backends/foundation_conditional/` directory
2. ✅ Created `Cargo.toml`: zero external dependencies, features `debug_trace` and `test_utils`
3. ✅ Created `src/lib.rs` with `debug!`, `trace!`, `test_util!`, `cfg_block!` macros
4. ✅ Added to `workspace.dependencies`
5. ✅ Replaced all `ewe_trace::{info,warn,error,debug}!` call sites with `tracing::` equivalents across:
   - `backends/foundation_html/src/parsers.rs`, `markup.rs`
   - `backends/foundation_packager/src/files.rs`, `package.rs`
   - `crates/devserver/src/cargo.rs`, `proxy.rs`, `operators.rs`, `streams.rs`, `watchers.rs`
   - `crates/watch_utils/src/lib.rs`
6. ✅ Updated `debug_trace` feature flags in consumers to point to `foundation_packager/debug_trace`
7. ✅ Deleted `crates/trace/`
8. ✅ Ran `cargo check` across workspace — all green

## Why not just use #[cfg()]

`#[cfg()]` works fine for functions, structs, and single expressions. But for inline blocks inside a function body, you have to wrap each statement individually:

```rust
#[cfg(feature = "debug_trace")]
let start = Instant::now();
#[cfg(feature = "debug_trace")]
tracing::debug!("processing file: {:?}", path);
#[cfg(feature = "debug_trace")]
let result = expensive_validation(&path);
#[cfg(feature = "debug_trace")]
tracing::debug!("validation took: {:?}", start.elapsed());
```

This is verbose, error-prone (easy to forget a `#[cfg]`), and harder to read.

## Design: Block-level macros

The `conditional` module provides macros that expand to the block when the feature is enabled, and to nothing when it's not:

```rust
use foundation_conditional::debug;

// Gates the entire block on the "debug_trace" feature
debug! {
    let start = Instant::now();
    tracing::debug!("processing file: {:?}", path);
    let result = expensive_validation(&path);
    tracing::debug!("validation took: {:?}", start.elapsed());
}
```

When `debug_trace` is not enabled, the macro expands to `{}` — the code inside is never compiled, never type-checked, never linked. Zero cost.

## Available macros

| Macro | Feature flag | Use case |
|---|---|---|
| `debug! { ... }` | `debug_trace` | Debug-only instrumentation, performance timing |
| `trace! { ... }` | `debug_trace` | Same as debug, semantically for trace-level |
| `test_util! { ... }` | `test_utils` | Test-only helper code in non-test functions |
| `cfg!(name => { ... })` | any feature name | General-purpose with explicit feature name |

## Implementation: declarative macro

```rust
/// Gates a block of code on the `debug_trace` feature.
/// Expands to the block when enabled, or `{}` when disabled.
#[macro_export]
macro_rules! debug {
    ($($block:tt)*) => {
        #[cfg(feature = "debug_trace")]
        { $($block)* }
        #[cfg(not(feature = "debug_trace"))]
        {}
    };
}
```

Key point: the `#[cfg]` sits **outside** the block, so the compiler never sees the contents when the feature is off. This means:
- Variables declared inside don't need to exist
- Function calls don't need to resolve
- Dependencies gated on that feature don't need to be available

## Caveat: variable scoping

Code after the block cannot reference variables declared inside it, since the block is a scope:

```rust
debug! {
    let result = expensive_computation();
};
// `result` is NOT visible here — it's in its own scope
// This is BY DESIGN: if the code compiled differently, `result` wouldn't exist
```

If you need the value outside the block, gate the entire usage:

```rust
debug! {
    let result = expensive_computation();
    tracing::debug!("result: {:?}", result);
    // use result here
};
```

## Tasks

1. Create `backends/foundation_conditional/` directory
2. Create `Cargo.toml`:
   - Package name: `foundation_conditional`
   - No external dependencies
   - Features: `debug_trace`, `test_utils`
3. Create `src/lib.rs` with `debug!`, `trace!`, `test_util!`, `cfg!` macros
4. Add to `workspace.dependencies`
5. Update all `ewe_trace::{info,warn,error,debug}!` call sites to use `tracing::info!` directly (the wrapper is unnecessary — `tracing` macros are already no-op when no subscriber is registered)
6. Replace patterns of repeated `#[cfg(feature = "debug_trace")]` with `debug! { ... }` blocks
7. Delete `crates/trace/`
8. Run `cargo check -p foundation_conditional --no-default-features` (should compile with zero deps)
9. Run `cargo check -p foundation_conditional` (with features)
10. Run workspace `cargo check` to verify no regressions
11. Commit and push

## Cargo.toml target

```toml
[package]
name = "foundation_conditional"
version = "0.1.0"
description = "Block-level conditional compilation macros for gating code on feature flags"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
keywords = ["conditional-compilation", "macros", "feature-flags"]

[features]
default = []
debug_trace = []
test_utils = []

[lints]
workspace = true
```

---

_Created: 2026-06-01_
