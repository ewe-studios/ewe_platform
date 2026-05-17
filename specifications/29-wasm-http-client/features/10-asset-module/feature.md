---
feature: "Asset Module"
description: "Native-only asset.rs with include_str!(megatron.js), Cargo.toml include for publishing, #[cfg(not(target_arch = "wasm32"))] gating"
status: "pending"
priority: "medium"
depends_on: []
estimated_effort: "small"
created: 2026-05-18
last_updated: 2026-05-18
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Asset Module

## Overview

This feature adds a native-only `asset.rs` module to foundation_wasm that exposes the megatron.js runtime source code via `include_str!` for build-time access (e.g., build.rs scripts, code generators, deployment tooling). It is gated behind `#[cfg(not(target_arch = "wasm32"))]` so the JS source is NOT embedded in the wasm binary. It also updates `Cargo.toml` to include `megatron.js` in the published package.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | asset module, include_str!, cfg gating | `.agents/skills/rust-clean-code/skill.md` |

## Architecture (COMPREHENSIVE)

### File Structure

- `backends/foundation_wasm/src/asset.rs` — New file: native-only asset module
- `backends/foundation_wasm/Cargo.toml` — Update `include` field

### asset.rs

```rust
//! Native-only access to bundled JS runtime assets.
//!
//! This module is only available when compiling for non-wasm targets.
//! It provides the source code of the megatron.js runtime via `include_str!`,
//! allowing build scripts and tooling to access the JS runtime without
//! reading from the filesystem.
//!
//! # Usage
//!
//! ```rust
//! // In a build.rs or tool:
//! use foundation_wasm::assets::MEGATRON_JS;
//! println!("megatron.js size: {} bytes", MEGATRON_JS.len());
//! ```

/// The complete source code of the megatron.js JS runtime.
///
/// This is a `&'static str` containing the entire contents of
/// `sdk/jsruntime/megatron.js` at compile time.
///
/// # Platform Availability
///
/// Only available when **not** compiling for `wasm32` targets.
/// The wasm binary should not include the JS source code.
#[cfg(not(target_arch = "wasm32"))]
pub const MEGATRON_JS: &str = include_str!("../sdk/jsruntime/megatron.js");

/// The file path of the megatron.js runtime relative to the crate root.
///
/// Useful for build scripts that need to write the file to disk.
#[cfg(not(target_arch = "wasm32"))]
pub const MEGATRON_JS_PATH: &str = "sdk/jsruntime/megatron.js";
```

### Conditional Compilation

The module is conditionally exposed in `lib.rs`:

```rust
// In lib.rs, alongside the existing module declarations:

#[cfg(not(target_arch = "wasm32"))]
pub mod asset;
```

When compiling for `wasm32`:
- The `asset` module is NOT compiled
- `MEGATRON_JS` is NOT embedded in the wasm binary
- No binary size impact

When compiling for native targets:
- The `asset` module IS compiled
- `MEGATRON_JS` contains the full megatron.js source
- Build scripts and tools can access it

### Cargo.toml Include Update

Current `Cargo.toml`:
```toml
include = ["/src", "/runtime"]
```

Updated:
```toml
include = [
    "/src",
    "/runtime",
    "/sdk/jsruntime/megatron.js",
    "/LICENSE",
    "/README.md",
]
```

This ensures that when `cargo publish` packages the crate:
1. `megatron.js` is included in the tarball
2. Users who download the crate have access to the JS runtime
3. `include_str!("../sdk/jsruntime/megatron.js")` works for consumers

### Why This Matters

Consumers of foundation_wasm need megatron.js to:
1. **Load it in the browser** — The JS runtime must be loaded alongside the wasm module
2. **Build scripts** — A build.rs might bundle megatron.js with the wasm output
3. **Code generation** — Tools might analyze the JS runtime for type extraction
4. **Deployment** — Packaging tools need to know which JS files to deploy

Without `include_str!`, consumers would need to:
- Know the relative path to megatron.js in the git repo
- Handle version mismatches between crate and JS file
- Deal with the file not being present in the published crate

### Verification: Binary Size

To verify megatron.js is NOT in the wasm binary:

```bash
# Build for wasm
cargo build --target wasm32-unknown-unknown --release

# Check binary size (should NOT include ~200KB of JS)
ls -la target/wasm32-unknown-unknown/release/foundation_wasm.wasm

# Search for JS content (should NOT find it)
strings target/wasm32-unknown-unknown/release/foundation_wasm.wasm | grep -c "Megatron"
# Should output 0 or near-zero
```

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| `include_str!` for JS access | Zero-copy, compile-time embedding, no runtime overhead | File I/O at runtime — breaks in build scripts |
| Gated behind `not(target_arch = "wasm32")` | Don't bloat wasm binary with JS source | Always include — wastes 200KB in wasm binary |
| Cargo.toml `include` lists megatron.js | Published crate contains the JS file | Don't include — consumers can't find the file |
| `asset` module (not `assets`) | Single file, no need for plural | `assets` module — would imply multiple files |

## Implementation

### Files to Create/Modify

- `backends/foundation_wasm/src/asset.rs` — New file: native-only asset module
- `backends/foundation_wasm/src/lib.rs` — Add conditional `pub mod asset;`
- `backends/foundation_wasm/Cargo.toml` — Update `include` field

### Tasks

- [ ] T1: Create `src/asset.rs` with `MEGATRON_JS` and `MEGATRON_JS_PATH` constants
- [ ] T2: Add `#[cfg(not(target_arch = "wasm32"))] pub mod asset;` to lib.rs
- [ ] T3: Update Cargo.toml `include` to list `/sdk/jsruntime/megatron.js`
- [ ] T4: Verify `cargo build` on native target exposes MEGATRON_JS
- [ ] T5: Verify `cargo build --target wasm32-unknown-unknown` does NOT include JS in binary
- [ ] T6: Verify `cargo package` includes megatron.js in the tarball

## Testing

### Test Cases

1. **Native build**: `MEGATRON_JS` is accessible and contains valid JS source
2. **Wasm build**: `foundation_wasm::asset` module does not exist for wasm32
3. **Binary size**: wasm binary size is small (< 1MB), no JS content embedded
4. **Package**: `cargo package` includes `sdk/jsruntime/megatron.js` in the package

## Success Criteria

- [ ] All tasks completed
- [ ] MEGATRON_JS accessible on native, not on wasm
- [ ] wasm binary does not contain JS source (verified via strings/size check)
- [ ] cargo package includes megatron.js
- [ ] No regressions on existing functionality

---

_Created: 2026-05-18_
