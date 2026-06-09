---
feature_name: "Examples Showcase — Programmatic API Walkthroughs"
description: "Restructure existing examples and add comprehensive new ones for every VFS and IPC API. Each example lives in its own directory with an extensive README.md covering purpose, usage, architecture, and expected output. Covers MemoryFs, NativeFs, OverlayFs, ObservableFs, DeltaStore variants, Arrow serialization, FUSE mount, IPC bus, scaffold macro, and async/sync bridge patterns."
status: "in-progress"
priority: "low"
phase: 6
created: 2026-06-07
updated: 2026-06-09
dependencies:
  - "01-core-traits"
  - "02-memory-impls"
  - "04-native-fs"
  - "05-directory-delta"
  - "06-sqlite-delta"
  - "07-fuse-adapter"
  - "10-valtron-integration"
  - "12-arrow-serialization"
  - "15-observable-fs"
  - "18-scaffold-macro"
  - "23-inode-native-vfs"
tasks:
  completed: 9
  uncompleted: 12
  total: 21
  completion_percentage: 43%
---

# Feature 25: Examples Showcase — Programmatic API Walkthroughs

## Overview

The existing examples in `foundation_nativeapis/examples/` are flat `.rs` files with minimal documentation. This feature restructures them and adds new examples covering every major API surface in the crate. Each example lives in its own directory with:

- `main.rs` — the runnable example
- `README.md` — extensive documentation: purpose, prerequisites, how to run, architecture walkthrough, expected output, and links to relevant source/specs

This is the **final feature** in spec-37, meant to be completed after all other features are done. It serves as living documentation and a validation that every API is usable end-to-end from a consumer's perspective.

## Directory Structure

```
backends/foundation_nativeapis/examples/
├── memory_fs/
│   ├── main.rs              # MemoryFs CRUD, directory traversal, inode queries
│   └── README.md
├── native_fs/
│   ├── main.rs              # NativeFs wrapping a real directory
│   └── README.md
├── overlay_fs/
│   ├── main.rs              # OverlayFileSystem<NativeFs, MemoryDelta> layering
│   └── README.md
├── observable_fs/
│   ├── main.rs              # ObservableFs event stream, subscriber patterns
│   └── README.md
├── sqlite_delta/
│   ├── main.rs              # LibsqlDelta — SQLite-backed DeltaStore
│   └── README.md
├── arrow_serialization/
│   ├── main.rs              # VfsMetadata/VfsDirEntry ↔ Arrow IPC roundtrip
│   └── README.md
├── fuse_mount/
│   ├── main.rs              # Mount MemoryFs via FUSE, interact from shell
│   └── README.md
├── scaffold_macro/
│   ├── main.rs              # scaffold!() delegation patterns
│   └── README.md
├── ipc_bus/
│   ├── main.rs              # Restructured from ipc_bench/ipc_triangle
│   └── README.md
├── fd_readiness/
│   ├── main.rs              # Restructured from fd_readiness.rs
│   └── README.md
├── sync_async_bridge/
│   ├── main.rs              # SyncFs<AsyncFs> via valtron exec_async
│   └── README.md
└── inode_native/
    ├── main.rs              # Inode allocation, path_by_inode, stat_by_inode
    └── README.md
```

## README.md Template

Each README follows this structure:

```markdown
# Example: <Name>

## Purpose

What this example demonstrates and why it matters.

## Prerequisites

- Feature flags required (`--features vfs`, `--features vfs-fuse`, etc.)
- System requirements (Linux, /dev/fuse, etc.)
- Dependencies (if any external setup needed)

## How to Run

\`\`\`bash
cargo run -p foundation_nativeapis --features <flags> --example <name>
\`\`\`

## Architecture

Walkthrough of the code structure — what each section does,
which traits/types are used, and how they compose.

## Expected Output

\`\`\`
<sample terminal output>
\`\`\`

## Key APIs Demonstrated

- `VfsFileSystem::stat()` — ...
- `OverlayFileSystem::new()` — ...

## Related

- Feature spec: `specifications/37-overlay-vfs/features/XX-name/feature.md`
- Source: `src/shared/vfs/overlay_fs.rs`
```

## Tasks

### Restructure Existing Examples

- [ ] Move `fd_readiness.rs` → `fd_readiness/main.rs` + add README.md
- [ ] Move `file_watcher.rs` → `file_watcher/main.rs` + add README.md
- [ ] Consolidate `ipc_bench.rs`, `ipc_latency.rs`, `ipc_multiple_type.rs`, `ipc_region_free.rs`, `ipc_rejoin.rs`, `ipc_triangle.rs` → `ipc_bus/` directory with individual binaries + shared README.md
- [x] Update Cargo.toml `[[example]]` entries for new directory structure

### New VFS Examples

- [x] `memory_fs/` — MemoryFs CRUD: create files/dirs, stat, read/write, rename, remove, inode queries, directory listing
- [x] `native_fs/` — NativeFs wrapping a temp directory, demonstrating OS inode passthrough and real file I/O
- [x] `overlay_fs/` — OverlayFileSystem layering MemoryDelta over NativeFs, demonstrating copy-on-write and whiteout semantics
- [x] `observable_fs/` — ObservableFs wrapping MemoryFs, subscribing to VfsEvent stream, demonstrating event-driven patterns
- [x] `sqlite_delta/` — LibsqlDelta as a DeltaStore, demonstrating SQLite-backed file operations and flush/reset
- [ ] `arrow_serialization/` — VfsMetadata and VfsDirEntry Arrow IPC roundtrip, batch serialization, schema inspection
- [ ] `fuse_mount/` — Mount a MemoryFs via FUSE, show how to interact with it from a shell, graceful unmount
- [x] `scaffold_macro/` — Demonstrate scaffold!() + #[scaffold_impl] delegation, showing trait forwarding patterns
- [x] `inode_native/` — Inode allocation in MemoryFs, path_by_inode reverse lookup, stat_by_inode, rename preserves inode
- [x] `sync_async_bridge/` — SyncFs wrapping an async VFS impl via valtron exec_async, demonstrating the sync/async bridge

### Conditional Examples (added when dependent features complete)

- [ ] `fjall_fs/` — FjallFs/FjallDelta example (after Feature 19 completes)
- [ ] `turso_delta/` — TursoDelta example (if turso feature is testable locally)
- [ ] `ipc_daemon/` — IPC VFS daemon example (after Feature 11 completes)

### Validation

- [ ] All examples compile: `cargo build -p foundation_nativeapis --examples --features <all>`
- [ ] All examples run without errors on Linux
- [ ] Each README.md follows the template and includes expected output

## Cargo.toml Example Entries

Directory-based examples require explicit `[[example]]` entries:

```toml
[[example]]
name = "memory_fs"
path = "examples/memory_fs/main.rs"
required-features = ["vfs"]

[[example]]
name = "overlay_fs"
path = "examples/overlay_fs/main.rs"
required-features = ["vfs"]

[[example]]
name = "fuse_mount"
path = "examples/fuse_mount/main.rs"
required-features = ["vfs-fuse"]

[[example]]
name = "arrow_serialization"
path = "examples/arrow_serialization/main.rs"
required-features = ["vfs-arrow"]
```

## Verification

- Every `done` feature in spec-37 has at least one corresponding example
- All examples compile with their required feature flags
- All READMEs contain runnable commands and expected output
- Existing IPC examples still work after restructuring

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

_Created: 2026-06-07 | Updated: 2026-06-07_
