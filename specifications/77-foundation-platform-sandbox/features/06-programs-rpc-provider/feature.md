---
feature: "Programs, RPC and Provider"
status: "pending"
priority: "high"
model: "sonnet"
phase: "MVP-A"
depends_on: ["05-firecracker-driver"]
crates: ["foundation_platform_sandbox", "foundation_deployment_platform", "agent_slave"]
---

# Feature 06: The programs, `SandboxService`, the remote client and the `Provider` facade (MVP-A)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); remote procedure
call (RPC); Unix domain socket (UDS); Transmission Control Protocol (TCP); Kernel-based
Virtual Machine (KVM); command-line interface (CLI); behaviour-driven development (BDD);
continuous integration (CI); AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. Goal

Give the core its boundaries on a host:

1. The command structures of `agent-sandbox` and `agent-slave`, in the library
   (`foundation_platform_sandbox::cli`).
2. The `SandboxService` server (the control plane over RPC) and `SandboxRpcClient`, which
   implements `SandboxControl` for a remote control plane.
3. The two program entry points.
4. The `Provider` facade in `foundation_deployment_platform`.
5. The CI job that runs the real Firecracker tests.

This feature completes MVP-A: a person runs
`agent-sandbox run --rootfs <image> -- echo hello` on a Linux host with KVM and sees
`hello`; Rust code gets the same through the library, through RPC, or through a `Provider`.

## 2. Story

- **S1**: "A person runs one command and sees a program run in a sandbox."
- **S2**: "An operator starts `agent-sandbox serve`, and other programs (agent-rpc, a CLI
  on the same machine) create and use sandboxes through it."
- **S3**: "A Rust program uses `SandboxRpcClient` and gets the same results as with
  `SandboxManager` in its own process."
- **S4**: "A caller with no token, or a wrong token, calls the served control plane."
- **S5**: "A caller that uses the platform `Provider` trait starts and stops a sandbox the
  same way as a Docker container."
- **S6**: "A person asks what this machine can run, and why not the rest."
- **S7**: "The real microVM tests run in CI."
- **S8**: "`agent-slave` is built as a static program and starts as PID 1."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Run one command | person | `agent-sandbox run ...` | Output streams; the exit code is the program's. |
| U2 | Serve | operator | `agent-sandbox serve --listen ...` | `SandboxService` on a UDS or TCP. |
| U3 | Remote control | program | `SandboxRpcClient` | Every `SandboxControl` and `Sandbox` call, over RPC. |
| U4 | Manage | person | `agent-sandbox create | exec | list | stop | kill` with `--connect` | The same calls from a shell. |
| U5 | Doctor | person | `agent-sandbox doctor` | One report for each driver. |
| U6 | Provider | orchestration code | `Provider::launch` and others | A `SandboxHandle`. |
| U7 | Slave start | kernel, container, test | `agent-slave [--listen ...]` | The slave serves. |

## 3. Bounded context

**Inside this feature:** argument parsing, the mapping between `SandboxService` messages
and `SandboxControl` calls (both directions), token checks, the `Provider` adapter, the
programs' `main` functions, the CI job, the user documentation.

**Outside:** all sandbox behaviour (features 01, 02, 05). A boundary translates. It does
not decide.

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| `SandboxCli`, `SlaveCli` | The parsed arguments of the two programs. | `cli::sandbox`, `cli::slave` |
| `ControlService` | The `SandboxService` implementation on an `Arc<dyn SandboxControl>`. | `shared::control::service` |
| `SandboxRpcClient` | `SandboxControl` on a `Transport`. | `shared::control::rpc_client` |
| Remote sandbox handle | `SandboxOps` on the scoped RPC calls. | `shared::control::rpc_client` |
| Access token | A shared secret for TCP listeners. | the operator; read by the boundary |
| `SandboxProvider`, `SandboxHandle` | The `Provider` adapter and its handle. | `foundation_deployment_platform::providers::sandbox` |
| `ProviderId` | The identifier of a provider kind. | `foundation_deployment_platform::providers` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| parse | entity | program start | Arguments to a command value, or a usage error. |
| serve | entity | `serve` | Binds a listener; routes calls to `ControlService`. |
| translate in | entity | an RPC call | A `SandboxControl` call with the same data. |
| translate out | entity | a `SandboxRpcClient` call | An RPC call with the same data. |
| authorise | entity | each call on TCP | Compares the bearer token in constant time. |
| adapt | entity | a `Provider` method | Blocks on the matching `SandboxControl` call. |

**The driver rule.** `ControlService`, `SandboxRpcClient`, the CLI and the `Provider` hold
no rule about sandboxes. A check, a retry or a time-out that appears only in one of them is
a defect: it moves a rule away from the core and its tests.

## 4. Rules

### 4.1 Law worksheet

```
Entity: The RPC pair (ControlService + SandboxRpcClient)
Conserved value:         for every call: client(service(control)).call(x) has the same outcome as control.call(x)
Bound:                   message <= max_message_bytes; data chunk <= chunk_bytes
Inverse pairs:           translate out / translate in
Idempotent verbs:        the same as the wrapped call
Ordering pairs:          none of its own
Monotone values:         none
Freeze points:           none
Links to other entities: errors keep their variant (SandboxError::to_connect / from_connect)

Entity: Served listener
Conserved value:         none
Bound:                   UDS file mode 0600
Inverse pairs:           bind / close
Idempotent verbs:        none
Ordering pairs:          authorise -> every call on TCP (enabling)
Monotone values:         none
Freeze points:           the token at start
Links to other entities: a UDS has no token; file permissions guard it

Entity: Provider adapter
Conserved value:         launch(spec) has the outcome of block_on(control.create(spec))
Bound:                   none of its own
Inverse pairs:           launch / stop
Idempotent verbs:        stop; is_running; host_health
Ordering pairs:          launch -> stop (enabling)
Monotone values:         none
Freeze points:           none
Links to other entities: is_running(handle) == (handle.sandbox.info().state == Ready)

Entity: CLI run
Conserved value:         the program's standard output and standard error bytes = the CLI's, in order per stream
Bound:                   none
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          create -> exec -> stop (always stop, also on an error or Ctrl-C)
Monotone values:         none
Freeze points:           none
Links to other entities: CLI exit code = the program's exit code, or 128 + signal
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | Every shared acceptance scenario (feature 02, section 7) has the same result through `SandboxManager` and through `SandboxRpcClient` + `ControlService`. |
| I2 | An error keeps its `SandboxError` variant and fields across RPC. |
| I3 | A TCP listener refuses every call with no token or a wrong token with `Unauthorized`, and the check is constant-time. A TCP listener does not start with no token. |
| I4 | A UDS listener file has mode `0600`. |
| I5 | `agent-sandbox run` stops its sandbox on every exit path: normal end, an error, `SIGINT` and `SIGTERM`. |
| I6 | `agent-sandbox run` exits with the program's exit code, or `128 + signal` when a signal ended the program. Its own failures use 125. |
| I7 | A `Provider` method has the outcome of the `SandboxControl` call that it wraps; the error is the same typed error. |
| I8 | `is_running(handle)` is true exactly when the sandbox state is `Ready`. |
| I9 | Without the `sandbox` feature, `foundation_deployment_platform` builds and tests as before. |
| I10 | The boundaries have no sandbox logic: a source scan finds no spec validation, retry loop or time-out constant in `cli/`, `service.rs`, `rpc_client.rs`, the `Provider` module and the `main` files. |
| I11 | `agent-slave` is a static program for both musl targets. |
| I12 | Blocking calls of this spec are only in `Provider` methods and `main` functions. |

### 4.3 Must have

- `agent-sandbox` with the sub-commands `run`, `serve`, `create`, `exec`, `list`, `stop`,
  `kill`, `doctor`.
- `agent-slave`.
- `ControlService`, `SandboxRpcClient`.
- `ProviderId::Firecracker`, `SandboxProvider`, `SandboxHandle`.
- The CI job, the README, `docs/sandbox.md`.

### 4.4 Must do

- Bump crate versions and every consumer (`.agents` reminder 12).
- Declare in `ci/packages.toml` which operating systems run which tests, with a reason for
  each opt-out (`.agents` rule 16).
- File the GitHub issues of section 6.10 (when the owner approves issue filing; decision
  D9).

### 4.5 Must not

- Must not call `Ctx::host()` in library code. Only `main` functions do.
- Must not print the token.
- Must not put the sandbox checks behind the `vms` feature.

## 5. Acceptance scenarios

```gherkin
Feature: Programs, RPC and Provider

  Scenario: One command runs in a sandbox
    Given a usable Firecracker host
    When "agent-sandbox run --rootfs <image> -- sh -c 'echo hello; exit 3'" runs
    Then its standard output is "hello\n"
    And its exit code is 3
    And no firecracker process of it is left

  Scenario: A signal ends the run and cleans up
    Given "agent-sandbox run --rootfs <image> -- sleep 60" is running
    When it gets SIGINT
    Then it exits in less than shutdown_grace + 3 s
    And no firecracker process of it is left

  Scenario: The remote client behaves as the local manager
    Given "agent-sandbox serve --listen unix:<path>" is running
    When the shared acceptance scenarios run with SandboxRpcClient on that socket
    Then each has the same result as with SandboxManager

  Scenario: An error keeps its type across RPC
    Given a served control plane
    When the remote caller creates a sandbox with a root kind that no driver accepts
    Then the error is UnsupportedRootFs

  Scenario: A TCP listener needs the token
    Given "agent-sandbox serve --listen tcp:127.0.0.1:0 --token-file <f>" is running
    When a client with no token calls List
    Then the call fails with Unauthorized
    When a client with the token calls List
    Then the call succeeds

  Scenario: A TCP listener does not start without a token
    When "agent-sandbox serve --listen tcp:127.0.0.1:0" runs with no token option
    Then it exits with code 64 and names the missing option

  Scenario: The shell commands work against a served control plane
    Given a served control plane on a Unix socket
    When "agent-sandbox create --connect unix:<path> --rootfs <image>" runs
    Then it prints a sandbox id
    When "agent-sandbox exec --connect unix:<path> <id> -- echo ok" runs
    Then it prints "ok"
    When "agent-sandbox stop --connect unix:<path> <id>" runs
    Then "agent-sandbox list --connect unix:<path>" does not show the id

  Scenario: Doctor says what is missing
    Given a host with no firecracker program
    When "agent-sandbox doctor" runs
    Then it prints the firecracker report with the failed check
    And its exit code is 1

  Scenario: A provider launches and stops a sandbox
    Given a SandboxProvider on a simulated manager
    When launch is called with a valid spec
    Then a handle is returned and is_running is true
    When stop is called
    Then the result is Ok and is_running is false

  Scenario: A provider passes errors through
    When launch is called with a root kind that the driver cannot use
    Then the error context is UnsupportedRootFs

  Scenario: Health mirrors the probe
    Given a manager whose probe has the checks ("kvm", ok) and ("slave", not ok, "missing")
    When host_health is called
    Then it returns exactly those two tuples

  Scenario: The slave program starts from its arguments
    When "agent-slave --listen unix:<path>" runs
    Then Info on that socket answers with is_pid1 false

  Scenario: The slave program refuses a bad argument
    When "agent-slave --listen nonsense" runs
    Then it exits with code 64
```

## 6. Technical breakdown

### 6.1 Where the code is

| Path | Content |
|---|---|
| `foundation_platform_sandbox/src/cli/mod.rs` | Shared argument helpers: `parse_listen`, `parse_connect`. Feature `cli`, native only. |
| `.../src/cli/sandbox.rs` | `SandboxCli` (a `clap` derive), `pub fn run(ctx: Ctx, cli: SandboxCli) -> i32`. |
| `.../src/cli/slave.rs` | `SlaveCli`, `pub fn run(ctx: Ctx, cli: SlaveCli) -> i32`. Reads `/proc/cmdline` for `ewe.listen` when `--listen` is not given. Calls `native::slave::run`. Needs the `slave` feature. |
| `.../src/shared/control/service.rs` | `ControlService`. |
| `.../src/shared/control/rpc_client.rs` | `SandboxRpcClient` and the remote handle. |
| `.../src/native/serve.rs` | `serve_control(ctx, control, listen, auth, shutdown)`: the HTTP server for `SandboxService`. |
| `foundation_deployment_platform/src/bin/agent_sandbox.rs` | `fn main()`: `Ctx::host()`, store `SandboxHost::native(..)`, parse `SandboxCli`, `std::process::exit(cli::sandbox::run(ctx, cli))`. |
| `bin/agent_slave/src/main.rs` | `fn main()`: `Ctx::host()`, parse `SlaveCli`, exit with `cli::slave::run(ctx, cli)`. |
| `foundation_deployment_platform/src/providers/sandbox/mod.rs` | The `Provider` facade. Feature `sandbox`. |

**Where `agent-slave` lives (measured).** Decision D4 puts the entry points in
`foundation_deployment_platform/src/bin/`. `agent-sandbox` is there. `agent-slave` must be
a static musl program, and a program in that directory compiles the library crate
`foundation_deployment_platform` for musl. On 2026-11-11,
`cargo check -p foundation_deployment_platform --target x86_64-unknown-linux-musl` failed:
the crate depends on `foundation_netio` with `ssl`, which builds `ring`, which needs a musl
C compiler; and its Docker client is not optional. So `agent-slave` has its own package,
`bin/agent_slave`, that depends only on `foundation_platform_sandbox` with
`default-features = false, features = ["slave", "cli"]`. Its `main` is the same three
lines. Requirements question Q3 asks the owner to confirm. If the owner wants it in
`foundation_deployment_platform`, that crate first needs its Docker client and TLS behind
features; that is a separate change.

### 6.2 `agent-sandbox`

```
agent-sandbox run     [--driver <id>] [--rootfs <path>] [--vcpus n] [--memory-mib n]
                      [--network none|host] [--env K=V]... [--workdir <path>] [--read-only]
                      [--connect <addr>] -- <program> [args...]
agent-sandbox serve   --listen <addr> [--token-file <path>] [--driver-order a,b]
agent-sandbox create  [--connect <addr>] <the spec options of run>
agent-sandbox exec    [--connect <addr>] <sandbox-id> [--env K=V]... [--workdir <path>] -- <program> [args...]
agent-sandbox list    [--connect <addr>]
agent-sandbox stop    [--connect <addr>] <sandbox-id>
agent-sandbox kill    [--connect <addr>] <sandbox-id>
agent-sandbox doctor
```

- `<addr>` is `unix:<path>` or `tcp:<host>:<port>`.
- `--rootfs <path>`: `.ext4` gives `Ext4Image`, `.squashfs` gives `SquashfsImage`, a
  directory gives `Directory`; any other file gives exit code 64 with a message. With no
  `--rootfs`, `run` and `create` use `store.firecracker_ci_rootfs(arch)` when the chosen
  driver is Firecracker.
- With no `--connect`, `run` and `doctor` build a local `SandboxManager` with every driver
  that this build has. `create`, `exec`, `list`, `stop` and `kill` need `--connect` or the
  environment variable `EWE_SANDBOX_CONNECT`; with neither they exit with 64 (a sandbox
  made by a local manager ends with the process, so a local `create` has no use).
- With `--connect tcp:...`, the token comes from `--token-file` or `EWE_SANDBOX_TOKEN`.
- Output: `run` and `exec` copy `Stdout` events to standard output and `Stderr` events to
  standard error as they arrive. Progress lines of `run` (`create_with_progress`) go to
  standard error with the prefix `agent-sandbox:`. `list` prints one line for each sandbox:
  ID, driver, state. `create` prints the ID.

`run` steps: create; `exec` with standard input piped from the CLI's standard input when it
is not a terminal, else `Null`; copy events; on `Exit`, `stop()`; exit with the code of
invariant I6. A `SIGINT` or `SIGTERM` handler (the repository's signal support,
`foundation_nativeapis` feature `signal`) sends `ExecSignal(15)` to the command, waits for
`Exit` up to `shutdown_grace`, then `stop()`. Any error path calls `kill()` before it
exits with 125.

| Exit code of `agent-sandbox` | Meaning |
|---|---|
| the program's code | `run` and `exec`, normal end |
| `128 + n` | The program ended by signal `n` |
| 1 | `doctor`: no driver is usable |
| 64 | Usage error |
| 125 | The sandbox or the control plane failed; the error is on standard error |

### 6.3 `ControlService`

```rust
pub struct ControlService { control: Arc<dyn SandboxControl>, limits: SandboxLimits }
impl SandboxService for ControlService { /* one method for each call */ }
```

| Call | Body |
|---|---|
| `probe`, `get`, `list`, `stop`, `kill` | Call the control plane; map the result. |
| `create` | `control.create_with_progress(spec)`; map `Pending(p)` to `CreateEvent::Progress(p)` and the result to `Created(info)` or the stream error. When the client drops the stream before the end, the create task is dropped, and the control plane cleans up (feature 02, section 6.4). |
| `exec` | `control.get(id)`, `sandbox.exec(command)`; the handle is stored in a session map under `(sandbox_id, exec_id)`; events are forwarded with new `seq` numbers from 0 (`Started` first). When the stream ends or the client drops it, the entry is removed and the handle is dropped. |
| `exec_input` | Finds the session; `Sequence::accept(seq)`; on `Apply`, `stdin.write(data)` and `close` on `eof`. `Duplicate` returns `next_seq`. `Gap` returns `SequenceGap`. |
| `exec_signal` | Finds the session; `handle.signal(n)`. |
| `file_write_begin`, `_chunk`, `_commit`, `_abort` | Pass through to the sandbox's slave client with the same messages. (The control plane exposes these four on `Sandbox` as crate-private calls for this purpose; `write_file` is built on them.) |
| `file_read` | `sandbox.read_file(path)`; forward chunks with offsets. |

The `exec_id` in a scoped `ExecRequest` is the client's. The service uses it as the key of
its session map; the control plane makes its own `exec_id` towards the slave. An `exec_id`
that is in use for that sandbox gives `AlreadyExists` (mapped as `SlaveError` with that
code).

Errors: `SandboxError::to_connect`. An unknown sandbox ID gives `NotFound { what: "sandbox" }`.

### 6.4 `SandboxRpcClient`

```rust
pub struct SandboxRpcClient { /* generated SandboxService client, ctx, limits */ }
impl SandboxRpcClient {
    pub fn new(ctx: Ctx, transport: Arc<dyn Transport>, limits: SandboxLimits) -> Self;
    /// Builds the transport for an address. Native only.
    pub fn connect(ctx: Ctx, addr: ConnectAddr, limits: SandboxLimits) -> SandboxResult<Self>;
    pub fn with_token(self, token: SecretString) -> Self;   // adds "Authorization: Bearer ..."
}
/// `Unix` and `Tcp` are the forms of `--connect`. `Url` is an `http://` or `https://`
/// base address (the Cloudflare Worker of feature 08).
pub enum ConnectAddr { Unix(PathBuf), Tcp { host: String, port: u16 }, Url(String) }
impl SandboxControl for SandboxRpcClient { /* each call maps to its RPC */ }
```

`connect` uses `H1Transport` on an HTTP client of `foundation_netio`: a UDS client for
`Unix`, a TCP client for `Tcp`, and a client with TLS for an `https://` `Url`. The base
address of the calls is `http://localhost` for `Unix`, `http://<host>:<port>` for `Tcp`,
and the `Url` as given.

The remote `Sandbox` handle implements `SandboxOps` with the scoped calls. `exec`, input,
signal, `write_file` and `read_file` have the same client logic as in feature 02
(`ExecHandle`, `ExecStdin`, chunking, sequence numbers, abort on failure). So that logic is
written one time: feature 02's handle code is generic over the trait `ExecFileCalls`
(feature 02, section 6.6),

```rust
/// The calls that a handle needs. Implemented by SlaveClient (local control plane) and by
/// the scoped SandboxService client (remote control plane).
pub trait ExecFileCalls: Send + Sync + 'static {
    fn exec(&self, req: ExecRequest) -> BoxFuture<'_, SandboxResult<ExecEventStream>>;
    fn exec_input(&self, req: ExecInputRequest) -> BoxFuture<'_, SandboxResult<ExecInputResponse>>;
    fn exec_signal(&self, req: ExecSignalRequest) -> BoxFuture<'_, SandboxResult<()>>;
    fn file_write_begin(&self, req: FileWriteBeginRequest) -> BoxFuture<'_, SandboxResult<FileWriteBeginResponse>>;
    fn file_write_chunk(&self, req: FileWriteChunkRequest) -> BoxFuture<'_, SandboxResult<FileWriteChunkResponse>>;
    fn file_write_commit(&self, req: FileWriteCommitRequest) -> BoxFuture<'_, SandboxResult<FileWriteCommitResponse>>;
    fn file_write_abort(&self, req: FileWriteAbortRequest) -> BoxFuture<'_, SandboxResult<()>>;
    fn file_read(&self, req: FileReadRequest) -> BoxFuture<'_, SandboxResult<FileChunkStream>>;
}
```

and this feature adds the second implementation (the scoped `SandboxService` client). `SecretString` is the secret type that the
workspace already has (`grep -rn "struct Secret" backends/*/src`); its `Debug` prints no
value.

### 6.5 Serving and authentication

`serve_control(ctx, control, listen, auth, shutdown)`:

- `unix:<path>`: check the socket path length; remove a stale socket file only when a
  connect to it fails with "connection refused"; bind; mode `0600`. No token.
- `tcp:<host>:<port>`: needs `auth = Some(token)`. An interceptor
  (`foundation_connectrpc` `shared/interceptor`) reads `Authorization: Bearer <token>`,
  compares SHA-256 digests of the two values in constant time, and returns `Unauthorized`
  on a mismatch. The check runs before the handler.
- HTTP/1.1, `ConnectRpcServe` on `HttpServer` (as in feature 01, section 6.4).
- The token file is read one time at start: the first line, trimmed. An empty file is a
  usage error. The token is never logged.

Requirements question Q4: mutual TLS comes with the agent-rpc work. A TCP listener is for
`127.0.0.1` or a private network until then; the README says so.

### 6.6 `agent-slave`

```
agent-slave [--listen vsock:<port> | unix:<path> | tcp:<host>:<port>]
```

`cli::slave::run`: take `--listen`; if it is not given, read `ewe.listen=` from
`/proc/cmdline`; if neither exists, the failure of feature 01, section 6.3. Build
`SlaveArgs` and call `native::slave::run`.

Package `bin/agent_slave`: one dependency (section 6.1). The workspace root gets the
profile `agent` (`inherits = "release"`, `opt-level = "s"`, `lto = true`,
`panic = "abort"`, `strip = true`, `codegen-units = 1`).

`mise` task `sandbox:slave`:

```bash
rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
cargo build -p agent_slave --profile agent --target x86_64-unknown-linux-musl
cargo build -p agent_slave --profile agent --target aarch64-unknown-linux-musl
```

For a target that is not the host, the task sets
`RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes"`. The slave has no C dependency
(feature 01, section 6.1), so no C cross compiler is necessary. The task copies each result
to `target/sandbox/agent-slave-<arch>`, which is the name that `HostConfig::discover`
looks for beside `agent-sandbox` when both are installed in one directory.

### 6.7 The `Provider` facade

`foundation_deployment_platform`, Cargo feature `sandbox`
(`sandbox = ["dep:foundation_platform_sandbox"]`, in `all-providers`, off by default).

```rust
/// A running sandbox, as a `Provider` handle.
pub struct SandboxHandle { pub sandbox: Sandbox, pub provider_id: ProviderId }
impl std::ops::Deref for SandboxHandle { type Target = Sandbox; /* ... */ }

/// A `Provider` on any control plane. `driver` fixes which driver each launch uses.
pub struct SandboxProvider { control: Arc<dyn SandboxControl>, driver: DriverId }

impl SandboxProvider {
    pub fn firecracker(control: Arc<dyn SandboxControl>) -> Self;
    pub fn new(control: Arc<dyn SandboxControl>, driver: DriverId) -> Self;
}
```

`ProviderId` gets the variant `Firecracker` (`firecracker`). Features 08, 09 and 10 add
`CloudflareContainer`, `Libkrun` and `DockerSandbox`. Update each `match` on `ProviderId`
(`grep -rn "ProviderId::" --include=*.rs .`).

`impl Provider for SandboxProvider` with `Handle = SandboxHandle`, `Config = SandboxSpec`,
`Error = ErrorTrace<SandboxError>`:

| Method | Body |
|---|---|
| `name`, `id` | From `driver`. |
| `launch(config)` | A copy of the spec with `driver = Some(self.driver)`; `block_on_future(self.control.create(spec))`. |
| `stop(handle)` | `block_on_future(handle.sandbox.stop())`; drop the report. |
| `is_running(handle)` | `handle.sandbox.info().state == SandboxState::Ready` |
| `resolved_ports(handle)` | `Ok(ResolvedPorts { ssh_port: 0, winrm_port: None, rdp_port: None, vnc_port: 0 })`. A sandbox has no such ports; `DockerProvider` returns 0 in the same case. |
| `host_health()` | `block_on_future(self.control.probe())`; the checks of the report for `self.driver`, as `(name, ok, message)`. |

`block_on_future` is `foundation_core::valtron::block_on_future`
(`executors/sendables.rs`, line 1004). The doc comment of the type says: "Async callers use
`SandboxControl` directly."

Because the provider takes any `SandboxControl`, the same type works on a local
`SandboxManager` and on a `SandboxRpcClient`.

### 6.8 CI

`.github/workflows/sandbox-e2e.yaml`:

- Runs on `ubuntu-latest`. That runner has `/dev/kvm`
  (`.github/workflows/platform-images-vm.yaml` uses it for QEMU).
- Steps: checkout with submodules; enable KVM access for the runner user (a udev rule
  `KERNEL=="kvm", GROUP="kvm", MODE="0666"`, then `udevadm trigger`); install the Rust
  toolchain and the musl target; download the pinned `firecracker` release (the version
  and SHA-256 are in `ci/sandbox.toml`, read by the workflow); `mise run sandbox:slave`;
  run `cargo test -p foundation_platform_sandbox --features testing --profile uat
  e2e_firecracker` and the `agent-sandbox` program tests.
- Triggers: a pull request that changes `backends/foundation_platform_sandbox/**`,
  `bin/agent_slave/**`, `backends/foundation_netio/**` or the workflow; pushes to
  `master`; nightly. A concurrency group cancels a superseded pull request run
  (`.agents` rule 17).
- The job fails when `probe()` says the runner is not usable. (On a developer machine the
  tests return with a message; in this job the environment variable
  `EWE_SANDBOX_E2E_REQUIRED=1` turns that return into a failure, so CI cannot pass by
  skipping.)

`ci/packages.toml`:

| Package | Linux | macOS | Windows |
|---|---|---|---|
| `foundation_platform_sandbox` | all features of this phase | default features with `firecracker` off; reason: "Firecracker needs KVM" | `windows-tests = "none"`; reason: "no driver for a Windows host; the crate's native code is unix-only" |
| `agent_slave` | build for both musl targets; its tests are the slave tests of feature 01 | `macos-tests = "none"`; reason: "a Linux guest program" | `windows-tests = "none"`; the same reason |
| `foundation_deployment_platform` | add the run `sandbox` to `runs` and `clippy-runs` | the same | as today |

### 6.9 Documentation

- `backends/foundation_platform_sandbox/README.md`: the model (control plane, slave,
  drivers), host requirements for Firecracker, how to build the slave, `agent-sandbox run`,
  the library example, and the security notes below.
- `docs/sandbox.md`: the architecture diagram of the requirements and a "first sandbox"
  guide for Linux.

Security notes to state plainly:

- The VM boundary isolates the guest. The VMM process itself runs with the rights of the
  caller; it is not confined in this version.
- `serve` on a UDS trusts every process that can open the socket. `serve` on TCP uses one
  shared token and no TLS; use it on `127.0.0.1` or a private network.
- A path in a spec is a path on the control plane's machine. A remote caller that can
  create sandboxes can read each root image that the control plane's user can read.

### 6.10 Issues to file

When the owner approves issue filing (decision D9):

| Issue | Label |
|---|---|
| One for each item of requirements section 7.3 | `enhancement` |
| `ResolvedPorts`: `ssh_port` and `vnc_port` are `u16`; a provider with no such port returns 0. Likely fix: `Option<u16>`. | `severity: low` |
| `foundation_deployment_platform` does not build for musl targets (TLS and the Docker client are not optional). | `severity: low` |
| `backends/foundation_deployment_docker/specs/swagger2openapi.py` is used by two crates. Likely fix: move it to `scripts/openapi/`. | `severity: low` |
| A binary codec for the two services (JSON carries bytes as base64). | `enhancement` |
| Each finding from features 00 to 05 that has no issue yet | its severity |

## 7. Tests

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| I1, "The remote client behaves as the local manager" | `rpc_parity`: the shared acceptance scenarios with `SandboxRpcClient` + `ControlService` over `in_memory_connector`-style pipes (simulation), and over a real UDS with the slave process driver | `foundation_platform_sandbox/tests/control_rpc_*.rs` | differential (local and remote give the same outcomes) |
| I2, "An error keeps its type across RPC" | `rpc_error_variants` (each variant that a control call can return) | the same | identity |
| I3, the two token scenarios | `tcp_needs_token`, `tcp_wrong_token`, `tcp_no_token_no_start` | `tests/serve_*.rs` | expected outcome |
| I4 | `unix_socket_mode` | the same | example |
| `create` stream dropped by the client | `rpc_create_dropped_cleans_up` (simulation: the driver handle got `kill` and `cleanup`) | `tests/control_rpc_*.rs` | fault |
| `exec_input` through the service | `rpc_input_duplicate_and_gap` | the same | expected outcome |
| I5, I6, the two `run` scenarios | `cli_run_exit_code`, `cli_run_signal_exit`, `cli_run_sigint_cleans_up` | `foundation_deployment_platform/tests/agent_sandbox_cli.rs` (real Firecracker; returns with a message when not usable) | expected outcome |
| argument parsing | `cli_parse` (each sub-command; each usage error gives 64) | `foundation_platform_sandbox/tests/cli_parse.rs` | example |
| "The shell commands work ..." | `cli_remote_flow` | `agent_sandbox_cli.rs` | example |
| "Doctor says what is missing" | `cli_doctor` (a fake environment with no `firecracker`) | the same | expected outcome |
| I7, I8, the three provider scenarios | `provider_launch_stop_sim`, `provider_error_is_typed`, `provider_host_health` | `foundation_deployment_platform/tests/sandbox_provider_tests.rs` | expected outcome |
| provider on a real host | `provider_e2e_firecracker`: `launch`, `handle.run(["echo", "ok"])` through `block_on_future`, `stop` | the same | example |
| I9 | the default-feature test run in section 8 | | build |
| I10 | `boundaries_have_no_logic` (a source scan for `validate(`, `boot_timeout`, `Duration::from_`, `loop` with a retry in the boundary files) | `tests/boundary_scan.rs` | example |
| I11, the two slave scenarios | `slave_cli_listen_unix`, `slave_cli_bad_arg`; `file` reports "statically linked" in the verify step | `bin/agent_slave/tests/` | example |

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features "cli testing slave" --all-targets --profile uat --no-deps -- -D warnings
cargo clippy -p foundation_deployment_platform --features sandbox --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "cli testing slave" --profile uat control_rpc serve_ cli_parse boundary_scan
cargo test -p foundation_deployment_platform --features sandbox --profile uat sandbox_provider agent_sandbox_cli
cargo test -p foundation_deployment_platform --profile uat          # default features still pass
mise run sandbox:slave
file target/sandbox/agent-slave-x86_64    # must say "statically linked"
cargo tree -i foundation_deployment_platform --workspace --depth 1  # then test each consumer
python3 scripts/ci/package_workspace.py
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes.
- `agent-sandbox run --rootfs <image> -- echo hello` prints `hello` on a Linux host with
  KVM, and the CI job `sandbox-e2e` is green.
- `agent-slave` is a static program for both musl targets.
- The versions of the changed crates are bumped, and each consumer uses the new version.
- The README and `docs/sandbox.md` exist.
- `requirements.md` frontmatter counts are current, and `progress.md` records MVP-A.
