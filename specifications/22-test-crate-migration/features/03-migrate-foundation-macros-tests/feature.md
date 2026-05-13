---
name: "Migrate foundation_macros Tests"
description: "Move foundation_macros tests from tests/backends/ to backends/foundation_macros/tests/"
status: "completed"
priority: "low"
dependencies: ["01-migrate-foundation-core-tests"]
estimated_effort: "small"
---

# Feature: Migrate foundation_macros Tests

## Overview

Move `test_macros_embeddeders.rs` from `tests/backends/` to `backends/foundation_macros/tests/`.

## Source File

| File | Uses |
|------|------|
| `test_macros_embeddeders.rs` | `foundation_macros::EmbedDirectoryAs`, `foundation_macros::EmbedFileAs` |

## File Contents Analysis

```rust
use foundation_macros::EmbedDirectoryAs;
use foundation_macros::EmbedFileAs;

#[derive(EmbedFileAs, Default)]
#[source = "../../assets/hello/world.js"]
#[with_utf16]
pub struct JSHostRuntime;

#[derive(EmbedDirectoryAs, Default)]
#[source = "../../assets/hello"]
#[with_utf16]
pub struct JSHostRuntimeAssets;

#[test]
fn can_read_data_from_js_host_runtime() {
    let runtime = JSHostRuntime::default();
    assert_eq!(runtime.read_utf8(), Some(vec![]));
}
```

## Destination

```
backends/foundation_macros/tests/
└── test_embeddeders.rs
```

## Required Changes

### 1. Move File
Copy to `backends/foundation_macros/tests/test_embeddeders.rs`

### 2. Update Source Path
The `#[source = "../../assets/hello/world.js"]` path needs adjustment:
- Current: relative to `tests/` (which is at repo root level)
- New: relative to `backends/foundation_macros/tests/`

New paths should be:
```rust
#[source = "../../../../assets/hello/world.js"]
// or
#[source = "/home/darkvoid/Boxxed/@dev/ewe_platform/assets/hello/world.js"]
```

Actually, better approach: use absolute path from workspace root:
```rust
#[source = "assets/hello/world.js"]
```

Check if `EmbedFileAs` supports workspace-relative paths.

### 3. Update backends/foundation_macros/tests/mod.rs
```rust
mod test_embeddeders;
```

### 4. Remove from tests/backends/
Remove `test_macros_embeddeders.rs` and update `mod.rs`.

## Verification Steps

1. Run `cargo test -p foundation_macros`
2. Verify macro tests compile and pass
3. Check asset paths resolve correctly

## Risk

**Medium**: The `#[source]` paths are relative and may need adjustment for the new location.
