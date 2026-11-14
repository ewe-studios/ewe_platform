---
feature: "Docker driver"
status: "pending"
priority: "medium"
model: "sonnet"
phase: "2"
depends_on: ["06-programs-rpc-provider"]
crates: ["foundation_platform_sandbox", "foundation_deployment_platform"]
---

# Feature 10: The Docker driver, and the control plane in a container

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); remote procedure
call (RPC); Unix domain socket (UDS); Transmission Control Protocol (TCP); process
identifier (PID); behaviour-driven development (BDD); continuous integration (CI);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

Two things, both on the existing Docker client (`foundation_deployment_docker`):

1. `DockerDriver`: a `SandboxDriver` that talks to the Docker Engine of the host. It starts
   a container from any image, puts `agent-slave` into it, and gives the control plane the
   way to reach that slave. The general case is a control plane that runs on the host
   itself (decision D12).
2. An image and a compose file that run `agent-sandbox serve` in a container, with the
   host's Docker socket. This is the operator's choice. The control plane in the container
   uses the same driver and starts sibling containers on the host's Docker Engine.
3. `DockerContainerControl`: the container handle of feature 00, section 6.13, on the
   Docker Engine. It starts, runs commands in, and stops a container with no sandbox
   record.

The Docker Sandboxes product (`sbx`) is a different system; feature 16 has its driver.

A container is a weaker boundary than a microVM. The README says so. Feature 11 adds
microVMs inside the container.

## 2. Story

- **S1**: "A caller creates a sandbox from a Docker image name; the image does not need the
  slave in it."
- **S2**: "The slave is the container's main process; or it is a sidecar beside the image's
  own main process."
- **S3**: "A caller runs commands and moves files in the container with the same API as in
  a microVM."
- **S4**: "The control plane restarts; its containers still run; it finds them again."
- **S5**: "The Docker daemon is not there, the image does not exist, or the container exits
  at once."
- **S6**: "An operator runs the control plane itself in a container."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Check the host | control plane | `probe` | Daemon reachable, API version, slave file present. |
| U2 | Launch | control plane | `launch` | A running container with the slave; an endpoint. |
| U3 | Attach | control plane | `attach` | The substrate of a container that still runs. |
| U4 | End | control plane | `terminate`, `kill`, `cleanup` | The container is removed. |
| U5 | Serve from a container | operator | `docker compose up` | `SandboxService` on a published port. |

## 3. Bounded context

**Inside this feature:** the Docker launch sequence, slave injection, the link choice (UDS
or TCP), the container handle, `attach`, the control-plane image, and the Docker container
control.

**Outside:** the Docker API client and its generated types (`foundation_deployment_docker`,
used as it is); the slave (feature 01); the control plane (feature 02).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Container | One Docker container, named `ewe-sbx-<sandbox_id>`. | the Docker daemon, through `DockerApi` |
| `DockerApi` | The trait for the calls that the driver needs. | defined here; implemented on `DockerClient` and by a fake |
| `DockerContainerControl`, `DockerContainerOps` | The container handle of feature 00, section 6.13, on Docker. | this feature |
| Slave file in the container | `/.ewe/agent-slave`, copied in before the start. | the driver |
| Run directory | `<state_dir>/<sandbox_id>/`, bind-mounted at `/.ewe/run` (UDS link). | the driver creates; the handle removes |
| Link | How the control plane reaches the slave: `Unix` or `Tcp`. | `DockerDriverConfig` |
| Slave mode | `Main` or `Sidecar`. | `DockerDriverConfig` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| probe | entity | control plane | Pings the daemon. |
| launch | story | control plane | create, inject, start, (sidecar: exec the slave); undo on a failed step. |
| attach | entity | restore | Inspect by name. |
| terminate / kill / cleanup | entity | control plane | Stop and remove the container; remove the run directory. |

## 4. Rules

### 4.1 Law worksheet

```
Entity: Launch (story)
Conserved value:         a failed launch leaves 0 containers named for that sandbox and 0 run directories
Bound:                   each Docker call <= rpc_timeout, except the image pull (pull_timeout)
Inverse pairs:           launch / cleanup
Idempotent verbs:        probe; cleanup; attach
Ordering pairs:          create -> inject -> start (enabling); in Sidecar mode: start -> exec slave
Monotone values:         progress values
Freeze points:           container name, labels, image
Links to other entities: container label ewe.sandbox.id = the sandbox ID; endpoint path or port = what the container was created with

Entity: Container (through the handle)
Conserved value:         one container for each sandbox
Bound:                   memory and CPU limits = the spec
Inverse pairs:           create / remove
Idempotent verbs:        remove of a missing container (the 404 answer is Ok)
Ordering pairs:          stop -> remove
Monotone values:         created -> running -> exited -> removed
Freeze points:           none
Links to other entities: attach finds it by its name
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | The user image is not changed and needs no slave in it. The slave is copied into the container's own layer before the start. |
| I2 | The driver adds no environment variable to the container. (Docker applies the image's own `ENV` to the container's main process; programs that the slave starts get only `Prepare.env` and their own, feature 01 invariant I13.) The container has the labels `ewe.sandbox.id` and `ewe.sandbox.driver`, and the name `ewe-sbx-<id>`. |
| I3 | With `NetworkMode::None` the container has Docker network mode `none`. With `Host` it is on the default bridge network. The Docker "host" network mode is never used. |
| I4 | No container is privileged, and `no-new-privileges` is set. (Feature 11 adds one opt-in exception.) |
| I5 | A failed `launch` removes the container (force) and the run directory. |
| I6 | After `cleanup`, no container with that name exists, and the run directory is gone. A second `cleanup` is `Ok`. |
| I7 | `attach` returns a substrate only for a container that runs and has the label of that sandbox ID. |
| I8 | In `Sidecar` mode the image's own main process runs with its own entry point; the slave is a second process; the sandbox is `Failed` when the main process exits. |
| I9 | The driver uses the generated Docker types. No hand-written request body. |
| I10 | With the TCP link, the port is published on `127.0.0.1` only (or the configured address), never on all interfaces. |

### 4.3 Must have

- `DockerDriver`, `DockerDriverConfig`, `DockerApi`.
- `ProviderId::DockerSandbox`, `SandboxProvider::docker`.
- `infrastructure/sandbox_control_image/` with a `Dockerfile` and a `compose.yaml`.
- A capability field `network_none` (section 6.5).

### 4.4 Must do

- Pull the image when the daemon does not have it, with progress.
- Put the tail of the container log in a start error.

### 4.5 Must not

- Must not mount the Docker socket into a sandbox container.
- Must not use `--privileged`.

## 5. Acceptance scenarios

```gherkin
Feature: Docker driver

  Scenario: A command runs in a container that has no slave in its image
    Given a Docker daemon and the image "alpine:3.22"
    When the caller creates a sandbox with RootFs::ContainerImage("alpine:3.22") and runs ["echo", "hello"]
    Then stdout is "hello\n" and the exit code is 0
    And the image "alpine:3.22" has no file /.ewe/agent-slave

  Scenario: The shared acceptance scenarios pass
    When the shared acceptance scenarios run with a manager that has the Docker driver
    Then each has the same result as on Firecracker

  Scenario: No network unless asked
    Given a spec with NetworkMode::None on a Linux host
    When the sandbox lists /sys/class/net
    Then the only interface is "lo"

  Scenario: The slave can be a sidecar
    Given slave mode Sidecar and the image "nginx:alpine"
    When the caller creates a sandbox and runs ["sh", "-c", "wget -q -O- http://127.0.0.1/ | head -c 15"]
    Then stdout starts with "<!DOCTYPE html>"
    And process 1 in the container is nginx

  Scenario: A sidecar sandbox fails when the main process exits
    Given a sidecar sandbox on an image whose main process exits after 1 second
    When 2 seconds pass
    Then the sandbox state is Stopped after Failed
    And the container is removed

  Scenario: A missing image is pulled
    Given the daemon does not have the image
    When the caller creates a sandbox
    Then the progress has Preparing more than one time before SubstrateStarted
    And the sandbox becomes Ready

  Scenario: An image that does not exist is a clear error
    When the caller creates a sandbox with RootFs::ContainerImage("ewe/does-not-exist:0")
    Then the result is SubstrateApi with status 404
    And no container is left

  Scenario: A restarted control plane finds its container
    Given a ready sandbox and a record store on disk
    When the control plane process restarts and calls restore
    Then get(id) returns the sandbox, and a new command runs in it

  Scenario: Stop removes the container
    When the caller stops a sandbox
    Then no container named ewe-sbx-<id> exists

  Scenario: No daemon is a clear report
    Given no Docker daemon at the socket
    When probe runs
    Then usable is false and the check "daemon" has the connect error

  Scenario: The control plane runs in a container
    Given "docker compose up" in infrastructure/sandbox_control_image
    When a client creates a sandbox through the published port and runs ["echo", "ok"]
    Then stdout is "ok\n"
```

## 6. Technical breakdown

### 6.1 Facts

| Fact | Source |
|---|---|
| `DockerClient` has `create_container`, `start_container`, `wait_container`, `stop_container`, `remove_container`, `inspect_container`. | `foundation_deployment_docker/src/client/mod.rs`, lines 752 to 853 |
| `upload_to_container` puts a tar archive into a container path; `container_logs` reads logs. | `.../client/containers_lifecycle.rs`, lines 541 and 270 |
| `image_pull` streams progress; `image_inspect` reads the configuration. | `.../client/images.rs`, lines 446 and 134 |
| Exec in a container: `exec_create`, `exec_start`. | `.../client/exec.rs`, lines 36 and 69 |
| `ContainerCreateBody` and `ContainerHostConfig` wrap the generated `HostConfig` and `Resources`. | `.../client/mod.rs`, lines 151 and 225 |

### 6.2 Configuration

```rust
pub struct DockerDriverConfig {
    pub link: DockerLink,                // default: Unix on Linux, Tcp on macOS
    pub slave_mode: SlaveMode,           // default Main
    pub publish_address: String,         // default "127.0.0.1"; the address the control plane dials (Tcp link)
    pub pull_timeout: Duration,          // default 10 minutes: the default pull deadline of containerd CRI (image-pull-progress-timeout is 5 min; a full pull gets twice that)
    pub pids_limit: i64,                 // default 4096: the default pids limit of Podman is 2048; a build tool chain needs more
}
pub enum DockerLink { Unix, Tcp }
pub enum SlaveMode { Main, Sidecar }
```

With the TCP link and the default `NetworkMode::None`, `validate` fails with
`UnsupportedNetwork` (`network_none` is false). The README tells a macOS user to set
`NetworkMode::Host` for the Docker driver, and says why.

Why two links. A UDS in a bind-mounted directory works on a Linux Docker host and needs no
container network, so `NetworkMode::None` works. Docker Desktop on macOS runs containers in
a VM, and a UDS does not work across its file sharing; there the link is a published TCP
port, which needs a container network, so `NetworkMode::None` is not available
(`network_none: false`).

### 6.3 Launch

A `#[task]` with the states `Prepare`, `Pull`, `Create`, `Inject`, `Start`, `Sidecar`,
`Done`.

| State | Work | Emits |
|---|---|---|
| `Prepare` | `RootFs::ContainerImage(image)` (`UnsupportedRootFs`). Resolve the slave path for the daemon's architecture (`system info`, `Architecture`). Make the run directory (UDS link). | `Pending(Preparing)` |
| `Pull` | `image_inspect(image)`. On 404: `image_pull(image)`; read the progress stream to its end, with the time-out `pull_timeout`; emit `Pending(Preparing)` for each progress line (at most one each second). | `Pending(Preparing)` |
| `Create` | `create_container` with the body of section 6.4. | |
| `Inject` | `upload_to_container(id, "/")` with a tar stream of two entries: the directory `.ewe/` (mode `0755`) and the file `.ewe/agent-slave` (mode `0755`, owner 0). The tar is written from the slave file in chunks. | |
| `Start` | `start_container`. | `Pending(SubstrateStarted)` |
| `Sidecar` (mode `Sidecar` only) | `exec_create` with the slave command, detached; `exec_start`. | |
| `Done` | Emit `Ready(Ok(substrate))`. | |

The slave command: `["/.ewe/agent-slave", "--listen", <addr>]`, with `<addr>` =
`unix:/.ewe/run/slave.sock` (UDS link) or `tcp:0.0.0.0:1024` (TCP link). With the UDS link
the command also has `--socket-mode 666`: the slave runs as root in the container, and the
control plane on the host is a normal user, so a socket with mode `0600` could not be
opened. The run directory has mode `0700` and belongs to the control plane's user, so no
other host user can reach the socket. `--socket-mode <octal>` is a new option of
`agent-slave` (default `600`), added by this feature with a test.

- Mode `Main`: the container's entry point is the slave command, and its `Cmd` is empty.
- Mode `Sidecar`: the container keeps the image's entry point and command.

The substrate: `root: RootSetup { source: Plain, mode: Overlay }`; no guest mounts (Docker
makes the binds); the endpoint is `Unix { path: <run_dir>/slave.sock }`, or
`Tcp { host: publish_address, port }` with the host port from `inspect_container`
(`NetworkSettings.Ports["1024/tcp"]`).

Each failed step: `remove_container(id, force = true)` when a container exists, remove the
run directory, emit `Ready(Err(..))`. A Docker error maps to
`SubstrateApi { status, message }` with the daemon's message.

### 6.4 The create body

| Field | Value |
|---|---|
| name | `ewe-sbx-<sandbox_id>` |
| `Image` | the image name |
| `Entrypoint`, `Cmd` | by slave mode (section 6.3) |
| `Env` | empty in mode `Main`. In mode `Sidecar` the image's own environment stays (Docker adds it); the driver adds nothing. |
| `Labels` | `ewe.sandbox.id`, `ewe.sandbox.driver = "docker"` |
| `HostConfig.NetworkMode` | `none` for `NetworkMode::None`; `bridge` for `Host` |
| `HostConfig.Binds` | UDS link: `<run_dir>:/.ewe/run`. Each `HostMount`: `<host>:<guest>[:ro]`. |
| `HostConfig.PortBindings`, `ExposedPorts` | TCP link: `1024/tcp` to `publish_address` with host port `0` (the daemon picks one) |
| `HostConfig.Memory` | `memory_mib * 1048576` |
| `HostConfig.NanoCpus` | `vcpus * 1_000_000_000` |
| `HostConfig.PidsLimit` | `pids_limit` |
| `HostConfig.SecurityOpt` | `["no-new-privileges"]` |
| `HostConfig.Privileged` | `false` |
| `HostConfig.AutoRemove` | `false` (the handle removes it, so the log is readable after an exit) |

### 6.5 Capabilities and the `network_none` field

`Capabilities.network_none` and its `validate` rule are in feature 00. It is `true` for
every other driver.

`DockerDriver`: `rootfs_container_image: true`, the other `rootfs_*` false,
`root_modes: [Overlay]`, `host_mounts: true`, `network_host: true`,
`network_none: link == Unix`, `max_vcpus`: the daemon's `NCPU`.

`NetworkMode::Host` means "the container has outbound network through the Docker bridge".

### 6.6 The handle and `attach`

| Method | Work |
|---|---|
| `exited()` | `wait_container(id)`; `SubstrateExit { code, .. }`. |
| `terminate(grace)` | `stop_container(id, timeout = grace)`; then `wait`. `forced` is true when the wait code is 137. |
| `kill()` | `remove_container(id, force = true)`. |
| `log_tail(max)` | `container_logs` with `tail`, cut to `max`. |
| `cleanup()` | `remove_container(id, force = true)` (404 is `Ok`); remove the run directory. |

`attach(ctx, record)`: `inspect_container("ewe-sbx-<id>")`. Running, with the label
`ewe.sandbox.id == id`: `Ok(Some(substrate))` with the endpoint rebuilt from the inspect
answer. Not found or not running: remove what exists (container, run directory), `Ok(None)`.

A control plane that must survive a restart needs a record store on disk. This feature
adds `FileRecordStore` (`native/`): one JSON file for each record in
`<state_dir>/records/`, written with a temporary file and a rename. `agent-sandbox serve`
uses it and calls `restore` at start.

### 6.7 `DockerApi`

A trait with one method for each `DockerClient` call that the driver uses, with the same
argument and result types. `DockerClient` implements it by delegation. The tests use a
fake that records calls. The trait and the driver are in `src/native/docker/`.

### 6.7.1 Container control on Docker

The Docker driver is one user of the Docker Engine. The platform also exposes a container
handle (feature 00, section 6.13), so a caller can start a container, run commands in it,
and stop it without a sandbox.

`DockerContainerControl` implements `ContainerControl`; `DockerContainerOps` implements
`ContainerOps`:

| Method | Work |
|---|---|
| `start` | `create_container` from `ContainerSpec` (image tag, entrypoint, env, network, resources) with the label `ewe.container.control=true`; `start_container`. The ID is the Docker container ID. A failure removes the container. |
| `get` | `inspect_container(id)`. A missing or stopped container gives `None`; it never starts one. |
| `list` | `list_containers` with the label, running only. |
| `info` | `ContainerInfo { id, driver: Docker, image, status, created_unix_ms, resources }`. |
| `capabilities` | `exec: true`, `port_fetch: false`, `signal: true`, `logs: true`. |
| `exec` | Docker exec: create, start, attach the output stream. The `ExecHandle` is the same type as a sandbox `exec`. |
| `fetch` | `UnsupportedCapability { capability: "port_fetch" }`. The Docker Engine has no HTTP fetch to a container port. A caller that needs HTTP uses the sandbox slave, or publishes the port. |
| `signal` | `kill_container(id, signal)`. |
| `wait` | `wait_container(id)`; the exit code from the daemon. |
| `stop(grace)` | `stop_container(id, timeout = grace)`; then `wait`; `forced` is true when the code is 137. |
| `destroy` | `remove_container(id, force = true)`; 404 is `Ok`; remove the run directory. |
| `log_tail` | `container_logs` with `tail`, cut to `max`. |

The driver's substrate handle returns `Some(handle)` from `container()`, and the driver
returns `Some(Arc<DockerContainerControl>)` from `containers()`. The driver and the control
share `DockerApi`; the control uses the same injected `DockerClient`.

`DockerApi` has one method for each `DockerClient` call that the control needs:
`create_container` with the `ContainerSpec` fields, `start_container`, `inspect_container`,
`list_containers`, `exec_create`, `exec_start`, `kill_container`, `wait_container`,
`stop_container`, `remove_container` and `container_logs`. Each is one `DockerClient` call
with the same argument and result types. A call that `DockerClient` does not have yet is a
new method on `DockerClient`, not a raw HTTP request in this crate.

A `ContainerSpec` for Docker maps as follows: `image` is a tag (`docker pull` when it is
missing); `entrypoint` overrides the image entrypoint when it is not empty; `env` is the
container environment; `network == Host` gives the bridge network, `None` gives `none`
(where the daemon allows it); `resources.vcpus` and `resources.memory_mib` map to the
Docker `NanoCpus` and `Memory` fields; `resources.instance_type` is rejected
(`UnsupportedCapability { capability: "instance_type" }`); `labels` are added to the
container.

### 6.8 The control plane in a container

`infrastructure/sandbox_control_image/`:

```dockerfile
FROM debian:stable-slim
COPY agent-sandbox /usr/local/bin/agent-sandbox
COPY agent-slave-x86_64 agent-slave-aarch64 /usr/local/bin/
ENV EWE_SANDBOX_STATE_DIR=/var/run/ewe-sbx
ENTRYPOINT ["/usr/local/bin/agent-sandbox", "serve", "--listen", "tcp:0.0.0.0:7070", "--token-file", "/run/secrets/sandbox_token"]
```

`compose.yaml` mounts `/var/run/docker.sock`, mounts the host directory
`/var/run/ewe-sbx` at the same path (so a run directory path means the same thing to the
control plane and to the daemon), publishes `127.0.0.1:7070`, and gives the token as a
secret. The README states: a process that has the Docker socket controls the host's
Docker; treat this container as trusted.

`HostConfig::discover` already finds the slave beside the program (`<exe_dir>/agent-slave-<arch>`).

## 7. Tests

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| I2, I3, I4, I9, I10, section 6.4 | `create_body` (one case for each link, mode and network mode; asserts every field) | `tests/docker_sim_*.rs` (fake `DockerApi`) | example |
| I5 | `launch_fault_matrix` (a failure at each state: the error, `remove_container(force)`, no run directory) | the same | fault |
| "A missing image is pulled", "An image that does not exist ..." | `pull_when_missing`, `pull_404` | the same | expected outcome |
| I6 | `cleanup_idempotent` | the same | expected outcome |
| I7 | `attach_cases` (running with the label; running without; exited; missing) | the same | expected outcome |
| `network_none` rule | `validate_network_none` | `tests/contract_*.rs` | expected outcome |
| `--socket-mode` | `slave_socket_mode` (the socket file has the given mode) | `tests/slave_*.rs` | example |
| `FileRecordStore` | `file_store_roundtrip`, `file_store_atomic` (a crash between the temporary file and the rename leaves the old record) | `tests/native_host_*.rs` | fault |
| I1, the first scenario, the shared scenarios | `e2e_docker_acceptance` | `tests/e2e_docker.rs` (real daemon; returns with the probe report when it is not usable) | the shared scenarios |
| "No network unless asked" | `e2e_docker_no_network` (Linux) | the same | expected outcome |
| I8, the two sidecar scenarios | `e2e_docker_sidecar`, `e2e_docker_sidecar_main_exit` | the same | expected outcome |
| "A restarted control plane ..." | `e2e_docker_restore` | the same | expected outcome |
| "Stop removes the container" | `e2e_docker_stop` | the same | conservation |
| "No daemon is a clear report" | `probe_no_daemon` | `tests/docker_sim_*.rs` | expected outcome |
| "The control plane runs in a container" | `e2e_control_image` (`docker compose up`, then the acceptance scenarios through `SandboxRpcClient`) | `infrastructure/sandbox_control_image/test/run.sh` | the shared scenarios |
| provider | `provider_docker_sim` | `foundation_deployment_platform/tests/` | expected outcome |
| Container control | `docker_container_control_sim` (a fake `DockerApi`: create, start, get, list, exec, signal, wait, stop, destroy) | `tests/docker_sim_*.rs` | example |
| Container capabilities | `docker_container_capabilities` (`port_fetch` is false; a fetch gives `UnsupportedCapability`) | the same | expected outcome |
| Container control idempotence | `docker_container_destroy_idempotent` (two removes; a missing container is `Ok`) | the same | conservation |
| Container control labels | `docker_container_labels` (the `ewe.container.control` label; `list` returns only those) | the same | expected outcome |

The real-daemon tests run in the existing Docker CI workflow
(`.github/workflows/platform-docker-tests.yml`).

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features "docker testing" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "docker testing" --profile uat docker_sim native_host contract
mise run sandbox:slave
EWE_SANDBOX_SLAVE=<slave path> cargo test -p foundation_platform_sandbox --features "docker testing" --profile uat e2e_docker -- --nocapture
infrastructure/sandbox_control_image/test/run.sh
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes.
- The shared acceptance scenarios give the same results as on Firecracker.
- A `ContainerHandle` starts, runs commands in, and stops a Docker container with no
  sandbox record.
- `agent-sandbox` has the Docker driver when built with the feature, and `doctor` shows
  its report.
- The control-plane image and compose file work, and the README has the trust note.
