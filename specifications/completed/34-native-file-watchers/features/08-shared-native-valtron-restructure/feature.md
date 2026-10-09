---
feature: "Shared / Native / Valtron Module Restructure"
description: "Restructure foundation_nativeapis into shared/ (always compiled), native/ (platform-specific, feature-gated), and valtron/ (executor integration, split into shared + native sub-modules). FdMonitorTask uses TaskStatus::Depends instead of Delayed."
status: "completed"
priority: "high"
depends_on: ["07-valtron-task-design"]
estimated_effort: "medium"
created: 2026-06-03
last_updated: 2026-06-03
author: "Main Agent"
tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
---

# Feature: Shared / Native / Valtron Module Restructure

## Problem

The `foundation_nativeapis` layout mixed cross-platform types with platform-specific syscalls at the same level. A flat `src/` namespace with `watcher`/`task` feature gates hid purely-stdlib types behind unnecessary feature flags.

Consequences:
- `SharedWatcher`, `NativeWatcher` trait, `PollWatcher` were pure stdlib but hidden behind `feature = "watcher"`
- `WatchError`, `WatchEvent`, `NativeAPI`, `WatcherBuilder` were always relevant but co-located with gated code
- Valtron tasks (`StopSignal`, `EventBroadcaster`) are **shareable** — they don't need inotify/epoll/kqueue, yet lived inside `task/`
- `fd_monitor.rs` was the only valtron type needing `native::fd` — blocking the rest from being shared
- `FdMonitorTask` used `TaskStatus::Delayed` — polling every N ms even when the fd was idle

## Solution

Three top-level submodules, with `valtron/` further split into shared vs native.

### 1. `shared/` — Always compiled, no platform dependencies

| Module | Contents |
|--------|----------|
| `shared::error` | `WatchError`, `Result<T>` |
| `shared::event` | `WatchEvent`, `WatchEventKind` |
| `shared::watcher` | `NativeWatcher` trait, `PollWatcher`, `SharedNativeWatcher<T>`, `SharedWatcher` |
| `shared::api` | `NativeAPI`, `WatcherBuilder`, `native_watcher()` |

### 2. `native/` — Feature-gated, platform-specific

| Module | Feature Gate | Platform |
|--------|-------------|----------|
| `native::poll` | `poll` | epoll, kqueue, IOCP |
| `native::fd` | `fd` (implies `poll`) | Unix only |
| `native::net` | `poll` | Unix only |
| `native::watcher` | `watcher-linux/macos/windows` | inotify, kqueue EVFILT_VNODE, ReadDirectoryChangesW |

### 3. `valtron/` — Feature-gated, split shared vs native

| Module | Contents | Depends on |
|--------|----------|------------|
| `valtron/broadcaster` | `EventBroadcaster<T>` | `foundation_core::synca::mpp` |
| `valtron/file_watcher` | `FileWatcherTask` | `shared::watcher`, `shared::api` |
| `valtron/stop_signal` | `StopSignal`, `CompositeReadiness` | `foundation_core::valtron::EventReadiness` |
| `valtron/native/fd_monitor` | `FdMonitorTask<T>` | `native::fd`, `native::poll` (behind `#[cfg(feature = "fd")]`) |

**Key fix:** `FdMonitorTask` now uses `TaskStatus::Depends(fd, stop_signal)` instead of `TaskStatus::Delayed(poll_interval)`. The executor parks the task and only wakes it when the OS signals fd readiness (epoll/kqueue). Zero wasted wakeups.

## Cargo.toml

```toml
[features]
default = ["task", "native"]

poll = []
fd = ["poll"]
watcher-linux = ["poll", "dep:inotify"]
watcher-macos = ["poll"]
watcher-windows = ["poll"]
uring = ["dep:io-uring"]
task = ["fd"]
ipc = ["dep:bincode", "dep:type-uuid"]

native = ["poll", "fd"]
native-linux = ["native", "watcher-linux", "uring"]
native-macos = ["native", "watcher-macos"]
native-windows = ["native", "watcher-windows"]
```

Note: `watcher` feature is **removed**. `PollWatcher` is always available. `task` no longer implies `watcher` — it implies `fd`.

## Directory Layout

```
src/
├── lib.rs                          # Declares shared, native, valtron; re-exports
├── shared/                         # Always compiled
│   ├── mod.rs
│   ├── error.rs
│   ├── event.rs
│   ├── api.rs
│   └── watcher/
│       ├── mod.rs                  # NativeWatcher trait + re-exports
│       ├── poll_watcher.rs         # stdlib-only polling
│       └── shared.rs               # SharedNativeWatcher, SharedWatcher
├── native/                         # Feature-gated, platform-specific
│   ├── mod.rs
│   ├── poll/                       # epoll/kqueue/IOCP
│   ├── fd/                         # Unix RegisteredFd
│   ├── net/                        # Unix TCP/UDP/UnixStream
│   └── watcher/
│       ├── mod.rs
│       ├── linux.rs                # inotify (watcher-linux)
│       ├── unix.rs                 # kqueue EVFILT_VNODE (watcher-macos)
│       └── windows.rs              # ReadDirectoryChangesW (watcher-windows)
└── valtron/                        # Feature-gated (task)
    ├── mod.rs                      # Re-exports shared + gates native
    ├── broadcaster.rs              # shared — EventBroadcaster
    ├── file_watcher.rs             # shared — FileWatcherTask
    ├── stop_signal.rs              # shared — StopSignal, CompositeReadiness
    └── native/
        ├── mod.rs                  # #[cfg(feature = "fd")]
        └── fd_monitor.rs           # native — FdMonitorTask
```

## Crate-Root Re-exports

```rust
// Always available
pub use shared::{
    native_watcher, NativeAPI, NativeWatcher, PollWatcher,
    SharedNativeWatcher, SharedWatcher, WatchError,
    WatchEvent, WatchEventKind, WatcherBuilder, Result,
};

// Feature-gated
#[cfg(feature = "poll")]
pub use native::poll::{Events, Interest, Poll, Registry, SourceFd, Token, Waker};

#[cfg(feature = "task")]
pub use valtron::{EventBroadcaster, FdMonitorTask, FileWatcherTask,
                  StopSignal, CompositeReadiness};
```

## Tasks

### Task 1: Move shared types to `shared/`
- [x] Create `src/shared/mod.rs`, move `error.rs`, `event.rs` into it
- [x] Create `src/shared/watcher/` with `mod.rs` (trait), `poll_watcher.rs`, `shared.rs`
- [x] Move `api.rs` → `shared/api.rs`, update imports
- [x] Update all `crate::error::`, `crate::event::`, `crate::watcher::`, `crate::api::` → `crate::shared::`

### Task 2: Move platform-specific code to `native/`
- [x] Create `src/native/mod.rs`
- [x] Move `poll/` → `native/poll/`, `fd/` → `native/fd/`, `net/` → `native/net/`
- [x] Move `watcher/linux.rs`, `unix.rs`, `windows.rs` → `native/watcher/`
- [x] Create `native/watcher/mod.rs` with platform cfg gates
- [x] Update all internal imports within native

### Task 3: Create `valtron/` module from `task/`, split shared vs native
- [x] Create `src/valtron/mod.rs` with shared types
- [x] Split into: `broadcaster.rs`, `file_watcher.rs`, `stop_signal.rs` (shared)
- [x] Move `fd_monitor.rs` → `valtron/native/fd_monitor.rs` (native, behind `#[cfg(feature = "fd")]`)
- [x] Update imports: `native::fd` for FdMonitorTask, `shared::watcher` for FileWatcherTask

### Task 4: Update `lib.rs` with new module declarations and re-exports
- [x] Declare `pub mod shared; pub mod native; pub mod valtron;`
- [x] Add cfg-gated re-exports at crate root for backward compatibility
- [x] Remove old flat module declarations

### Task 5: Update `Cargo.toml` feature gates
- [x] Remove `watcher` feature (PollWatcher always available)
- [x] Make `watcher-linux/macos/windows` imply `poll`
- [x] Change `task` from `["watcher", "fd"]` → `["fd"]`
- [x] Set `default = ["task", "native"]` for sensible out-of-the-box behavior
- [x] Update `native*` composite features

### Task 6: FdMonitorTask use Depends instead of Delayed
- [x] Replace `TaskStatus::Delayed(poll_interval)` with `TaskStatus::Depends(CompositeReadiness(fd, stop_signal))`
- [x] Store fd behind `Arc<RegisteredFd<T>>` for `Arc<dyn EventReadiness>` cast
- [x] Remove unused `poll_interval` field and `with_poll_interval()` method
- [x] Add `Send + Sync + 'static` bounds for trait object casting

### Task 7: Verify all tests pass
- [x] `cargo check -p foundation_nativeapis` (no features) — clean
- [x] `cargo test -p foundation_nativeapis --features task,watcher-linux` — 53 tests pass
- [x] Fix all broken imports and paths across 7 test files

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
| `crate::task::FdMonitorTask` | `crate::valtron::native::FdMonitorTask` |

---

_Created: 2026-06-03_
