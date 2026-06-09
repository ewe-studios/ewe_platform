# Example: MemoryFs

## Purpose

A comprehensive walkthrough of `MemoryFs` — the in-memory VFS implementation that serves as the foundation for overlay filesystems, delta stores, and testing harnesses. This example demonstrates the full CRUD lifecycle: creating files and directories, reading and writing data, seeking within files, querying metadata, renaming, removing, and working with symlinks.

## Prerequisites

- Rust toolchain with workspace dependencies
- Feature flag: `vfs`

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs --example memory_fs
```

## Architecture

`MemoryFs` implements the `VfsFileSystem` trait entirely in RAM, using a `BTreeMap` indexed by path for O(log n) lookups. Each entry stores:

- **Inode number** — Unique identifier allocated monotonically
- **File type** — Regular file, directory, or symlink
- **Permissions** — Unix-style mode bits
- **Data** — For files, a `Vec<u8>` buffer; for directories, a set of child entries
- **Symlink target** — For symbolic links, the path they resolve to

The example walks through every major operation:

1. **Directory creation** — `mkdir()` creates nested directories
2. **File creation and I/O** — `create()`, `write_at()`, `read_file()`
3. **Seekable file access** — `open_seekable()` with `SeekFrom` positioning
4. **Metadata queries** — `stat()`, `inode()`, `path_by_inode()`
5. **Directory listing** — `open_directory().list()` returns typed entries
6. **Rename** — Preserves inode, updates internal path mapping
7. **Removal** — Single-file `remove()` and recursive `remove_all()`
8. **Symlinks** — `symlink()` and `readlink()` for link resolution

## Expected Output

```
=== MemoryFs Example ===

Created /docs and /docs/notes
Created /docs/hello.txt (20 bytes)
Read: "Hello from MemoryFs!"
Read at offset 6: "from"
stat: inode=3, size=20, type=File, perms=644
inode 3 → path "/docs/hello.txt"

/docs contains:
  Directory "notes" (inode 2)
  File "hello.txt" (inode 3)

Renamed hello.txt → greeting.txt
Inode preserved after rename: 3 == 3
Created and removed /docs/notes/tmp.txt
Symlink /docs/link.txt → /docs/greeting.txt
Symlink read matches original

remove_all /docs — gone

=== Done ===
```

## Key APIs Demonstrated

- `VfsFileSystem::create(path, mode)` — Create a new regular file, returns `VfsFile`
- `VfsFileSystem::mkdir(path)` — Create a directory
- `VfsFileSystem::write_file(path, data)` — Atomic write entire file
- `VfsFileSystem::read_file(path)` — Read entire file contents
- `VfsFileSystem::stat(path)` — Get metadata (inode, size, type, permissions)
- `VfsFileSystem::open_seekable(path, mode)` — Open a seekable file handle
- `SeekableVfsFile::seek(SeekFrom)` — Reposition file cursor
- `SeekableVfsFile::read(&mut [u8])` — Read at current cursor position
- `VfsFileSystem::inode(path)` — Resolve path to inode number
- `VfsFileSystem::path_by_inode(ino)` — Reverse inode lookup to path
- `VfsFileSystem::rename(from, to)` — Rename file (preserves inode)
- `VfsFileSystem::remove(path)` — Remove a file or empty directory
- `VfsFileSystem::remove_all(path)` — Recursively remove a tree
- `VfsFileSystem::symlink(target, link)` — Create a symbolic link
- `VfsFileSystem::readlink(path)` — Read symlink target
- `VfsDirectory::list()` — List directory entries with type and inode

## Where to Use This

- **Testing harness** — Fast, isolated filesystem for unit tests without disk I/O
- **Overlay base layer** — Serve as the read-only base in an `OverlayFileSystem`
- **Delta store backend** — Wrap in `MemoryDelta` for copy-on-write overlays
- **Build system sandboxing** — Isolate intermediate build artifacts in memory
- **IDE virtual filesystem** — Represent project files without touching disk
- **Prototyping** — Rapid iteration on VFS logic before integrating with native backends

## Related

- Feature spec: `specifications/37-overlay-vfs/features/01-memory-fs/feature.md`
- Source: `src/shared/vfs/memory_fs.rs`
- Trait definition: `src/shared/vfs/traits.rs` (`VfsFileSystem`, `VfsFile`, `VfsDirectory`)
