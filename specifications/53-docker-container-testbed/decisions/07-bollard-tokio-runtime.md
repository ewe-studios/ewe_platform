# 01 — Bollard + Internal Tokio Runtime

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Use **bollard** as the Rust Docker Engine API client, with an internal
**single-threaded tokio runtime singleton** (`LazyLock<Runtime>`) to bridge
bollard's async API to sync contexts (proc macro wrappers, tests, `main()`).

## Table of Contents

1. [Why bollard](#why-bollard)
2. [Why bollard requires tokio](#why-bollard-requires-tokio)
3. [Why a separate runtime, not valtron](#why-a-separate-runtime-not-valtron)
4. [Runtime singleton design](#runtime-singleton-design)
5. [Why not Docker CLI shell-out](#why-not-docker-cli-shell-out)
6. [Dependency budget](#dependency-budget)

---

## Why bollard

Bollard is the de facto standard Rust Docker SDK. It provides:

- **Complete API coverage** — Auto-generated from Docker's OpenAPI spec (v1.44+).
  Every endpoint: containers, images, networks, volumes, exec, logs, events.
- **Typed responses** — `ContainerInspectResponse`, `NetworkSettings`, `PortBinding`
  are deserialized into Rust structs. No string parsing.
- **Streaming endpoints** — Logs, events, image pulls return `Stream<Item = ...>`.
  Build output, container attach, exec output are all streamed.
- **SSH transport** — `Docker::connect_with_ssh(...)` tunnels the Docker API over
  SSH. No exposed TCP socket, no TLS management.
- **Active maintenance** — 47+ releases, community standard.

The alternative is shelling out to the `docker` CLI via
`foundation_deployment::core::shell::ShellExecutor`. This works for build
pipelines but is wrong for programmatic container management:

| Concern | Bollard | Docker CLI |
|---------|---------|------------|
| Port mapping resolution | `inspect_container()` → typed `PortBinding` | `docker inspect` + jq/string parsing |
| Wait for readiness | `logs()` stream + `inspect_container()` health | `docker logs` polling loop |
| Error discrimination | Typed `DockerError` variants | Exit code + stderr string matching |
| Streaming | Typed `Stream<Item = LogOutput>` | Raw byte chunks |
| SSH to remote | Native API | `ssh host docker ...` |
| No CLI prerequisite | Yes (socket direct) | No (needs `docker` binary) |

---

## Why bollard requires tokio

Bollard uses `hyper` as its HTTP transport to communicate with the Docker
daemon over Unix sockets or TCP. Hyper is built on tokio — it spawns tokio
tasks internally for connection management, request/response handling, and
streaming. This is not configurable; bollard's public API returns
`impl Future<Output = ...>` that internally depends on a running tokio reactor.

The workspace already includes tokio (91 entries in `Cargo.lock`). No new
runtime is being introduced — bollard just surfaces what hyper already requires.

---

## Why a separate runtime, not valtron

Valtron provides `block_on_future(future) -> F::Output` in
`foundation_core::valtron::executors::sendables`. This polls a single future to
completion on the valtron engine. However:

1. **Bollard spawns tokio tasks internally.** Hyper creates tokio tasks for
   connection pooling, DNS resolution, and TLS handshakes. These tasks require a
   tokio reactor — they panic if no tokio runtime is active. Valtron's
   `block_on_future` only polls the top-level future; it does not provide a
   tokio reactor.

2. **Valtron and tokio are different executors.** Valtron is a work-stealing
   task executor for CPU-bound work. Tokio is an I/O event loop. They serve
   different purposes. Bollard's futures need tokio's I/O driver.

3. **Clean separation.** A `LazyLock<tokio::runtime::Runtime>` used exclusively
   for Docker API calls avoids entanglement with valtron. The proc macro wrapper
   (sync context) calls `DOCKER_RUNTIME.block_on(bollard_future)`. The user's
   test body runs on valtron (if `#[valtron_test]` is used) or sync (if not).
   Neither interferes with the other.

---

## Runtime singleton design

```rust
use std::sync::LazyLock;
use tokio::runtime::Runtime;

/// A lazily-initialized single-threaded tokio Runtime for bollard Docker API
/// calls. Separate from valtron — exists specifically because bollard/hyper
/// require a tokio reactor.
///
/// Single-threaded is sufficient: Docker API calls are network-bound I/O, not
/// CPU work. The runtime is created once on first use and lives for the process
/// lifetime.
static DOCKER_RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create Docker async runtime")
});

/// Block on a future using the Docker runtime. Used by the proc macro's
/// generated setup/teardown code and available for manual use.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    DOCKER_RUNTIME.block_on(future)
}
```

### Nesting safety

`tokio::runtime::Runtime::block_on` panics if called from within the same
runtime's context. This is safe because:

- The `DOCKER_RUNTIME` is only entered from sync code (proc macro wrappers,
  `main()`, `#[test]` functions).
- It is never entered from within a tokio task (no code in this crate spawns
  tokio tasks that call back into `block_on`).
- If the user's function body uses tokio (e.g., `#[tokio::test]`), that is
  a *different* runtime — no nesting conflict.

---

## Why not Docker CLI shell-out

`foundation_deployment::core::shell::ShellExecutor` already provides a robust
command-spawning abstraction over valtron. We could shell out to `docker` for
container lifecycle. Reasons not to:

1. **Port resolution is fragile.** `docker run -P` assigns random host ports.
   Parsing `docker port <container> <port>` output is brittle across Docker
   versions and platforms.

2. **No structured error types.** `docker run` fails with exit code 125 for
   many different reasons. Discriminating "Docker not running" from "image not
   found" from "port conflict" requires string matching on stderr.

3. **Wait strategies require polling.** `WaitFor::Stdout` needs log streaming.
   With CLI: `docker logs -f <id> | grep -m1 "Ready"`. This is a pipeline of
   three processes that must be coordinated. With bollard: a typed
   `Stream<Item = LogOutput>` with `take_while`.

4. **SSH to remote requires nested commands.** `ssh host docker run ...` with
   argument escaping. Bollard: `Docker::connect_with_ssh(host, keys, user, port)`.

---

## Dependency budget

Bollard adds approximately 55 transitive dependencies (hyper, tokio, bytes,
http, futures, tower, etc.). This is acceptable:

1. `foundation_deployment_docker` is a **dev-tooling crate**, not pulled into
   production binaries.
2. Many deps overlap with the existing workspace (tokio is already present,
   hyper/http/bytes are used by `foundation_netio`).
3. The crate is feature-gated — crates that don't need Docker don't pay for it.
