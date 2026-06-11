# foundation_nativeapis

Cross-platform native APIs for I/O readiness, file watching, interprocess messaging, **and** a trait-based virtual filesystem with overlay composition, pluggable delta stores, and transparent mount adapters.

## Overview

This crate provides two major subsystems:

| Subsystem | What it provides |
|-----------|-----------------|
| **Native APIs** | epoll/kqueue/IOCP poll selector, file descriptor readiness, inotify/kqueue file watchers, Unix socket IPC bus |
| **VFS** | Trait-based virtual filesystem (`VfsFileSystem`, `DeltaStore`), overlay composition (`OverlayFileSystem`), 10+ backends (memory, native, SQLite, fjall, Cloudflare D1/R2), mount adapters (FUSE, NFS, ptrace, LD_PRELOAD) |

Both subsystems integrate with the valtron executor for async execution — no tokio dependency, runtime is always at the edge.

---

## Feature Flags

### Native API Features

| Feature | What it provides | Platforms |
|---------|-----------------|-----------|
| `native` | poll selector + fd readiness (default) | All |
| `poll` | I/O readiness selector only | All |
| `fd` | File descriptor readiness tracking (`RegisteredFd`, `ReadyGuard`) | Unix |
| `watcher-linux` | inotify file watcher | Linux |
| `watcher-macos` | kqueue file watcher (EVFILT_VNODE) | macOS, BSD |
| `watcher-windows` | ReadDirectoryChangesW file watcher | Windows |
| `ipc` | Unix socket interprocess message bus (bincode) | Unix |
| `uring` | io_uring async I/O layer | Linux 5.1+ |

### VFS Features

| Feature | What it provides | Platforms |
|---------|-----------------|-----------|
| `vfs` | Core traits, types, errors, `MemoryFs`, `MemoryDelta` | All (incl. WASM) |
| `vfs-native` | `NativeFs` passthrough, `DirectoryDelta` | Linux, macOS, Windows |
| `vfs-sqlite` | `LibsqlDelta` — libsql-backed delta store | All (incl. WASI) |
| `vfs-fjall` | `FjallFs`/`FjallDelta` — fjall LSM-tree + cacache CAS | Linux, macOS, Windows |
| `vfs-fuse` | `FuseMount` — FUSE adapter | Linux |
| `vfs-nfs` | `VfsNfs` — NFS v3 loopback adapter | Linux, macOS |
| `vfs-ptrace` | `PtraceInterceptor` — syscall interception | Linux |
| `vfs-turso` | `TursoDelta` — Turso edge SQLite delta store | All |
| `vfs-d1` | `D1Delta` — Cloudflare D1 delta store | All (via HTTP) |
| `vfs-r2` | `R2Delta` — Cloudflare R2 delta store | All (via HTTP) |
| `vfs-ipc` | `VfsDaemon` + `VfsClient` — IPC bus for VFS | All |

### Convenience Stacks

```toml
# Minimal: poll + fd readiness
foundation_nativeapis = "0.0.1"

# Linux native stack
foundation_nativeapis = { version = "0.0.1", features = ["native-linux"] }

# VFS core + native backends
foundation_nativeapis = { version = "0.0.1", features = ["vfs", "vfs-native"] }

# Full VFS stack with FUSE + SQLite
foundation_nativeapis = { version = "0.0.1", features = ["vfs", "vfs-native", "vfs-sqlite", "vfs-fuse"] }

# Cloudflare edge stack
foundation_nativeapis = { version = "0.0.1", features = ["vfs", "vfs-d1", "vfs-r2"] }
```

---

## Module Structure

```
src/
├── shared/                    # Always compiled — cross-platform types
│   ├── vfs/                   # VFS subsystem
│   │   ├── mod.rs             # Re-exports
│   │   ├── traits.rs          # VfsFileSystem, VfsFile, SeekableVfsFile, VfsDirectory, DeltaStore
│   │   ├── async_traits.rs    # Async counterparts of all VFS traits
│   │   ├── sync_bridge.rs     # SyncFs<A> — valtron-backed sync wrapper
│   │   ├── types.rs           # VfsMetadata, VfsDirEntry, OpenMode, VfsCapabilities, Checksum
│   │   ├── error.rs           # VfsError via foundation_errstack
│   │   ├── overlay_fs.rs      # OverlayFileSystem<B, D> — CoW, whiteouts, directory merging
│   │   ├── memory_fs.rs       # MemoryFs — in-memory VfsFileSystem
│   │   ├── memory_delta.rs    # MemoryDelta — in-memory DeltaStore
│   │   ├── observable_fs.rs   # ObservableFs — audit event decorator
│   │   └── inode.rs           # Inode-native VFS support
│   ├── error.rs               # WatchError, Result<T>
│   ├── event.rs               # WatchEvent, WatchEventKind
│   ├── fd_state.rs            # FdState enum
│   ├── watcher/               # NativeWatcher trait, PollWatcher, SharedWatcher
│   └── api.rs                 # WatcherBuilder
├── native/                    # Feature-gated — platform-specific implementations
│   ├── poll/                  # epoll / kqueue / IOCP selector
│   ├── fd/                    # RegisteredFd, ReadyGuard, edge-triggered readiness
│   ├── net/                   # TcpStream, UdpSocket, UnixStream wrappers
│   ├── watcher/               # InotifyWatcher, KqueueWatcher, WinWatcher
│   └── vfs/                   # VFS backends
│       ├── native_fs.rs       # NativeFs — host filesystem passthrough
│       ├── dir_delta.rs      # DirectoryDelta — shadow directory with whiteouts
│       ├── fuse.rs            # FuseMount — FUSE adapter (Linux)
│       ├── nfs.rs             # VfsNfs — NFS v3 loopback
│       ├── ptrace/            # PtraceInterceptor (Linux)
│       ├── d1_delta/          # D1Delta — Cloudflare D1
│       ├── r2_delta/          # R2Delta — Cloudflare R2
│       └── fjall/             # FjallFs / FjallDelta
└── valtron/                   # Always available — executor integration
    ├── broadcaster.rs         # Broadcaster<T> (re-exported from foundation_core)
    ├── file_watcher.rs        # FileWatcherTask, FileWatcherBuilder
    ├── stop_signal.rs         # StopSignal, CompositeReadiness
    ├── vfs_task.rs            # VfsTask — VFS event emission
    └── native/
        └── fd_monitor.rs      # FdMonitorTask
```

---

## Quick Start — Native APIs

### File Watching (Simple)

```rust
use foundation_nativeapis::valtron::FileWatcherBuilder;

let stream = FileWatcherBuilder::new()
    .watch("/path/to/dir", true)?;

for event in stream {
    println!("File changed: {:?}", event.path);
}
```

### FD Readiness

```rust
use foundation_nativeapis::native::fd::{RegisteredFd, PollResult};
use foundation_nativeapis::{Poll, Token, Interest};

let poll = Poll::new()?;
let registered = RegisteredFd::new(my_fd, &poll.registry(), Token(0))?;

match registered.poll_readable() {
    PollResult::Ready(mut guard) => { /* fd is readable */ }
    PollResult::NotReady => { /* wait and retry */ }
    PollResult::Error(e) => { /* fd closed or errored */ }
}
```

---

## Quick Start — VFS

### In-Memory Filesystem

```rust
use foundation_nativeapis::shared::vfs::{MemoryFs, VfsFileSystem};

let fs = MemoryFs::new();
fs.mkdir("/src")?;
fs.write_file("/src/main.rs", b"fn main() {}")?;
let data = fs.read_file("/src/main.rs")?;
```

### Overlay Filesystem (CoW + Whiteouts)

```rust
use foundation_nativeapis::shared::vfs::{
    MemoryFs, MemoryDelta, OverlayFileSystem, VfsFileSystem, DeltaStore,
};

let base = MemoryFs::new();
base.write_file("/template.txt", b"original")?;

let overlay = OverlayFileSystem::new(base, MemoryDelta::new());

// Read-through from base
let data = overlay.read_file("/template.txt")?;
assert_eq!(&data, b"original");

// Copy-on-write: writing creates a delta override
overlay.write_file("/template.txt", b"modified")?;

// Delete: whiteout in delta, base untouched
overlay.remove("/template.txt")?;
assert!(!overlay.exists("/template.txt")?);

// Reset delta: base is visible again
overlay.delta().reset()?;
assert!(overlay.exists("/template.txt")?);
```

### Sync Bridge (valtron-backed sync API)

Async-native backends (SQLite, D1, R2, fjall) only implement async traits.
`SyncFs<A>` wraps them through valtron's executor:

```rust
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;
use foundation_nativeapis::shared::vfs::MemoryDelta;

let sync: SyncFs<MemoryDelta> = SyncFs::new(MemoryDelta::new());
sync.mkdir("/data")?;           // goes through valtron executor
sync.write_file("/data/x.txt", b"hello")?;
```

### Cloudflare Delta Stores

```rust
use foundation_nativeapis::native::vfs::d1_delta::D1Delta;
use foundation_nativeapis::native::vfs::r2_delta::R2Delta;
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;

// D1 (edge SQLite)
let d1 = D1Delta::new(api_token, account_id, database_id)?;
let sync_d1: SyncFs<D1Delta> = SyncFs::new(d1);
sync_d1.write_file("/kv.txt", b"Hello from the edge!")?;

// R2 (S3-compatible object storage)
let r2 = R2Delta::new(api_token, account_id, bucket)?;
let sync_r2: SyncFs<R2Delta> = SyncFs::new(r2);
sync_r2.write_file("/blob.bin", &[0, 1, 2, 3])?;
```

---

## Testing

### Native API Tests

```bash
# Poll + FD tests
cargo test -p foundation_nativeapis --features "poll,fd" --test fd

# File watcher tests (Linux)
cargo test -p foundation_nativeapis --features "watcher-linux" --test watcher

# IPC tests
cargo test -p foundation_nativeapis --features "ipc" --test ipc_vfs
```

### VFS Tests

```bash
# Core VFS (MemoryFs, MemoryDelta, Overlay, async traits, errors)
cargo test -p foundation_nativeapis --features "vfs" --test valtron_vfs

# NativeFs + DirectoryDelta
cargo test -p foundation_nativeapis --features "vfs-native" --test native_vfs --test dir_delta_vfs

# SQLite delta
cargo test -p foundation_nativeapis --features "vfs-sqlite" --test valtron_vfs -- sqlite

# fjall LSM-tree + cacache
cargo test -p foundation_nativeapis --features "vfs-fjall" --test fjall_vfs

# FUSE adapter (Linux)
cargo test -p foundation_nativeapis --features "vfs-fuse" --test fuse_vfs

# NFS adapter
cargo test -p foundation_nativeapis --features "vfs-nfs" --test nfs_vfs

# IPC daemon + client
cargo test -p foundation_nativeapis --features "vfs-ipc" --test ipc_vfs

# Arrow serialization roundtrip
cargo test -p foundation_nativeapis --features "vfs" --test arrow_vfs

# ObservableFs audit events
cargo test -p foundation_nativeapis --features "vfs" --test observable_vfs
```

### Cloudflare Tests (local emulator)

Requires [wrangler](https://developers.cloudflare.com/workers/wrangler/) with `--local` flag:

```bash
# Start local wrangler dev server
mise run cf:start

# Run VFS R2Delta tests
mise run test:vfs:cf:r2

# Run VFS D1Delta tests
mise run test:vfs:cf:d1

# Run all VFS Cloudflare tests
mise run test:vfs:cf

# Stop wrangler
mise run cf:stop
```

Or manually:
```bash
export CF_INTEGRATION_TEST=1
export LOCAL_CF_API_BASE="http://localhost:8789"
export LOCAL_R2_BUCKET="test-bucket"
export LOCAL_D1_DATABASE_ID="test-db"

cargo test -p foundation_nativeapis --features "vfs-r2" --test r2_delta_vfs
cargo test -p foundation_nativeapis --features "vfs-d1" --test d1_delta_vfs
```

Tests skip gracefully when the emulator isn't available — API surface tests always run.

---

## Examples

All examples are runnable with `cargo run -p foundation_nativeapis --features <feat> --example <name>`.
Each has a `README.md` in its directory with full documentation.

| Example | Feature | Description |
|---------|---------|-------------|
| [`memory_fs`](examples/memory_fs/) | `vfs` | In-memory VFS — full CRUD lifecycle |
| [`overlay_fs`](examples/overlay_fs/) | `vfs` | Overlay composition — CoW, whiteouts, merged directory listing |
| [`native_fs`](examples/native_fs/) | `vfs-native` | Host filesystem passthrough with path containment |
| [`sqlite_delta`](examples/sqlite_delta/) | `vfs-sqlite` | SQLite-backed delta store with ACID guarantees |
| [`vfs_sqlite_delta`](examples/vfs_sqlite_delta/) | `vfs-sqlite` | OverlayFileSystem<NativeFs, LibsqlDelta> end-to-end |
| [`vfs_turso_delta`](examples/vfs_turso_delta/) | `vfs-turso` | Turso edge SQLite delta store |
| [`fuse_mount`](examples/fuse_mount/) | `vfs-fuse` | FUSE adapter — mount VFS as Linux mountpoint |
| [`observable_fs`](examples/observable_fs/) | `vfs` | Audit event decorator — full operation logging |
| [`sync_async_bridge`](examples/sync_async_bridge/) | `vfs` | SyncFs<A> — valtron-backed sync API for async backends |
| [`scaffold_macro`](examples/scaffold_macro/) | `vfs` | `#[scaffold]` derive — trait impl delegation for wrapper types |
| [`inode_native`](examples/inode_native/) | `vfs` | Inode-aware VFS — inode-to-path reverse lookup |
| [`ipc_bus`](examples/ipc_bus/) | `vfs-ipc` | VFS daemon + client over Unix socket IPC |
| [`arrow_serialization`](examples/arrow_serialization/) | `vfs` | Arrow zero-copy serialization for VFS types |
| [`vfs_shim`](examples/vfs_shim/) | `vfs` | VFS shim layer — type erasure and dynamic dispatch |
| [`fd_readiness`](examples/fd_readiness/) | `fd` | File descriptor readiness tracking |
| [`file_watcher`](examples/file_watcher/) | `watcher-linux` | Cross-platform file watching via valtron |

---

## VFS Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     User Code                            │
│                                                          │
│  let fs = OverlayFileSystem::new(NativeFs::new("./project"), │
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

### Core Design Principles

1. **Trait-first** — `VfsFileSystem`, `DeltaStore` are the product; implementations plug in
2. **Files and directories are distinct types** — no conflation
3. **Async-first, sync wraps** — implementations are async; `SyncFs<A>` bridges via valtron; no tokio
4. **DeltaStore = VfsFileSystem + whiteouts + lifecycle** — any filesystem can be a delta store
5. **Default impls with override** — `copy`, `remove_all`, `mkdir_all` have defaults
6. **Errors via foundation_errstack** — `VfsError` with trace info
7. **Checksums always present** — blake3 by default

---

## Platform Support

### Native APIs

| Platform | Poll | FD | File Watcher | IPC |
|----------|------|----|-------------|-----|
| Linux | epoll ✓ | ✓ | inotify ✓ | Unix socket ✓ |
| macOS | kqueue ✓ | ✓ | kqueue EVFILT_VNODE ✓ | Unix socket ✓ |
| BSD | kqueue ✓ | ✓ | kqueue EVFILT_VNODE ✓ | Unix socket ✓ |
| Windows | IOCP ✓ | - | ReadDirectoryChangesW ✓ | Named pipe ✓ |
| WASM | shell stub | - | PollWatcher ✓ | - |

### VFS

| Implementation | Linux | macOS | Windows | WASM | WASI |
|---------------|-------|-------|---------|------|------|
| MemoryFs/MemoryDelta | ✓ | ✓ | ✓ | ✓ | ✓ |
| NativeFs | ✓ | ✓ | ✓ | - | ✓ |
| DirectoryDelta | ✓ | ✓ | ✓ | - | ✓ |
| LibsqlDelta | ✓ | ✓ | ✓ | ✓ | ✓ |
| FjallFs/FjallDelta | ✓ | ✓ | ✓ | - | - |
| FuseMount | ✓ | - | - | - | - |
| VfsNfs | ✓ | ✓ | - | - | - |
| PtraceInterceptor | ✓ | - | - | - | - |
| D1Delta/R2Delta | ✓ | ✓ | ✓ | - | ✓ |
| VfsDaemon/VfsClient | ✓ | ✓ | ✓ | - | ✓ |
