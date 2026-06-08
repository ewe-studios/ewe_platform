---
description: "OverlayFileSystem — cross-platform virtual filesystem with trait-based abstraction, overlay composition (read-only base + pluggable delta store), copy-on-write, whiteout semantics, and transparent mounting adapters (FUSE, NFS, ptrace)."
status: "in-progress"
priority: "high"
created: 2026-06-04
updated: 2026-06-09
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - filesystem
    - vfs
    - overlay
    - cross-platform
    - fuse
    - wasm
    - wasi
    - foundation
has_features: true
has_fundamentals: false
builds_on:
  - "specifications/34-native-file-watchers"
related_specs:
  - "specifications/34-native-file-watchers"
tasks:
  completed: 18
  uncompleted: 7
  total: 25
  completion_percentage: 72%
---

# OverlayFileSystem — Cross-Platform Virtual File System

## Overview

Build a trait-based virtual filesystem abstraction where the traits are the product. Three core handle types (VfsFile, SeekableVfsFile, VfsDirectory) and two core system traits (VfsFileSystem, DeltaStore) define all filesystem operations. Everything else — native passthrough, in-memory storage, SQLite, S3, FUSE, ptrace — is an implementation that plugs in.

The central composition primitive is `OverlayFileSystem<B, D>` — an overlay that combines a read-only base filesystem with a writable delta store, providing copy-on-write semantics, whiteout-based deletion tracking, and directory entry merging.

## Research Sources

- **AgentFS** (explored: `/home/darkvoid/Boxxed/@dev/repo-expolorations/src.AgentSandbox/src.agentfs/markdown`): SQLite-backed overlay VFS — copy-on-write, whiteout tombstones, directory entry merging, FUSE/NFS mounting, ptrace syscall interception, 4KB chunk storage
- **iii-filesystem** (explored: `/home/darkvoid/Boxxed/@dev/repo-expolorations/src.AgentSandbox/src.iii-filesystem/markdown`): Production FUSE passthrough — zero-copy I/O, synthetic inode table, path containment via `openat2(RESOLVE_BENEATH)`, cross-platform errno/flag translation, dual-key inode map

## Core Design Principles

1. **Trait-first** — VfsFile, SeekableVfsFile, VfsDirectory, VfsFileSystem, DeltaStore are the product
2. **Files and directories are distinct types** — no conflation, each has its own capabilities
3. **Open model: enum + methods** — `OpenMode` enum (Read/Write/ReadWrite) × method (`open`/`open_seekable`/`open_directory`)
4. **Async-first, sync wraps** — implementations are `async fn`. Sync API wraps async via valtron execution. No tokio — runtime is always at the edge.
5. **Permissions are metadata** — always stored/retrieved, enforcement is implementation's business
6. **Checksums always present** — blake3 by default, implementation may vary
7. **DeltaStore = VfsFileSystem + whiteouts + lifecycle** — any filesystem can be a delta store
8. **Default impls with override** — `copy`, `remove_all`, `mkdir_all`, `read_file`, `write_file` have defaults, implementations optimize
9. **Concurrency pushed to implementation** — traits require `Send + Sync`, implementation decides locking strategy
10. **Symlinks followed during CoW** — overlay follows symlinks by default
11. **CoW whole-file at trait level** — implementations may chunk/CAS/diff internally
12. **Event emission optional** — delta mutations emit events via Broadcaster, reported via `capabilities()`
13. **Errors via foundation_errstack** — `VfsError` with trace info, `Unsupported` variant for unimplemented ops

## Feature Index

| Feature | Description | Phase | Status |
|---------|-------------|-------|--------|
| [00-schema-derive-codegen](features/00-schema-derive-codegen/) | ArrowSchema, ArrowJsonSchema, JsonSchema derives in foundation_macros + foundation_codegentools codegen binary | 3 | done |
| [01-core-traits](features/01-core-traits/) | VfsFile, SeekableVfsFile, VfsDirectory, VfsFileSystem, DeltaStore traits + types + errors | 1 | done |
| [02-memory-impls](features/02-memory-impls/) | MemoryFs (VfsFileSystem) + MemoryDelta (DeltaStore) — in-memory, all platforms | 1 | done |
| [03-foundation-fs](features/03-foundation-fs/) | OverlayFileSystem\<B, D\> overlay — CoW, whiteouts, directory merging | 1 | done |
| [04-native-fs](features/04-native-fs/) | NativeFs passthrough — std::fs, path containment, Arc\<PathBuf\> root | 2 | done |
| [05-directory-delta](features/05-directory-delta/) | DirectoryDelta — shadow directory with sentinel whiteout files | 2 | done |
| [06-sqlite-delta](features/06-sqlite-delta/) | LibsqlDelta — libsql-backed delta store, feature-gated | 3 | done |
| [18-scaffold-macro](features/18-scaffold-macro/) | #[scaffold] derive macro — trait impl delegation for wrapper types | 3 | done |
| [20-async-first-migration](features/20-async-first-migration/) | Async trait counterparts for all VFS traits, SyncFs\<A\> bridge, exec_async centralized | 3 | done |
| [21-seekable-sync-cleanup](features/21-seekable-sync-cleanup/) | Remove Arc\<Mutex\<Option\<A\>\>\> from SyncSeekableFile, per-backend seekable wrappers | 3 | done |
| [22-vfs-valtron-tests](features/22-vfs-valtron-tests/) | Comprehensive valtron-backed integration tests — 120 tests, all sync bridge paths | 3 | done |
| [12-arrow-serialization](features/12-arrow-serialization/) | foundation_arrow crate — Arrow zero-copy serialization for any type, derive macro, IPC streaming | 3 | done |
| [07-fuse-adapter](features/07-fuse-adapter/) | FuseMount — FUSE adapter exposing VfsFileSystem as mount (Linux) | 4 | done |
| [23-inode-native-vfs](features/23-inode-native-vfs/) | Inode-native VFS — push inode awareness into VfsMetadata/VfsDirEntry, inode-to-path reverse lookup | 4 | done |
| [11-ipc-daemon](features/11-ipc-daemon/) | VfsDaemon + VfsClient — host VfsFileSystem over IPC bus, client library, binary | 4 | done |
| [09-ptrace-interceptor](features/09-ptrace-interceptor/) | PtraceInterceptor — dual-backend ptrace syscall interception, nix + reverie backends (Linux) | 4 | in-progress |
| [10-valtron-integration](features/10-valtron-integration/) | VfsTask — event emission via Broadcaster, spec-34 watcher integration | 5 | done |
| [15-observable-fs](features/15-observable-fs/) | ObservableFs — decorator wrapping any VfsFileSystem, full audit event emission for ALL operations | 5 | done |
| [24-scaffold-macro-clippy](features/24-scaffold-macro-clippy/) | scaffold!() clippy warning suppression — never-type technique from todo!() | 5 | done |
| [19-fjall-cacache-vfs](features/19-fjall-cacache-vfs/) | FjallFs/FjallDelta — fjall LSM-tree + cacache CAS, SCRU128 keys, hierarchical prefix index | 3 | pending |
| [08-nfs-adapter](features/08-nfs-adapter/) | NfsMount — NFS v3 loopback adapter (macOS, Linux) | 4 | pending |
| [16-ld-preload-shim](features/16-ld-preload-shim/) | LD_PRELOAD/DYLD_INSERT_LIBRARIES shim — libc interception, redirects to VFS daemon | 5 | pending |
| [13-cloudflare-d1-delta](features/13-cloudflare-d1-delta/) | D1Delta — Cloudflare D1 (edge SQLite) as DeltaStore | 5 | pending |
| [14-cloudflare-r2-delta](features/14-cloudflare-r2-delta/) | R2Delta — Cloudflare R2 (S3-compatible) as DeltaStore | 5 | pending |
| [25-examples-showcase](features/25-examples-showcase/) | Examples showcase — programmatic API walkthroughs for every VFS and IPC API | 6 | pending |
| [17-projfs-windows](features/17-projfs-windows/) | ProjFS — Windows 10+ native projection, provider callbacks, auto-promote on write | 6 | deferred |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     User Code                            │
│                                                          │
│  let fs = OverlayFileSystem::new(NativeFs::new("./project"),  │
│                             MemoryDelta::new());         │
│  let file = fs.open("/src/main.rs", Read)?;              │
│  let dir = fs.open_directory("/src")?;                   │
│       │                                                  │
│       ▼                                                  │
│  OverlayFileSystem<B, D> (overlay composition)                │
│       │                                                  │
│       ├── Lookup: whiteout? → delta? → base? → NotFound  │
│       ├── Read: serve from delta or passthrough base     │
│       ├── Write: CoW from base to delta, then write      │
│       ├── Delete: whiteout tombstone                     │
│       └── Readdir: merge delta ∪ base - whiteouts        │
│               │                     │                    │
│               ▼                     ▼                    │
│         DeltaStore              VfsFileSystem             │
│         (writable)              (read-only base)          │
│         ┌─────────┐            ┌──────────┐              │
│         │MemoryΔ  │            │ NativeFs │              │
│         │SqliteΔ  │            │ MemoryFs │              │
│         │DirΔ     │            │ WasiFs   │              │
│         │S3Δ      │            │ etc.     │              │
│         └─────────┘            └──────────┘              │
│                                                          │
│  Mount Adapters (transparent access for external tools)  │
│  ┌──────┐  ┌──────┐  ┌─────────┐                        │
│  │ FUSE │  │ NFS  │  │ ptrace  │                        │
│  └──────┘  └──────┘  └─────────┘                        │
└──────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/shared/vfs/            # Always compiled, cross-platform
    mod.rs                 # Re-exports
    traits.rs              # VfsFileSystem, VfsFile, SeekableVfsFile, VfsDirectory, DeltaStore
    async_traits.rs        # Async counterparts of all traits
    types.rs               # VfsMetadata, VfsDirEntry, OpenMode, VfsCapabilities, Checksum
    error.rs               # VfsError via foundation_errstack
    overlay_fs.rs          # OverlayFileSystem<B, D>
    memory_fs.rs           # MemoryFs
    memory_delta.rs        # MemoryDelta

src/native/vfs/            # Feature-gated, OS-specific
    native_fs.rs           # NativeFs passthrough
    dir_delta.rs           # DirectoryDelta
    fuse.rs                # FUSE adapter (Linux)
    nfs.rs                 # NFS adapter (macOS)
    ptrace.rs              # Ptrace interceptor (Linux)

src/valtron/
    vfs_task.rs            # VfsTask + event emission
```

## Feature Flags

```toml
vfs = []                     # Core traits + OverlayFileSystem + MemoryFs/MemoryDelta (zero new deps)
vfs-native = ["vfs"]         # NativeFs + DirectoryDelta
vfs-sqlite = ["vfs"]         # SqliteDelta (adds sqlite dep)
vfs-fuse = ["vfs-native"]   # Linux FUSE (adds fuser dep)
vfs-nfs = ["vfs-native"]    # macOS NFS (adds nfsserve dep)
vfs-ptrace = ["vfs-native"] # Linux ptrace (adds reverie dep)
```

## Language Stack

- **Rust** — all implementation

## Success Criteria

- [x] Core traits compile on all targets (native, wasm32-unknown-unknown, wasm32-wasi)
- [x] MemoryFs + MemoryDelta pass full CRUD test suite
- [x] OverlayFileSystem overlay passes CoW, whiteout, and directory merging tests
- [x] NativeFs passthrough reads real files with path containment
- [x] DirectoryDelta persists writes to shadow directory
- [x] OverlayFileSystem<NativeFs, DirectoryDelta> end-to-end: mount dir, read existing, write new, delete, rollback
- [x] FUSE adapter mounts and serves files to external processes (Linux)
- [x] Event emission delivers delta change events via Broadcaster
- [x] IPC daemon hosts VfsFileSystem over IPC bus with client library
- [x] ObservableFs decorator provides full audit event emission
- [x] Inode-native VFS with inode-to-path reverse lookup
- [x] Arrow zero-copy serialization with derive macro
- [x] Async-first traits with SyncFs bridge
- [ ] Ptrace interceptor: reverie backend, remaining edge cases and integration tests
- [ ] NFS v3 loopback adapter (macOS)
- [ ] FjallFs/FjallDelta LSM-tree + CAS backend

## Module References

- `backends/foundation_nativeapis/` — target crate
- `backends/foundation_core/src/valtron/` — valtron task infrastructure
- `backends/foundation_errstack/` — error stack utilities

---

_Created: 2026-06-04_
