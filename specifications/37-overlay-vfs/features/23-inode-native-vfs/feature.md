---
feature_name: "Inode-Native VFS"
description: "Push inode awareness into the VFS layer itself — VfsMetadata and VfsDirEntry carry inode numbers, VfsFileSystem implementations own inode allocation and provide inode-to-path reverse lookup. Eliminates the synthetic inode cache in FuseMount and aligns the VFS abstraction with how filesystems actually work."
status: "pending"
priority: "high"
phase: 4
created: 2026-06-07
updated: 2026-06-07
dependencies:
  - "01-core-traits"
  - "02-memory-impls"
  - "07-fuse-adapter"
tasks:
  completed: 0
  uncompleted: 37
  total: 37
  completion_percentage: 0%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 23: Inode-Native VFS

## Motivation

FUSE, NFS, and any kernel-facing adapter speak inodes — every operation arrives as `(parent_ino, name)` or just `ino`, never as a path string. Today `FuseMount` maintains its own synthetic `HashMap<u64, InodeEntry>` + `HashMap<String, u64>` cache to translate between the two worlds. This is wrong for three reasons:

1. **Duplicated state.** The underlying VFS already tracks its entries. A second inode-to-path map is redundant bookkeeping that can go stale.
2. **Lost information.** `NativeFs` wraps real files that have real OS inodes (`MetadataExt::ino()`). The FUSE adapter throws these away and mints synthetic ones, losing consistency with `stat(2)` on the underlying filesystem.
3. **Wrong abstraction level.** Inodes are a filesystem concept, not a FUSE concept. Every filesystem — memory, disk, SQLite, object-store — can assign a stable identifier to each entry. The VFS trait should express this.

`LibsqlDelta` and `TursoDelta` already use inodes internally (the `ino` column in `vfs_dentry`/`turso_dentry`) but don't surface them through the trait. This feature makes inodes a first-class part of the VFS contract.

## Design

### Core Principle

**Each VfsFileSystem owns its inode space.** The filesystem assigns inode numbers, stores the inode-to-path mapping, and exposes reverse lookup. Callers (FUSE, NFS, IPC) receive inodes from `stat()` / `readdir()` and can use them for subsequent operations without maintaining their own cache.

### Inode Rules

1. **Inode 0 is reserved** — means "no inode" / unset. Valid inodes start at 1.
2. **Inode 1 is root** — the root directory `/` always has inode 1. This matches FUSE convention.
3. **Inodes are stable within a mount session** — once assigned, an inode number does not change for the lifetime of that filesystem instance.
4. **Inodes are never reused** — monotonic counters, not recycled. Prevents stale-reference bugs.
5. **Inodes are `u64`** — matches FUSE, NFS, and POSIX `ino_t` on 64-bit systems.

## Tasks

### Types (`shared/vfs/types.rs`)

- [ ] Add `inode: u64` field to `VfsMetadata`
- [ ] Add `inode: u64` field to `VfsDirEntry`
- [ ] Update `VfsMetadata::new_file(size, permissions)` → `VfsMetadata::new_file(inode, size, permissions)`
- [ ] Update `VfsMetadata::new_directory(permissions)` → `VfsMetadata::new_directory(inode, permissions)`
- [ ] Update `VfsMetadata::new_symlink()` → `VfsMetadata::new_symlink(inode)`

### Traits (`shared/vfs/traits.rs`)

- [ ] Add `fn inode(&self, path: &str) -> VfsResult<u64>` to `VfsFileSystem` — returns inode for a path
- [ ] Add `fn path_by_inode(&self, ino: u64) -> VfsResult<String>` to `VfsFileSystem` — reverse lookup
- [ ] Add `fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata>` to `VfsFileSystem` — stat by inode

### MemoryFs (`shared/vfs/memory_fs.rs`)

- [ ] Add `next_inode: u64` counter to `MemoryFsInner`
- [ ] Add `ino_to_path: HashMap<u64, String>` reverse index to `MemoryFsInner`
- [ ] Store `inode` in each `MemoryNode`'s `VfsMetadata` at creation time
- [ ] Assign inode 1 to root `/` on `MemoryFs::new()`
- [ ] Assign next inode on `create()`, `mkdir()`, `symlink()`
- [ ] Update `ino_to_path` on `rename()` — remap inode to new path
- [ ] Remove from `ino_to_path` on `remove()`
- [ ] Implement `inode()` — lookup path in nodes, return `metadata.inode`
- [ ] Implement `path_by_inode()` — lookup in `ino_to_path`
- [ ] Implement `stat_by_inode()` — `path_by_inode` then `stat`
- [ ] Update `list_children()` to populate `VfsDirEntry.inode`

### NativeFs (`native/vfs/native_fs.rs`)

- [ ] Implement `inode()` — call `std::fs::metadata(path)` and return `MetadataExt::ino()`
- [ ] Implement `path_by_inode()` — return `Unsupported` (OS doesn't provide efficient reverse lookup without scanning)
- [ ] Implement `stat_by_inode()` — return `Unsupported` (same reason)
- [ ] Populate `VfsMetadata.inode` from `MetadataExt::ino()` in stat/metadata calls
- [ ] Populate `VfsDirEntry.inode` from dir entry metadata in `list()` / `get_entry()`

### OverlayFs (`shared/vfs/overlay_fs.rs`)

- [ ] Implement `inode()` — delegate to upper layer first, then lower
- [ ] Implement `path_by_inode()` — delegate to upper, then lower
- [ ] Implement `stat_by_inode()` — delegate to upper, then lower

### ObservableFs (`shared/vfs/observable_fs.rs`)

- [ ] Implement `inode()` — delegate to inner
- [ ] Implement `path_by_inode()` — delegate to inner
- [ ] Implement `stat_by_inode()` — delegate to inner

### DeltaStore impls (`memory_delta.rs`, `libsql_delta/`, `turso_delta/`)

- [ ] MemoryDelta: implement `inode()`, `path_by_inode()`, `stat_by_inode()` — delegate to inner MemoryFs
- [ ] LibsqlDelta: implement using existing `ino` column from `vfs_dentry` table
- [ ] TursoDelta: implement using existing `ino` column from `turso_dentry` table

### Arrow Serialization (`shared/vfs/arrow/mod.rs`)

- [ ] Add `inode` field (UInt64) to `VfsMetadata` Arrow schema and ToArrow/FromArrow impls
- [ ] Add `inode` field (UInt64) to `VfsDirEntry` Arrow schema and ToArrow/FromArrow impls

### FuseMount refactor (`native/vfs/fuse.rs`)

- [ ] Remove synthetic `inodes: HashMap<u64, InodeEntry>` cache
- [ ] Remove synthetic `path_to_ino: HashMap<String, u64>` cache
- [ ] Remove `next_ino: AtomicU64` counter
- [ ] Remove `InodeEntry` struct, `lookup_or_insert()`, `get_path()`, and `alloc_ino()`
- [ ] Rewrite `lookup()` — call `fs.stat(child_path)`, read `meta.inode`, return directly
- [ ] Rewrite `forget()` — no-op (filesystem owns inode lifecycle)
- [ ] Rewrite `getattr()` — call `fs.stat_by_inode(ino)` or `fs.path_by_inode(ino)` then `fs.stat(path)`
- [ ] Rewrite `readdir()` — read `VfsDirEntry.inode` from directory listing, no allocation needed
- [ ] Update all other FUSE ops to use `fs.path_by_inode(ino)` instead of `get_path(ino)`

### Tests

- [ ] Test: MemoryFs assigns inode 1 to root
- [ ] Test: MemoryFs assigns monotonically increasing inodes to new entries
- [ ] Test: MemoryFs `inode()` returns correct inode for a path
- [ ] Test: MemoryFs `path_by_inode()` returns correct path
- [ ] Test: MemoryFs `stat_by_inode()` returns correct metadata
- [ ] Test: MemoryFs `rename()` updates inode-to-path mapping (same inode, new path)
- [ ] Test: MemoryFs `remove()` invalidates inode (path_by_inode returns NotFound)
- [ ] Test: NativeFs `inode()` returns real OS inode (matches `std::fs::metadata().ino()`)
- [ ] Test: NativeFs populates `VfsMetadata.inode` and `VfsDirEntry.inode` from OS
- [ ] Test: FuseMount with inode-native MemoryFs — mount, read, write still work
- [ ] Test: VfsDirEntry Arrow roundtrip preserves inode field

## Migration Impact

This is a breaking change to `VfsMetadata`, `VfsDirEntry`, `VfsFileSystem`, and all their constructors.

**Files affected:**
- `shared/vfs/types.rs` — struct fields
- `shared/vfs/traits.rs` — 3 new trait methods
- `shared/vfs/memory_fs.rs` — inode allocation, reverse index
- `shared/vfs/memory_delta.rs` — delegate new methods
- `shared/vfs/overlay_fs.rs` — delegate new methods
- `shared/vfs/observable_fs.rs` — delegate new methods
- `shared/vfs/libsql_delta/` — surface existing `ino` column
- `shared/vfs/turso_delta/` — surface existing `ino` column
- `shared/vfs/arrow/mod.rs` — add inode to Arrow schemas
- `native/vfs/native_fs.rs` — use real OS inodes
- `native/vfs/fuse.rs` — remove synthetic cache, delegate to VFS
- All test files constructing VfsMetadata or VfsDirEntry

## Verification

- `cargo check -p foundation_nativeapis --features vfs` passes
- `cargo check -p foundation_nativeapis --features vfs-fuse` passes
- `cargo test -p foundation_nativeapis --features vfs-fuse` — all existing + new tests pass
- `cargo test -p foundation_nativeapis --features vfs` — all memory/overlay/observable tests pass
- FuseMount no longer contains any HashMap inode cache
