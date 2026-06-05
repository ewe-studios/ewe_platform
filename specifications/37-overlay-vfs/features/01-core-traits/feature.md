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
  completed: 22
  uncompleted: 0
  total: 22
  completion_percentage: 100%
---

# Feature 01: Core VFS Traits + Types

## Overview

The foundational layer — every other feature depends on this. Define the trait hierarchy and supporting types that all VFS implementations will use. Zero external dependencies beyond `foundation_errstacks` and `derive_more`.

This feature produces **no implementations** — only trait definitions, type definitions, and error types. Feature 02 (MemoryFs/MemoryDelta) provides the first concrete implementations.

## Design Decisions

### Sync-first for Phase 1, async layered later

The spec says "async-first, sync wraps." For Phase 1, we define **sync traits only** because:
- MemoryFs and MemoryDelta are purely in-memory — no I/O, no blocking
- OverlayFileSystem composition logic is synchronous
- Async trait definitions are laid out in `async_traits.rs` as a skeleton for Phase 2+
- NativeFs (Phase 2) is where async becomes meaningful

The sync traits are the testable surface for Phase 1. Async wrapping comes when we have implementations that actually benefit from it.

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

## Trait Hierarchy

```
VfsFile (offset-based byte I/O on an open file handle)
    └── SeekableVfsFile: VfsFile (adds cursor-position state)

VfsDirectory (directory listing, child creation, path resolution within subtree)

VfsFileSystem (top-level namespace: open/create/stat/remove by absolute path)
    └── DeltaStore: VfsFileSystem (adds whiteout tracking + lifecycle)
```

### Relationship diagram

```
                    VfsFileSystem
                   ┌─────────────────────────────────────┐
                   │ capabilities() → VfsCapabilities     │
                   │ stat(path) → VfsMetadata             │
                   │ exists(path) → bool                  │
                   │ open(path, mode) → Self::File        │
                   │ open_seekable(path, mode) → ...      │
                   │ open_directory(path) → Self::Dir     │
                   │ create(path, mode) → Self::File      │
                   │ mkdir(path)                          │
                   │ remove(path)                         │
                   │ rename(from, to)                     │
                   │ chmod(path, mode)                    │
                   │ symlink(target, link)                │
                   │ readlink(path) → String              │
                   │ read_file(path) → Vec<u8>  [default] │
                   │ write_file(path, data)     [default] │
                   │ copy(from, to)             [default] │
                   │ remove_all(path)           [default] │
                   │ mkdir_all(path)            [default] │
                   └──────────┬──────────────────────────┘
                              │ extends
                              ▼
                    DeltaStore: VfsFileSystem
                   ┌─────────────────────────────────────┐
                   │ add_whiteout(path, version)          │
                   │ is_whiteout(path) → Option<u64>      │
                   │ remove_whiteout(path)                │
                   │ list_whiteouts(dir) → Vec<(S, u64)>  │
                   │ flush()                              │
                   │ reset()                              │
                   └─────────────────────────────────────┘

    VfsFile                        SeekableVfsFile: VfsFile
   ┌─────────────────────┐       ┌─────────────────────────┐
   │ read_at(buf, off)   │       │ read(buf) → usize       │
   │ write_at(buf, off)  │       │ write(buf) → usize      │
   │ sync()              │       │ seek(pos) → u64         │
   │ size() → u64        │       │ position() → u64        │
   │ truncate(size)      │       └─────────────────────────┘
   │ metadata() → Meta   │
   └─────────────────────┘

    VfsDirectory
   ┌──────────────────────────────────────────────────────┐
   │ path() → &str                                        │
   │ metadata() → VfsMetadata                              │
   │ list() → Vec<VfsDirEntry>                             │
   │ get_entry(name) → Option<VfsDirEntry>                 │
   │ create_file(name, mode) → File                        │
   │ create_dir(name) → Box<dyn VfsDirectory>              │
   │ remove_entry(name)                                    │
   │ rename_entry(old, new)                                │
   │ open(path, mode) → File        (path resolution)     │
   │ open_seekable(path, mode)      (path resolution)     │
   │ open_directory(path) → Box<dyn VfsDirectory>          │
   │ stat(path) → VfsMetadata       (path resolution)     │
   │ exists(path) → bool            (path resolution)     │
   │ remove_all(name)               [default impl]        │
   │ mkdir_all(path)                [default impl]        │
   │ copy(from, to)                 [default impl]        │
   └──────────────────────────────────────────────────────┘
```

## Type Definitions

### Core types (`src/shared/vfs/types.rs`)

```rust
// --- OpenMode ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
}

// --- VfsFileType ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsFileType {
    Regular,
    Directory,
    Symlink,
}

// --- Checksum ---
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checksum {
    Blake3([u8; 32]),
    None,
}

// --- VfsEntryState ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsEntryState {
    Ready,
    Pending,
}

// --- VfsMetadata ---
#[derive(Debug, Clone)]
pub struct VfsMetadata {
    pub size: u64,
    pub file_type: VfsFileType,
    pub permissions: u32,
    pub owner: (u32, u32),        // (uid, gid)
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub checksum: Checksum,
    pub version: u64,
    pub state: VfsEntryState,
}

// --- VfsDirEntry ---
#[derive(Debug, Clone)]
pub struct VfsDirEntry {
    pub name: String,
    pub file_type: VfsFileType,
}

// --- VfsCapabilities ---
#[derive(Debug, Clone, Copy, Default)]
pub struct VfsCapabilities {
    pub seekable: bool,
    pub symlinks: bool,
    pub permissions_enforced: bool,
    pub event_emission: bool,
    pub persistent: bool,
}
```

### Simplifications from the requirements.md

- `Checksum`: Only `Blake3` and `None` for Phase 1. Md5/Crc32 deferred — no use case yet. Adding variants later is non-breaking.
- `VfsCapabilities`: Plain struct with bool fields, not bitflags. Simpler, equally readable, no extra dep.
- `VfsDirEntry`: No lazy metadata — just name + type. Stat on demand via `stat(path)`. Keeps the dir listing fast and avoids lifetime complexity.
- `VfsMetadata.owner`: Kept as `(u32, u32)` tuple. On WASM, stored as `(0, 0)`.
- `SystemTime`: Re-exported from `std::time`. On WASM, timestamps are `None`.

### Error types (`src/shared/vfs/error.rs`)

```rust
use derive_more::{Display, Error, From};
use foundation_errstacks::ErrorTrace;

#[derive(Debug, Display, Error)]
pub enum VfsError {
    #[display("not found: {path}")]
    NotFound { path: String },

    #[display("already exists: {path}")]
    AlreadyExists { path: String },

    #[display("permission denied: {path}")]
    PermissionDenied { path: String },

    #[display("not a file: {path}")]
    NotAFile { path: String },

    #[display("not a directory: {path}")]
    NotADirectory { path: String },

    #[display("operation not supported: {operation}")]
    Unsupported { operation: String },

    #[display("I/O error: {source}")]
    Io { source: std::io::Error },

    #[display("invalid path: {path}")]
    InvalidPath { path: String },

    #[display("filesystem is read-only")]
    ReadOnly,

    #[display("entry is pending (CoW in progress): {path}")]
    EntryPending { path: String },
}

pub type VfsResult<T> = Result<T, ErrorTrace<VfsError>>;
```

Key decisions:
- **`VfsResult<T>`** wraps `ErrorTrace<VfsError>` — callers get full stack traces with context
- **Named fields** over tuple variants — `NotFound { path }` not `NotFound(String)` — clear at construction site
- **No `WhiteoutConflict`** — whiteout resolution is internal to OverlayFileSystem, never exposed to callers. Callers see `NotFound` when a whiteout hides a path.
- **`EntryPending`** replaces the spec's "stat during CoW" behavior — callers who attempt operations on in-flight entries get a clear error

## Tasks

### Types (`src/shared/vfs/types.rs`)

- [x] Define `OpenMode` enum: `Read`, `Write`, `ReadWrite` — derive Debug, Clone, Copy, PartialEq, Eq
- [x] Define `VfsFileType` enum: `Regular`, `Directory`, `Symlink` — derive Debug, Clone, Copy, PartialEq, Eq
- [x] Define `Checksum` enum: `Blake3([u8; 32])`, `None` — derive Debug, Clone, PartialEq, Eq
- [x] Define `VfsEntryState` enum: `Ready`, `Pending` — derive Debug, Clone, Copy, PartialEq, Eq
- [x] Define `VfsMetadata` struct with all fields — derive Debug, Clone
- [x] Implement `VfsMetadata::new_file(size, permissions)` and `VfsMetadata::new_directory(permissions)` constructors
- [x] Define `VfsDirEntry` struct: name (String), file_type — derive Debug, Clone
- [x] Define `VfsCapabilities` struct with bool fields — derive Debug, Clone, Copy, Default
- [x] Re-export `std::io::SeekFrom` in the module

### Error Types (`src/shared/vfs/error.rs`)

- [x] Define `VfsError` enum with all variants using `derive_more::{Display, Error}`
- [x] Define `VfsResult<T>` as `Result<T, ErrorTrace<VfsError>>`
- [x] Implement `From<std::io::Error>` for convenience conversions

### Sync Traits (`src/shared/vfs/traits.rs`)

- [x] Define `VfsFile: Send + Sync` trait with methods: `read_at`, `write_at`, `sync_data`, `size`, `truncate`, `metadata`
- [x] Define `SeekableVfsFile: VfsFile` trait with methods: `read`, `write`, `seek`, `position`
- [x] Define `VfsDirectory: Send + Sync` trait with associated types `type File: VfsFile` and path-resolution methods
- [x] Define `VfsFileSystem: Send + Sync` trait with associated types, all path operations, and default impls for convenience methods
- [x] Define `DeltaStore: VfsFileSystem` trait with whiteout methods and lifecycle
- [x] Provide default implementations for `read_file`, `write_file`, `copy`, `remove_all`, `mkdir_all` on VfsFileSystem

### Module Setup (`src/shared/vfs/mod.rs`)

- [x] Create `src/shared/vfs/mod.rs` with re-exports of all public types and traits
- [x] Wire into `src/shared/mod.rs` under `#[cfg(feature = "vfs")]`
- [x] Add `vfs` feature flag to `Cargo.toml` with dependencies: `foundation_errstacks`, `derive_more`
- [x] Ensure `vfs` feature is NOT in `default` — opt-in only

## File Layout

```
src/shared/vfs/
    mod.rs          — pub mod + re-exports
    types.rs        — OpenMode, VfsFileType, Checksum, VfsMetadata, VfsDirEntry, VfsCapabilities, VfsEntryState
    error.rs        — VfsError, VfsResult
    traits.rs       — VfsFile, SeekableVfsFile, VfsDirectory, VfsFileSystem, DeltaStore
```

## Verification

- `cargo check -p foundation_nativeapis --features vfs` passes with zero warnings
- All types derive appropriate standard traits (Debug, Clone where sensible)
- No external dependencies beyond workspace crates (`foundation_errstacks`, `derive_more`, `tracing`)
- `VfsResult<T>` correctly wraps `ErrorTrace<VfsError>`

---

_Created: 2026-06-04 | Updated: 2026-06-05_
