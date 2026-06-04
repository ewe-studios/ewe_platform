---
feature_name: "FUSE Adapter"
description: "FuseMount — exposes any VfsFileSystem as a FUSE mount point on Linux. Synthetic inode-to-path cache, FUSE operation mapping, performance negotiation."
status: "pending"
priority: "medium"
phase: 4
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# Feature 07: FUSE Adapter

## Overview

Makes any VfsFileSystem transparent to external processes by mounting it as a real directory. Uses the `fuser` crate. Maps FUSE inode-based operations to path-based VfsFileSystem calls via a synthetic inode-to-path cache (inspired by iii-filesystem's dual-key BTreeMap and AgentFS's path cache).

Linux only. Feature-gated behind `vfs-fuse`.

## Tasks

### Core (`src/native/vfs/fuse.rs`)

- [ ] Define `FuseMount<F: VfsFileSystem>` struct: wraps VfsFileSystem + inode cache + handle table
- [ ] Implement inode-to-path cache: `HashMap<u64, String>` + `HashMap<String, u64>` bidirectional
- [ ] Implement inode allocation: monotonic counter starting at 2 (1 = root)
- [ ] Implement handle table: `HashMap<u64, OpenFileHandle>` for open file/dir handles
- [ ] Implement FUSE operations via `fuser::Filesystem` trait:
  - `init()` — negotiate capabilities (ASYNC_READ, WRITEBACK_CACHE, PARALLEL_DIROPS, CACHE_SYMLINKS)
  - `lookup()` — resolve name in parent, allocate inode, populate cache
  - `forget()` — reference count management
  - `getattr()` — stat via VfsFileSystem, translate to FUSE attr
  - `readdir()` / `readdirplus()` — list via VfsDirectory, translate entries
  - `open()` / `read()` / `write()` / `release()` — file I/O via VfsFile handles
  - `create()` — create + open in one operation
  - `unlink()` / `rmdir()` — remove via VfsFileSystem
  - `mkdir()` — create directory
  - `rename()` — rename via VfsFileSystem
  - `setattr()` — chmod/truncate
  - `readlink()` / `symlink()` — symlink operations
  - `statfs()` — filesystem statistics
- [ ] Implement `FuseMount::mount(fs, mountpoint)` — start FUSE session
- [ ] Implement `FuseMount::unmount()` — clean shutdown
- [ ] Implement attribute TTL caching (configurable entry_timeout, attr_timeout)

### Tests

- [ ] Test: mount MemoryFs, read file from mountpoint via std::fs
- [ ] Test: write through mount, verify in underlying VfsFileSystem
- [ ] Test: unmount cleanly

## Verification

- Tests pass on Linux (requires FUSE kernel module or user namespace)
- External tools (`cat`, `ls`, `stat`) work against mount point
- `cargo check -p foundation_nativeapis --features vfs-fuse` passes
