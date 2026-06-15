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
  completed: 18
  uncompleted: 0
  total: 18
  completion_percentage: 100%
---

# Feature 03: OverlayFileSystem Overlay

## Overview

The intellectual core of the spec. `OverlayFileSystem<B: VfsFileSystem, D: DeltaStore>` itself implements `VfsFileSystem`, so overlays can stack. All reads check delta-then-base (with whiteout filtering). All writes go to delta (with CoW from base when needed).

Tested entirely with MemoryFs (base) + MemoryDelta (delta) — no OS dependencies.

## Architecture

### Struct definition

```rust
pub struct OverlayFileSystem<B: VfsFileSystem, D: DeltaStore> {
    base: B,
    delta: D,
    version: Arc<AtomicU64>,
}
```

The `version` counter is owned by OverlayFileSystem, NOT by the base or delta. It provides a single global ordering for all overlay operations. Every mutation through the overlay increments it.

### Lookup state machine

Every path operation follows this resolution:

```
                    ┌──────────────┐
                    │  Input path  │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │  Normalize   │
                    │  path        │
                    └──────┬───────┘
                           │
                    ┌──────▼──────────────────┐
                    │  delta.is_whiteout(path) │
                    │  → returns Option<u64>   │
                    └──────┬──────────────────┘
                           │
                  ┌────────┴────────┐
             Some(wh_ver)       None
                  │                 │
                  │          ┌──────▼──────────┐
                  │          │  delta.exists()  │
                  │          └──────┬──────────┘
                  │            ┌────┴────┐
                  │         exists    not exists
                  │            │         │
                  │            │  ┌──────▼──────────┐
                  │            │  │  base.exists()   │
                  │            │  └──────┬──────────┘
                  │            │    ┌────┴────┐
                  │            │  exists    not exists
                  │            │    │         │
                  │            │    │    ┌────▼──────┐
                  │            │    │    │  NotFound  │
                  │            │    │    └───────────┘
                  │            │    │
                  │        ┌───▼────▼───┐
                  │        │  Return    │
                  │        │  result    │
                  │        └────────────┘
                  │
           ┌──────▼──────────────────────┐
           │  Check delta for path       │
           │  Does delta entry exist     │
           │  with version > wh_ver?     │
           └──────┬──────────────────────┘
                  │
          ┌───────┴───────┐
       version > wh     version <= wh
       (superseded)      (still hidden)
          │                    │
     ┌────▼──────┐     ┌──────▼──────┐
     │  Return   │     │  NotFound   │
     │  delta    │     └─────────────┘
     └───────────┘
```

### Copy-on-Write (CoW) flow

Triggered when `open(path, Write|ReadWrite)` and path exists only in base:

```
open(path, Write)
    │
    ├── Check delta → not found
    ├── Check whiteout → not whiteout'd
    ├── Check base → exists
    │
    ▼
CoW: copy base → delta
    │
    ├── 1. Read full content from base: base.read_file(path)
    ├── 2. Read metadata from base: base.stat(path)
    ├── 3. Ensure parent dirs exist in delta (mkdir_all)
    ├── 4. Create file in delta: delta.create(path, permissions)
    ├── 5. Write content to delta: delta file handle write
    ├── 6. Update metadata (version = next_version())
    │
    ├── On failure at any step:
    │   ├── Clean up partial delta write (remove if created)
    │   └── Return error (base file remains untouched)
    │
    └── On success:
        └── Return delta file handle (Write mode)
```

### Whiteout semantics (versioned)

```
Scenario 1: Delete base file
─────────────────────────────
  base has /src/main.rs
  overlay.remove("/src/main.rs")
    → delta.add_whiteout("/src/main.rs", version=5)
    → overlay.exists("/src/main.rs") → false

Scenario 2: Delete delta file (also in base)
─────────────────────────────────────────────
  base has /src/main.rs
  delta has /src/main.rs (from earlier CoW)
  overlay.remove("/src/main.rs")
    → delta.remove("/src/main.rs")   (remove delta copy)
    → delta.add_whiteout("/src/main.rs", version=6)  (hide base)
    → overlay.exists("/src/main.rs") → false

Scenario 3: Delete delta-only file
───────────────────────────────────
  delta has /new_file.txt (no base equivalent)
  overlay.remove("/new_file.txt")
    → delta.remove("/new_file.txt")
    → NO whiteout needed (nothing in base to hide)

Scenario 4: Recreate after delete
─────────────────────────────────
  whiteout on "/src/" at version 5  (hides base /src/ and all children)
  overlay.create("/src/new.rs")
    → delta.mkdir("/src") with version=8  (if not already in delta)
    → delta.create("/src/new.rs") with version=9
    → overlay.exists("/src/new.rs") → true  (version 9 > whiteout 5)
    → overlay.exists("/src/main.rs") → false  (base file, hidden by whiteout 5, no delta override)

Scenario 5: Directory whiteout inheritance
──────────────────────────────────────────
  whiteout on "/a" at version 3
  overlay.exists("/a/b/c.txt") → false  (inherited from /a whiteout)
  overlay.exists("/a") → false
  overlay.create("/a/b/c.txt")
    → delta.mkdir_all("/a/b") with version 4, 5
    → delta.create("/a/b/c.txt") with version 6
    → overlay.exists("/a/b/c.txt") → true  (version 6 > 3)
    → overlay.exists("/a") → true  (delta dir version 4 > 3)
    → overlay.list("/a") → ["b"]  (only delta entries, base hidden by whiteout)
```

### Directory entry merging algorithm

```
fn list_directory(overlay, dir_path) → Vec<VfsDirEntry>:
    // 1. Collect delta entries
    delta_entries = delta.open_directory(dir_path).list()
    delta_names = {entry.name for entry in delta_entries}

    // 2. Collect base entries, filtering
    merged = delta_entries.clone()
    for entry in base.open_directory(dir_path).list():
        if entry.name in delta_names:
            continue  // delta shadows base
        full_path = dir_path + "/" + entry.name
        if delta.is_whiteout(full_path) is Some(wh_ver):
            // Check if a delta entry supersedes the whiteout
            // (already handled above — if delta has it, it's in delta_names)
            continue  // whiteout hides base entry
        merged.push(entry)

    // 3. Return merged, already deduplicated
    return merged
```

Edge cases:
- Base dir doesn't exist (only delta has it): return delta entries only
- Delta dir doesn't exist (base only): return filtered base entries
- Both exist: merge as above
- Neither exists: NotFound error
- Dir is whiteout'd but delta has higher-version dir: return delta entries only (base hidden)

### Rename across layers

Atomic three-step operation:

```
rename(from_path, to_path):
    1. Ensure source exists (delta or base)
    2. If source in base only → CoW to delta at to_path
       If source in delta → delta.rename(from_path, to_path)
    3. If source was in base → add whiteout at from_path
    4. On failure: rollback any partial state
```

### File handle types

```rust
pub enum OverlayFile<BF: VfsFile, DF: VfsFile> {
    Base(BF),
    Delta(DF),
}
```

Implements `VfsFile` by dispatching to inner. `Base` variant is read-only — `write_at` returns `ReadOnly` error.

```rust
pub enum OverlaySeekableFile<BF: SeekableVfsFile, DF: SeekableVfsFile> {
    Base(BF),
    Delta(DF),
}
```

Same pattern for seekable files.

### OverlayDirectory

```rust
pub struct OverlayDirectory<B: VfsFileSystem, D: DeltaStore> {
    overlay: Arc<OverlayFileSystem<B, D>>,  // or reference back
    path: String,
}
```

Directory handle that delegates back to the overlay for all operations, scoped to its path. The `list()` method implements the merging algorithm above.

**Design note:** OverlayDirectory needs a reference back to the full OverlayFileSystem to do lookups. Since OverlayFileSystem owns both base and delta, the directory holds a shared reference (Arc) to the overlay.

This means OverlayFileSystem needs interior mutability for the version counter (AtomicU64) and delegates actual mutation to the delta store (which has its own internal locking).

## Tasks

### Core Implementation (`src/shared/vfs/overlay_fs.rs`)

- [x] Define `OverlayFileSystem<B, D>` struct with base, delta, version counter
- [x] Implement `OverlayFileSystem::new(base, delta)` constructor
- [x] Implement `next_version(&self) -> u64` — atomic increment
- [x] Implement `fn resolve_path(&self, path: &str) -> PathResolution` — the lookup state machine
- [x] Implement `VfsFileSystem` for `OverlayFileSystem<B, D>`
- [x] Implement `stat()`: whiteout → delta → base
- [x] Implement `exists()`: whiteout → delta → base
- [x] Implement `open(path, Read)`: delta file if in delta, else base file (wrapped as read-only)
- [x] Implement `open(path, Write|ReadWrite)`: CoW if needed, return delta file
- [x] Implement `open_seekable()`: same logic with seekable handles
- [x] Implement `create()`: create in delta with next version
- [x] Implement `mkdir()`: create directory in delta
- [x] Implement `remove()`: handle delta-only, base-only, and both-exist cases
- [x] Implement `rename()`: atomic rename across layers
- [x] Implement `chmod()`, `symlink()`, `readlink()` with overlay semantics
- [x] Implement `open_directory()`: return OverlayDirectory

### Directory Entry Merging

- [x] Implement `OverlayDirectory` struct
- [x] Implement `list()` with the merge algorithm (delta ∪ base − whiteouts)
- [x] Implement path resolution methods delegating to overlay

### File Handle Types

- [x] Define `OverlayFile<BF, DF>` enum + implement VfsFile
- [x] Define `OverlaySeekableFile<BF, DF>` enum + implement SeekableVfsFile
- [x] Base variant write methods return ReadOnly error

### Public API

- [x] `OverlayFileSystem::base(&self) -> &B` — access base (read-only inspection)
- [x] `OverlayFileSystem::delta(&self) -> &D` — access delta (inspection, flush, reset)
- [x] `OverlayFileSystem::version(&self) -> u64` — current version counter

## Tests (`tests/vfs_overlay_tests.rs`)

All tests use `MemoryFs` as base + `MemoryDelta` as delta. `#[traced_test]` on all.

### Read passthrough
- [x] `test_read_base_file` — file in base, read through overlay, contents match
- [x] `test_read_base_directory` — list directory from base through overlay
- [x] `test_stat_base_file` — stat base file through overlay, metadata correct
- [x] `test_exists_base_file` — exists returns true for base file

### Write to delta
- [x] `test_create_new_file` — create file through overlay, exists in delta, not in base
- [x] `test_mkdir_through_overlay` — mkdir through overlay, visible in overlay listing
- [x] `test_write_file_convenience` — write_file through overlay, read_file returns same data

### Copy-on-Write
- [x] `test_cow_on_write` — open base file for write, triggers CoW, base unchanged, delta has modified copy
- [x] `test_cow_preserves_content` — after CoW, delta file starts with base file's original content
- [x] `test_cow_preserves_metadata` — after CoW, delta file metadata matches base (except version)
- [x] `test_cow_creates_parent_dirs` — CoW on /a/b/c.txt creates /a/ and /a/b/ in delta if needed

### Delete (whiteout)
- [x] `test_delete_base_file` — remove base file, whiteout created, exists returns false
- [x] `test_delete_delta_file` — remove delta-only file, no whiteout needed
- [x] `test_delete_cow_file` — remove file that was CoW'd: delta file removed + whiteout
- [x] `test_delete_returns_not_found` — remove non-existent path → NotFound

### Whiteout + recreation
- [x] `test_create_at_whiteout_path` — whiteout /foo.txt, create /foo.txt, new version supersedes whiteout
- [x] `test_directory_whiteout_hides_children` — whiteout /src, all /src/** from base hidden
- [x] `test_recreate_in_whiteout_directory` — whiteout /src, create /src/new.rs, new file visible, old base files still hidden
- [x] `test_version_ordering` — verify version comparisons resolve correctly

### Directory merging
- [x] `test_merge_base_and_delta_entries` — files in both base and delta, list returns union minus duplicates
- [x] `test_merge_excludes_whiteouts` — whiteout'd base entries excluded from merged listing
- [x] `test_merge_delta_shadows_base` — same filename in base and delta, delta version returned
- [x] `test_list_empty_overlay_dir` — empty directory returns empty list
- [x] `test_list_nonexistent_dir` — listing non-existent directory → NotFound

### Rename
- [x] `test_rename_delta_file` — rename delta-only file, old path gone, new path has content
- [x] `test_rename_base_file` — rename base file: CoW to new path + whiteout old
- [x] `test_rename_to_existing_path` — rename to path that exists → AlreadyExists
- [x] `test_rename_nonexistent` — rename non-existent → NotFound

### Reset / lifecycle
- [x] `test_reset_restores_base` — after writes+deletes, delta.reset(), all base files visible again
- [x] `test_version_after_reset` — version counter not reset (monotonic for the overlay lifetime)

### Stacked overlays
- [x] `test_stacked_overlay` — `OverlayFileSystem<OverlayFileSystem<MemoryFs, MemoryDelta>, MemoryDelta>` — write to outer, read falls through to inner base
- [x] `test_stacked_whiteouts` — whiteout in inner overlay, outer overlay still sees NotFound

### Edge cases
- [x] `test_open_read_only_base_file_for_write` — triggers CoW, not ReadOnly error
- [x] `test_concurrent_version_increment` — multiple mutations, versions strictly monotonic
- [x] `test_root_directory_always_exists` — root `/` exists even in empty overlay
- [x] `test_path_normalization` — paths with trailing slashes, double slashes resolved correctly

## Verification

- All tests pass: `cargo test -p foundation_nativeapis --features vfs -- vfs_overlay`
- `cargo check -p foundation_nativeapis --features vfs` — zero warnings
- No OS-specific deps — pure in-memory testing


## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

_Created: 2026-06-04 | Updated: 2026-06-05_
