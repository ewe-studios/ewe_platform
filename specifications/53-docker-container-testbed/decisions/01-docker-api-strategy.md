# 01 — Docker API Strategy

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Use **bollard** (the Rust Docker Engine API client) as the primary programmatic
interface for container lifecycle management, and **keep Docker Compose files**
(`compose.yaml`) alongside the crate as the declarative, human-editable
definition of test environments. The two are complementary: bollard handles
imperative operations (create/start/stop/inspect/exec) at runtime, while Compose
files serve as the source of truth for multi-service topologies. We do NOT
generate Compose files from Rust; we read and execute them via the Docker CLI
as a convenience path, with bollard handling the fine-grained control path.

## Table of Contents

1. [Options considered](#options-considered)
2. [Why bollard over Docker CLI shell-out](#why-bollard-over-docker-cli-shell-out)
3. [Why keep Compose files (not generate them)](#why-keep-compose-files-not-generate-them)
4. [Dual-mode architecture](#dual-mode-architecture)
5. [What bollard handles vs what Compose handles](#what-bollard-handles-vs-what-compose-handles)
6. [Dependency budget](#dependency-budget)
7. [SSH transport for remote Docker](#ssh-transport-for-remote-docker)

---

## Options considered

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **A: bollard SDK** | Full Rust Docker API client | Async, complete API, BuildKit, SSH, streaming | ~55 transitive deps |
| **B: Docker CLI shell-out** | `std::process::Command` spawning `docker` | Zero deps, familiar error messages | String parsing, no streaming types, fragile |
| **C: bollard + Compose files** (chosen) | Bollard for runtime ops, Compose for definitions | Best of both; Compose files are debuggable without Rust | Two surfaces to maintain |
| **D: Generate Compose from Rust** | `compose_spec` crate to serialize Rust structs → YAML | Type-safe, single source of truth | Yet another dep; Compose YAML is already the universal format |

---

## Why bollard over Docker CLI shell-out

The `foundation_deployment` crate already has a `ShellExecutor` that could
spawn `docker` commands. However, the testbed's needs differ from a build
pipeline:

1. **Streaming with types** — Container logs, attach, exec output, and events
   are streaming endpoints. Bollard returns `Stream<Item = LogOutput>` rather
   than raw byte chunks we must parse. This matters for structured test output.

2. **Programmatic inspection** — After a container starts, we need to resolve
   its IP address, exposed ports, health status, and exit code. Bollard gives
   typed structs; shell-out requires `docker inspect` + `jq` or string parsing.

3. **Exec with exit codes** — Running build commands inside containers needs
   the exit code. Bollard's `create_exec` + `start_exec` returns
   `ExecResults { exit_code, .. }`. Shell-out requires fragile logic.

4. **Event stream for lifecycle** — Waiting for a container to be "healthy"
   (healthcheck passing) is a first-class operation with bollard's event stream.
   Shell-out requires polling `docker ps` in a loop.

5. **No Docker CLI prerequisite** — Bollard talks to the Docker socket directly
   (Unix socket or TCP). The Docker CLI binary need not be installed. This
   matters for minimal CI runners and embedded environments.

6. **SSH transport to remote daemons** — Bollard supports SSH connections to
   remote Docker daemons natively. This enables the Hetzner cloud deployment
   pattern (local bollard → SSH → remote dockerd).

The ~55 transitive dependencies are acceptable in a dev-tooling crate
(`foundation_testbed` already pulls in ssh2 + OpenSSL, indicatif, tar, flate2,
xz2, deno_core/V8, etc.). Bollard is NOT pulled into production crates.

---

## Why keep Compose files (not generate them)

1. **Human debuggability** — A developer can `cd` to the compose file directory
   and run `docker compose up` directly to reproduce a test environment without
   going through the Rust crate. This is the "escape hatch" when programmatic
   control isn't working.

2. **Existing ecosystem** — Dockurr images, CI pipelines, and deployment tools
   already speak Compose. Generating YAML from Rust types adds indirection
   with no practical benefit.

3. **Compose files are the source of truth** — The `ContainerProfile` (our Rust
   config struct) is a *subset* of what Compose can express. Rather than try to
   model the full Compose spec in Rust, we let Compose handle the full
   expressiveness and use bollard for the runtime operations we actually need.

4. **No `compose_spec` dependency** — Avoids pulling in another crate just to
   serialize YAML. We already have `serde`/`serde_json`; reading an existing
   compose file into a partial struct is simpler than generating one.

---

## Dual-mode architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    DockerProvider                            │
│                                                             │
│  ┌──────────────────┐    ┌──────────────────────────────┐   │
│  │  Bollard path     │    │  Compose path                 │   │
│  │  (programmatic)   │    │  (declarative)                │   │
│  │                   │    │                              │   │
│  │  bollard::Docker  │    │  compose.yaml beside crate    │   │
│  │  → connect to     │    │  → docker compose up -d       │   │
│  │    docker.sock    │    │  → bollard for exec/inspect   │   │
│  │  → pull image     │    │  → docker compose down        │   │
│  │  → create container│   │                              │   │
│  │  → start          │    │                              │   │
│  │  → exec (build)   │    │                              │   │
│  │  → stop/remove    │    │                              │   │
│  └──────────────────┘    └──────────────────────────────┘   │
│                                                             │
│  Both paths:                                                │
│  → SSH into container (ssh2, same as today)                 │
│  → File push/pull (docker cp or bind mount)                 │
│  → Binary validation (same as today)                        │
└─────────────────────────────────────────────────────────────┘
```

The `DockerProvider` selects between paths based on configuration:

- **Bollard path** (default for programmatic use): Full lifecycle via bollard.
  Generates container configs from `ContainerProfile` programmatically, creates
  networks, manages volumes.
- **Compose path** (opt-in, or for complex topologies): Reads `compose.yaml`,
  shells out to `docker compose up -d` for the initial launch, then uses
  bollard to connect to running containers for exec/inspect/stop.

The Compose path intentionally uses the Docker CLI for `up`/`down` because
Compose is a client-side orchestrator (it computes the diff between desired
and actual state, then issues individual container API calls). Reimplementing
Compose's diff logic in Rust is not valuable.

---

## What bollard handles vs what Compose handles

| Operation | bollard | Compose (CLI) |
|-----------|---------|---------------|
| Pull image | `create_image()` | `docker compose pull` |
| Create container | `create_container()` | Implicit in `up` |
| Start container | `start_container()` | Implicit in `up` |
| Stop container | `stop_container()` | `docker compose down` |
| Remove container | `remove_container()` | `docker compose down` |
| Create network | `create_network()` | Defined in compose.yaml |
| Exec command | `create_exec()` + `start_exec()` | `docker compose exec` |
| Stream logs | `logs()` → Stream | `docker compose logs -f` |
| Inspect container | `inspect_container()` | `docker compose ps` |
| Health status | Event stream | `docker compose ps` |
| Multi-service orchestration | Manual (sequential API calls) | Native (`depends_on`, profiles) |

---

## Dependency budget

Bollard adds approximately 55 transitive dependencies (hyper, tokio, bytes,
http, futures, tower, etc.). This is acceptable because:

1. `foundation_testbed` is a **dev-tooling crate**, not a production
   dependency. It already has heavy deps (deno_core + V8 ~100MB, ssh2 +
   vendored OpenSSL).
2. Bollard is feature-gated behind `vms` (the existing feature flag). Crates
   that depend on `foundation_testbed` for its wasm harness don't pull in
   Docker at all.
3. Bollard's deps overlap heavily with crates already in the workspace
   (tokio, hyper, http, bytes, futures — used by `foundation_netio`,
   `foundation_http`, etc.).

## SSH transport for remote Docker

Bollard supports connecting to a remote Docker daemon via SSH:

```rust
use bollard::Docker;
let docker = Docker::connect_with_ssh(
    "hetzner-box.example.com",
    &["/home/user/.ssh/id_ed25519"],
    "root",
    22,
    // No TLS verification needed — SSH provides the secure tunnel
)?;
```

This enables the Hetzner cloud deployment pattern: the local testbed CLI
connects to a remote Docker daemon over SSH, deploys containers, runs builds,
and streams results back. No Docker socket exposed over TCP, no TLS
certificate management — SSH handles authentication and encryption.

This aligns with the `vm-uncloud` pattern where `uc machine init` installs
Docker on the remote machine and all subsequent operations go over SSH.
