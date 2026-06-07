---
feature_name: "Ptrace/Reverie Interceptor"
description: "PtraceInterceptor — dual-backend syscall interception for transparent VFS sandboxing on Linux. Shared SyscallInterceptor trait with nix-based (raw ptrace) and reverie-based backends behind sub-feature flags. Routes filesystem syscalls to VfsFileSystem, non-fs syscalls pass through."
status: "in-progress"
priority: "low"
phase: 4
created: 2026-06-04
updated: 2026-06-07
dependencies:
  - "01-core-traits"
tasks:
  completed: 21
  uncompleted: 7
  total: 28
  completion_percentage: 75%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 09: Ptrace/Reverie Interceptor

## Overview

The most transparent mounting mechanism — intercepts filesystem syscalls at the kernel boundary using ptrace (via the Reverie crate from AgentFS). A spawned process's file operations are silently routed through the VfsFileSystem. Non-filesystem syscalls (network, memory, process) pass through unmodified.

Linux only. Feature-gated behind `vfs-ptrace`. Highest complexity feature.

Inspired by AgentFS's sandbox design: mount table routing, virtual FD table, syscall entry/exit handlers.

### Dual Backend Architecture

Both a raw `nix`-based ptrace backend and a `reverie`-based backend are implemented behind separate sub-feature flags, sharing a common `SyscallInterceptor` trait.

```rust
/// Shared trait — both backends implement this.
pub trait SyscallInterceptor: Send + Sync {
    fn spawn<F: VfsFileSystem>(
        &self,
        fs: Arc<F>,
        mount_table: MountTable,
        command: &str,
        args: &[&str],
    ) -> VfsResult<InterceptorHandle>;
}

pub struct InterceptorHandle {
    pub child_pid: u32,
    join_handle: JoinHandle<VfsResult<i32>>,
}
```

**nix backend (`vfs-ptrace-nix`)**: Raw `ptrace(2)` via the `nix` crate (`nix::sys::ptrace`). No external framework dependency. Uses `PTRACE_SYSCALL` to trap syscall entry/exit, reads/writes tracee memory via `process_vm_readv`/`process_vm_writev`. More code but zero framework risk.

**reverie backend (`vfs-ptrace-reverie`)**: Uses the `reverie` + `reverie-ptrace` crates from Meta (facebookexperimental/reverie). Higher-level API with structured syscall dispatch. Experimental but functional. Pin to a specific version.

### Module Structure

```
src/native/vfs/ptrace/
    mod.rs              # SyscallInterceptor trait, MountTable, VirtualFdTable, InterceptorHandle
    mount_table.rs      # Path prefix → VFS backend routing
    fd_table.rs         # Virtual FD allocation and tracking
    memory.rs           # Tracee memory read/write helpers (process_vm_readv/writev)
    syscall_dispatch.rs # Shared syscall → VFS method mapping logic
    nix_backend.rs      # nix-based raw ptrace implementation
    reverie_backend.rs  # reverie-based implementation
```

### Reverie Crate Status

Reverie is a ptrace-based syscall interception framework originally developed at Meta (facebookexperimental/reverie). Status considerations:

- **Crate**: `reverie` and `reverie-ptrace` on crates.io
- **Maturity**: Experimental but functional. Used in production at Meta for syscall tracing.
- **Version**: Pin to a specific version in Cargo.toml (check latest at implementation time). The API has been unstable across versions.

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

### Shared Core (`src/native/vfs/ptrace/mod.rs`)

- [x] Define `SyscallInterceptor` trait with `spawn()` method
- [x] Define `InterceptorHandle` struct: child_pid + JoinHandle for wait
- [x] Define `MountTable`: maps path prefixes to VFS backends (virtual path → DynFs, real path → passthrough)
- [x] Define `VirtualFdTable`: tracks virtual FDs keyed by (pid, fd) with Arc-based sharing
- [x] Define `VirtualFdEntry` enum: File { handle: ErasedFile, path, offset, mode } | Directory { handle: ErasedDir, path, dir_offset }
- [x] Implement virtual FD allocation: monotonic counter starting at FD_VIRTUAL_BASE (10_000)

### Tracee Memory Helpers (`src/native/vfs/ptrace/memory.rs`)

- [x] Implement `read_tracee_string(pid, addr) -> String` — read NUL-terminated path via `process_vm_readv`
- [x] Implement `read_tracee_buf(pid, addr, len) -> Vec<u8>` — read buffer from tracee
- [x] Implement `write_tracee_buf(pid, addr, data)` — write buffer via `process_vm_writev`
- [x] Implement `write_tracee_stat(pid, addr, metadata)` — write x86_64 stat struct to tracee memory

### Syscall Dispatch (`src/native/vfs/ptrace/syscall_dispatch.rs`)

- [x] Implement shared syscall → VFS method dispatch logic (used by both backends):
  - `open`/`openat` → mount table lookup → VFS open or passthrough
  - `read`/`pread64` → if virtual FD, read from VfsFile; else passthrough
  - `write`/`pwrite64` → if virtual FD, write to VfsFile; else passthrough
  - `close` → if virtual FD, close VfsFile handle; else passthrough
  - `stat`/`lstat`/`fstat`/`newfstatat` → if virtual path/FD, stat from VFS; else passthrough
  - `getdents64` → if virtual FD, readdir from VfsDirectory; else passthrough
  - `access`/`faccessat` → if virtual path, check VFS; else passthrough
  - `unlink`/`unlinkat`/`rmdir`/`mkdir`/`mkdirat`/`rename`/`renameat`/`renameat2` → if virtual path, VFS op; else passthrough
  - `readlink`/`readlinkat`/`symlink`/`symlinkat` → if virtual path, VFS op; else passthrough
  - `lseek`/`truncate`/`ftruncate`/`chmod`/`fchmod` → if virtual, VFS op; else passthrough

### nix Backend (`src/native/vfs/ptrace/nix_backend.rs`)

- [x] Implement `NixInterceptor` struct implementing `SyscallInterceptor`
- [x] Implement raw ptrace loop: `PTRACE_TRACEME` on child, `PTRACE_SYSCALL` for entry/exit trapping
- [x] Read syscall number + args from registers (`PTRACE_GETREGS`) on syscall-entry-stop
- [x] Dispatch to shared syscall handler, modify registers/memory, set return value on syscall-exit-stop
- [x] Handle `fork`/`clone` propagation via `PTRACE_O_TRACEFORK | PTRACE_O_TRACECLONE | PTRACE_O_TRACEVFORK`

### reverie Backend (`src/native/vfs/ptrace/reverie_backend.rs`)

- [ ] Implement `ReverieInterceptor` struct implementing `SyscallInterceptor`
- [ ] Implement Reverie `Tool` trait with syscall entry/exit handlers
- [ ] Dispatch intercepted syscalls to shared syscall handler
- [ ] Handle fork/clone via Reverie's built-in child tracing

### Fork/Clone

- [x] Set `PTRACE_O_TRACEFORK | PTRACE_O_TRACECLONE | PTRACE_O_TRACEVFORK` on traced processes
- [x] Clone virtual FD table entries on fork (shared via Arc for clone with CLONE_FILES)
- [ ] Handle `execve()`: close virtual FDs marked `O_CLOEXEC`

### Security

- [x] Path validation: prevent escape from virtual mount points (resolve `..`, symlinks)
- [x] FD isolation: virtual FDs cannot leak to non-intercepted processes
- [x] Intercept `dup2`/`dup3`: reject if target FD is in virtual range

### Edge Cases

- [x] Handle `openat` with `AT_FDCWD`: resolve relative to tracee's cwd (read from `/proc/<pid>/cwd`)
- [ ] Handle `O_CREAT | O_EXCL` atomicity: VfsFileSystem `create()` returns `AlreadyExists` mapped to `-EEXIST`
- [ ] Handle partial reads/writes: tracee buffer may be smaller than requested

### Tests

- [ ] Test: spawn `cat /virtual/file.txt`, verify output matches VFS content
- [ ] Test: spawn process that writes, verify write goes to delta store
- [ ] Test: spawn process that forks, child inherits virtual FDs
- [ ] Test: non-virtual paths pass through to real filesystem unchanged
- [ ] Test: virtual FD range does not collide with real FDs

## Feature Flags

```toml
[features]
vfs-ptrace-nix = ["vfs-native", "dep:nix"]       # nix must gain "ptrace", "process", "signal" features
vfs-ptrace-reverie = ["vfs-native", "dep:reverie", "dep:reverie-ptrace"]
vfs-ptrace = ["vfs-ptrace-nix"]                   # default to nix backend
```

## Verification

- Tests pass on Linux
- `cargo check -p foundation_nativeapis --features vfs-ptrace-nix` passes
- `cargo check -p foundation_nativeapis --features vfs-ptrace-reverie` passes
- Real-world command (`ls /virtual/`, `cp /virtual/a /tmp/b`) works correctly under both backends
- Non-filesystem syscalls (network, memory) are unaffected
