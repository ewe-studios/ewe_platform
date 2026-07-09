# 01 — Docker API Strategy

**Date:** 2026-07-09
**Status:** Resolved

## Decision

Use **bollard** (the Rust Docker Engine API client) as the programmatic interface
for container lifecycle management. The primary source of truth is a Rust-native
**`ContainerServiceDefinition`** struct — a builder-pattern type that describes a
container's image, ports, env vars, volumes, network, and wait strategy entirely
in Rust. Users interact with Docker either via the `#[docker_container(...)]`
proc macro (declarative, applied to functions) or via a programmatic API that
takes `Vec<ContainerServiceDefinition>` and returns a **`ContainerGroup`** handle.
Compose YAML generation is a serialization convenience (not the primary path).

## Table of Contents

1. [Options considered](#options-considered)
2. [Why bollard over Docker CLI shell-out](#why-bollard-over-docker-cli-shell-out)
3. [ContainerServiceDefinition — Rust-native source of truth](#containerservicedefinition--rust-native-source-of-truth)
4. [ContainerGroup — grouped lifecycle](#containergroup--grouped-lifecycle)
5. [Programmatic API design](#programmatic-api-design)
6. [Compose YAML as optional serialization](#compose-yaml-as-optional-serialization)
7. [Log output configuration](#log-output-configuration)
8. [Dependency budget](#dependency-budget)
9. [SSH transport for remote Docker](#ssh-transport-for-remote-docker)

---

## Options considered

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **A: bollard SDK** | Full Rust Docker API client | Async, complete API, BuildKit, SSH, streaming | ~55 transitive deps |
| **B: Docker CLI shell-out** | `std::process::Command` spawning `docker` | Zero deps, familiar error messages | String parsing, no streaming types, fragile |
| **C: bollard + Compose files** | Bollard for runtime ops, Compose for definitions | Compose files are debuggable without Rust | Two surfaces to maintain |
| **D: `ContainerServiceDefinition` + bollard** (chosen) | Rust-native service definitions, Compose YAML as optional export | Single source of truth, type-safe, macros + programmatic API | New type to design; Compose round-trip fidelity not guaranteed |

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

7. **Per-container handle with health API** — `ContainerGroup::container(name)`
   returns a `ContainerHandle` with `wait_till_health().await?` and other
   inspection methods.

The ~55 transitive dependencies (hyper, tokio, bytes, http, futures, tower, etc.)
are acceptable because:
1. `foundation_deployment_docker` is a **dev-tooling crate**, not pulled into
   production binaries.
2. Many of bollard's deps overlap with crates already in the workspace (tokio
   is at 97 lockfile entries, hyper/http/bytes are used by `foundation_netio`).
3. For comparison, other dev-tooling crates in the workspace pull heavyweight
   dependencies: `foundation_testbed` already includes ssh2 + vendored
   OpenSSL, indicatif, tar, flate2, xz2, and (optionally) deno_core/V8.

**Resolved**: The Docker API (`ContainerServiceDefinition`, `ContainerHandle`, `ContainerGroup`,
`WaitFor`, `DockerClient`) lives in `foundation_deployment_docker` — a standalone
crate alongside `foundation_deployment`. `foundation_testbed` DEPENDS on it
(rather than owning it), so the Docker interaction layer is available to ANY
workspace crate that needs programmatic container management, not just the testbed.

---

## ContainerServiceDefinition — Rust-native source of truth

Rather than generating Compose YAML and shelling out, the canonical definition
of a container service lives in Rust:

```rust
/// A declarative description of a single container service. This is the
/// Rust-native equivalent of a `compose.yaml` service entry — it can be
/// constructed statically, at runtime, or via the `#[docker_container]`
/// proc macro's parsed attributes.
pub struct ContainerServiceDefinition {
    /// Docker image (e.g. "redis:7", "postgres:16", "dockurr/windows:5.15")
    pub image: String,
    /// Optional container name. Auto-generated if absent.
    pub name: Option<String>,
    /// Ports to expose: container_port -> optional host_port (None = auto)
    pub ports: Vec<PortMapping>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Volume mounts
    pub volumes: Vec<VolumeMount>,
    /// Network to attach to
    pub network: Option<String>,
    /// Network aliases for DNS resolution
    pub network_aliases: Vec<String>,
    /// Wait strategy
    pub wait: WaitFor,
    /// Stop timeout (seconds)
    pub stop_timeout: u64,
    /// Memory limit (e.g. "512m", "2g")
    pub memory: Option<String>,
    /// CPU limit
    pub cpus: Option<u32>,
    /// Force pull on every start
    pub always_pull: bool,
    /// Command override
    pub command: Option<Vec<String>>,
    /// Devices (for KVM, TUN, etc.)
    pub devices: Vec<DeviceMapping>,
    /// Linux capabilities to add
    pub cap_add: Vec<String>,
    /// Where container logs are forwarded
    pub log_output: LogOutput,
    /// Whether this container is required (failure = panic, no graceful skip)
    pub required: bool,
}
```

### Builder API

```rust
impl ContainerServiceDefinition {
    pub fn new(image: impl Into<String>) -> Self;
    pub fn port(mut self, container_port: u16) -> Self;
    pub fn port_mapped(mut self, container_port: u16, host_port: u16) -> Self;
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn network(mut self, name: impl Into<String>) -> Self;
    pub fn network_alias(mut self, alias: impl Into<String>) -> Self;
    pub fn volume(mut self, source: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self;
    pub fn wait(mut self, strategy: WaitFor) -> Self;
    pub fn memory(mut self, mem: impl Into<String>) -> Self;
    pub fn cpus(mut self, count: u32) -> Self;
    pub fn always_pull(mut self) -> Self;
    pub fn command(mut self, cmd: Vec<String>) -> Self;
    pub fn device(mut self, host_path: impl Into<PathBuf>) -> Self;
    pub fn cap_add(mut self, cap: impl Into<String>) -> Self;
    pub fn log_to(mut self, output: LogOutput) -> Self;
    pub fn required(mut self) -> Self;
}
```

`ContainerServiceDefinition` can be:
- Constructed **statically** — `const REDIS: ContainerServiceDefinition = ...`
- Built at **runtime** — pulling env vars, reading config files
- Generated by the **proc macro** — `#[docker_container]` attributes parse into this struct

---

## ContainerGroup — grouped lifecycle

```rust
/// A handle to a group of running containers. Created by
/// `ContainerGroup::start(vec![def1, def2, ...])`. On Drop, stops and removes
/// all containers in dependency order (reverse start order).
///
/// The group handle allows individual container access for inspection,
/// health checks, and exec.
pub struct ContainerGroup {
    handles: Vec<ContainerHandle>,
}

impl ContainerGroup {
    /// Start all containers from their definitions. Returns a group handle
    /// that will clean up all containers on Drop.
    pub async fn start(
        definitions: Vec<ContainerServiceDefinition>,
    ) -> Result<Self, DockerError>;

    /// Get a handle to an individual container by its service name or
    /// container name.
    pub fn container(&self, name: &str) -> Option<&ContainerHandle>;

    /// Get all container handles.
    pub fn containers(&self) -> &[ContainerHandle];
}

impl Drop for ContainerGroup {
    fn drop(&mut self) {
        // Reverse order teardown: stop + remove all containers.
        // Best-effort, errors logged.
    }
}
```

### Usage example (programmatic)

```rust
use foundation_deployment_docker::*;

#[valtron_test]
fn test_app_with_db() {
    let group = block_on(ContainerGroup::start(vec![
        ContainerServiceDefinition::new("postgres:16")
            .port(5432)
            .env("POSTGRES_PASSWORD", "test")
            .env("POSTGRES_DB", "app_test")
            .wait(wait_for::port(5432)),
        ContainerServiceDefinition::new("redis:7")
            .port(6379)
            .wait(wait_for::stdout("Ready to accept connections")),
    ])).expect("Failed to start containers");

    let pg = group.container("postgres:16").unwrap();
    let redis = group.container("redis:7").unwrap();

    // Connect: localhost:<pg.host_port(5432)>
    // Connect: localhost:<redis.host_port(6379)>

    // Group drops here — both containers stopped and removed.
}
```

### Usage example (macro)

```rust
#[docker_container(image = "redis:7", port = 6379)]
#[valtron_test]
fn test_redis_cache() {
    // Redis running, auto-cleaned up after this function returns.
}
```

For multi-container via macro, stacked attributes:

```rust
#[docker_container(image = "redis:7", port = 6379)]
#[docker_container(image = "postgres:16", port = 5432, env("POSTGRES_PASSWORD", "test"))]
#[valtron_test]
fn test_with_db_and_cache() {
    // Both running, cleaned up in reverse order.
}
```

---

## Programmatic API design

Users have three entry points, all backed by the same `ContainerServiceDefinition`:

| Path | Mechanism | Use case |
|------|-----------|----------|
| **Proc macro** | `#[docker_container(...)]` on a function | Quick test isolation, single or stacked containers |
| **Builder API** | `ContainerGroup::start(vec![...])` | Tests with dynamic config, multi-container with ordering |
| **Static definitions** | `const MY_SERVICE: ContainerServiceDefinition = ...` | Reusable service profiles across a codebase |

All three paths converge on `ContainerHandle::start()` internally. This means:
- The same `WaitFor` strategies, error handling, and lifecycle guarantees apply
  regardless of how the container was started.
- The `ContainerGroup` returned by the builder API is the same type the macro
  generates internally.

---

## Compose YAML as optional serialization

Compose files are NOT the source of truth — they are an export format for
debugging and interoperability. `ContainerServiceDefinition` provides
`.to_compose_yaml() -> String`:

```rust
let defs = vec![
    ContainerServiceDefinition::new("postgres:16").port(5432).env("POSTGRES_PASSWORD", "test"),
    ContainerServiceDefinition::new("redis:7").port(6379),
];
let yaml = ContainerServiceDefinition::to_compose_yaml(&defs);
std::fs::write("docker-compose.yml", yaml)?;
// → docker compose up    (optional escape hatch)
```

This is a **one-way export** — we do not parse Compose YAML back into
`ContainerServiceDefinition`. The Rust struct is the canonical form.

Additionally, a `container_compose!` macro can accept a path to an existing
`compose.yaml` and generate the equivalent `ContainerServiceDefinition`
instances at compile time (for users who prefer starting from Compose):

```rust
let defs = container_compose!("compose.yaml");
let group = block_on(ContainerGroup::start(defs))?;
```

But this is a convenience — the Rust-native definition is the recommended path.

---

## Log output configuration

Each `ContainerServiceDefinition` specifies where container logs go:

```rust
pub enum LogOutput {
    /// Forward to stdout (interleaved with process output)
    Stdout,
    /// Forward to stderr (interleaved with process output)
    Stderr,
    /// Write to a file path
    File(PathBuf),
    /// Discard logs
    Null,
    /// Capture in-memory (accessible via handle)
    Captured,
}
```

The macro equivalent:

```rust
#[docker_container(image = "redis:7", port = 6379, log_output = "file", log_path = "/tmp/redis.log")]
fn test() { ... }
```

`ContainerHandle` provides `fn logs(&self) -> Result<String, DockerError>` for
the `Captured` variant, returning accumulated stdout/stderr.

---

## Dependency budget

Bollard adds approximately 55 transitive dependencies (hyper, tokio, bytes,
http, futures, tower, etc.). This is acceptable because:

1. `foundation_deployment_docker` is a **dev-tooling crate**, not a production
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
)?;
```

This enables the Hetzner cloud deployment pattern: the local testbed CLI
connects to a remote Docker daemon over SSH, deploys containers, runs builds,
and streams results back. No Docker socket exposed over TCP, no TLS
certificate management — SSH handles authentication and encryption.

This aligns with the `vm-uncloud` pattern where `uc machine init` installs
Docker on the remote machine and all subsequent operations go over SSH.

## Related decisions

The SSH transport layer and reverse-proxy/ingress concerns identified here
warrant dedicated foundation crates. See:

- **Decision 13 — `foundation_sshkit`** — A dedicated SSH crate for key
  management, connection pooling, and command execution across the workspace.
- **Decision 14 — `foundation_proxy`** — A proxy/sidecar crate with SSL
  termination, automatic cert provisioning, and VFS-backed cert storage.
