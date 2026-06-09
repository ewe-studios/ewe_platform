# Example: FUSE Mount

## Purpose

Demonstrates how to mount a VFS filesystem via FUSE (Filesystem in Userspace). Any VFS implementation (`MemoryFs`, `NativeFs`, `OverlayFileSystem`, etc.) can be exposed as a real mount point that standard shell commands and applications can interact with.

## Prerequisites

- Feature flags: `vfs-fuse`
- Linux with FUSE support (`/dev/fuse` device)
- User must be in the `fuse` group, or system allows unprivileged FUSE mounts (`/etc/fuse.conf` with `user_allow_other`)

## How to Run

```bash
# Mount a MemoryFs at /tmp/vfs-fuse
cargo run -p foundation_nativeapis --features vfs-fuse --example fuse_mount /tmp/vfs-fuse

# In another terminal, interact with the mounted filesystem:
ls -la /tmp/vfs-fuse/
cat /tmp/vfs-fuse/hello.txt
echo "new content" > /tmp/vfs-fuse/new.txt
ls -la /tmp/vfs-fuse/

# When done, unmount:
fusermount -u /tmp/vfs-fuse
```

## Architecture

The example:

1. Creates a `MemoryFs` base with initial content
2. Wraps it in an `OverlayFileSystem<MemoryFs, MemoryDelta>` for write support
3. Mounts it via `FuseMount` which implements the `fuser::Filesystem` trait
4. Any process can now read/write the VFS through standard POSIX calls

The FUSE adapter translates VFS trait methods (`open`, `read`, `write`, `readdir`, etc.) into FUSE kernel operations. Key design points:

- **Inode table**: Maps VFS inodes to FUSE inodes with reference counting
- **Directory handles**: Manages `DIR*` lifecycle for `readdir` operations
- **Zero-copy I/O**: Uses `pread`/`pwrite` semantics through VFS `read_at`/`write_at`
- **Path containment**: All paths resolved within the mount root

## Expected Output

```
=== FUSE Mount Example ===

Virtual filesystem ready:
  /hello.txt     (42 bytes)
  /readme.md     (32 bytes)
  /src/main.rs   (35 bytes)

Mount point: /tmp/vfs-fuse
Starting FUSE server... (Ctrl+C to unmount)
```

## Key APIs Demonstrated

- `FuseMount::new()` — creates a FUSE filesystem from any `VfsFileSystem`
- `FuseMount::mount()` — mounts at a path (blocks until unmount)
- `OverlayFileSystem` — provides writable overlay on read-only base

## Related

- Feature spec: `specifications/37-overlay-vfs/features/07-fuse-adapter/feature.md`
- Source: `src/native/vfs/fuse.rs`
