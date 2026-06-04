---
feature_name: "NFS Adapter"
description: "NfsMount — NFS v3 loopback server exposing VfsFileSystem as a mount on macOS. No kernel extension required."
status: "pending"
priority: "medium"
phase: 4
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Feature 08: NFS Adapter

## Overview

macOS alternative to FUSE. Runs an NFS v3 server on loopback, mounted via `mount_nfs`. No kernel extension (kext) needed — works with Apple's security model. Inspired by AgentFS's macOS mounting strategy.

Feature-gated behind `vfs-nfs`.

## Tasks

### Core (`src/native/vfs/nfs.rs`)

- [ ] Define `NfsMount<F: VfsFileSystem>` struct: wraps VfsFileSystem + inode-to-path cache
- [ ] Implement NFS v3 operations mapped to VfsFileSystem trait calls: LOOKUP, GETATTR, READDIR, READ, WRITE, CREATE, REMOVE, MKDIR, RMDIR, RENAME, READLINK, SYMLINK
- [ ] Implement inode-to-path cache (similar to FUSE adapter)
- [ ] Implement `NfsMount::mount(fs, mountpoint)` — start NFS server on loopback, invoke `mount_nfs`
- [ ] Implement `NfsMount::unmount()` — clean shutdown + `umount`
- [ ] Mount options: `resvport,soft,timeo=30,retrans=5`

### Tests

- [ ] Test: mount, read, write, unmount on macOS
- [ ] Test: external tools work against mount point

## Verification

- Tests pass on macOS
- `cargo check -p foundation_nativeapis --features vfs-nfs` passes
