---
feature_name: "FUSE Adapter"
description: "FuseMount — exposes any VfsFileSystem as a FUSE mount point on Linux. Synthetic inode-to-path cache, FUSE operation mapping, performance negotiation."
status: "done"
priority: "medium"
phase: 4
created: 2026-06-04
updated: 2026-06-07
dependencies:
  - "01-core-traits"
tasks:
  completed: 20
  uncompleted: 0
  total: 20
  completion_percentage: 100%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 07: FUSE Adapter

## Overview

Makes any VfsFileSystem transparent to external processes by mounting it as a real directory. Uses the `fuser` crate. Maps FUSE inode-based operations to path-based VfsFileSystem calls via a synthetic inode-to-path cache (inspired by iii-filesystem's dual-key BTreeMap and AgentFS's path cache).

Linux only. Feature-gated behind `vfs-fuse`.

### Crate Dependency

Uses `fuser` crate (>= 0.14). This is the maintained successor to `rust-fuse` / `polaris-fuse`. The `fuser::Filesystem` trait is implemented synchronously (each callback blocks its thread). For async VfsFileSystem backends, calls block on the async runtime within each FUSE handler.

### FUSE errno to VfsError Mapping

Every FUSE reply requires a `libc` errno on failure. VfsError variants map as follows:

| VfsError variant      | FUSE errno            | Notes |
|-----------------------|-----------------------|-------|
| `NotFound`            | `ENOENT`              |       |
| `AlreadyExists`       | `EEXIST`              |       |
| `PermissionDenied`    | `EACCES`              |       |
| `NotAFile`            | `EISDIR`              | When caller expected a file but got a directory |
| `NotADirectory`       | `ENOTDIR`             |       |
| `Unsupported`         | `ENOSYS`              |       |
| `Io`                  | `EIO`                 | Fallback for all I/O errors |
| `InvalidPath`         | `EINVAL`              |       |
| `ReadOnly`            | `EROFS`               |       |
| `EntryPending`        | `EAGAIN`              | CoW in progress, retry later |

Implement as `fn vfs_error_to_errno(e: &VfsError) -> i32` in `fuse.rs`.

### Inode Lifecycle

FUSE operates on inodes, not paths. The adapter maintains a synthetic inode-to-path mapping:

1. **Root inode**: `ino = 1` is always `/` (FUSE convention). Allocated at mount time.
2. **Inode allocation**: Monotonic `AtomicU64` counter starting at 2. Each `lookup()` call that discovers a new path assigns the next inode. Inodes are never reused during a mount session.
3. **Lookup refcount**: Each `lookup()` call increments a refcount on the inode. The kernel calls `forget(ino, nlookup)` when it drops its reference. The adapter decrements the refcount by `nlookup`. When refcount reaches 0, the inode-to-path mapping MAY be evicted (but can be kept for performance).
4. **Bidirectional maps**: `HashMap<u64, InodeEntry>` where `InodeEntry { path: String, refcount: u64, file_type: VfsFileType }` plus `HashMap<String, u64>` for reverse lookup (path to inode).
5. **Stale inodes**: If the underlying VfsFileSystem removes a path, the inode mapping becomes stale. On next `getattr()` or `open()` for a stale inode, the adapter calls `stat()` on the VFS and returns `ENOENT` if the path no longer exists.

### File Handle Lifecycle

FUSE `open()` returns a file handle (u64) that the kernel sends back on subsequent `read`/`write`/`release` calls:

1. **`open(ino, flags)` / `create(parent, name, mode, flags)`**: Calls `VfsFileSystem::open(path, mode)` or `create(path, mode)`, stores the resulting `VfsFile` (or `SeekableVfsFile`) in a handle table (`HashMap<u64, OpenFileHandle>`). Returns the handle ID.
2. **`read(ino, fh, offset, size)`**: Looks up `fh` in the handle table, calls `read_at(buf, offset)` on the stored VfsFile.
3. **`write(ino, fh, offset, data)`**: Looks up `fh` in the handle table, calls `write_at(data, offset)` on the stored VfsFile.
4. **`release(ino, fh)`**: Removes `fh` from the handle table, dropping the VfsFile (which closes it). Returns `Ok(())`.
5. **Handle IDs**: Monotonic `AtomicU64` counter, never reused during a mount session.
6. **Directory handles**: `opendir()` / `releasedir()` use a separate handle table for `VfsDirectory` instances.

```rust
enum OpenFileHandle {
    Regular(Box<dyn VfsFile>),
    Seekable(Box<dyn SeekableVfsFile>),
}

struct OpenDirHandle {
    dir: Box<dyn VfsDirectory>,
    /// Cached entry list for readdirplus (populated on first readdir call)
    cached_entries: Option<Vec<VfsDirEntry>>,
}
```

### Attribute TTL Defaults

FUSE attribute and entry TTLs control how long the kernel caches metadata before re-querying:

- **`attr_timeout`**: Default `Duration::from_secs(1)`. How long `getattr` results are cached.
- **`entry_timeout`**: Default `Duration::from_secs(1)`. How long `lookup` results are cached.
- For read-only or immutable VfsFileSystem backends, these can be raised (e.g., 300s).
- Configurable via `FuseMountOptions { attr_timeout: Duration, entry_timeout: Duration }`.
- For mutable overlays, low TTLs (1s) ensure consistency at the cost of more FUSE round-trips.

### Dirty Unmount Behavior

If the FUSE session is terminated without `unmount()` (process killed, panic, power loss):

1. The kernel mount point becomes stale. Any access returns `ENOTCONN` or hangs.
2. Cleanup requires `fusermount -u <mountpoint>` or `umount -l <mountpoint>` (lazy unmount).
3. Open file handles in the handle table are dropped (VfsFile destructors run if the process is still alive).
4. The underlying VfsFileSystem is unaffected -- no corruption since VFS state is independent of FUSE state.
5. `FuseMount::mount()` should register a `ctrlc`/signal handler or `Drop` impl that calls `fuser::Session::unmount()` for clean shutdown on SIGTERM/SIGINT.

## Tasks

### Core (`src/native/vfs/fuse.rs`)

- [x] Define `FuseMount<F: VfsFileSystem>` struct: wraps VfsFileSystem + inode cache + handle table
- [x] Implement inode-to-path cache: `HashMap<u64, InodeEntry>` + `HashMap<String, u64>` bidirectional
- [x] Implement inode allocation: monotonic `AtomicU64` counter starting at 2 (1 = root)
- [x] Implement handle table: `HashMap<u64, OpenFileHandle>` + `HashMap<u64, OpenDirHandle>` for open file/dir handles
- [x] Implement FUSE operations via `fuser::Filesystem` trait:
  - `init()` / `destroy()` — lifecycle logging
  - `lookup()` — resolve name in parent, allocate inode, populate cache
  - `forget()` — reference count management, evict on zero
  - `getattr()` — stat via VfsFileSystem, translate to FUSE attr
  - `readdir()` — list via VfsDirectory with `.`/`..` entries and offset cursor
  - `open()` / `read()` / `write()` / `release()` — file I/O via VfsFile handles
  - `create()` — create + open in one operation
  - `unlink()` / `rmdir()` — remove via VfsFileSystem
  - `mkdir()` — create directory
  - `rename()` — rename via VfsFileSystem, update inode/path maps
  - `setattr()` — chmod/truncate
  - `readlink()` / `symlink()` — symlink operations
  - `statfs()` — filesystem statistics
  - `opendir()` / `releasedir()` — directory handle lifecycle
- [x] Implement `FuseMount::mount(fs, mountpoint)` — start FUSE session via `fuser::Session`
- [x] Implement `FuseMount::mount_background()` — spawns FUSE loop in background thread, unmounts on drop via `BackgroundSession`
- [x] Implement attribute TTL caching (configurable entry_timeout, attr_timeout via `FuseMountOptions`)

### Error Mapping (`src/native/vfs/fuse.rs`)

- [x] Implement `vfs_error_to_errno(e: &VfsError) -> i32` mapping function (all 13 VfsError variants mapped)
- [x] Handle unknown/unexpected errors as `EIO` fallback (`Backend` variant)

### Edge Cases

- [x] Handle stale inode access (path deleted from VFS after lookup): `getattr`/`open` call `stat()` which returns `ENOENT`
- [x] Handle concurrent access: `FuseMount` fields protected by `RwLock` for thread safety
- [x] Clean shutdown on drop: handled by `fuser::Session`/`BackgroundSession` Drop impls (calls `destroy()` + unmounts)
- [x] Handle `readdir` offset/cursor: FUSE sends an offset for continuation; cache directory listing in `OpenDirHandle` and resume from offset

### Tests (unit — in `fuse.rs`)

- [x] Test: vfs_error_to_errno mapping for all 12 VfsError variants
- [x] Test: flags_to_open_mode (O_RDONLY, O_WRONLY, O_RDWR)
- [x] Test: child_path construction (root + non-root parents)
- [x] Test: FuseMount::new initializes root inode at ino=1
- [x] Test: inode allocation is monotonic starting at 2
- [x] Test: lookup_or_insert creates new and increments refcount on existing
- [x] Test: file handle allocation is monotonic starting at 1
- [x] Test: metadata_to_attr for regular file (size, kind, perm, nlink=1)
- [x] Test: metadata_to_attr for directory (kind, nlink=2)
- [x] Test: default FuseMountOptions values

### Tests (integration — `tests/fuse_vfs/`)

- [x] Test: FuseMount construction with default options
- [x] Test: FuseMount construction with custom options
- [x] Test: FuseMount with pre-populated MemoryFs
- [x] Test: mount MemoryFs, read file from mountpoint via std::fs, verify directory listing
- [x] Test: write through mount, read back via std::fs
- [x] Test: unmount cleanly via BackgroundSession::join()

## Verification

- Tests pass on Linux (requires FUSE kernel module or user namespace)
- External tools (`cat`, `ls`, `stat`) work against mount point
- `cargo check -p foundation_nativeapis --features vfs-fuse` passes
