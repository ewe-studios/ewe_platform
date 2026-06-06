---
feature_name: "Ptrace/Reverie Interceptor"
description: "PtraceInterceptor — Reverie-based syscall interception for transparent VFS sandboxing on Linux. Routes filesystem syscalls to VfsFileSystem, non-fs syscalls pass through."
status: "pending"
priority: "low"
phase: 4
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
tasks:
  completed: 0
  uncompleted: 21
  total: 21
  completion_percentage: 0%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 09: Ptrace/Reverie Interceptor

## Overview

The most transparent mounting mechanism — intercepts filesystem syscalls at the kernel boundary using ptrace (via the Reverie crate from AgentFS). A spawned process's file operations are silently routed through the VfsFileSystem. Non-filesystem syscalls (network, memory, process) pass through unmodified.

Linux only. Feature-gated behind `vfs-ptrace`. Highest complexity feature.

Inspired by AgentFS's sandbox design: mount table routing, virtual FD table, syscall entry/exit handlers.

### Reverie Crate Status

Reverie is a ptrace-based syscall interception framework originally developed at Meta (facebookexperimental/reverie). Status considerations:

- **Crate**: `reverie` and `reverie-ptrace` on crates.io
- **Maturity**: Experimental but functional. Used in production at Meta for syscall tracing.
- **Version**: Pin to a specific version in Cargo.toml (check latest at implementation time). The API has been unstable across versions.
- **Fallback**: If Reverie is unmaintained or too unstable at implementation time, fall back to raw `ptrace(2)` via the `nix` crate (`nix::sys::ptrace`). This is more work but has no external framework dependency. The adapter architecture remains the same -- only the syscall interception layer changes.

### Syscall Number to VFS Operation Mapping

Linux x86_64 syscall numbers (from `<asm/unistd_64.h>`):

| Syscall (nr)           | VfsFileSystem Method                | Entry Handler | Exit Handler |
|------------------------|-------------------------------------|---------------|--------------|
| `open` (2)             | `open(path, mode)`                  | Resolve path, check mount table | Replace return FD with virtual FD |
| `openat` (257)         | `open(path, mode)`                  | Resolve dirfd + path, check mount table | Replace return FD with virtual FD |
| `read` (0)             | `VfsFile::read_at(buf, offset)`     | Check if FD is virtual | Write result to tracee memory |
| `pread64` (17)         | `VfsFile::read_at(buf, offset)`     | Check if FD is virtual | Write result to tracee memory |
| `write` (1)            | `VfsFile::write_at(data, offset)`   | Check if FD is virtual, read data from tracee | Set return value to bytes written |
| `pwrite64` (18)        | `VfsFile::write_at(data, offset)`   | Check if FD is virtual, read data from tracee | Set return value to bytes written |
| `close` (3)            | Drop VfsFile handle                 | Check if FD is virtual | Remove from FD table |
| `stat` (4)             | `stat(path)`                        | Resolve path | Write stat struct to tracee memory |
| `fstat` (5)            | `VfsFile::metadata()`               | Check if FD is virtual | Write stat struct to tracee memory |
| `lstat` (6)            | `stat(path)` (no follow)            | Resolve path | Write stat struct to tracee memory |
| `newfstatat` (262)     | `stat(path)`                        | Resolve dirfd + path | Write stat struct to tracee memory |
| `getdents64` (217)     | `VfsDirectory::list()`              | Check if FD is virtual dir | Write dirent structs to tracee memory |
| `access` (21)          | `stat(path)` + permission check     | Resolve path | Set return value |
| `faccessat` (269)      | `stat(path)` + permission check     | Resolve dirfd + path | Set return value |
| `unlink` (87)          | `remove(path)`                      | Resolve path | Set return value |
| `unlinkat` (263)       | `remove(path)` or `rmdir`           | Resolve dirfd + path, check flags | Set return value |
| `mkdir` (83)           | `mkdir(path)`                       | Resolve path | Set return value |
| `mkdirat` (258)        | `mkdir(path)`                       | Resolve dirfd + path | Set return value |
| `rmdir` (84)           | `remove(path)`                      | Resolve path | Set return value |
| `rename` (82)          | `rename(from, to)`                  | Resolve both paths | Set return value |
| `renameat` (264)       | `rename(from, to)`                  | Resolve dirfd + paths | Set return value |
| `renameat2` (316)      | `rename(from, to)`                  | Resolve dirfd + paths, check flags | Set return value |
| `readlink` (89)        | `readlink(path)`                    | Resolve path | Write target to tracee memory |
| `readlinkat` (267)     | `readlink(path)`                    | Resolve dirfd + path | Write target to tracee memory |
| `symlink` (88)         | `symlink(target, link)`             | Resolve link path | Set return value |
| `symlinkat` (266)      | `symlink(target, link)`             | Resolve dirfd + link path | Set return value |
| `lseek` (8)            | `SeekableVfsFile::seek(pos)`        | Check if FD is virtual | Set return value to new position |
| `truncate` (76)        | `VfsFile::truncate(size)`           | Resolve path | Set return value |
| `ftruncate` (77)       | `VfsFile::truncate(size)`           | Check if FD is virtual | Set return value |
| `chmod` (90)           | `chmod(path, mode)`                 | Resolve path | Set return value |
| `fchmod` (91)          | `chmod(path, mode)`                 | Check if FD is virtual | Set return value |

All other syscalls (network, memory, process, signal, etc.) pass through unmodified.

### Virtual FD Allocation Strategy

Virtual FDs must not collide with real kernel FDs. Strategy:

1. **High-range allocation**: Virtual FDs start at `FD_VIRTUAL_BASE = 10_000`. The kernel rarely allocates FDs this high (it starts from the lowest available, typically 3+). This provides a simple `fd >= FD_VIRTUAL_BASE` check to distinguish virtual from real FDs.
2. **Monotonic counter**: `AtomicU64` counter starting at `FD_VIRTUAL_BASE`. Each `open()`/`openat()` on a virtual path increments and returns the next value.
3. **FD table structure**:

```rust
struct VirtualFdTable {
    next_fd: AtomicU64,  // starts at FD_VIRTUAL_BASE (10_000)
    entries: HashMap<u64, VirtualFdEntry>,
}

enum VirtualFdEntry {
    File {
        handle: Box<dyn VfsFile>,
        path: String,
        offset: u64,  // current seek position
        mode: OpenMode,
    },
    Directory {
        handle: Box<dyn VfsDirectory>,
        path: String,
        dir_offset: u64,  // getdents offset tracking
    },
}
```

4. **Collision avoidance**: If the tracee explicitly calls `dup2(real_fd, 10000+)`, the virtual FD could collide. Mitigation: intercept `dup2`/`dup3` and reject if the target is in the virtual range (return `-EBADF`).
5. **Per-process tables**: Each traced process (and its children) shares the same virtual FD table, but FD numbers are per-process. The FD table is keyed by `(pid, fd)`.

### Fork/Clone Propagation

When a traced process calls `fork()` or `clone()`:

1. **Reverie/ptrace automatically traces children**: The `PTRACE_O_TRACEFORK | PTRACE_O_TRACECLONE | PTRACE_O_TRACEVFORK` options ensure child processes are also traced.
2. **FD inheritance**: On `fork()`, the child inherits the parent's FD table. Virtual FD entries are cloned (the underlying VfsFile handles are shared via `Arc` or re-opened).
3. **Mount table inheritance**: The child process sees the same mount table as the parent. Changes to the mount table by the parent do not affect the child (snapshot semantics).
4. **Thread handling**: `clone()` with `CLONE_FILES` means the child shares the FD table with the parent (same as threads sharing file descriptors). The virtual FD table must be `Arc<RwLock<...>>` to handle this correctly.
5. **`exec()` handling**: On `execve()`, the FD table is preserved (same as real FDs). `O_CLOEXEC` virtual FDs should be closed.

### Performance Overhead Expectations

Ptrace interception has inherent overhead due to context switches between tracee and tracer:

- **Each intercepted syscall**: 2 context switches (entry + exit) via ptrace stop/continue. Estimated ~5-20 microseconds per syscall on modern hardware.
- **Non-filesystem syscalls**: Pass through with minimal overhead (~1-2 microseconds for the ptrace check-and-continue).
- **I/O amplification**: Reading/writing tracee memory via `process_vm_readv`/`process_vm_writev` adds latency proportional to buffer size.
- **Expected overhead**: 2-10x slower than native filesystem operations for small I/O. Large sequential reads/writes are less affected (amortized over fewer syscalls).
- **Comparison**: FUSE adapter has lower per-operation overhead (~2-5 microseconds per FUSE request). Ptrace is chosen when FUSE is not available or when full syscall transparency is needed (no kernel module, no mount privileges).
- **Optimization**: Batch `process_vm_readv`/`process_vm_writev` for large buffers. Cache frequently-accessed paths in the mount table lookup.

## Tasks

### Core (`src/native/vfs/ptrace.rs`)

- [ ] Define `PtraceInterceptor<F: VfsFileSystem>` struct: wraps VfsFileSystem + mount table + FD table
- [ ] Define mount table: maps path prefixes to VFS backends (virtual path → VfsFileSystem, real path → passthrough)
- [ ] Define FD table: tracks virtual FDs (VfsFile handles) vs real kernel FDs
- [ ] Implement Reverie syscall handlers for filesystem ops:
  - `open`/`openat` → mount table lookup → VFS open or passthrough
  - `read`/`pread64` → if virtual FD, read from VfsFile; else passthrough
  - `write`/`pwrite64` → if virtual FD, write to VfsFile; else passthrough
  - `close` → if virtual FD, close VfsFile handle; else passthrough
  - `stat`/`lstat`/`fstat` → if virtual path/FD, stat from VFS; else passthrough
  - `getdents64` → if virtual FD, readdir from VfsDirectory; else passthrough
  - `access` → if virtual path, check VFS; else passthrough
  - `unlink`/`rmdir`/`mkdir`/`rename` → if virtual path, VFS operation; else passthrough
- [ ] Implement `PtraceInterceptor::spawn(fs, command, args)` — launch child process under ptrace
- [ ] Implement clean shutdown and FD cleanup
- [ ] Handle `fork`/`clone` — propagate interception to child processes

### Fork/Clone

- [ ] Set `PTRACE_O_TRACEFORK | PTRACE_O_TRACECLONE | PTRACE_O_TRACEVFORK` on traced processes
- [ ] Clone virtual FD table entries on fork (shared via Arc for clone with CLONE_FILES)
- [ ] Handle `execve()`: close virtual FDs marked `O_CLOEXEC`

### Security

- [ ] Path validation: prevent escape from virtual mount points (resolve `..`, symlinks)
- [ ] FD isolation: virtual FDs cannot leak to non-intercepted processes
- [ ] Intercept `dup2`/`dup3`: reject if target FD is in virtual range

### Edge Cases

- [ ] Handle `openat` with `AT_FDCWD`: resolve relative to tracee's cwd (read from `/proc/<pid>/cwd`)
- [ ] Handle `O_CREAT | O_EXCL` atomicity: VfsFileSystem `create()` returns `AlreadyExists` mapped to `-EEXIST`
- [ ] Handle partial reads/writes: tracee buffer may be smaller than requested

### Tests

- [ ] Test: spawn `cat /virtual/file.txt`, verify output matches VFS content
- [ ] Test: spawn process that writes, verify write goes to delta store
- [ ] Test: spawn process that forks, child inherits virtual FDs
- [ ] Test: non-virtual paths pass through to real filesystem unchanged
- [ ] Test: virtual FD range does not collide with real FDs

## Verification

- Tests pass on Linux
- `cargo check -p foundation_nativeapis --features vfs-ptrace` passes
- Real-world command (`ls /virtual/`, `cp /virtual/a /tmp/b`) works correctly
- Non-filesystem syscalls (network, memory) are unaffected
