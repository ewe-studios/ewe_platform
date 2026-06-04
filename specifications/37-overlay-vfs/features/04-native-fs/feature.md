---
feature_name: "NativeFs Passthrough"
description: "NativeFs — host filesystem passthrough via std::fs with path containment (prevent directory escape), platform-specific optimizations (openat2 on Linux 5.6+)."
status: "pending"
priority: "high"
phase: 2
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature 04: NativeFs Passthrough

## Overview

Wraps `std::fs` operations behind VfsFileSystem. The root directory is configurable — all path resolution is confined within it. On Linux 5.6+, uses `openat2(RESOLVE_BENEATH)` for atomic kernel-enforced containment. Falls back to manual `..` rejection on older kernels and other platforms.

Inspired by iii-filesystem's path containment model.

## Tasks

### Core (`src/native/vfs/native_fs.rs`)

- [ ] Define `NativeFs` struct: root path, openat2 availability flag
- [ ] Implement `NativeFs::new(root: impl Into<PathBuf>)` — open root fd, probe openat2
- [ ] Implement path containment: reject `..` traversal, resolve within root
- [ ] Implement `VfsFile` for `NativeFile`: wraps `std::fs::File` with `read_at`/`write_at` via `FileExt::read_at`/`write_at` (or pread/pwrite)
- [ ] Implement `SeekableVfsFile` for `SeekableNativeFile`: wraps `std::fs::File` with seek
- [ ] Implement `VfsDirectory` for `NativeDirectory`: wraps directory path, uses `std::fs::read_dir`
- [ ] Implement `VfsFileSystem` for `NativeFs`: stat via `std::fs::metadata`, readdir, mkdir, remove, rename, symlink, etc.
- [ ] Implement `capabilities()`: seekable=true, symlinks=platform-dependent, event_emission=false
- [ ] Implement checksum: compute blake3 on file read/stat

### Platform Optimizations

- [ ] Linux: `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` for path containment (feature-detect)
- [ ] Linux: `preadv64`/`pwritev64` for offset-based I/O
- [ ] Fallback: manual path normalization + `..` rejection for macOS/Windows

### Tests (`tests/native_fs_test.rs`)

- [ ] Test: read existing file through NativeFs
- [ ] Test: path containment — `../../../etc/passwd` rejected
- [ ] Test: stat returns correct metadata (size, type, permissions, checksum)

## Verification

- Tests pass on Linux and macOS (CI)
- Path containment prevents directory escape
- `cargo check -p foundation_nativeapis --features vfs-native` passes
