---
feature_name: "Core VFS Traits + Types"
description: "Define VfsFile, SeekableVfsFile, VfsDirectory, VfsFileSystem, DeltaStore traits plus supporting types (VfsMetadata, VfsDirEntry, OpenMode, VfsCapabilities, Checksum) and error types via foundation_errstacks."
status: "done"
priority: "critical"
phase: 1
created: 2026-06-04
updated: 2026-06-05
dependencies: []
tasks:
  completed: 14
  uncompleted: 0
  total: 14
  completion_percentage: 100%
---

# Feature 01: Core VFS Traits + Types

## Overview

The foundational layer — every other feature depends on this. Defines the trait hierarchy and supporting types that all VFS implementations use. Zero external dependencies beyond `foundation_errstacks` and `derive_more` (both unconditional project deps).

## Design Decisions

### Sync-first for Phase 1, async layered later

Phase 1 defines **sync traits only** because MemoryFs, MemoryDelta, and OverlayFileSystem are purely in-memory — no I/O, no blocking. Async trait definitions can be added in a later phase when NativeFs or IPC needs them.

### Object safety considerations

Traits use associated types (`type File`, `type Directory`) rather than generics, making them suitable for both static dispatch (monomorphization) and dynamic dispatch via trait objects when needed. `VfsDirectory` methods that return directories use `Box<dyn VfsDirectory>` to enable recursive directory traversal.

### Path conventions

All paths in the VFS API are **string slices** (`&str`), not `std::path::Path`:
- Cross-platform: `Path` behavior varies by OS, VFS paths are always `/`-separated
- WASM-compatible: no OS path semantics needed
- Always absolute within the VFS namespace: `/foo/bar.txt`
- No trailing slashes, no `//`
- `.` components are silently stripped
- `..` components are **rejected with `InvalidPath` error** — traversal is never allowed
- Root is `/`

Path normalization is centralized in `path_utils::normalize_vfs_path` (`src/shared/vfs/path_utils.rs`), which returns `VfsResult<String>`. All modules use this shared function. This is the first line of defense against path traversal. NativeFs adds a second line (canonicalize + starts_with root check). DirectoryDelta adds a third for whiteout sentinels (resolved path must start with shadow root).

## Implemented Files

| File | Contents |
|------|----------|
| `src/shared/vfs/types.rs` | OpenMode, VfsFileType, Checksum, VfsEntryState, VfsMetadata, VfsDirEntry, VfsCapabilities, SeekFrom re-export |
| `src/shared/vfs/error.rs` | VfsError enum (derive_more), VfsResult<T> = Result<T, ErrorTrace<VfsError>> |
| `src/shared/vfs/traits.rs` | VfsFile, SeekableVfsFile, VfsDirectory, VfsFileSystem, DeltaStore traits with default impls |
| `src/shared/vfs/path_utils.rs` | normalize_vfs_path (rejects ..), parent_path, file_name |
| `src/shared/vfs/mod.rs` | Module declaration + re-exports |

## Tasks

- [x] Define `OpenMode` enum: `Read`, `Write`, `ReadWrite`
- [x] Define `VfsFileType` enum: `Regular`, `Directory`, `Symlink`
- [x] Define `Checksum` enum: `Blake3([u8; 32])`, `None`
- [x] Define `VfsEntryState` enum: `Ready`, `Pending`
- [x] Define `VfsMetadata` struct with constructors `new_file`, `new_directory`, `new_symlink`
- [x] Define `VfsDirEntry` struct: name (String), file_type
- [x] Define `VfsCapabilities` struct with bool fields
- [x] Define `VfsError` enum with all variants using `derive_more`
- [x] Define `VfsResult<T>` as `Result<T, ErrorTrace<VfsError>>`
- [x] Define `VfsFile: Send + Sync` trait
- [x] Define `SeekableVfsFile: VfsFile` trait
- [x] Define `VfsDirectory: Send + Sync` trait with associated types
- [x] Define `VfsFileSystem: Send + Sync` trait with associated types and default impls
- [x] Define `DeltaStore: VfsFileSystem` trait with whiteout methods and lifecycle

## Verification

- `cargo check -p foundation_nativeapis --features vfs` passes with zero VFS warnings
- All types derive appropriate standard traits (Debug, Clone where sensible)

---

_Created: 2026-06-04 | Completed: 2026-06-05_
