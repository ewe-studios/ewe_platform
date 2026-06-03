---
feature: "Shared / Native / Valtron Module Restructure"
description: "Restructure foundation_nativeapis into shared/ (always compiled, WASM-compatible), native/ (platform-specific, feature-gated), and valtron/ (executor integration, feature-gated but platform-agnostic). Valtron tasks are shareable — they don't need native OS APIs, only the valtron executor model."
status: "completed"
priority: "high"
depends_on: ["07-valtron-task-design"]
estimated_effort: "medium"
created: 2026-06-03
last_updated: 2026-06-03
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Feature: Shared / Native / Valtron Module Restructure

## Problem

The current `foundation_nativeapis` layout mixes cross-platform types with platform-specific syscalls at the same level. Everything shares a flat `src/` namespace and `watcher`/`task` features gate entire modules — but these modules contain a mix of code that *could* and *should* compile everywhere versus code that *must* be platform-specific.

Consequences:
- `SharedWatcher`, `NativeWatcher` trait, `PollWatcher` are pure stdlib but hidden behind `feature = "watcher"`
- `WatchError`, `WatchEvent`, `NativeAPI`, `WatcherBuilder` are always relevant but co-located with gated code
- Valtron tasks (`StopSignal`, `CompositeReadiness`, `EventBroadcaster`) are **shareable** — they don't need inotify/epoll/kqueue. Yet they live inside `task/` which implies "native"
- `fd/` and `net/` are Unix-only but share the crate root namespace with `error.rs` and `event.rs`
- WASM consumers can't use the generic types without pulling in platform-specific feature gates

## Solution

Restructure into three top-level submodules, each with a clear contract:

### 1. `shared/` — Always compiled, no platform dependencies

Types that work everywhere (native, WASM, any `#[cfg]`). Pure stdlib + `thiserror`/`serde`/`foundation_core::valtron::EventReadiness`.

| Module | Contents | Why shared |
|--------|----------|------------|
| `shared::error` | `WatchError`, `Result<T>` | Pure error enum, no syscalls |
| `shared::event` | `WatchEvent`, `WatchEventKind` | Pure data types |
| `shared::watcher` | `NativeWatcher` trait, `PollWatcher` (stdlib-only), `SharedNativeWatcher<T>`, `SharedWatcher` | Trait is an interface; PollWatcher uses only `std::fs::metadata`; shared wrappers are generic |
| `shared::api` | `NativeAPI` enum, `WatcherBuilder`, `native_watcher()` | `WatcherBuilder` dispatches to native watchers behind cfg, but the *builder itself* is cross-platform — it just returns `UnsupportedPlatform` when the native backend isn't available |

### 2. `native/` — Feature-gated, platform-specific APIs

Code that requires OS-specific syscalls (epoll, kqueue, IOCP, inotify, `RawFd`, etc.).

| Module | Feature Gate | Platform |
|--------|-------------|----------|
| `native::poll` | `poll` | epoll (Linux), kqueue (macOS/BSD), IOCP/WSAPoll (Windows), shell stub (unsupported) |
| `native::fd` | `fd` (implies `poll`) | Unix only (`RawFd`, `OwnedFd`) |
| `native::net` | `poll` | Unix only (`TcpStream`, `UdpSocket`, `UnixStream` via `AsRawFd`) |
| `native::watcher` | `watcher-linux`, `watcher-macos`, `watcher-windows` | `linux.rs` (inotify), `unix.rs` (kqueue EVFILT_VNODE), `windows.rs` (ReadDirectoryChangesW) |

**Critical rule:** Nothing in `native/` is imported by `shared/`. The dependency direction is one-way: `native/` imports from `shared/`.

### 3. `valtron/` — Feature-gated, but platform-agnostic

Valtron executor integration. These types are **shareable** — they work with *any* `EventReadiness` impl and don't call OS syscalls directly. They depend on `foundation_core::valtron` (the executor model) but not on `native/`.

| Module | Feature Gate | Contents |
|--------|-------------|----------|
| `valtron::stop_signal` | `task` | `StopSignal`, `CompositeReadiness` — generic readiness combinators |
| `valtron::broadcaster` | `task` | `EventBroadcaster<T>` — multi-subscriber mpp channel broadcaster |
| `valtron::file_watcher` | `task` | `FileWatcherTask` — wraps `SharedWatcher` + broadcaster into a `TaskIterator` |
| `valtron::fd_monitor` | `task` | `FdMonitorTask<T>` — wraps `RegisteredFd` (this one *does* need `native::fd` for the fd type, but the task logic itself is generic) |

**Key insight:** `StopSignal` and `CompositeReadiness` are so generic they could be in `foundation_core` itself. But they live here because they're primarily used by watcher tasks. `EventBroadcaster` is even more generic — it's just `Vec<Sender<T>>` with a `broadcast` method.

## Feature Gates (new Cargo.toml layout)

```toml
[features]
default = []

# I/O readiness layer — extracted from mio
poll = []

# File descriptor readiness tracking
fd = ["poll"]

# File watching layer (PollWatcher always available via shared)
# Platform-specific watchers:
watcher-linux = ["poll", "dep:inotify"]
watcher-macos = ["poll"]
watcher-windows = ["poll"]

# io_uring layer (Linux only)
uring = ["dep:io-uring"]

# Valtron task integration (shareable, not native)
task = ["fd"]

# Interprocess message bus
ipc = ["dep:bincode", "dep:type-uuid"]

# Full feature set for current platform
native = ["poll", "fd"]
native-linux = ["native", "watcher-linux", "uring"]
native-macos = ["native", "watcher-macos"]
native-windows = ["native", "watcher-windows"]
```

Note: `watcher` feature is **removed**. `PollWatcher` is always available via `shared::watcher`. Platform-specific watchers are individually feature-gated. `task` no longer implies `watcher` — it implies `fd` (for FdMonitorTask).

## New Directory Layout

```
src/
├── lib.rs                          # Declares shared, native, valtron; re-exports
├── shared/                         # Always compiled
│   ├── mod.rs
│   ├── error.rs                    # (moved from src/error.rs)
│   ├── event.rs                    # (moved from src/event.rs)
│   ├── watcher/                    # (moved from src/watcher/)
│   │   ├── mod.rs                  # NativeWatcher trait
│   │   ├── poll_watcher.rs         # stdlib-only polling
│   │   └── shared.rs               # SharedNativeWatcher, SharedWatcher
│   └── api.rs                      # WatcherBuilder, NativeAPI
├── native/                         # Feature-gated, platform-specific
│   ├── mod.rs
│   ├── poll/                       # (moved from src/poll/)
│   ├── fd/                         # (moved from src/fd/)
│   ├── net/                        # (moved from src/net/)
│   └── watcher/                    # Platform-specific watcher impls
│       ├── mod.rs
│       ├── linux.rs                # (moved from src/watcher/linux.rs)
│       ├── unix.rs                 # (moved from src/watcher/unix.rs)
│       └── windows.rs              # (moved from src/watcher/windows.rs)
└── valtron/                        # Feature-gated, shareable
    ├── mod.rs
    ├── stop_signal.rs              # (moved from src/task/stop_signal.rs)
    ├── broadcaster.rs              # (moved from src/task/mod.rs)
    ├── file_watcher.rs             # (moved from src/task/mod.rs)
    └── fd_monitor.rs               # (moved from src/task/fd_monitor.rs)
```

## Crate-Root Re-exports (Backward Compatibility)

The `lib.rs` re-exports everything at the crate root so existing imports don't break:

```rust
// Always available
pub use shared::{NativeWatcher, SharedWatcher, SharedNativeWatcher,
                 WatchError, WatchEvent, WatchEventKind,
                 NativeAPI, WatcherBuilder, native_watcher, Result};

// Feature-gated
#[cfg(feature = "poll")]
pub use native::poll::{Events, Interest, Poll, Registry, SourceFd, Token, Waker};

#[cfg(feature = "task")]
pub use valtron::{EventBroadcaster, FdMonitorTask, FileWatcherTask, StopSignal, CompositeReadiness};
```

## Tasks

### Task 1: Move shared types to `shared/`

- [ ] Create `src/shared/mod.rs`, move `error.rs`, `event.rs` into it
- [ ] Create `src/shared/watcher/` with `mod.rs` (trait), `poll_watcher.rs`, `shared.rs`
- [ ] Move `api.rs` → `shared/api.rs`, update imports
- [ ] Update all `crate::error::`, `crate::event::`, `crate::watcher::`, `crate::api::` → `crate::shared::`

### Task 2: Move platform-specific code to `native/`

- [ ] Create `src/native/mod.rs`
- [ ] Move `poll/` → `native/poll/`, `fd/` → `native/fd/`, `net/` → `native/net/`
- [ ] Move `watcher/linux.rs`, `unix.rs`, `windows.rs` → `native/watcher/`
- [ ] Create `native/watcher/mod.rs` with platform cfg gates
- [ ] Update all internal `crate::` → correct `super::` paths within native

### Task 3: Create `valtron/` module from `task/`

- [ ] Create `src/valtron/mod.rs`
- [ ] Split `task/mod.rs` into: `valtron/stop_signal.rs`, `valtron/broadcaster.rs`, `valtron/file_watcher.rs`
- [ ] Move `task/fd_monitor.rs` → `valtron/fd_monitor.rs`
- [ ] Update imports: `native::fd` for FdMonitorTask, `shared::watcher` for FileWatcherTask

### Task 4: Update `lib.rs` with new module declarations and re-exports

- [ ] Declare `pub mod shared; pub mod native; pub mod valtron;`
- [ ] Add cfg-gated re-exports at crate root for backward compatibility
- [ ] Remove old flat module declarations

### Task 5: Update `Cargo.toml` feature gates

- [ ] Remove `watcher` feature (PollWatcher is always available)
- [ ] Make `watcher-linux/macos/windows` imply `poll` (for the poll selector)
- [ ] Change `task` from `["watcher", "fd"]` → `["fd"]` (no longer needs watcher feature)
- [ ] Update `native*` composite features

### Task 6: Verify all tests pass

- [ ] `cargo check -p foundation_nativeapis` (no features)
- [ ] `cargo test -p foundation_nativeapis --features task,watcher-linux`
- [ ] Fix any broken imports or paths

## Import Path Migration

| Old Path | New Path |
|----------|----------|
| `crate::error::Result` | `crate::shared::error::Result` |
| `crate::event::WatchEvent` | `crate::shared::event::WatchEvent` |
| `crate::watcher::NativeWatcher` | `crate::shared::watcher::NativeWatcher` |
| `crate::watcher::PollWatcher` | `crate::shared::watcher::PollWatcher` |
| `crate::watcher::SharedWatcher` | `crate::shared::watcher::SharedWatcher` |
| `crate::api::native_watcher` | `crate::shared::api::native_watcher` |
| `crate::poll::Poll` | `crate::native::poll::Poll` |
| `crate::fd::RegisteredFd` | `crate::native::fd::RegisteredFd` |
| `crate::task::FileWatcherTask` | `crate::valtron::FileWatcherTask` |
| `crate::task::StopSignal` | `crate::valtron::StopSignal` |

---

_Created: 2026-06-03_
