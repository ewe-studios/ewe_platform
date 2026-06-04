---
feature_name: "LD_PRELOAD VFS Shim"
description: "Shared library (.so/.dylib) that intercepts libc filesystem calls via LD_PRELOAD (Linux) / DYLD_INSERT_LIBRARIES (macOS), redirecting configured paths to a VFS daemon over IPC. Semi-transparent — works for dynamically linked applications without FUSE or kernel modules."
status: "pending"
priority: "low"
phase: 5
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
  - "11-ipc-daemon"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature 16: LD_PRELOAD VFS Shim

## Overview

A shared library (`.so` on Linux, `.dylib` on macOS) that interposes libc filesystem functions via `LD_PRELOAD` / `DYLD_INSERT_LIBRARIES`. When loaded into a process, it intercepts calls like `open()`, `read()`, `write()`, `stat()`, `opendir()`, etc. For paths matching configured prefixes, calls are redirected to a VFS daemon over IPC (feature 11). For all other paths, calls pass through to the real libc.

This gives semi-transparent VFS access to any dynamically linked application — no FUSE, no kernel module, no driver install. Useful for:
- Running build tools (`gcc`, `make`, `python`) against a virtual overlay
- Sandboxing interpreted language runtimes
- macOS where FUSE requires a kext and NFS has overhead

### Limitations

- **Statically linked binaries** — not intercepted (Go programs with raw syscalls, musl-static builds)
- **Raw `syscall()` invocations** — not intercepted (only libc wrappers are replaced)
- **macOS SIP** — `DYLD_INSERT_LIBRARIES` is stripped for system binaries under `/usr/bin/`, `/usr/sbin/`, etc. Works for user-installed binaries.
- **Thread safety** — must be safe for multi-threaded applications

### How It Works

```
Application process
  │
  ├── open("/virtual/file.txt") ──► Shim intercepts (libc interposition)
  │                                  ├── Path matches prefix? → Send VfsRequest to daemon via IPC
  │                                  │                          ← Receive VfsResponse (fd or data)
  │                                  │                          Return synthetic fd to application
  │                                  └── Path doesn't match?  → Call real libc open()
  │
  ├── read(fd) ──► Shim checks fd table
  │                 ├── Virtual fd? → Send ReadAt to daemon via IPC
  │                 └── Real fd?    → Call real libc read()
  │
  └── (application sees normal POSIX behavior)
```

## Tasks

### Shim Library (`shims/foundation-vfs-preload/`)

- [ ] Create separate crate/build target producing a `.so`/`.dylib`
- [ ] Implement libc function interposition using `dlsym(RTLD_NEXT, ...)` to get real function pointers
- [ ] Intercepted functions: `open`, `open64`, `openat`, `close`, `read`, `write`, `pread`, `pwrite`, `lseek`, `fstat`, `stat`, `lstat`, `access`, `unlink`, `rename`, `mkdir`, `rmdir`, `opendir`, `readdir`, `readdir_r`, `closedir`, `readlink`, `symlink`, `chmod`, `fchmod`, `truncate`, `ftruncate`, `fsync`
- [ ] Path prefix configuration: read from environment variable (e.g., `FOUNDATION_VFS_PREFIX=/virtual`) or config file
- [ ] IPC connection: connect to VFS daemon on init (socket path from environment variable `FOUNDATION_VFS_SOCKET`)
- [ ] Virtual fd table: map synthetic fd numbers (high range, e.g., 10000+) to VFS daemon handles
- [ ] Thread safety: fd table behind a lock, IPC connection thread-safe

### Platform Support

- [ ] Linux: `LD_PRELOAD=libfoundation_vfs.so` — standard interposition
- [ ] macOS: `DYLD_INSERT_LIBRARIES=libfoundation_vfs.dylib` — note SIP restrictions
- [ ] Build both targets from same source with `#[cfg(target_os)]` for platform differences

### Launcher Helper

- [ ] Helper script/binary: `foundation-vfs-run <command>` that sets `LD_PRELOAD`/`DYLD_INSERT_LIBRARIES` and environment variables, then execs the command

### Tests

- [ ] Test: `LD_PRELOAD` shim + VFS daemon — spawn child process with preload, child reads virtual file, verify correct content
- [ ] Test: non-virtual paths pass through to real filesystem
- [ ] Test: write through preloaded process goes to delta store
- [ ] Test: multi-threaded application doesn't deadlock on fd table

## Verification

- Tests pass on Linux
- `cat /virtual/file.txt` (with preload) returns VFS content
- `ls /virtual/` (with preload) shows VFS directory listing
- Real filesystem access unaffected
- No segfaults or deadlocks in multi-threaded usage
