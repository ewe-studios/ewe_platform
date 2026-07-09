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

## Table of Contents

1. [Why bollard requires tokio](#why-bollard-requires-tokio)
2. [Core API is async](#core-api-is-async)
3. [valtron handles the sync boundary](#valtron-handles-the-sync-boundary)
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
// All Docker operations are async fn.
impl ContainerHandle {
    pub async fn start(config: ContainerServiceDefinition) -> Result<Self, DockerError>;
    pub async fn shutdown(&self) -> Result<(), DockerError>;
    pub async fn is_running(&self) -> Result<bool, DockerError>;
    pub fn host_port(&self, container_port: u16) -> u16;  // sync — cached post-start
    pub fn id(&self) -> &str;                               // sync
}

impl ContainerGroup {
    pub async fn start(defs: Vec<ContainerServiceDefinition>) -> Result<Self, DockerError>;
    pub fn container(&self, name: &str) -> Option<&ContainerHandle>;
}

impl DockerClient {
    pub async fn connect_local() -> Result<Self, DockerError>;
    pub async fn connect_ssh(host: &str, keys: &[PathBuf], user: &str, port: u16) -> Result<Self, DockerError>;
    pub async fn is_available(&self) -> bool;
    pub async fn ping(&self) -> Result<(), DockerError>;
}
```

---

## valtron handles the sync boundary

No custom `block_on` anywhere. The workspace already has the boundary:

- **`#[valtron_test]`** — initializes the valtron pool + tokio reactor, wraps
  the test body with `block_on_future(async { ... })`. The test signature can
  be `async fn` and the macro bridges it to `#[test] fn`.
- **`#[valtron]`** — same bridge for `main()` or regular functions.

The `#[docker_container]` macro generates code INSIDE this boundary — it
`.await`s the Docker futures directly. The user never sees `block_on`.

For standalone use without valtron (CLI, tokio::main):

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
bollard. These are async calls but Drop is sync. Under `#[valtron_test]` or
`#[tokio::main]`, the tokio reactor is live — `Handle::current()` succeeds
and Drop can drive the futures to completion. If dropped outside any tokio
context, this panics — a programmer error (equivalent to using `tokio::spawn`
without a runtime).

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
