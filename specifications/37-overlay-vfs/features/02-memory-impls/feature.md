---
feature_name: "Memory Implementations"
description: "MemoryFs (VfsFileSystem) and MemoryDelta (DeltaStore) — in-memory implementations for WASM, testing, and ephemeral sessions. Zero external dependencies beyond core traits."
status: "done"
priority: "critical"
phase: 1
created: 2026-06-04
updated: 2026-06-05
dependencies:
  - "01-core-traits"
tasks:
  completed: 16
  uncompleted: 0
  total: 16
  completion_percentage: 100%
---

# Feature 02: Memory Implementations

## Overview

In-memory implementations of VfsFileSystem and DeltaStore. These are the **reference implementations** — used for WASM (no real filesystem), testing (fast, isolated), and ephemeral sessions. They validate the trait design and provide the foundation for testing Feature 03 (OverlayFileSystem overlay).

Both `MemoryFs` and `MemoryDelta` are pure in-memory, zero-dep (beyond the core traits), and work on all platforms including `wasm32-unknown-unknown`.

## Architecture

### Internal data model

```
MemoryFs
┌────────────────────────────────────────────────────────────────┐
│  inner: Arc<RwLock<MemoryFsInner>>                             │
│                                                                │
│  MemoryFsInner                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  nodes: HashMap<String, MemoryNode>                      │  │
│  │         key = normalized absolute path (e.g. "/src/main.rs")│
│  │                                                          │  │
│  │  version: AtomicU64  (global monotonic counter)          │  │
│  │                                                          │  │
│  │  MemoryNode variants:                                    │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │ File {                                              │ │  │
│  │  │   content: Arc<RwLock<Vec<u8>>>,                    │ │  │
│  │  │   metadata: VfsMetadata,                            │ │  │
│  │  │ }                                                   │ │  │
│  │  │                                                     │ │  │
│  │  │ Directory {                                         │ │  │
│  │  │   metadata: VfsMetadata,                            │ │  │
│  │  │ }                                                   │ │  │
│  │  │                                                     │ │  │
│  │  │ Symlink {                                           │ │  │
│  │  │   target: String,                                   │ │  │
│  │  │   metadata: VfsMetadata,                            │ │  │
│  │  │ }                                                   │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

### Path storage model

All paths stored as **flat keys** in a single HashMap:
```
"/"                → Directory
"/src"             → Directory
"/src/main.rs"     → File { content: [...], ... }
"/src/lib.rs"      → File { content: [...], ... }
"/readme.md"       → Symlink { target: "/README.md" }
```

Directory listing is computed by scanning keys with the prefix `{dir_path}/` and extracting immediate children (no second `/` in the remainder). This is O(n) where n is total entries, acceptable for an in-memory reference implementation.

### Concurrency model

`MemoryFs` wraps `MemoryFsInner` in `Arc<RwLock<_>>`:
- All read operations (stat, exists, list, open for read) take a **read lock**
- All write operations (create, mkdir, remove, rename, write) take a **write lock**
- File handles hold `Arc<RwLock<Vec<u8>>>` references to content — reads/writes on open file handles don't need the global MemoryFs lock after the handle is obtained
- This means: open the file (briefly locks MemoryFs), then read/write the content (locks only the individual file content)

### Path normalization

All incoming paths are normalized before lookup:
1. Ensure leading `/`
2. Collapse `//` → `/`
3. Remove trailing `/` (except for root `/`)
4. No `.` or `..` resolution (callers normalize — VFS doesn't do path traversal)

Implemented as a private `fn normalize_path(path: &str) -> String` in the module.

### Version counter

`MemoryFsInner` owns a `version: u64` counter. Every mutation (create, write, delete, rename) increments it and stamps the affected entry's `metadata.version`. This provides ordering for overlay semantics.

## MemoryFs Implementation

### VfsFileSystem trait implementation

| Method | Behavior |
|--------|----------|
| `capabilities()` | Returns `VfsCapabilities { seekable: true, symlinks: true, permissions_enforced: false, event_emission: false, persistent: false }` |
| `stat(path)` | Lookup node, return metadata clone. Follow symlinks. |
| `exists(path)` | Lookup node, return true/false. Follow symlinks. |
| `open(path, Read)` | Lookup file node, return `MemoryFile` with shared content ref |
| `open(path, Write\|ReadWrite)` | Lookup file node (must exist), return `MemoryFile` with shared content ref |
| `open_seekable(path, mode)` | Same as open, return `SeekableMemoryFile` wrapping `MemoryFile` with position=0 |
| `open_directory(path)` | Lookup dir node, return `MemoryDirectory` scoped to that path |
| `create(path, mode)` | Create new file node (parent dir must exist), return `MemoryFile` |
| `mkdir(path)` | Create new directory node (parent must exist, path must not exist) |
| `remove(path)` | Remove node (must exist, directories must be empty) |
| `rename(from, to)` | Move node — update key in HashMap. For directories, update all children keys too. |
| `chmod(path, mode)` | Update permissions in metadata |
| `symlink(target, link)` | Create symlink node at `link` pointing to `target` |
| `readlink(path)` | Return symlink target (error if not a symlink) |
| `read_file(path)` | Override default: direct HashMap lookup + content clone, avoids open/read/close overhead |
| `write_file(path, data)` | Override default: create-or-update in one lock acquisition |

### Symlink resolution

MemoryFs follows symlinks transparently for `stat`, `open`, `exists`, etc.:
1. Lookup path in nodes
2. If `Symlink { target }`, resolve target recursively (max 40 hops, then error)
3. Return resolved node

`readlink()` does NOT follow — it returns the symlink's target string.

### MemoryFile

```rust
pub struct MemoryFile {
    content: Arc<RwLock<Vec<u8>>>,
    mode: OpenMode,
}
```

- `read_at(buf, offset)`: Read lock on content, copy bytes from offset
- `write_at(buf, offset)`: Write lock on content, extend if needed, copy bytes at offset. Error if `mode == Read`.
- `sync()`: No-op (in-memory)
- `size()`: Read lock, return content.len()
- `truncate(size)`: Write lock, resize content
- `metadata()`: Returns metadata snapshot from the MemoryFs inner (requires brief MemoryFs read lock)

### SeekableMemoryFile

```rust
pub struct SeekableMemoryFile {
    inner: MemoryFile,
    position: u64,
}
```

- `read(buf)`: Call `inner.read_at(buf, self.position)`, advance position
- `write(buf)`: Call `inner.write_at(buf, self.position)`, advance position
- `seek(pos)`: Compute new position from SeekFrom, clamp to content length
- `position()`: Return current position

### MemoryDirectory

```rust
pub struct MemoryDirectory {
    fs: Arc<RwLock<MemoryFsInner>>,
    path: String,
}
```

Scoped view into the MemoryFs tree. All path-resolution methods resolve relative to `self.path`.

## MemoryDelta Implementation

```
MemoryDelta
┌──────────────────────────────────────────────────────┐
│  fs: MemoryFs                (wraps an inner MemoryFs)│
│  whiteouts: RwLock<HashMap<String, u64>>              │
│             key = path, value = whiteout version      │
└──────────────────────────────────────────────────────┘
```

### DeltaStore trait implementation

| Method | Behavior |
|--------|----------|
| `add_whiteout(path, version)` | Insert into whiteouts HashMap |
| `is_whiteout(path)` | Check exact path AND all ancestor paths. Return highest matching version. |
| `remove_whiteout(path)` | Remove from whiteouts HashMap |
| `list_whiteouts(dir)` | Return all whiteouts with prefix `{dir}/` |
| `flush()` | No-op (in-memory, nothing to persist) |
| `reset()` | Clear all nodes from inner MemoryFs + clear all whiteouts |

### Whiteout inheritance

`is_whiteout("/a/b/c.txt")` checks:
1. Exact match: `"/a/b/c.txt"` in whiteouts?
2. Parent: `"/a/b"` in whiteouts?
3. Grandparent: `"/a"` in whiteouts?
4. Root: `"/"` in whiteouts?

Returns `Some(version)` of the **highest version** whiteout found across all ancestors, or `None` if no whiteout matches.

### VfsFileSystem delegation

MemoryDelta delegates all VfsFileSystem methods to its inner MemoryFs. It does NOT check whiteouts in its VfsFileSystem methods — whiteout checking is the overlay's responsibility, not the delta store's.

## Tasks

### MemoryFs (`src/shared/vfs/memory_fs.rs`)

- [x] Define `MemoryNode` enum: `File`, `Directory`, `Symlink` variants with content/metadata
- [x] Define `MemoryFsInner` struct: `HashMap<String, MemoryNode>` + version counter
- [x] Define `MemoryFs` struct: `Arc<RwLock<MemoryFsInner>>`
- [x] Implement `MemoryFs::new()` constructor (creates root `/` directory)
- [x] Implement `fn normalize_path(path: &str) -> String`
- [x] Implement `MemoryFile` struct + VfsFile trait
- [x] Implement `SeekableMemoryFile` struct + SeekableVfsFile trait
- [x] Implement `MemoryDirectory` struct + VfsDirectory trait
- [x] Implement `VfsFileSystem` for `MemoryFs`
- [x] Implement symlink resolution with cycle detection (max 40 hops)

### MemoryDelta (`src/shared/vfs/memory_delta.rs`)

- [x] Define `MemoryDelta` struct: inner MemoryFs + whiteouts HashMap
- [x] Implement `DeltaStore` for `MemoryDelta`
- [x] Implement whiteout inheritance (ancestor path checking)
- [x] Delegate VfsFileSystem methods to inner MemoryFs

### Tests (`tests/vfs_memory_tests.rs`)

All tests use `#[traced_test]` and follow the three-validation pattern (valid, invalid, edge case).

**MemoryFs — File Operations:**
- [x] `test_create_and_read_file` — create file, write data, read back, verify contents match
- [x] `test_create_file_in_nonexistent_dir` — create file when parent dir doesn't exist → NotFound
- [x] `test_create_file_already_exists` — create file at existing path → AlreadyExists
- [x] `test_open_nonexistent_file` — open path that doesn't exist → NotFound
- [x] `test_open_directory_as_file` — open a directory path with open() → NotAFile
- [x] `test_write_to_read_only_file` — open with Read mode, attempt write → ReadOnly
- [x] `test_file_read_at_offset` — write known data, read_at various offsets, verify
- [x] `test_file_write_at_offset` — write at offset beyond current size, verify gap is zero-filled
- [x] `test_file_truncate` — truncate to smaller size, verify size, read past old end fails
- [x] `test_file_size` — create file, write data, verify size matches written bytes

**MemoryFs — Seekable File Operations:**
- [x] `test_seekable_sequential_read` — write data, read sequentially, position advances
- [x] `test_seekable_seek_and_read` — seek to middle, read, verify correct bytes
- [x] `test_seekable_seek_from_end` — SeekFrom::End, verify position
- [x] `test_seekable_seek_from_current` — SeekFrom::Current positive and negative offsets

**MemoryFs — Directory Operations:**
- [x] `test_mkdir_and_list` — create dirs, list root, verify entries
- [x] `test_mkdir_nested` — mkdir requires parent to exist
- [x] `test_remove_empty_dir` — remove empty directory succeeds
- [x] `test_remove_nonempty_dir` — remove non-empty directory fails
- [x] `test_readdir_mixed` — directory with files, subdirs, symlinks — list returns all with correct types

**MemoryFs — Metadata:**
- [x] `test_stat_returns_correct_metadata` — create file with permissions, stat, verify all fields
- [x] `test_metadata_version_increments` — mutations increment version monotonically
- [x] `test_chmod_updates_permissions` — chmod, verify stat reflects new mode

**MemoryFs — Symlinks:**
- [x] `test_symlink_create_and_readlink` — create symlink, readlink returns target
- [x] `test_symlink_transparent_open` — open symlink opens target file
- [x] `test_symlink_chain` — symlink → symlink → file, transparent resolution
- [x] `test_symlink_cycle_detection` — A → B → A, error after max hops

**MemoryFs — Convenience Methods:**
- [x] `test_read_file_convenience` — read_file returns full contents
- [x] `test_write_file_convenience` — write_file creates or updates file
- [x] `test_mkdir_all` — mkdir_all creates intermediate directories
- [x] `test_remove_all` — remove_all recursively deletes directory tree
- [x] `test_copy` — copy duplicates file content at new path

**MemoryFs — Rename:**
- [x] `test_rename_file` — rename file, old path gone, new path has same content
- [x] `test_rename_directory` — rename dir, all children accessible under new prefix
- [x] `test_rename_to_existing` — rename to path that exists → AlreadyExists

**MemoryDelta — Whiteout Operations:**
- [x] `test_whiteout_add_and_check` — add whiteout, is_whiteout returns Some(version)
- [x] `test_whiteout_remove` — add then remove whiteout, is_whiteout returns None
- [x] `test_whiteout_inheritance` — whiteout on `/a/b`, is_whiteout on `/a/b/c.txt` returns Some
- [x] `test_whiteout_no_false_inheritance` — whiteout on `/a/bc`, is_whiteout on `/a/b` returns None (prefix matching must be path-aware)
- [x] `test_whiteout_list` — list_whiteouts returns all whiteouts under a directory
- [x] `test_whiteout_reset_clears_all` — reset clears files AND whiteouts

**MemoryDelta — VfsFileSystem Delegation:**
- [x] `test_delta_create_and_read` — create file in delta, read back
- [x] `test_delta_does_not_check_whiteouts` — delta VfsFileSystem methods ignore whiteouts (overlay's job)

## Verification

- All tests pass: `cargo test -p foundation_nativeapis --features vfs -- vfs_memory`
- `cargo check -p foundation_nativeapis --features vfs` — zero warnings
- No OS-specific deps — pure in-memory, works on all targets

---

_Created: 2026-06-04 | Updated: 2026-06-05_
