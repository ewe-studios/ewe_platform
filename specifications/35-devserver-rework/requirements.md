---
description: "Migrate ewe_devserver from crates/devserver into backends/foundation_toolings as a tokio/axum-free dev server built on ewe_platform primitives (valtron task iterators, foundation_nativeapis watchers, foundation_core networking). Remove all tokio, axum, async-trait, hyper server dependencies. Rebuild proxy, file watching, cargo building, SSE reload, and HTTP dev service using platform-native constructs."
status: "pending"
priority: "high"
created: 2026-06-01
updated: 2026-06-01
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - devserver
    - migration
    - foundation_toolings
    - tokio-removal
    - axum-removal
    - valtron
    - native-apis
has_features: true
has_fundamentals: false
builds_on:
  - "specifications/34-native-file-watchers"
related_specs:
  - "specifications/34-native-file-watchers"
  - "specifications/33-valtron-singleton"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# DevServer Rework — foundation_toolings Migration

## Overview

Migrate the devserver (`crates/devserver`) into a new crate `backends/foundation_toolings/` that eliminates **all** tokio, axum, async-trait, hyper server, and tower dependencies. Rebuild the dev server entirely using ewe_platform primitives:

- **valtron** task iterators (`TaskIterator`, `ExecutionAction`, `ExecutionEngine`) instead of `tokio::spawn` + `tokio::select!`
- **foundation_nativeapis** `NativeWatcher` instead of `ewe_watch_utils::watch_path` (which uses `notify`)
- **foundation_netio** networking (`Connection`, `Listener`, `simple_http`, `event_source`) instead of hyper/axum/tower
- **foundation_core** concurrency (`concurrent_queue`, `synca` primitives) instead of `tokio::sync::broadcast`
- **foundation_runtimes** `AssetReloader` for embedded reloader.js (already used, keep as-is)

## Known Issues / Limitations

- `ewe_devserver` (0.0.2) is deeply async: every component uses `tokio::spawn`, `tokio::select!`, `tokio::sync::broadcast`, `tokio::process::Command`, `tokio::net::TcpListener`, `tokio::io::*`
- HTTP proxy layer depends on axum (SSE endpoints), hyper (server + client), tower, h2, h3
- File watcher depends on `ewe_watch_utils::watch_path` which wraps `notify` + `notify-debouncer-full`
- `Operator` trait is async: `fn run(&self, signal: broadcast::Receiver<()>) -> JoinHandle<()>`
- `HttpDevService::start()` is `async fn` returning `JoinHandle<()>`
- Cargo build/run uses `tokio::process::Command` (async spawn)
- SSE reload uses `tokio_stream::wrappers::BroadcastStream` + axum's `Sse` responder
- Templates and examples directly import `ewe_devserver::{HttpDevService, ProjectDefinition, ...}` + `tokio::sync::broadcast`

## Feature Index

| Feature | Description | Status |
|---------|-------------|--------|
| [01-crate-scaffolding](features/01-crate-scaffolding/) | Create `backends/foundation_toolings/` with clean Cargo.toml, module structure, error types | pending |
| [02-task-operators](features/02-task-operators/) | Replace async `Operator` trait with valtron `TaskIterator` — all devserver components become TaskIterators | pending |
| [03-native-watching](features/03-native-watching/) | Replace `DirectoryWatcher` (notify-based) with `NativeWatcher` from foundation_nativeapis | pending |
| [04-cargo-builder](features/04-cargo-builder/) | Replace `CargoShellBuilder` (tokio::process) with sync spawning + valtron task iteration | pending |
| [05-binary-runner](features/05-binary-runner/) | Replace `BinaryApp` (tokio::spawn loops) with valtron TaskIterator managing `std::process::Child` | pending |
| [06-native-proxy](features/06-native-proxy/) | Replace hyper/axum/tower proxy with foundation_netio (Connection, Listener, simple_http) | pending |
| [07-sse-reload](features/07-sse-reload/) | Use foundation_netio::event_source (EventWriter, SseEvent, SseResponse) for SSE reload | pending |
| [08-dev-service](features/08-dev-service/) | Replace `HttpDevService` with valtron-coordinated `DevService` — spawn all components as child tasks | pending |
| [09-api-compat](features/09-api-compat/) | Public API surface: `ProjectDefinition`, `ProxyRemoteConfig`, `ProxyType`, `Http1`/`Http2`/`Http3`, `VecStringExt` — preserve signatures where possible | pending |
| [10-cleanup](features/10-cleanup/) | Deprecate/remove `crates/devserver`, update workspace Cargo.toml, update templates/examples/bin/platform imports | pending |
| [11-integration](features/11-integration/) | End-to-end tests: spawn dev service, verify proxy forwarding, verify file-change-triggered rebuild, verify SSE reload | pending |

## Architecture

### Before (current devserver)

```
┌──────────────────────────────────────────────────────────────┐
│                     HttpDevService (async)                    │
│                                                               │
│  tokio::spawn {                                               │
│    ParrellelOps::run()                                        │
│      ├─ DirectoryWatcher (notify + tokio::spawn_blocking)     │
│      ├─ CargoShellBuilder (tokio::process::Command + select!) │
│      ├─ BinaryApp (tokio::spawn + std::process::Command)      │
│      ├─ StreamTCPApp (tokio::net::TcpListener + hyper)        │
│      └─ DirectoryWatcher (reload, notify)                     │
│                                                               │
│    Proxy: axum + hyper server + tower                         │
│      ├─ Http1Service (hyper::service::Service)                │
│      ├─ SSE endpoint (axum::response::Sse)                    │
│      └─ Static script endpoint                                │
│                                                               │
│    Channels: tokio::sync::broadcast throughout                │
│    Signal: tokio::sync::broadcast::Receiver<()>               │
│  }                                                            │
└──────────────────────────────────────────────────────────────┘
```

### After (foundation_toolings)

```
┌──────────────────────────────────────────────────────────────┐
│                     DevService (valtron)                       │
│                                                               │
│  valtron engine spawns coordinated TaskIterators:             │
│                                                               │
│    ├─ NativeWatcherTask (foundation_nativeapis)               │
│    │    └─ polls NativeWatcher, delivers WatchEvent to queue   │
│                                                               │
│    ├─ CargoBuilderTask (sync std::process::Command)           │
│    │    └─ on WatchEvent::Rust → cargo check → cargo build    │
│    │    └─ on build complete → signals BinaryRunnerTask        │
│                                                               │
│    ├─ BinaryRunnerTask (sync std::process::Child lifecycle)   │
│    │    └─ kill old binary → spawn new → wait                  │
│                                                               │
│    ├─ ProxyTask (foundation_netio)                            │
│    │    └─ Listener (netcap) → accept → forward               │
│    │    └─ Route dispatch: /static/sse/reload → SSE handler (local)
│    │    └─ All other paths → bidirectional stream to upstream
│    │    └─ Non-HTTP (tunnel) → raw bidirectional stream
│                                                               │
│    └─ ReloadWatcherTask (foundation_nativeapis)              │
│         └─ watches reload dirs, signals ProxyTask for SSE     │
│                                                               │
│  Channels: concurrent_queue + synca broadcast                  │
│  Signal: synca signals (foundation_core::synca)               │
│  Process: std::process::{Command, Child} (sync spawn/kill)    │
└──────────────────────────────────────────────────────────────┘
```

## Language Stack

- **Rust** — all implementation

## Success Criteria

- [ ] `backends/foundation_toolings/` compiles with `cargo check -p foundation_toolings`
- [ ] **Zero** tokio, axum, async-trait, tower, hyper, h2, h3 dependencies in foundation_toolings
- [ ] **Zero** `tokio::sync::broadcast` usage — replaced with platform primitives
- [ ] **Zero** `tokio::process::Command` — replaced with `std::process::{Command, Child}`
- [ ] **Zero** `tokio::net::TcpListener` — replaced with `foundation_netio::netcap::Listener`
- [ ] **Zero** `tokio::io::{AsyncRead, AsyncWrite}` — replaced with `std::io::{Read, Write}` + poll-layer
- [ ] **Zero** `axum::response::Sse` — replaced with raw HTTP SSE response generator
- [ ] All `Operator` types become `TaskIterator` implementations
- [ ] `HttpDevService` equivalent works: accepts `ProjectDefinition`, starts dev server via valtron
- [ ] Proxy HTTP/1 forwarding works (source → destination)
- [ ] File change → cargo build → binary restart chain works
- [ ] SSE reload endpoint works (browser reloads on file change)
- [ ] `crates/devserver` is deprecated, templates/examples updated to use foundation_toolings
- [ ] Cross-platform: compiles on Linux, macOS (Windows stubs acceptable)

## Module References

- `crates/devserver/` — current devserver implementation (to be replaced)
- `backends/foundation_nativeapis/` — NativeWatcher trait, WatchEvent, poll-layer (spec-34 feature 01)
- `backends/foundation_netio/` — Connection, Listener, simple_http (HTTP), event_source (SSE)
- `backends/foundation_core/src/valtron/` — TaskIterator, ExecutionEngine, ExecutionIterator
- `backends/foundation_core/src/synca/` — signals, broadcast, entry, mpp
- `backends/foundation_core/src/io/` — memory, ubytes, stream_ext
- `backends/foundation_runtimes/` — AssetReloader (embedded reloader.js, keep as-is)
- `crates/watch_utils/` — current watch_path implementation (to be replaced by NativeWatcher)
- `bin/platform/src/local/mod.rs` — CLI consumer of ewe_devserver
- `examples/devbinary/` — template consumer of ewe_devserver
- `templates/*/` — project templates using ewe_devserver

---

_Created: 2026-06-01_
