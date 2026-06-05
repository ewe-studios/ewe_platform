---
feature_name: "OverlayFileSystem Overlay"
description: "OverlayFileSystem<B, D> — the overlay composition primitive. Combines read-only base (VfsFileSystem) with writable delta (DeltaStore). Implements copy-on-write, whiteout semantics, and directory entry merging."
status: "done"
priority: "critical"
phase: 1
created: 2026-06-04
updated: 2026-06-05
dependencies:
  - "01-core-traits"
  - "02-memory-impls"
tasks:
  completed: 10
  uncompleted: 0
  total: 10
  completion_percentage: 100%
---

# Feature 03: OverlayFileSystem Overlay

## Overview

The intellectual core of the spec. `OverlayFileSystem<B: VfsFileSystem, D: DeltaStore>` itself implements `VfsFileSystem`, so overlays can stack. All reads check delta-then-base (with whiteout filtering). All writes go to delta (with CoW from base when needed).

Tested entirely with MemoryFs (base) + MemoryDelta (delta) — no OS dependencies.

## Implemented Files

| File | Contents |
|------|----------|
| `src/shared/vfs/overlay_fs.rs` | OverlayFileSystem<B, D>, OverlayFile enum, OverlaySeekableFile enum, OverlayDirectory |
| `tests/vfs_overlay_tests.rs` | 34 tests covering read passthrough, CoW, whiteouts, merging, rename, reset, stacking |

## Key Design Decisions

**Visibility rule:** Delta entry at exact path always wins over whiteouts. No cross-counter version comparison needed — the overlay contract ensures delta entries are only created after whiteout handling.

**File handles:** `OverlayFile<BF, DF>` enum dispatches to Base or Delta. Base variant write methods return ReadOnly error.

**OverlayDirectory:** Holds raw pointers to base/delta (owned by OverlayFileSystem). Requires `'static` bounds on B and D. Merges directory listings from both layers, excluding whiteout'd base entries.

**Version counter:** AtomicU64 owned by OverlayFileSystem, separate from delta's internal counter. Incremented on every mutation through the overlay.

## Tasks

- [x] OverlayFileSystem<B, D> struct with base, delta, Arc<AtomicU64> version
- [x] VfsFileSystem for OverlayFileSystem — visibility check, CoW, whiteout, all path ops
- [x] OverlayFile/OverlaySeekableFile enums with VfsFile dispatch
- [x] OverlayDirectory with merged directory listing (delta ∪ base − whiteouts)
- [x] CoW: read from base → ensure parent dirs in delta → create in delta → write content
- [x] Delete: remove from delta if present + whiteout if in base
- [x] Rename across layers: CoW to new path + whiteout old if in base
- [x] Stacked overlays: OverlayFileSystem<OverlayFileSystem<...>, ...> works
- [x] Reset: delta.reset() restores all base files
- [x] 34 tests covering all overlay semantics

## Test Coverage (34 tests)

**Read passthrough (4):** read base file, read base dir, stat base, exists base

**Write to delta (3):** create new file, mkdir, write_file convenience

**Copy-on-Write (4):** CoW on write, preserves content, preserves metadata, creates parent dirs

**Delete/whiteout (4):** delete base file, delete delta file, delete CoW'd file, delete nonexistent

**Whiteout + recreation (3):** create at whiteout path, dir whiteout hides children, recreate in whiteout'd dir

**Directory merging (4):** merge base+delta, excludes whiteouts, delta shadows base, list nonexistent

**Rename (4):** rename delta file, rename base file, rename to existing, rename nonexistent

**Reset/lifecycle (2):** reset restores base, version monotonic after reset

**Stacked overlays (2):** stacked read-through, stacked whiteouts

**Edge cases (4):** base file write triggers CoW not ReadOnly, version increments, root always exists, path normalization

---

_Created: 2026-06-04 | Completed: 2026-06-05_
