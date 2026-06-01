---
feature: "Crate Scaffolding"
description: "Create backends/foundation_toolings/ with clean Cargo.toml, module structure, error types — zero tokio/axum dependencies"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: Crate Scaffolding

## Problem

Need to create the new crate `foundation_toolings` in `backends/` with a clean dependency graph that explicitly excludes tokio, axum, hyper, tower, async-trait, and all async HTTP frameworks.

## Solution

Create the crate scaffold with:
- Minimal Cargo.toml depending only on foundation_core, foundation_nativeapis, foundation_runtimes, foundation_nostd, tracing, derive_more, itertools, concurrent-queue, serde/serde_json
- Module structure mirroring the devserver's logical components but as valtron TaskIterators
- Error types using foundation_errstacks (or thiserror → derive_more per project convention)

### Crate Structure

```
backends/foundation_toolings/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs              # public API, re-exports
│   ├── error.rs            # ToolingError enum
│   ├── types.rs            # ProjectDefinition, ProxyRemoteConfig, ProxyType, Http1/2/3
│   ├── config.rs           # HyperFuncMap equivalent as raw HTTP handler registry
│   ├── vec_ext.rs          # VecStringExt trait (copy from devserver, no deps)
│   ├── watcher/
│   │   └── mod.rs          # FileChange enum + watcher task (delegates to NativeWatcher)
│   ├── builder/
│   │   └── mod.rs          # CargoBuilderTask (sync cargo check/build)
│   ├── runner/
│   │   └── mod.rs          # BinaryRunnerTask (sync process lifecycle)
│   ├── proxy/
│   │   ├── mod.rs          # ProxyTask (Listener + accept loop + route dispatch)
│   │   ├── http1.rs        # Http1 request handler (simple_http parser, route dispatch)
│   │   ├── tunnel.rs       # TCP tunnel (raw bidirectional copy)
│   │   └── sse.rs          # SSE reload handler (SseResponse + EventWriter + reload_queue)
│   └── service/
│       └── mod.rs          # DevService — valtron-coordinated top-level service
└── tests/
    └── integration.rs
```

### Cargo.toml

```toml
[package]
name = "foundation_toolings"
version = "0.0.1"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Development tooling server: reverse proxy, file watcher, hot reload — built on valtron and native APIs, zero async runtime deps"
keywords = ["devserver", "proxy", "hot-reload", "valtron", "native-apis"]

[dependencies]
foundation_core = { workspace = true }
foundation_nativeapis = { workspace = true, features = ["watcher"] }
foundation_netio = { workspace = true }
foundation_nostd = { workspace = true }
foundation_runtimes = { workspace = true }
foundation_errstacks = { workspace = true }

tracing.workspace = true
derive_more.workspace = true
itertools = "0.14"
concurrent-queue = "2.5"
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
tracing-test = { version = "0.2", features = ["no-env-filter"] }
tracing-subscriber = "0.3"

[features]
default = []

[lints]
workspace = true
```

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| `foundation_netio` | Primary networking crate | Provides `Connection`, `Listener`, `simple_http`, `event_source` — covers all networking needs |
| `tokio::sync::broadcast` | Replace with `synca` | foundation_core already provides broadcast/signal primitives |
| `tokio::process` | Replace with `std::process` | Sync spawn + valtron tick for polling process status |
| `tokio::net` | Replace with `foundation_netio::netcap` | `Connection`/`Listener` provide sync TCP without async runtime |
| `axum` / `hyper` | Remove entirely | `foundation_netio::simple_http` handles HTTP parsing/responses |
| `async-trait` | Remove | TaskIterator is a sync trait — no async needed |
| `http` crate | Not needed directly | `foundation_netio::simple_http` provides its own `Status`, `Proto`, headers |
| `bytes` crate | Not needed | `foundation_netio` handles body I/O internally |

### Task Breakdown

1. [ ] Create `backends/foundation_toolings/` directory structure
2. [ ] Write `Cargo.toml` with approved dependencies
3. [ ] Write `src/lib.rs` with module declarations and re-exports
4. [ ] Write `src/error.rs` with `ToolingError` enum (Io, Watch, Build, Run, Proxy, SSE)
5. [ ] Write `src/vec_ext.rs` — copy `VecStringExt` from devserver (no dependencies)
6. [ ] Add `foundation_toolings` to workspace `Cargo.toml` dependencies
7. [ ] Verify `cargo check -p foundation_toolings` passes

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/Cargo.toml` | Create |
| `backends/foundation_toolings/README.md` | Create |
| `backends/foundation_toolings/src/lib.rs` | Create |
| `backends/foundation_toolings/src/error.rs` | Create |
| `backends/foundation_toolings/src/vec_ext.rs` | Create (copy from devserver) |
| Root `Cargo.toml` | Edit — add `foundation_toolings` to workspace |

---

_Created: 2026-06-01_
