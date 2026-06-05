---
feature_name: "DirectoryDelta"
description: "DirectoryDelta — shadow directory-based DeltaStore. Writes go to a shadow directory as real files. Whiteouts stored as sentinel files (.wh.<name>). Simple, inspectable, no extra dependencies."
status: "done"
priority: "high"
phase: 2
created: 2026-06-04
updated: 2026-06-05
dependencies:
  - "01-core-traits"
  - "04-native-fs"
tasks:
  completed: 11
  uncompleted: 0
  total: 11
  completion_percentage: 100%
---

# Feature 05: DirectoryDelta

## Overview

The simplest persistent DeltaStore — mirrors the filesystem structure under a shadow directory. Modified and new files are stored as real files. Whiteouts are sentinel files named `.wh.<basename>`. The directory is fully inspectable with standard tools (`ls`, `cat`).

Uses NativeFs internally for all filesystem operations on the shadow directory.

## Architecture

### Struct definition

```rust
pub struct DirectoryDelta {
    fs: NativeFs,                              // pointed at shadow directory
    whiteouts: RwLock<HashMap<String, u64>>,   // in-memory whiteout cache
}
```

### Shadow directory layout

```
shadow_dir/                    ← root of DirectoryDelta
├── src/
│   ├── main.rs                ← modified file (CoW'd from base)
│   ├── new.rs                 ← new file (created through overlay)
│   └── .wh.deleted.rs         ← whiteout sentinel for /src/deleted.rs
├── .wh.old_readme.md          ← whiteout sentinel for /old_readme.md
└── build/
    └── output.bin             ← new file
```

### Whiteout sentinel files

Format: `.wh.<basename>` in the same directory as the whiteout'd path.

Content: version number as UTF-8 text (e.g., `"7"`). This allows version recovery after restart.

Examples:
- Whiteout `/src/main.rs` → create `shadow/src/.wh.main.rs` containing version
- Whiteout `/readme.md` → create `shadow/.wh.readme.md` containing version

### Whiteout inheritance

For directory whiteouts (whiteout on `/src`), create `.wh.src` in the parent dir. `is_whiteout("/src/anything")` checks:
1. Exact: `shadow/src/.wh.anything` exists?
2. Parent whiteout: `shadow/.wh.src` exists?
3. Grandparent whiteout: (continue up)

In-memory cache (`HashMap<String, u64>`) is built at construction by scanning for `.wh.*` files. Updated on add/remove. This avoids filesystem round-trips for every `is_whiteout` check.

### VfsFileSystem delegation

DirectoryDelta delegates all VfsFileSystem methods to its inner NativeFs. The `.wh.*` sentinel files are hidden from directory listings (filtered out).

### Lifecycle

- `flush()`: Sync shadow directory to disk
- `reset()`: Remove all files in shadow directory (rm -rf contents, recreate root)

## Tasks

### Core (`src/native/vfs/dir_delta.rs`)

- [x] Define `DirectoryDelta` struct with inner NativeFs + whiteout cache
- [x] Implement `DirectoryDelta::new(shadow_path: impl Into<PathBuf>)` — create shadow dir, scan for existing whiteouts
- [x] Implement `fn scan_whiteouts(root: &Path) -> HashMap<String, u64>` — recursive scan for `.wh.*` files
- [x] Implement `DeltaStore` for `DirectoryDelta`:
  - `add_whiteout(path, version)` → create `.wh.<basename>` sentinel with version content + update cache
  - `is_whiteout(path)` → check cache (exact + ancestors)
  - `remove_whiteout(path)` → delete sentinel file + remove from cache
  - `list_whiteouts(dir)` → filter cache by directory prefix
  - `flush()` → fsync shadow directory
  - `reset()` → rm all contents of shadow dir + clear cache
- [x] Implement `VfsFileSystem` delegation to inner NativeFs with `.wh.*` filtering on `list()`
- [x] Implement directory listing that filters out `.wh.*` sentinel files

### Tests (`tests/vfs_dir_delta_tests.rs`)

Uses `tempfile::TempDir` for isolation.

- [x] `test_write_creates_file_in_shadow` — write through delta, verify file exists on disk
- [x] `test_whiteout_creates_sentinel` — add_whiteout, verify `.wh.` file exists on disk
- [x] `test_whiteout_check_from_cache` — is_whiteout returns Some after add_whiteout
- [x] `test_whiteout_inheritance` — parent whiteout hides children
- [x] `test_whiteout_scan_on_construction` — create sentinel files manually, new DirectoryDelta finds them
- [x] `test_reset_clears_shadow` — reset removes all files and whiteouts
- [x] `test_listing_excludes_sentinels` — directory listing doesn't show `.wh.*` files
- [x] `test_end_to_end_overlay` — OverlayFileSystem<NativeFs, DirectoryDelta> full workflow

## Verification

- `cargo check -p foundation_nativeapis --features vfs-native` passes
- `cargo test -p foundation_nativeapis --features vfs-native --test vfs_dir_delta_tests` passes
- Shadow directory inspectable with `ls -la`
- Sentinel files contain version numbers

---

_Created: 2026-06-04 | Updated: 2026-06-05_
