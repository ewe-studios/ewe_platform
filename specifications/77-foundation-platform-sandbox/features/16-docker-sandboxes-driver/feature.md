---
feature: "Docker Sandboxes product driver"
status: "draft"
priority: "low"
model: "sonnet"
phase: "later"
depends_on: ["06-programs-rpc-provider", "10-docker-driver"]
crates: ["foundation_platform_sandbox", "foundation_deployment_platform"]
---

# Feature 16: The driver for the Docker Sandboxes product (`sbx`)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** remote procedure call (RPC); virtual machine (VM);
Kernel-based Virtual Machine (KVM); command-line interface (CLI); JavaScript Object
Notation (JSON); Transmission Control Protocol (TCP); application programming interface
(API); personal access token (PAT); continuous integration (CI); behaviour-driven
development (BDD); AeroSpace and Defence Industries Association of Europe (ASD), the owner
of the STE standard ASD-STE100.

> **Draft, not a must have (owner decision D14, 2026-11-12).** The Docker Sandboxes product
> is not ready. This feature is not scheduled and no agent gets it. No other feature
> depends on it. Before it is scheduled, read the product documents again and update
> section 6.1, because the `sbx` commands and their output can change.

## 1. Goal

Docker Sandboxes is a Docker product. Its program is `sbx`. Each sandbox is a microVM with
its own Docker daemon. This feature adds `SbxDriver`: a `SandboxDriver` that makes one
Docker Sandboxes sandbox for each sandbox of this crate, puts `agent-slave` into it, and
gives the control plane a TCP port to the slave.

This is not feature 10. Feature 10 talks to a Docker Engine and starts a container.
This feature talks to the `sbx` program and starts a microVM that Docker manages.

```mermaid
sequenceDiagram
  participant M as SandboxManager
  participant D as SbxDriver
  participant X as sbx program
  participant V as Docker sandbox (microVM)
  M->>D: launch(id, spec)
  D->>X: sbx create --name ewe-sbx-ID --publish 1024 ... shell
  D->>X: sbx cp agent-slave ewe-sbx-ID:/tmp/ewe-agent-slave
  D->>X: sbx exec -u root ewe-sbx-ID (start the slave); the child stays
  X->>V: the slave listens on TCP 1024
  D->>X: sbx ports ewe-sbx-ID --json
  D-->>M: Substrate with endpoint Tcp(127.0.0.1, host port)
  M->>V: SlaveService Info, Prepare, Exec
```

_Caption: the driver uses four `sbx` commands to start a sandbox, then the control plane talks to the slave on a published port._

## 2. Story

- **S1**: "A caller on a machine with `sbx` creates a sandbox and runs a command in it,
  with the same API as on Firecracker."
- **S2**: "The machine has no `sbx`, an old `sbx`, or no Docker sign-in; the caller gets a
  clear report."
- **S3**: "The caller stops the sandbox; Docker has no sandbox left with its name."
- **S4**: "The control plane restarts; it finds its Docker sandboxes again, or removes
  them."
- **S5**: "A step of the launch fails; nothing stays."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Check the host | control plane | `probe` | `sbx` found, output format correct, signed in, slave file present. |
| U2 | Launch | control plane | `launch` | A running Docker sandbox with the slave; a TCP endpoint. |
| U3 | Attach | control plane | `restore` | The substrate of a Docker sandbox that still runs. |
| U4 | End | control plane | `terminate`, `kill`, `cleanup` | The Docker sandbox is removed. |

## 3. Bounded context

**Inside this feature:** the `sbx` command lines, the parse of their JSON output, the
launch sequence, the held `sbx exec` process, the handle, `attach`.

**Outside:** the `sbx` program and its daemon `sandboxd` (a Docker product, used as it is);
the slave (feature 01); the control plane (feature 02); `ProcessSpawner` and `HostFs`
(features 00 and 05); `FileRecordStore` (feature 10).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Docker sandbox | One sandbox of the product, named `ewe-sbx-<sandbox_id>`. | `sandboxd`, through `SbxApi` |
| `SbxApi` | The trait for the `sbx` operations that the driver needs. | defined here; implemented by `SbxCli` and by a fake |
| `SbxCli` | Runs the `sbx` program through `ProcessSpawner`. | this feature |
| Held process | The `sbx exec` child that runs the slave. It lives as long as the slave. | the driver starts it; the handle signals it |
| Run directory | `<state_dir>/<sandbox_id>/`: the output files of each `sbx` command. | the driver creates; the handle removes |
| Published port | The host port that Docker maps to sandbox port 1024. | `sandboxd` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| probe | entity | control plane | Runs `sbx version` and `sbx ls --json`. |
| launch | story | control plane | create, inject, start the slave, read the port; undo on a failed step. |
| attach | entity | restore | Finds the sandbox by name and reads its port. |
| terminate / kill / cleanup | entity | control plane | Stops and removes the Docker sandbox; removes the run directory. |

## 4. Rules

### 4.1 Law worksheet

```
Entity: Launch (story)
Conserved value:         a failed launch leaves 0 Docker sandboxes named for that sandbox, 0 held processes and 0 run directories
Bound:                   each sbx command <= command_timeout, except create (create_timeout)
Inverse pairs:           launch / cleanup
Idempotent verbs:        probe; cleanup; attach
Ordering pairs:          create -> inject -> start slave -> read port (enabling)
Monotone values:         progress values
Freeze points:           sandbox name, template, CPU and memory values
Links to other entities: Docker sandbox name = "ewe-sbx-" + sandbox ID; endpoint port = the host port of sandbox port 1024

Entity: Docker sandbox (through the handle)
Conserved value:         one Docker sandbox for each sandbox
Bound:                   CPU and memory = the spec
Inverse pairs:           create / remove
Idempotent verbs:        remove of a missing sandbox
Ordering pairs:          stop -> remove
Monotone values:         created -> running -> stopped -> removed
Freeze points:           none
Links to other entities: attach finds it by its name
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | The template image is not changed and needs no slave in it. The slave is copied into the sandbox after the create. |
| I2 | The driver gives no `--env`, no workspace path, no `--mount`, no kit and no secret to `sbx create`. The sandbox has no host directory. |
| I3 | The endpoint host is a loopback address. A port row with another `host_ip` fails the launch with `SubstrateApi`. |
| I4 | A failed `launch` signals the held process, removes the Docker sandbox (`sbx rm --force`) and removes the run directory. |
| I5 | After `cleanup`, `sbx ls --json` has no row with that name, and the run directory is gone. A second `cleanup` is `Ok`. |
| I6 | `attach` returns a substrate only for a sandbox with that name, the status `running` and a port row for sandbox port 1024. |
| I7 | The driver reads only structured output (`--json`). It does not parse a table or a message for a decision. Text from `stderr` goes only into error messages. |
| I8 | Each `sbx` child gets only the environment in `SbxDriverConfig.env`. Nothing is inherited (feature 00, `ProcessSpec.env`). |
| I9 | The output of an `sbx` command is read with a bound (`max_output_bytes`). More output is an error, not a silent cut. |
| I10 | The driver never passes `--cloud`. |

### 4.3 Must have

- `SbxDriver`, `SbxDriverConfig`, `SbxApi`, `SbxCli` in `src/native/sbx/`, behind the
  crate feature `docker-sbx`.
- `DriverId::DockerSbx` (display string `docker-sbx`), `HostConfig.sbx_path`.
- `ProviderId::DockerSbx`, `SandboxProvider::docker_sbx`.
- The `agent-sandbox` option `--driver docker-sbx`, and its report in `doctor`.

### 4.4 Must do

- Put the tail of the failed command's `stderr` file in each `SubstrateApi` error.
- Reject a spec that Docker Sandboxes cannot give, before any `sbx` command runs
  (section 6.5).

### 4.5 Must not

- Must not use the Docker Sandboxes cloud, the HTTP API or the `sbx` secret store
  (section 6.9).
- Must not put this driver in the default `driver_order`. A caller selects it by name.
- Must not answer a confirmation prompt. Each command that can ask has `--force`.

## 5. Acceptance scenarios

```gherkin
Feature: Docker Sandboxes driver

  Scenario: A command runs in a Docker sandbox
    Given a host where the probe of docker-sbx is usable
    When the caller creates a sandbox with driver docker-sbx and RootFs::ContainerImage(<template>)
    And runs ["echo", "hello"]
    Then stdout is "hello\n" and the exit code is 0

  Scenario: The shared acceptance scenarios pass
    Given a ready sandbox on docker-sbx
    Then each shared scenario of requirements section 8 gives the same result as on Firecracker

  Scenario: The create command line is exact
    Given a spec with 2 vcpus, 1024 MiB and RootFs::ContainerImage("example/t:1")
    When the driver launches with the fake SbxApi
    Then the create request is name "ewe-sbx-<id>", cpus 2, memory_mib 1024, template "example/t:1", publish 1024, agent "shell"
    And it has no env, no workspace and no mount

  Scenario: No sbx program is a clear report
    Given sbx_path names a file that does not exist
    When the driver probes
    Then usable is false and the check "sbx program" has ok false

  Scenario: An old sbx is a clear report
    Given sbx ls --json prints an object, not an array
    When the driver probes
    Then usable is false and the check "sbx output format" names version 0.48.0

  Scenario: No sign-in is a clear report
    Given sbx ls --json exits with a status that is not 0
    When the driver probes
    Then usable is false and the check "sbx daemon and sign-in" has the stderr tail

  Scenario: Less than 512 MiB is refused before any command
    Given a spec with 256 MiB
    When the caller creates a sandbox with driver docker-sbx
    Then the result is InvalidSpec { field: "memory_mib" } and the fake SbxApi got no call

  Scenario: A failed step leaves nothing
    Given the fake SbxApi fails at <step>
    When the driver launches
    Then the result is an error
    And the held process got a signal when it existed
    And remove was called when create was done
    And the run directory does not exist

    Examples:
      | step |
      | create |
      | copy_in |
      | exec_hold |
      | ports |

  Scenario: A port on a public address is refused
    Given sbx ports gives host_ip "0.0.0.0" for sandbox port 1024
    When the driver launches
    Then the result is SubstrateApi and the Docker sandbox is removed

  Scenario: Stop removes the Docker sandbox
    Given a ready sandbox on docker-sbx
    When the caller stops it
    Then sbx ls --json has no row named "ewe-sbx-<id>"

  Scenario: A restarted control plane finds or removes its sandbox
    Given a ready sandbox on docker-sbx and a record store on disk
    When the control plane stops with DropPolicy::Detach and a new one calls restore
    Then the sandbox is Ready and runs a command, or it is Failed and no Docker sandbox has its name
```

## 6. Technical breakdown

### 6.1 Facts

Source: the `docker/docs` repository at the commit of 2026-11-12 (`learnings.md`,
section 8, has each file).

| Fact | Source file in `docker/docs` |
|---|---|
| `sbx` is its own program. It needs no Docker Desktop and no Docker Engine. Local sandboxes need KVM (Linux, Ubuntu 24.04 or later), macOS 14 on Apple silicon, or Windows 11 with the Windows Hypervisor Platform. | `content/manuals/ai/sandboxes/install.md` |
| A sign-in is necessary (`sbx login`; a PAT with `--password-stdin` where no browser exists). | `install.md`, `workflows/automation.md` |
| `sbx create AGENT [PATH...]`: flags `--name`, `--cpus`, `--memory` (minimum 512 MiB), `--template`, `--publish`, `--no-progress`, `--env`. With no path, the sandbox has no host directory (version 0.42.0 or later). | `data/sbx_cli/sbx_create.yaml`, `architecture.md` |
| The agent `shell` has no agent program in it. | `agents/shell.md` |
| `sbx exec [-u USER] [-w DIR] [-e K=V] SANDBOX COMMAND [ARG...]` returns the command's exit status. 125 means that `sbx` could not run it; 127 means that the program is missing. It has no detach flag. | `data/sbx_cli/sbx_exec.yaml`, `workflows/automation.md` |
| `sbx cp SRC DST`; a sandbox path is `NAME:/absolute/path`. | `usage.md`, lines 421 to 433 |
| `--publish SANDBOX_PORT` lets the system pick the host port. The default host address is `127.0.0.1`. `sbx ports NAME --json` gives rows with `host_ip`, `host_port`, `sandbox_port`, `protocol`. | `usage.md`, lines 439 to 483; `workflows/automation.md` |
| From version 0.48.0: `sbx ls --json` prints an array of sandboxes; `sbx rm --json` and `sbx stop --json` print an array of results with `action`, `name`, `error`. Field names are snake case. | `workflows/automation.md` |
| Confirmations need a terminal. `sbx rm --force` skips them. | `workflows/automation.md` |
| The default user in the Docker images is `agent`, not root, with `sudo`. | `customize/author/base-images.md` |
| All outbound traffic of a sandbox goes through a proxy on the host with a policy. | `architecture.md` |

### 6.2 Configuration

```rust
pub struct SbxDriverConfig {
    /// The full environment of each sbx child. The entry point of agent-sandbox fills it
    /// from its own environment with HOME, PATH, XDG_CONFIG_HOME, XDG_DATA_HOME,
    /// XDG_RUNTIME_DIR and USER, when each is set. Core code does not read the process
    /// environment (root rule 11).
    pub env: Vec<(String, String)>,
    pub command_timeout: Duration,   // default 60 s: the default request time-out of the Docker CLI's API client
    pub create_timeout: Duration,    // default 10 minutes: the same as DockerDriverConfig.pull_timeout (a create can pull the template)
    pub status_poll: Duration,       // default 10 s: the default period of a Kubernetes liveness probe
    pub max_output_bytes: usize,     // default 1 MiB: the default max body of nginx (client_max_body_size)
    pub max_vcpus: u32,              // default 16: the sbx limit on Linux arm64, the lowest limit in its documents
}
```

`HostConfig.sbx_path: Option<PathBuf>` names the `sbx` program. `agent-sandbox` fills it
from `--sbx-path`, or from a search of `PATH` at its entry point.

### 6.3 `SbxApi`

```rust
pub trait SbxApi: Send + Sync + 'static {
    fn version(&self) -> BoxFuture<'_, SandboxResult<String>>;
    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<SbxSandbox>>>;
    fn create(&self, request: SbxCreate) -> BoxFuture<'_, SandboxResult<()>>;
    fn copy_in(&self, name: &str, host_file: &Path, sandbox_path: &str) -> BoxFuture<'_, SandboxResult<()>>;
    /// Starts `sbx exec` and returns the child. The child lives as long as the command.
    fn exec_hold(&self, name: &str, user: &str, argv: &[String], run_dir: &Path)
        -> SandboxResult<Box<dyn ChildProcess>>;
    fn ports(&self, name: &str) -> BoxFuture<'_, SandboxResult<Vec<SbxPort>>>;
    fn stop(&self, name: &str) -> BoxFuture<'_, SandboxResult<()>>;
    /// `Ok` when the sandbox is removed or was not there.
    fn remove(&self, name: &str) -> BoxFuture<'_, SandboxResult<()>>;
}
pub struct SbxCreate {
    pub name: String, pub cpus: u32, pub memory_mib: u64,
    pub template: String, pub publish: u16,
}
#[derive(serde::Deserialize)]
pub struct SbxSandbox { pub name: String, pub status: String }
#[derive(serde::Deserialize)]
pub struct SbxPort { pub host_ip: String, pub host_port: u16, pub sandbox_port: u16, pub protocol: String }
#[derive(serde::Deserialize)]
pub struct SbxResult { pub action: String, pub name: String, pub error: Option<String> }
```

The three `Deserialize` structures ignore unknown fields. Docker gives no schema file for
the CLI output, so no generator exists for them (root rule 5 does not apply). The structures
are in one file, `src/native/sbx/output.rs`, with the document source in the doc comment.

`SbxCli` runs one `sbx` child for each call:

| Method | Command line | Success |
|---|---|---|
| `version` | `sbx version` | exit 0; the trimmed `stdout` |
| `list` | `sbx ls --json` | exit 0; `stdout` parses as `Vec<SbxSandbox>` |
| `create` | `sbx create --name <name> --cpus <n> --memory <n>m --template <template> --publish <port> --no-progress shell` | exit 0 |
| `copy_in` | `sbx cp <host_file> <name>:<sandbox_path>` | exit 0 |
| `exec_hold` | `sbx exec -u <user> <name> <argv...>` | the child started |
| `ports` | `sbx ports <name> --json` | exit 0; `stdout` parses as `Vec<SbxPort>` |
| `stop` | `sbx stop --json <name>` | exit 0, and no result has the action `failed` |
| `remove` | `list`; when no row has the name, `Ok`. Else `sbx rm --force --json <name>`. | exit 0, and no result has the action `failed` |

How one command runs (a `#[task]`, no thread and no blocking wait):

1. `ProcessSpawner::spawn` with `program = sbx_path`, `env = config.env`,
   `cwd = run_dir`, `stdout = <run_dir>/<n>.out`, `stderr = <run_dir>/<n>.err`,
   `kill_with_parent = true`. `<n>` is a counter of the handle.
2. Wait for `ChildProcess::wait()` or the time-out (the clock of `Ctx`), whichever is
   first. On the time-out: signal 9, then the error `Timeout { operation }`.
3. `HostFs::read(out, max_output_bytes + 1)`. More than `max_output_bytes` is
   `SubstrateApi { status: 0, message: "sbx output is too large" }`.
4. An exit status that is not 0 is `SubstrateApi { status: <exit status>, message: <tail of the stderr file, 4 KiB> }`.
5. A parse error is `SubstrateApi { status: 0, message: <the serde error> }`.

`exec_hold` uses the files `slave.out` and `slave.err`, and `kill_with_parent = false`.
The reason: a control plane with `DropPolicy::Detach` must not kill the slave when it
stops.

### 6.4 Probe

| Check name | How | Not ok |
|---|---|---|
| `sbx program` | `HostFs::exists(sbx_path)`; then `version` | the path is `None` or missing, or the exit is not 0 |
| `sbx daemon and sign-in` | `list` exits 0 | message: the `stderr` tail and the text "run sbx login" |
| `sbx output format` | the `list` output is a JSON array | message: "sbx 0.48.0 or later is necessary" |
| `slave file` | `HostConfig.slave_path` exists | message: the path |

`usable` is true when each check is ok. The probe makes its own temporary run directory
and removes it.

### 6.5 Capabilities and validation

`rootfs_container_image: true`, the other `rootfs_*` false; `root_modes: [Overlay]`;
`host_mounts: false`; `network_host: true`; `network_none: false`;
`max_vcpus: config.max_vcpus`.

Why. `RootFs::ContainerImage(name)` is the `--template` value. `NetworkMode::Host` means
"outbound traffic follows the user's `sbx` network policy". Docker Sandboxes has no mode
with no network, and the link needs a published port, so `NetworkMode::None` gives
`UnsupportedNetwork`. A host directory is mounted at its own host path, not at a guest
path of the caller's choice, so `HostMount` gives `UnsupportedMount`.

One more rule, in `SbxDriver::launch` before the first command: `memory_mib < 512` gives
`InvalidSpec { field: "memory_mib" }`.

### 6.6 Launch

A `#[task]` with the states `Prepare`, `Create`, `Inject`, `StartSlave`, `ReadPort`,
`Done`. Each wait has the launch `deadline` as its upper limit.

| State | Work | Emits |
|---|---|---|
| `Prepare` | The memory rule. Make the run directory (mode `0700`). | `Pending(Preparing)` |
| `Create` | `create(SbxCreate { name: "ewe-sbx-<id>", cpus: vcpus, memory_mib, template, publish: 1024 })`, time-out `create_timeout`. | `Pending(SubstrateStarted)` |
| `Inject` | `copy_in(name, slave_path, "/tmp/ewe-agent-slave")`. | |
| `StartSlave` | `exec_hold(name, "root", ["/bin/sh", "-c", "chmod 0755 /tmp/ewe-agent-slave && exec /tmp/ewe-agent-slave --listen tcp:0.0.0.0:1024"], run_dir)`. Then `try_wait()`: an exit at once is the error of section 6.8. | |
| `ReadPort` | `ports(name)`. Take the row with `sandbox_port == 1024` and `protocol` that starts with `tcp`. No row: `SubstrateApi`. `host_ip` must parse as a loopback address (I3). | |
| `Done` | Emit `Ready(Ok(substrate))`. | |

The slave runs as root, as in the Docker driver: it must change the user and the root
directory for the programs that it starts. The shell is necessary only for the `chmod`,
because `sbx cp` does not promise to keep the file mode. The control plane starts no
program through a shell.

The substrate: `root: RootSetup { source: Plain, mode: Overlay }`; no guest mounts; the
endpoint is `Tcp { host: <host_ip>, port: <host_port> }`.

Each failed step: signal 15 to the held process when it exists, `remove(name)` when
`Create` was done, remove the run directory, emit `Ready(Err(..))`. An error from the
undo is logged, and the first error is the result.

### 6.7 The handle and `attach`

| Method | Work |
|---|---|
| `exited()` | With a held process: `ChildProcess::wait()`. With no held process (after `attach`): call `list` each `status_poll` until no row has the name with the status `running`; then `SubstrateExit { code: None, signal: None, forced: false }`. |
| `terminate(grace)` | `stop(name)`. Then wait up to `grace` for `exited()`. On the time-out: `kill()`, and `forced` is true. |
| `kill()` | Signal 9 to the held process when it exists; `remove(name)`. |
| `log_tail(max)` | `HostFs::tail(<run_dir>/slave.err, max)`. |
| `cleanup()` | `remove(name)`; remove the run directory. |

`attach(ctx, record)`: `list`. A row named `ewe-sbx-<id>` with the status `running`: call
`ports`; with a port row for 1024 on a loopback address, return `Ok(Some(substrate))`
with no held process. Each other case: `remove(name)`, remove the run directory,
`Ok(None)`.

`restore` (feature 02) then calls `Info` on the endpoint. When the slave does not answer,
`restore` marks the sandbox `Failed` and calls `cleanup`. So the design is correct for
both answers to this question: "does the slave stay when the `sbx exec` client stops?".
The first real run records the answer in `progress.md`.

### 6.8 Errors

| Fault | Result |
|---|---|
| `sbx` exit status not 0 | `SubstrateApi { status, message: stderr tail }` |
| A command passes its time-out | `Timeout { operation: "sbx <verb>" }` |
| The held process exits with 125 | `exit_error` gives `SubstrateApi { status: 125, message: log tail }` |
| The held process exits with 127 | `exit_error` gives `SubstrateApi { status: 127, message: "the template has no /bin/sh" }` |
| The held process exits with another status | the default `SubstrateExited { code, log_tail }` |
| The launch deadline passes | `BootTimeout` (feature 02), then the undo of section 6.6 |

### 6.9 Not in this feature

Each item is in requirements section 7.3 and becomes a GitHub issue when the feature
lands.

- **The Docker Sandboxes cloud (`sbx --cloud`).** A cloud sandbox publishes a port as a
  public HTTPS address. The slave has no authentication on its link today, so this would
  open the slave to the Internet. It needs slave authentication first.
- **The Docker Sandboxes HTTP API (version 1).** Its management address is the Docker
  cloud. The documents name a local socket but give no path for it. When Docker documents
  the local socket, a second `SbxApi` implementation with generated types can replace
  `SbxCli`; the driver does not change.
- **Host directories, kits, secrets, the gateway for tool servers.** These are functions
  of the product for coding agents. A caller who needs them uses `sbx` directly.

## 7. Tests

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| I2, I10, "The create command line is exact" | `sbx_create_request`; `sbx_cli_command_lines` (each `SbxCli` method through a fake `ProcessSpawner`: the exact argument list, the environment, no `--cloud`) | `tests/sbx_sim_*.rs` | example |
| I7, I9 | `sbx_cli_parse` (array and object for `ls`; unknown fields; a result with `failed`; output over the bound) | the same | example, fault |
| I8 | `sbx_cli_env_only_config` | the same | example |
| I3, "A port on a public address is refused" | `sbx_port_must_be_loopback` | the same | expected outcome |
| I4, "A failed step leaves nothing" | `sbx_launch_fault_matrix` (a failure at each step) | the same | fault |
| I5 | `sbx_cleanup_idempotent` | the same | expected outcome |
| I6 | `sbx_attach_cases` (running with the port; running without; stopped; missing) | the same | expected outcome |
| the three probe scenarios | `sbx_probe_no_program`, `sbx_probe_old_format`, `sbx_probe_no_sign_in` | the same | expected outcome |
| the memory scenario, section 6.5 | `sbx_memory_minimum`, `sbx_capabilities` | the same | expected outcome |
| section 6.8 | `sbx_exit_error_cases`, `sbx_command_timeout` (`SimulatedClock`) | the same | fault |
| `exited()` after `attach` | `sbx_exited_by_poll` (`SimulatedClock`) | the same | expected outcome |
| I1, the first scenario, the shared scenarios | `e2e_sbx_acceptance` | `tests/e2e_sbx.rs` (real `sbx`; returns with the probe report when it is not usable) | the shared scenarios |
| "Stop removes the Docker sandbox" | `e2e_sbx_stop` | the same | conservation |
| "A restarted control plane ..." | `e2e_sbx_restore` (accepts both results) | the same | expected outcome |
| provider | `provider_docker_sbx_sim` | `foundation_deployment_platform/tests/` | expected outcome |

CI. The real tests need a Docker account. The workflow `sandbox-e2e` (feature 06) gets a
job `sbx` on `ubuntu-24.04`: install `docker-sbx` from Docker's `apt` repository, sign in
with the secrets `DOCKER_SBX_USER` and `DOCKER_SBX_PAT`, build the slave, run `e2e_sbx`.
The job has the condition that the secret exists. A pull request from a fork has no
secret, so the job does not run there. The simulated tests run in the normal crate tests
on each pull request.

The crate has no code for one operating system only in this feature, so root rule 15
adds no key to `ci/packages.toml`.

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features "docker-sbx testing" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "docker-sbx testing" --profile uat sbx_
cargo test -p foundation_deployment_platform --profile uat provider_docker_sbx_sim
mise run sandbox:slave
EWE_SANDBOX_SLAVE=<slave path> cargo test -p foundation_platform_sandbox --features "docker-sbx testing" --profile uat e2e_sbx -- --nocapture
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes. The real tests passed one
  time or more on a machine with `sbx`, and `progress.md` has the `sbx` version of that
  run.
- `agent-sandbox doctor` shows the `docker-sbx` report, and
  `agent-sandbox run --driver docker-sbx --rootfs <template> -- echo hello` prints `hello`
  (`--rootfs` takes the template name for this driver, as it takes the image name for the
  Docker driver).
- `progress.md` records: the template name that the README recommends, and whether the
  slave stays when the `sbx exec` client stops.
- The README says: this driver needs a Docker sign-in; the sandbox's outbound traffic
  follows the user's `sbx` policy; the slave port is open to each user of the host
  machine on the loopback address.
- The versions of the changed crates are bumped, and `requirements.md` and `progress.md`
  are current.
