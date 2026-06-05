---
feature_name: "NativeFs Passthrough"
description: "NativeFs — host filesystem passthrough via std::fs with path containment (canonicalize + starts_with), Arc<PathBuf> root for cheap cloning."
status: "done"
priority: "high"
phase: 2
created: 2026-06-04
updated: 2026-06-05
dependencies:
  - "01-core-traits"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# Feature 04: NativeFs Passthrough

## Overview

Wraps `std::fs` operations behind VfsFileSystem. All paths are confined within a configurable root directory via canonicalize + starts_with check. Uses `pread`/`pwrite` (via `FileExt`) on Unix for offset-based I/O.

## Implemented Files

| File | Contents |
|------|----------|
| `src/native/vfs/native_fs.rs` | NativeFs (Arc<PathBuf> root), NativeFile, SeekableNativeFile, NativeDirectory |
| `tests/vfs_native_tests.rs` | 16 tests covering read/write, path containment, symlink escape, seekable, metadata |

## Key Design Decisions

**Arc<PathBuf> root:** Cheap cloning for NativeFs and NativeDirectory instances — sharing the same root allocation.

**Path containment (defense in depth):**
1. `normalize_vfs_path` rejects `..` components (shared layer)
2. `resolve_path` canonicalizes and checks `starts_with(root)` (NativeFs layer)
3. `exists()` returns `Ok(false)` on resolve failure instead of erroring

**NativeFile:** Uses `FileExt::read_at`/`write_at` on Unix (pread/pwrite) for offset-based I/O without changing file offset. Non-Unix returns `Unsupported`.

**SeekableNativeFile:** Wraps `std::fs::File` in `RwLock` for thread-safe seek + read/write. Position tracked via `AtomicU64`.

## Tasks

- [x] NativeFs struct with Arc<PathBuf> root, resolve_path containment, Clone derive
- [x] NativeFile (VfsFile) with pread/pwrite on Unix
- [x] SeekableNativeFile (SeekableVfsFile) with RwLock<File> + AtomicU64 position
- [x] NativeDirectory (VfsDirectory) with Arc<PathBuf> fs_root
- [x] VfsFileSystem for NativeFs — all path operations, metadata mapping
- [x] Path containment: canonicalize + starts_with, exists() catches resolve errors
- [x] capabilities(): seekable=true, symlinks=true, permissions_enforced=cfg(unix), persistent=true
- [x] 16 tests: read/write, path traversal rejection, symlink escape, stat, mkdir, rename, remove, seekable, capabilities

## Test Coverage (16 tests)

**Basic I/O (3):** read existing file, write+read back, create+read via handle

**Path containment (2):** traversal rejected, symlink escape rejected (Unix)

**Metadata (1):** stat returns correct size/type/modified

**Directory ops (2):** mkdir+list, mkdir_all

**File ops (4):** rename, remove, read_at offset, remove_all

**Seekable (1):** seek+read with position tracking

**Symlinks (1):** symlink within root works normally

**Other (2):** capabilities, root directory exists

---

_Created: 2026-06-04 | Completed: 2026-06-05_
