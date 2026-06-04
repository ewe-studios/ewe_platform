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
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Feature 09: Ptrace/Reverie Interceptor

## Overview

The most transparent mounting mechanism — intercepts filesystem syscalls at the kernel boundary using ptrace (via the Reverie crate from AgentFS). A spawned process's file operations are silently routed through the VfsFileSystem. Non-filesystem syscalls (network, memory, process) pass through unmodified.

Linux only. Feature-gated behind `vfs-ptrace`. Highest complexity feature.

Inspired by AgentFS's sandbox design: mount table routing, virtual FD table, syscall entry/exit handlers.

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

### Security

- [ ] Path validation: prevent escape from virtual mount points
- [ ] FD isolation: virtual FDs cannot leak to non-intercepted processes

### Tests

- [ ] Test: spawn `cat /virtual/file.txt`, verify output matches VFS content
- [ ] Test: spawn process that writes, verify write goes to delta store

## Verification

- Tests pass on Linux
- `cargo check -p foundation_nativeapis --features vfs-ptrace` passes
