---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F37-surface-2-native-lib"
this_file: "specifications/52-tauri-foundation-platform/features/F37-surface-2-native-lib/feature.md"

status: resolved
priority: medium
created: 2026-07-21
updated: 2026-07-21

depends_on:
  - "F13-cross-platform-builds"
  - "F17-app-build-pipeline"

tasks:
  completed: 1
  uncompleted: 0
  total: 1
  completion_percentage: 100%
---

# F37 — Surface 2: native static library

**Resolution: No work required.** Surface 2 is already available.

Users who need native performance add their crate as a dependency to
`src-tauri/Cargo.toml` and call it directly. The compiler handles
`.a`/`.so` linking automatically — this is standard Rust crate linking,
not a platform feature. No annotation, no codegen, no build.rs changes.

The `src-tauri` crate already:
- Compiles as `staticlib` + `cdylib` (`crate-type = ["lib", "cdylib", "staticlib"]`)
- Links against `foundation_platform` and all other backends
- Any dependency added to `src-tauri/Cargo.toml` is automatically linked

Users write native code in their own crates, add them to `src-tauri/Cargo.toml`,
and call them directly from `src-tauri/src/lib.rs`. Zero additional platform work.

**Status: Resolved (zero platform work needed).**
