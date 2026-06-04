---
feature_name: "Memory Implementations"
description: "MemoryFs (VfsFileSystem) and MemoryDelta (DeltaStore) — in-memory implementations for WASM, testing, and ephemeral sessions. Zero external dependencies."
status: "pending"
priority: "critical"
phase: 1
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 16
  total: 16
  completion_percentage: 0%
---

# Feature 02: Memory Implementations

## Overview

In-memory implementations of VfsFileSystem and DeltaStore. These are the reference implementations — used for WASM (no real filesystem), testing (fast, isolated), and ephemeral sessions. They validate the trait design and provide the foundation for testing Feature 03 (OverlayFileSystem overlay).

## Tasks

### MemoryFs (`src/shared/vfs/memory_fs.rs`)

- [ ] Implement `MemoryFs` struct: `HashMap<String, MemoryNode>` where `MemoryNode` is `File(Vec<u8>, VfsMetadata)` or `Directory(VfsMetadata)` or `Symlink(String, VfsMetadata)`
- [ ] Implement `MemoryFile` (VfsFile): reads/writes against `Arc<RwLock<Vec<u8>>>`
- [ ] Implement `SeekableMemoryFile` (SeekableVfsFile): wraps MemoryFile with position tracking
- [ ] Implement `MemoryDirectory` (VfsDirectory): scoped view into MemoryFs tree
- [ ] Implement `VfsFileSystem` for `MemoryFs`: all path operations, typed open methods
- [ ] Implement `capabilities()` returning full capability set (seekable, symlinks, etc.)
- [ ] Implement blake3 checksum computation on metadata retrieval
- [ ] Concurrency: `RwLock<HashMap>` for thread safety

### MemoryDelta (`src/shared/vfs/memory_delta.rs`)

- [ ] Implement `MemoryDelta` struct: wraps `MemoryFs` + `HashSet<String>` for whiteouts
- [ ] Implement `DeltaStore` for `MemoryDelta`: whiteout add/check/remove/list, flush (no-op), reset (clear all)
- [ ] Whiteout inheritance: `is_whiteout("/a/b")` checks `/a/b` AND all parent paths

### Tests (`tests/vfs_memory_test.rs`)

- [ ] Test MemoryFs: create file, read back, verify contents
- [ ] Test MemoryFs: mkdir, readdir, nested directories
- [ ] Test MemoryFs: stat returns correct metadata (size, type, permissions, checksum)
- [ ] Test MemoryFs: open_seekable, seek, read at position
- [ ] Test MemoryDelta: whiteout add, check, remove, inheritance

## Verification

- All tests pass
- `cargo check --features vfs --target wasm32-unknown-unknown` passes (no OS deps)
