# Example: NativeFs

## Purpose

Demonstrates `NativeFs` — a VFS implementation that wraps a real OS directory, passing through actual filesystem operations including OS-assigned inodes, permissions, and file I/O. This example shows how the VFS abstraction layers over native POSIX/Win32 syscalls, enabling portable code that can swap between in-memory and on-disk backends without changing business logic.

## Prerequisites

- Linux, macOS, or Windows (POSIX syscalls on Unix, Win32 APIs on Windows)
- Rust toolchain with workspace dependencies
- Feature flag: `vfs-native`

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs-native --example native_fs
```

## Architecture

`NativeFs` implements `VfsFileSystem` by delegating every operation to the underlying OS:

1. **Root isolation** — All paths are resolved relative to a configured root directory (e.g., `/tmp/foundation_native_fs_example`). Operations cannot escape this sandbox.

2. **OS inode passthrough** — `stat()` returns real `st_ino` values from the kernel, enabling integration with tools that depend on stable inode identity (e.g., `find -inum`, build system caching).

3. **Real file I/O** — `create()` uses `open(O_CREAT)`, `write_at()` uses `pwrite()`, and `read_file()` uses `read()`. Data is persisted to disk immediately.

4. **Directory traversal** — `open_directory().list()` uses `readdir()` to enumerate entries, returning `VfsFileType::File` or `VfsFileType::Directory` based on `stat()` results.

5. **Atomic rename** — `rename()` calls the OS `rename(2)` syscall, ensuring atomicity on POSIX systems.

The example creates a temporary directory, performs typical file operations, verifies persistence by reading the file directly via `std::fs`, then cleans up.

## Expected Output

```
=== NativeFs Example ===

NativeFs root: "/tmp/foundation_native_fs_example"
Created /src and /src/models
Created /src/main.rs (42 bytes)
Contents:
fn main() {
    println!("hello");
}

stat: inode=12345678, size=42, type=File, perms=644
/src contains:
  Directory "models" (inode 12345679)
  File "main.rs" (inode 12345680)

Verify on disk at "/tmp/foundation_native_fs_example/src/models/user.rs": "pub struct User { pub name: String }"
Renamed user.rs → account.rs
Removed /src tree

=== Done ===
```

## Key APIs Demonstrated

- `NativeFs::new(root)` — Construct a NativeFs rooted at a specific directory
- `NativeFs::root()` — Get the root path
- `VfsFileSystem::create(path, mode)` — Create a file (persisted via `open(O_CREAT)`)
- `VfsFileSystem::mkdir(path)` — Create a directory (via `mkdir(2)`)
- `VfsFileSystem::write_file(path, data)` — Atomic write (writes to temp file, then `rename()`)
- `VfsFileSystem::read_file(path)` — Read entire file via `read(2)`
- `VfsFileSystem::stat(path)` — Get metadata including real OS inode
- `VfsFileSystem::rename(from, to)` — Atomic rename via `rename(2)`
- `VfsFileSystem::remove_all(path)` — Recursive removal via `rm -rf` equivalent
- `VfsDirectory::list()` — List directory via `readdir()`

## Where to Use This

- **Persistent storage backend** — Replace `MemoryFs` when data must survive restarts
- **Native interop** — Access real OS inodes for integration with `find`, `rsync`, or build tools
- **Hybrid architectures** — Use NativeFs for production and MemoryFs for testing without code changes
- **Sandboxed file access** — Root the VFS at a subdirectory to prevent path traversal
- **Migration layer** — Gradually port legacy file-handling code to the VFS trait interface
- **Performance benchmarking** — Measure real disk I/O latency vs. in-memory baselines

## Related

- Feature spec: `specifications/37-overlay-vfs/features/02-native-fs/feature.md`
- Source: `src/native/vfs/native_fs.rs`
- Trait definition: `src/shared/vfs/traits.rs` (`VfsFileSystem`)
