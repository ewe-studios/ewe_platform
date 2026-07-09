# 07 — Bollard + Async Architecture

**Date:** 2026-07-09
**Status:** Resolved

## Decision

Use **bollard** as the Rust Docker Engine API client. Bollard is async-native
(requires a tokio reactor — hyper runs on tokio). The entire API surface is
`async fn`. The `#[docker_container]` proc macro generates async setup/teardown
code that `.await`s. Users drive execution with `#[valtron_test]` /
`#[valtron]` (which already bridge async → sync via `block_on_future`) or any
tokio runtime. No custom `block_on` wrapper, no internal tokio singleton.

### Principle: async core, sync wrappers at the boundary

Every type is **async-first**: `ContainerHandle`, `ContainerGroup`,
`DockerClient`, `NetworkHandle` — all methods that touch the Docker API are
`async fn`. Sync callers (the `Provider` trait, `Drop`, CLI, tests without
valtron) call `futures_lite::block_on` (re-exported) at the boundary. This
gives us the best of both worlds:

```
Caller                    Bridge                     Core
──────────────────────────────────────────────────────────
#[valtron_test] async fn  → .await                   async fn
#[valtron_test] sync fn   → block_on_future → .await  async fn
Provider trait (sync)     → futures_lite::block_on   async fn
ContainerHandle::Drop     → futures_lite::block_on   async fn
#[tokio::main] async fn   → .await                   async fn
```

No sync wrappers pollute the core types — `ContainerHandle` has exactly one
`start()` method, `async fn`. Sync callers pay the one-line `block_on` tax
at their boundary. This keeps the core clean and the sync concern localized.

Since nothing mandates `async fn start()` (no trait), we provide both:

- **`start()`** — sync convenience that calls `start_async()` via `futures_lite::block_on`.
  The default, ergonomic path for the 90% case (Provider trait, Drop, CLI).
- **`start_async()`** — the async core. `.await` this in valtron/tokio contexts.
  The macro generates `start_async().await` for async test bodies.

Same pattern applies to `shutdown()`/`shutdown_async()`, `is_running()`/etc.

## Table of Contents

1. [Why bollard requires tokio](#why-bollard-requires-tokio)
2. [Core API is async](#core-api-is-async)
3. [Sync bridges at the boundary](#sync-bridges-at-the-boundary)
4. [Macro generates async code](#macro-generates-async-code)
5. [Drop behavior](#drop-behavior)
6. [Dependency budget](#dependency-budget)

---

## Why bollard requires tokio

Bollard uses `hyper` as its HTTP transport. Hyper is built on tokio — it
spawns tokio tasks internally for connection management, request/response
handling, and streaming. Bollard's public API returns
`impl Future<Output = ...>` that depends on a running tokio reactor.

Tokio is already in the workspace (97 lockfile entries). No new runtime.

---

## Core API is async

```rust
// Every method touching the Docker API has an async core (_async suffix).
// A sync convenience (no suffix) delegates via futures_lite::block_on.
impl ContainerHandle {
    // ── Async core ──
    pub async fn start_async(config: ContainerServiceDefinition) -> Result<Self, DockerError>;
    pub async fn shutdown_async(&self) -> Result<(), DockerError>;
    pub async fn is_running_async(&self) -> Result<bool, DockerError>;

    // ── Sync convenience (delegates to _async via futures_lite::block_on) ──
    pub fn start(config: ContainerServiceDefinition) -> Result<Self, DockerError> {
        block_on(Self::start_async(config))
    }
    pub fn shutdown(&self) -> Result<(), DockerError> {
        block_on(self.shutdown_async())
    }
    pub fn is_running(&self) -> bool {
        block_on(self.is_running_async()).unwrap_or(false)
    }

    // ── Always sync (cached data, no API call) ──
    pub fn host_port(&self, container_port: u16) -> u16;
    pub fn id(&self) -> &str;
}

impl ContainerGroup {
    pub async fn start_async(defs: Vec<ContainerServiceDefinition>) -> Result<Self, DockerError>;
    pub fn start(defs: Vec<ContainerServiceDefinition>) -> Result<Self, DockerError> {
        block_on(Self::start_async(defs))
    }
    pub fn container(&self, name: &str) -> Option<&ContainerHandle>;
}

impl DockerClient {
    pub async fn connect_local_async() -> Result<Self, DockerError>;
    pub fn connect_local() -> Result<Self, DockerError> {
        block_on(Self::connect_local_async())
    }
    // Same pattern for connect_ssh, is_available, ping...
}
```

*Why both:* `start()` is the 90% case — Provider trait (sync), Drop, CLI,
sync tests. `start_async()` is for valtron/tokio contexts where `.await`
composes naturally. Nothing mandates one over the other (no trait).

---

## Sync bridges at the boundary

The core is async. Sync callers bridge at their boundary using the simplest
tool available:

| Caller | Bridge | What happens |
|--------|--------|-------------|
| `#[valtron_test] async fn` | `.await` | valtron pool + tokio reactor already live; bollard futures compose naturally |
| `#[valtron_test] sync fn` | `block_on_future(async { ... })` | valtron bridges the whole async block to `#[test] fn` |
| `Provider` trait (sync) | `futures_lite::block_on(...)` | `DockerProvider::launch()` → `block_on(ContainerHandle::start(cfg))` |
| `ContainerHandle::Drop` | `futures_lite::block_on(...)` | Drop is sync; bollard stop/remove calls are async — bridged inline |
| `#[tokio::main]` | `.await` | Standard tokio async main

For standalone use without valtron (CLI):

```rust
#[tokio::main]
async fn main() {
    let group = ContainerGroup::start(vec![...]).await?;
    // ...
}
```

If a direct sync call is genuinely needed (rare — valtron covers tests),
we re-export `futures_lite::block_on` rather than writing one or reaching
for tokio's:

```rust
pub use futures_lite::future::block_on;
```

---

## Macro generates async code

```rust
// User writes:
#[valtron_test]
#[docker_container(image = "redis:7", port = 6379)]
async fn test_redis() {
    let mut conn = redis::connect("...").await?;
    // ...
}

// Expands to (simplified):
#[test]
fn test_redis() {
    async fn __docker_body() { /* original async body */ }
    foundation_core::valtron::block_on_future(async move {
        let __guard = {
            let __cfg = ContainerConfig::new("redis:7")
                .port(6379)
                .wait(WaitFor::Port { port: 6379, timeout: Duration::from_secs(30) });
            match ContainerHandle::start(__cfg).await {
                Ok(h) => h,
                Err(e) if e.is_connection_error() => {
                    tracing::warn!("SKIP: Docker not available ({e})");
                    return;
                }
                Err(e) => panic!("Failed to start Docker container: {e}"),
            }
        };
        __docker_body().await;
        drop(__guard);
    });
}
```

The macro generates `async { setup().await; body().await; drop(guard); }`.
valtron's `block_on_future` bridges the whole thing to `#[test] fn`.

---

## Drop behavior

`ContainerHandle::Drop` calls `stop_container` + `remove_container` via
bollard. These are async calls but Drop is sync. Drop uses
`futures_lite::block_on` (re-exported) — same bridge as the `Provider`
trait methods — to drive the async cleanup inside the Drop scope. No
dependency on the caller's runtime. Works regardless of whether the caller
is valtron, tokio, or bare sync.

---

## Dependency budget

Bollard adds approximately 55 transitive dependencies (hyper, tokio, bytes,
http, futures, tower, etc.). Acceptable because:

1. `foundation_deployment_platform` is a dev-tooling crate, not pulled into
   production binaries.
2. Many deps overlap with the workspace (tokio is at 97 lockfile entries,
   hyper/http/bytes used by `foundation_netio`).
3. `foundation_testbed` already pulls ssh2 + vendored OpenSSL, indicatif, tar,
   flate2, xz2, and (optionally) deno_core/V8 — bollard is comparable.
