---
feature_name: "NFS Adapter"
description: "NfsMount — NFS v3 loopback server exposing VfsFileSystem as a mount on macOS. No kernel extension required."
status: "pending"
priority: "medium"
phase: 4
created: 2026-06-04
updated: 2026-06-07
dependencies:
  - "01-core-traits"
  - "23-inode-native-vfs"
tasks:
  completed: 0
  uncompleted: 17
  total: 17
  completion_percentage: 0%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 08: NFS Adapter

## Overview

macOS alternative to FUSE. Runs an NFS v3 server on loopback, mounted via `mount_nfs`. No kernel extension (kext) needed — works with Apple's security model. Inspired by AgentFS's macOS mounting strategy.

Feature-gated behind `vfs-nfs`.

### NFS Library / Protocol Implementation

Two options evaluated:

1. **Raw NFS v3 protocol implementation** -- Implement the NFS v3 RPC/XDR protocol directly over TCP on loopback. This avoids external crate dependencies but requires significant protocol work (ONC RPC, XDR encoding, MOUNT protocol for initial mount, NFS program for operations).
2. **`nfs3` crate** -- If a suitable Rust NFS v3 server crate exists and is maintained, prefer it. As of writing, the Rust NFS crate ecosystem is thin; AgentFS implemented raw protocol handling.

**Decision**: Start with a minimal raw implementation. NFS v3 is a well-documented protocol (RFC 1813). Only implement the subset of operations needed for VfsFileSystem mapping. The server runs on localhost only -- no authentication, no export restrictions, no network security concerns.

### NFS v3 Operation to VfsFileSystem Mapping

| NFS v3 Procedure  | VfsFileSystem Method          | Notes |
|-------------------|-------------------------------|-------|
| `NULL`            | (no-op)                       | Health check, returns void |
| `GETATTR`         | `stat(path)`                  | Translate VfsMetadata to NFS fattr3 |
| `SETATTR`         | `chmod(path, mode)`           | Only permissions supported; size via truncate |
| `LOOKUP`          | `stat(path)` + inode alloc    | Resolve child name in parent directory |
| `ACCESS`          | `stat(path)` + permission check | Check read/write/execute bits |
| `READLINK`        | `readlink(path)`              | Read symlink target |
| `READ`            | `open(path, Read)` + `read_at(buf, offset)` | Open, read, close per request (or cache handle) |
| `WRITE`           | `open(path, Write)` + `write_at(data, offset)` | Open, write, close per request |
| `CREATE`          | `create(path, mode)`          | Create file, return new file handle |
| `MKDIR`           | `mkdir(path)`                 | Create directory |
| `SYMLINK`         | `symlink(target, link)`       | Create symbolic link |
| `REMOVE`          | `remove(path)`                | Remove file |
| `RMDIR`           | `remove(path)`                | Remove directory (VFS uses same method) |
| `RENAME`          | `rename(from, to)`            | Rename file or directory |
| `READDIR`         | `open_directory(path)` + `list()` | List directory entries |
| `READDIRPLUS`     | `open_directory(path)` + `list()` + `stat()` per entry | Directory entries with attributes |
| `FSSTAT`          | `capabilities()`              | Filesystem statistics (synthetic) |
| `FSINFO`          | `capabilities()`              | Filesystem capabilities |
| `PATHCONF`        | (hardcoded)                   | Path config: max name length, etc. |
| `COMMIT`          | (no-op or flush)              | Server-side flush for async writes |

### NFS File Handle Model

NFS uses **opaque file handles** (up to 64 bytes in NFSv3) to identify files across requests. The client sends the handle it received from LOOKUP/CREATE/MKDIR. The server must be able to resolve a handle back to a path.

**Handle strategy**: Leverage the inode-native VFS (Feature 23) — the VfsFileSystem owns inode allocation and provides `path_by_inode()` reverse lookup. The NFS file handle encodes the inode number (u64) in the first 8 bytes, with the remaining bytes reserved. On each NFS operation, the server decodes the inode from the handle, calls `fs.path_by_inode(ino)`, and dispatches to the VfsFileSystem. No separate inode cache needed (unlike the old FuseMount design).

```rust
/// NFS file handle: 32 bytes (well within 64-byte NFSv3 limit)
#[derive(Clone, Copy)]
#[repr(C)]
struct NfsFileHandle {
    ino: u64,          // inode number from inode-to-path cache
    generation: u64,   // generation counter to detect stale handles
    _reserved: [u8; 16],
}
```

**Stale handle detection**: If the inode cache has evicted a mapping (or the path was deleted), the server returns `NFS3ERR_STALE`. The generation counter increments on each mount session -- handles from a previous session are immediately rejected.

### NFS Error Code Mapping

| VfsError variant      | NFS v3 Status           |
|-----------------------|-------------------------|
| `NotFound`            | `NFS3ERR_NOENT`         |
| `AlreadyExists`       | `NFS3ERR_EXIST`         |
| `PermissionDenied`    | `NFS3ERR_ACCES`         |
| `NotAFile`            | `NFS3ERR_ISDIR`         |
| `NotADirectory`       | `NFS3ERR_NOTDIR`        |
| `Unsupported`         | `NFS3ERR_NOTSUPP`       |
| `Io`                  | `NFS3ERR_IO`            |
| `InvalidPath`         | `NFS3ERR_INVAL`         |
| `ReadOnly`            | `NFS3ERR_ROFS`          |
| `EntryPending`        | `NFS3ERR_JUKEBOX`       |

### Loopback Server Architecture

```
                    ┌──────────────────────────────┐
                    │      macOS Kernel (NFS)       │
                    │  mount_nfs -o ... 127.0.0.1:N │
                    └────────────┬─────────────────┘
                                 │ NFS v3 over TCP
                                 ▼
                    ┌──────────────────────────────┐
                    │      NfsMount<F> Server       │
                    │  - Listens on 127.0.0.1:port  │
                    │  - ONC RPC / XDR decode       │
                    │  - Dispatches to VfsFileSystem │
                    │  - Inode ↔ path cache         │
                    └──────────────────────────────┘
```

1. `NfsMount::mount()` binds a TCP listener on `127.0.0.1:0` (ephemeral port).
2. Spawns a background thread (or async task) to accept and handle NFS RPC connections.
3. Invokes `mount_nfs -o resvport,soft,timeo=30,retrans=5,vers=3,proto=tcp,port=N 127.0.0.1:/ <mountpoint>`.
4. Returns the `NfsMount` handle. The server runs until `unmount()` is called.
5. `unmount()` calls `umount <mountpoint>`, then shuts down the TCP listener and joins the server thread.

### Mount Command Details

```bash
# macOS mount command
mount_nfs -o resvport,soft,timeo=30,retrans=5,vers=3,proto=tcp,port=<PORT> \
    127.0.0.1:/ <MOUNTPOINT>
```

Options explained:
- `resvport`: Use a reserved port (<1024) for the client -- required by macOS NFS
- `soft`: Return errors on timeout rather than hanging indefinitely
- `timeo=30`: 3-second timeout (in tenths of a second)
- `retrans=5`: Retry 5 times before returning error
- `vers=3`: NFS version 3 explicitly
- `proto=tcp`: Use TCP (more reliable than UDP for loopback)
- `port=<PORT>`: Explicit port since we don't run a portmapper

## Tasks

### Core (`src/native/vfs/nfs.rs`)

- [ ] Define `NfsMount<F: VfsFileSystem>` struct: wraps VfsFileSystem + inode-to-path cache + generation counter
- [ ] Define `NfsFileHandle` struct: ino (u64) + generation (u64) + reserved bytes
- [ ] Use inode-native VFS (Feature 23) — `fs.path_by_inode(ino)` for handle resolution, `fs.stat_by_inode(ino)` for GETATTR, no separate cache
- [ ] Implement ONC RPC / XDR request parsing (minimal: only NFS v3 program number 100003)
- [ ] Implement NFS v3 operations mapped to VfsFileSystem trait calls:
  - `NULL` -- no-op health check
  - `LOOKUP` -- resolve child name, allocate inode, return file handle
  - `GETATTR` -- stat via VfsFileSystem, translate to fattr3
  - `SETATTR` -- chmod/truncate
  - `READ` -- open + read_at (cache file handle for duration of NFS session or per-request)
  - `WRITE` -- open + write_at
  - `CREATE` / `MKDIR` / `SYMLINK` -- create via VfsFileSystem
  - `REMOVE` / `RMDIR` -- remove via VfsFileSystem
  - `RENAME` -- rename via VfsFileSystem
  - `READDIR` / `READDIRPLUS` -- list directory entries with optional attributes
  - `READLINK` -- read symlink target
  - `ACCESS` -- permission check against VfsMetadata
  - `FSSTAT` / `FSINFO` / `PATHCONF` -- synthetic filesystem capabilities
  - `COMMIT` -- no-op or flush
- [ ] Implement VfsError to NFS status code mapping
- [ ] Implement TCP listener on loopback with background server thread
- [ ] Implement `NfsMount::mount(fs, mountpoint)` -- start NFS server, invoke `mount_nfs` command
- [ ] Implement `NfsMount::unmount()` -- `umount` + shutdown TCP listener + join server thread

### Edge Cases

- [ ] Handle stale file handles: return `NFS3ERR_STALE` when inode mapping is evicted or path deleted
- [ ] Handle concurrent NFS requests: server thread uses `RwLock` on VfsFileSystem or spawns per-connection handler threads
- [ ] Implement `Drop` for `NfsMount` that calls `unmount()` for clean shutdown

### Tests

- [ ] Test: mount MemoryFs, read file from mountpoint via std::fs (macOS)
- [ ] Test: write through mount, verify in underlying VfsFileSystem
- [ ] Test: unmount cleanly (no stale mount points)
- [ ] Test: external tools (`cat`, `ls`, `stat`) work against mount point
- [ ] Test: stale file handle returns appropriate NFS error

## Verification

- Tests pass on macOS
- `cargo check -p foundation_nativeapis --features vfs-nfs` passes
- Mount point is usable by standard POSIX tools
- No stale mount points left after `unmount()`
