---
feature_name: "Core VFS Traits + Types"
description: "Define VfsFile, SeekableVfsFile, VfsDirectory, VfsFileSystem, DeltaStore traits plus supporting types (VfsMetadata, VfsDirEntry, OpenMode, VfsCapabilities, Checksum) and error types via foundation_errstack."
status: "pending"
priority: "critical"
phase: 1
created: 2026-06-04
updated: 2026-06-04
dependencies: []
tasks:
  completed: 0
  uncompleted: 22
  total: 22
  completion_percentage: 0%
---

# Feature 01: Core VFS Traits + Types

## Overview

The foundational layer — every other feature depends on this. Define the trait hierarchy and supporting types that all VFS implementations will use. Both sync and async variants. Zero external dependencies.

## Trait Hierarchy

```
VfsFile (offset-based byte I/O)
    └── SeekableVfsFile (adds seek-position state)

VfsDirectory (directory-specific operations, path resolution)

VfsFileSystem (path + typed open, produces File/SeekableFile/Directory)
    └── DeltaStore (adds whiteout management + lifecycle)
```

## Tasks

### Types (`src/shared/vfs/types.rs`)

- [ ] Define `OpenMode` enum: `Read`, `Write`, `ReadWrite`
- [ ] Define `VfsFileType` enum: `Regular`, `Directory`, `Symlink`
- [ ] Define `Checksum` enum: `Blake3([u8; 32])`, `Md5([u8; 16])`, `Crc32(u32)`
- [ ] Define `VfsMetadata` struct: size, file_type, permissions (u32), owner (uid, gid), created/modified/accessed timestamps, checksum
- [ ] Define `VfsDirEntry` struct: name, file_type, metadata (or lazy metadata)
- [ ] Define `VfsCapabilities` struct/bitflags: seekable, symlinks, permissions_enforced, event_emission, checksum_algorithm, etc.
- [ ] Define `SeekFrom` (or re-export from std::io)

### Error Types (`src/shared/vfs/error.rs`)

- [ ] Define `VfsError` using `foundation_errstack` + `derive_more`: NotFound, AlreadyExists, PermissionDenied, NotAFile, NotADirectory, Unsupported, IoError, InvalidPath, ReadOnly, WhiteoutConflict
- [ ] Define `VfsResult<T>` type alias

### Sync Traits (`src/shared/vfs/traits.rs`)

- [ ] Define `VfsFile` trait: `read_at`, `write_at`, `sync`, `size`, `truncate`, `metadata`
- [ ] Define `SeekableVfsFile: VfsFile` trait: `read`, `write`, `seek`, `position`
- [ ] Define `VfsDirectory` trait: `path`, `metadata`, `list`, `get_entry`, `create_file`, `create_dir`, `remove_entry`, `rename_entry`, `open`, `open_seekable`, `open_directory`, `stat`, `exists`, `remove_all` (default), `mkdir_all` (default), `copy` (default)
- [ ] Define `VfsFileSystem` trait: `capabilities`, `stat`, `exists`, `chmod`, `symlink`, `readlink`, `rename`, `remove`, `open`, `open_seekable`, `open_directory`, `create`, `mkdir`, `read_file` (default), `write_file` (default), `copy` (default), `remove_all` (default), `mkdir_all` (default)
- [ ] Define `DeltaStore: VfsFileSystem` trait: `add_whiteout`, `is_whiteout`, `remove_whiteout`, `list_whiteouts`, `flush`, `reset`

### Async Traits — THE Primary Implementation (`src/shared/vfs/async_traits.rs`)

**Async is the real implementation. Sync wraps it.** All implementations are written as `async fn`. The async traits are the primary surface. The sync traits are thin wrappers that use valtron to execute the async implementations synchronously. No tokio — valtron handles execution.

- [ ] Define `AsyncVfsFile` trait (`async fn` methods) — primary
- [ ] Define `AsyncSeekableVfsFile` trait (`async fn` methods) — primary
- [ ] Define `AsyncVfsDirectory` trait (`async fn` methods) — primary
- [ ] Define `AsyncVfsFileSystem` trait (`async fn` methods) — primary
- [ ] Define `AsyncDeltaStore` trait (`async fn` methods) — primary

### Sync Wrappers (`src/shared/vfs/sync_wrappers.rs`)

- [ ] Implement sync VfsFile/VfsFileSystem/etc as wrappers that call async impls via valtron execution

### Module Setup (`src/shared/vfs/mod.rs`)

- [ ] Create `src/shared/vfs/mod.rs` with re-exports
- [ ] Wire into `src/shared/mod.rs` and `src/lib.rs` under `vfs` feature flag

## Verification

- `cargo check -p foundation_nativeapis --features vfs` passes
- `cargo check -p foundation_nativeapis --features vfs --target wasm32-unknown-unknown` passes
- All types derive appropriate standard traits (Debug, Clone where sensible)
