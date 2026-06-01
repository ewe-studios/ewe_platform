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
  completed: 0
  uncompleted: 38
  total: 38
  completion_percentage: 0%
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
| [01-native-apis](features/01-native-apis/) | `foundation_nativeapis` crate: `NativeWatcher` trait + platform backends (inotify, kqueue, ReadDirectoryChangesW) | pending |
| [02-valtron-watcher-task](features/02-valtron-watcher-task/) | Valtron task that wraps a `NativeWatcher`, polls on each tick, delivers events to subscriber queues | pending |
| [03-integration](features/03-integration/) | Tests, examples, and documentation; verify cross-platform compilation | pending |

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

- [ ] `foundation_nativeapis` compiles on Linux (inotify), macOS (kqueue), and has stubs for Windows (ReadDirectoryChangesW)
- [ ] `NativeWatcher` trait is clean, minimal, pluggable — new providers implement one trait
- [ ] `FileWatcherTask` works as a valtron task: spawn, add watches, poll events, deliver to queues
- [ ] Other tasks can subscribe and receive events through broadcast channels
- [ ] `cargo check -p foundation_nativeapis` passes on native target
- [ ] `cargo check -p foundation_nativeapis --target wasm32-unknown-unknown` passes (stub/no-op)
- [ ] Valtron integration tests: spawn watcher, touch file, receive event

## Module References

- `backends/foundation_nativeapis/` — new crate
- `backends/foundation_core/src/valtron/` — existing valtron task infrastructure

---

_Created: 2026-06-01_
