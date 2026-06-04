---
feature_name: "OverlayFileSystem Overlay"
description: "OverlayFileSystem<B, D> — the overlay composition primitive. Combines read-only base (VfsFileSystem) with writable delta (DeltaStore). Implements copy-on-write, whiteout semantics, and directory entry merging."
status: "pending"
priority: "critical"
phase: 1
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
  - "02-memory-impls"
tasks:
  completed: 0
  uncompleted: 18
  total: 18
  completion_percentage: 0%
---

# Feature 03: OverlayFileSystem Overlay

## Overview

The intellectual core of the spec. `OverlayFileSystem<B: VfsFileSystem, D: DeltaStore>` itself implements `VfsFileSystem`, so overlays can stack. All reads check delta-then-base (with whiteout filtering). All writes go to delta (with CoW from base when needed).

## Lookup Semantics

1. Check whiteouts — if whiteout'd, return NotFound
2. Check delta — if exists, return delta version
3. Check base — passthrough
4. Neither — NotFound

## Tasks

### Core Implementation (`src/shared/vfs/foundation_fs.rs`)

- [ ] Define `OverlayFileSystem<B: VfsFileSystem, D: DeltaStore>` struct
- [ ] Implement `OverlayFileSystem::new(base: B, delta: D)` constructor
- [ ] Implement `VfsFileSystem` for `OverlayFileSystem<B, D>`
- [ ] Implement `stat()`: whiteout check → delta → base
- [ ] Implement `exists()`: whiteout check → delta → base
- [ ] Implement `open(path, Read)`: return delta file if in delta, else base file (read-only)
- [ ] Implement `open(path, Write|ReadWrite)`: CoW from base to delta if not in delta, return delta file. CoW is atomic — partial failure cleans up, returns rich error with partial data location for caller resumption.
- [ ] Implement `open_seekable()`: same logic as open, seekable handles
- [ ] Implement `open_directory()`: returns FoundationFsDirectory with merged view
- [ ] Implement `create()`: create in delta with new version. If path has a whiteout, delta entry's higher version supersedes it (whiteout stays).
- [ ] Implement `mkdir()`: create in delta
- [ ] Implement `remove()`: remove from delta if present + add whiteout if in base
- [ ] Implement `rename()`: atomic — CoW source if needed, write to delta at new path, whiteout old path. If write fails, no whiteout, cleanup partial state.
- [ ] Implement `chmod()`, `symlink()`, `readlink()` with overlay semantics

### Directory Entry Merging

- [ ] Implement `readdir()` / directory `list()`: delta entries ∪ base entries − whiteouts, deduplicated
- [ ] Handle versioned whiteout resolution: whiteout at version N hides base entries; delta entries at version M > N supersede the whiteout
- [ ] Handle nested whiteout inheritance: whiteout `/a/b` at version N hides `/a/b/**` from base

### OverlayFileSystem File Handle

- [ ] Define `FoundationFsFile` enum: `Base(B::File)` or `Delta(D::File)` — internal dispatch, no boxing
- [ ] Implement `VfsFile` for `FoundationFsFile`: dispatch read_at/write_at to inner

### Tests (`tests/foundation_fs_test.rs`)

- [ ] Test: read existing file passes through base
- [ ] Test: write creates file in delta, not base
- [ ] Test: write to existing base file triggers CoW — base unchanged, delta has modified version
- [ ] Test: delete base file creates whiteout, file no longer visible
- [ ] Test: create at whiteout'd path — higher version supersedes whiteout, base files stay hidden
- [ ] Test: readdir merges base + delta, excludes whiteouts
- [ ] Test: whiteout inheritance — delete dir hides all children
- [ ] Test: reset clears delta, base files visible again
- [ ] Test: stacked overlays work — `OverlayFileSystem<OverlayFileSystem<MemoryFs, MemoryDelta>, MemoryDelta>`
- [ ] Test: rename across layers is atomic — write failure leaves file at original path
- [ ] Test: CoW failure cleans up partial delta write, returns rich error with partial data context
- [ ] Test: stat during CoW returns base metadata (delta not yet visible)
- [ ] Test: version ordering — recreate whiteout'd directory, new files visible, old base files still hidden

## Verification

- All tests pass with `MemoryFs` base + `MemoryDelta` delta (pure in-memory, no OS deps)
- `cargo check --features vfs --target wasm32-unknown-unknown` passes
