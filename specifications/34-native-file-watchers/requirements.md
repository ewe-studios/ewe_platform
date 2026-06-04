---
description: "Build minimal cross-platform file watching using native OS APIs (inotify, kqueue, ReadDirectoryChangesW) in foundation_nativeapis, plus a valtron watcher task that other tasks can start, subscribe to, and receive file events via queues."
status: "pending"
priority: "high"
created: 2026-06-01
updated: 2026-06-01
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - file-watching
    - native-apis
    - inotify
    - kqueue
    - valtron
    - cross-platform
    - pluggable
has_features: true
has_fundamentals: false
builds_on: []
related_specs:
  - "specifications/33-valtron-singleton"
tasks:
  completed: 75
  uncompleted: 7
  total: 82
  completion_percentage: 91%
---

# Native File Watchers + Valtron Integration

## Overview

Build a minimal, fast, cross-platform file watching library backed by native OS primitives — no heavy abstraction layers, no built-in debouncing, no config parsing. Just raw events delivered through a clean pluggable trait.

Then wire it into valtron as a first-class task type so other valtron tasks can start watchers, subscribe to event streams, and react to file changes through queues.

## Known Issues / Limitations

- Previous attempt (`crates/watchers`) was a thin wrapper around `notify` + `notify-debouncer-full` with config parsing and command execution tightly coupled — not reusable, not minimal.
- `notify` carries unnecessary weight (crossbeam, filetime, walkdir, internal debouncers) for our use case.
- No valtron integration existed — file watching was always a separate thread-per-watcher model.

## Feature Index

| Feature | Description | Status |
|---------|-------------|--------|
| [01-native-apis](features/01-native-apis/) | `foundation_nativeapis` crate: `NativeWatcher` trait + platform backends (inotify, kqueue, ReadDirectoryChangesW) + mio poll layer extraction + io_uring abstractions | ✅ completed |
| [02-fd-management](features/02-fd-management/) | File descriptor management: `RegisteredFd`, `poll_readable()`/`poll_writable()`, `ReadyGuard`, edge-triggered readiness tracking, `try_io` auto-clear | ✅ completed |
| [03-integration](features/03-integration/) | Tests, examples, and documentation; verify cross-platform compilation | ✅ completed |
| [04-ipc-bus](features/04-ipc-bus/) | Interprocess message bus (from ipmb) — typed messaging, shared memory, object (FD/Handle) passing, FFI | ⚠️ in_progress (56%) |
| [05-valtron-watcher-task](features/05-valtron-watcher-task/) | Thin valtron adapter: `FileWatcherTask` wraps `NativeWatcher::poll()` in a tick loop with broadcast channels | ✅ completed |
| [06-correctness-fixes](features/06-correctness-fixes/) | Remove stubs, fix error handling, implement missing backends, net module, TaskIterator impls | ✅ completed |
| [07-valtron-task-design](features/07-valtron-task-design/) | Critical design rules: `has_events()` caching, `SharedWatcher`, `StopSignal`, `collect_one` | ✅ completed |
| [08-shared-native-valtron-restructure](features/08-shared-native-valtron-restructure/) | `shared/` / `native/` / `valtron/` split, `FdMonitorTask` uses `Depends`, feature flags | ✅ completed |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      User Code                               │
│                                                              │
│  valtron.spawn::<FileWatcherTask, ...>()                     │
│       │                                                      │
│       ▼                                                      │
│  FileWatcherTask (valtron task)                              │
│       │                                                      │
│       ├── NativeWatcher trait (pluggable)                    │
│       │    ├── InotifyWatcher  (Linux)                       │
│       │    ├── KqueueWatcher   (macOS/BSD)                   │
│       │    ├── WinWatcher      (Windows)                     │
│       │    └── PollWatcher     (fallback)                    │
│       │                                                      │
│       ├── watch("src/") ── register paths                    │
│       ├── poll() ─────── read events from OS                 │
│       └── subscribers ─── Arc<[BroadcastChannel<Event>]>     │
│                                                              │
└──────┬───────────────────────────────────────────────────────┘
       │ FileEvent pushed to queue
       ▼
┌──────────────────────────┐
│  Subscriber Task         │
│  recv().for_each(|e| {   │
│     handle(e.kind, e.path)│
│  })                      │
└──────────────────────────┘
```

## Language Stack

- **Rust** — all implementation

## Success Criteria

- [x] `foundation_nativeapis` compiles on Linux (inotify), macOS (kqueue), and has stubs for Windows (ReadDirectoryChangesW)
- [x] `NativeWatcher` trait is clean, minimal, pluggable — new providers implement one trait
- [x] `FileWatcherTask` works as a valtron task: spawn, add watches, poll events, deliver to queues
- [x] Other tasks can subscribe and receive events through broadcast channels
- [x] `cargo check -p foundation_nativeapis` passes on native target
- [x] `cargo check -p foundation_nativeapis --target wasm32-unknown-unknown` passes (stub/no-op)
- [x] IPC bus: `join()` connects to bus, `send()` routes messages, `recv()` receives them
- [x] IPC cross-platform: macOS (mach_msg/kqueue), Windows (named pipes/IOCP) implemented
- [x] IPC shared memory: `MemoryRegion` uses mmap on all platforms, zero-copy
- [x] IPC typed messages: `MessageBox` blanket impl with `TypeUuid` type identification
- [ ] Valtron integration tests: spawn watcher, touch file, receive event

## Module References

- `backends/foundation_nativeapis/` — new crate
- `backends/foundation_core/src/valtron/` — existing valtron task infrastructure

---

_Created: 2026-06-01_
