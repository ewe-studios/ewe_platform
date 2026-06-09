---
feature_name: "LD_PRELOAD VFS Shim"
description: "Shared library (cdylib) integrated into foundation_nativeapis (.so/.dylib) that intercepts libc filesystem calls via LD_PRELOAD (Linux) / DYLD_INSERT_LIBRARIES (macOS), redirecting configured paths through OverlayFileSystem with pluggable delta stores (memory, sqlite, turso, dir, d1, r2). Integrated into foundation_nativeapis crate as a feature-gated cdylib."
status: "in-progress"
priority: "low"
phase: 5
created: 2026-06-04
updated: 2026-06-09
dependencies:
  - "01-core-traits"
  - "04-native-fs"
  - "02-memory-impls"
  - "03-foundation-fs"
tasks:
  completed: 12
  uncompleted: 10
  total: 22
  completion_percentage: 55%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 16: LD_PRELOAD VFS Shim

## Overview

A shared library (`.so` on Linux, `.dylib` on macOS) that interposes libc filesystem functions via `LD_PRELOAD` / `DYLD_INSERT_LIBRARIES`. Built as a `cdylib` target within the `foundation_nativeapis` crate (feature `vfs-preload`). Routes virtual paths through the real VFS stack — `OverlayFileSystem<NativeFs, Delta>` — where the delta store is pluggable at runtime via environment variable.

### Delta Stores

| Env value | Store | Persistence | Feature flag |
|-----------|-------|-------------|--------------|
| `memory` (default) | MemoryDelta | None | `vfs-preload` |
| `sqlite` | LibsqlDelta | Local SQLite | `vfs-sqlite` |
| `turso` | TursoDelta | Remote libSQL | `vfs-turso` |
| `dir` | DirectoryDelta | Shadow directory | `vfs-native` |
| `d1` | D1Delta (pending) | Cloudflare D1 | `vfs-d1` |
| `r2` | R2Delta (pending) | Cloudflare R2 | `vfs-r2` |

Each falls back to `memory` if the feature flag isn't enabled or initialization fails.

### Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `FOUNDATION_VFS_PREFIX` | Colon-separated virtual path prefixes (e.g., `/virtual`) | (none) |
| `FOUNDATION_VFS_ROOT` | Base directory for NativeFs overlay | `.` |
| `FOUNDATION_VFS_DELTA` | Delta store backend | `memory` |
| `FOUNDATION_VFS_DELTA_PATH` | Path for persistent delta stores | `/tmp/vfs-delta.{db,dir}` |
| `FOUNDATION_VFS_SOCKET` | IPC daemon socket (future) | — |

## Tasks

### Shim Library (`backends/foundation_nativeapis/src/native/vfs/shim/`)

- [x] Integrated into main crate as `native::vfs::shim` module (feature-gated behind `vfs-preload`)
- [x] Crate produces both `rlib` and `cdylib` (same source, no duplicate crate)
- [x] Uses real VFS infrastructure: `OverlayFileSystem<NativeFs, Delta>` with `DynFs` type erasure
- [x] Pluggable delta stores: memory, sqlite, turso, dir, d1 (scaffolded), r2 (scaffolded)
- [x] Falls back to memory delta when feature flag missing or init fails
- [x] Base filesystem configurable via `FOUNDATION_VFS_ROOT` (defaults to `.`)
- [x] Virtual prefix configurable via `FOUNDATION_VFS_PREFIX` (colon-separated)
- [x] Intercepted functions: `open`, `open64`, `openat`, `__openat64_time64`, `close`, `read`, `write`, `lseek`, `fstat`, `stat`, `lstat`, `access`, `unlink`, `rename`, `mkdir`, `rmdir`, `opendir`
- [ ] Intercepted functions: `pread`, `pwrite`, `readdir`, `readdir_r`, `readlink`, `symlink`, `chmod`, `fchmod`, `truncate`, `ftruncate`, `fsync`, `openat2`
- [x] Virtual fd table: synthetic FDs (10000+) map to `ErasedFile` handles
- [x] Thread safety: fd table uses `Mutex`, file handles use `Arc<Mutex<>>`
- [ ] Directory listing: `opendir` returns ENOSYS (needs DIR* wrapper for VfsDirectory → dirent)
- [x] Example: `cargo run --example vfs_shim` demonstrates all scenarios

### Platform Support

- [x] Linux: `LD_PRELOAD=libfoundation_nativeapis.so` — standard interposition
- [ ] macOS: `DYLD_INSERT_LIBRARIES=libfoundation_nativeapis.dylib` — note SIP restrictions
- [ ] Build both targets from same source with `#[cfg(target_os)]` for platform differences

### Launcher Helper

- [ ] Helper script/binary: `foundation-vfs-run <command>` that sets `LD_PRELOAD`/`DYLD_INSERT_LIBRARIES` and environment variables, then execs the command

### Tests

- [ ] Test: `LD_PRELOAD` shim — spawn child process with preload, child reads virtual file, verify correct content
- [ ] Test: non-virtual paths pass through to real filesystem
- [ ] Test: write through preloaded process goes to delta store
- [ ] Test: multi-threaded application doesn't deadlock on fd table
- [ ] Test: sqlite delta persists across process restarts
- [ ] Test: directory delta shows files on disk

## Verification

- [x] Example runs: `cargo run -p foundation_nativeapis --features vfs-preload --example vfs_shim`
- [ ] Tests pass on Linux
- [ ] `cat /virtual/file.txt` (with preload) returns VFS content
- [ ] `ls /virtual/` (with preload) shows VFS directory listing
- [ ] Real filesystem access unaffected
- [ ] No segfaults or deadlocks in multi-threaded usage

---

_Created: 2026-06-04 | Updated: 2026-06-09_
