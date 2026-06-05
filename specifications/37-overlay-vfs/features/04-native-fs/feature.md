---
feature_name: "NativeFs Passthrough"
description: "NativeFs — host filesystem passthrough via std::fs with path containment (prevent directory escape), platform-specific optimizations."
status: "done"
priority: "high"
phase: 2
created: 2026-06-04
updated: 2026-06-05
dependencies:
  - "01-core-traits"
tasks:
  completed: 14
  uncompleted: 0
  total: 14
  completion_percentage: 100%
---

# Feature 04: NativeFs Passthrough

## Overview

Wraps `std::fs` operations behind VfsFileSystem. All paths are confined within a configurable root directory. Uses manual canonicalization + prefix check for path containment on all platforms. Inspired by iii-filesystem's path containment model.

## Architecture

### Struct definition

```rust
pub struct NativeFs {
    root: PathBuf,       // canonical root path
}
```

### Path containment model

All incoming VFS paths (`/foo/bar.txt`) are joined with the root and canonicalized:

```
VFS path:    "/src/main.rs"
Root:        "/home/user/project"
Resolved:    "/home/user/project/src/main.rs"  ← must start with root
```

Security checks:
1. Join VFS path with root: `root.join(vfs_path.trim_start_matches('/'))`
2. Canonicalize (resolves symlinks, `..`, `.`)
3. Verify result starts with canonicalized root
4. If not → `VfsError::PermissionDenied` (path escape attempt)

This catches:
- `../../../etc/passwd` → after canonicalization, doesn't start with root
- Symlinks pointing outside root → canonicalized target doesn't start with root
- Unicode tricks, double encoding → canonicalization normalizes

### File handle types

```rust
pub struct NativeFile {
    file: std::fs::File,
    path: PathBuf,
    mode: OpenMode,
}
```

Uses `FileExt::read_at` / `FileExt::write_at` on Unix (`pread`/`pwrite` underneath) for offset-based I/O without changing the file offset.

```rust
pub struct SeekableNativeFile {
    file: std::fs::File,
    path: PathBuf,
    mode: OpenMode,
}
```

Uses `std::io::Seek` + `std::io::Read` + `std::io::Write`.

```rust
pub struct NativeDirectory {
    root: PathBuf,          // NativeFs root
    dir_path: String,       // VFS path of this directory
}
```

### Metadata mapping

```
std::fs::Metadata  →  VfsMetadata
  .len()           →  size
  .is_file()       →  VfsFileType::Regular
  .is_dir()        →  VfsFileType::Directory
  .is_symlink()    →  VfsFileType::Symlink
  .permissions()   →  permissions (mode bits on Unix)
  .modified()      →  modified
  .created()       →  created (may be None on some platforms)
  .accessed()      →  accessed
```

Owner (uid/gid): On Unix, read from `std::os::unix::fs::MetadataExt`. On Windows, set to `(0, 0)`.

Checksum: `Checksum::None` for Phase 2. Computing blake3 on every stat would be expensive — deferred to explicit request or lazy computation.

Version: Based on modification time converted to a monotonic counter relative to the NativeFs instance creation time. Or simply set to 0 for NativeFs (versioning is the overlay's concern, not the base filesystem's).

### Symlink handling

NativeFs follows symlinks transparently (via canonicalize). `readlink` returns the raw target. `symlink` creates a symlink.

Path containment applies AFTER symlink resolution — a symlink pointing outside the root is rejected at access time, not at creation time.

## Tasks

### Core (`src/native/vfs/native_fs.rs`)

- [x] Define `NativeFs` struct with root PathBuf
- [x] Implement `NativeFs::new(root: impl Into<PathBuf>)` — canonicalize root, verify exists
- [x] Implement `fn resolve_path(&self, vfs_path: &str) -> VfsResult<PathBuf>` — path containment
- [x] Implement `fn to_vfs_path(&self, fs_path: &Path) -> String` — convert OS path back to VFS path
- [x] Implement `NativeFile` struct + VfsFile trait (using pread/pwrite on Unix)
- [x] Implement `SeekableNativeFile` struct + SeekableVfsFile trait
- [x] Implement `NativeDirectory` struct + VfsDirectory trait
- [x] Implement `VfsFileSystem` for `NativeFs`
- [x] Implement metadata mapping (`std::fs::Metadata` → `VfsMetadata`)
- [x] Implement `capabilities()`: seekable=true, symlinks=true, permissions_enforced=cfg(unix), persistent=true

### Tests (`tests/vfs_native_tests.rs`)

Uses `tempfile::TempDir` for isolation.

- [x] `test_read_existing_file` — write file with std::fs, read through NativeFs
- [x] `test_write_and_read_back` — create+write through NativeFs, verify with std::fs
- [x] `test_path_containment_rejects_traversal` — `../../etc/passwd` → PermissionDenied
- [x] `test_path_containment_rejects_symlink_escape` — symlink pointing outside root → rejected
- [x] `test_stat_returns_correct_metadata` — size, type, permissions
- [x] `test_mkdir_and_list` — create directory, list contents
- [x] `test_rename_file` — rename, verify old gone, new exists
- [x] `test_remove_file` — remove, verify gone
- [x] `test_seekable_read_write` — seek, read, write with position tracking
- [x] `test_symlink_within_root` — symlink inside root works normally
- [x] `test_capabilities` — persistent=true, seekable=true
- [x] `test_root_directory_exists` — `/` resolves to the root dir, always exists

## Verification

- `cargo check -p foundation_nativeapis --features vfs-native` passes
- `cargo test -p foundation_nativeapis --features vfs-native --test vfs_native_tests` passes
- Path containment prevents directory escape in all tests

---

_Created: 2026-06-04 | Updated: 2026-06-05_
