---
description: "foundation_sandbox: one core library that starts sandboxes (Firecracker, Cloudflare containers and the Cloudflare Sandbox product, libkrun, Docker containers, the Docker Sandboxes product) and runs commands in them. A control plane (agent-sandbox) drives an in-sandbox slave (agent-slave) over Connect RPC. Binaries, the Cloudflare Durable Object and the Provider implementations are thin boundaries on the library."
status: "in-progress"
priority: "high"
created: 2026-11-11
author: "Main Agent"
metadata:
  version: "0.4"
  last_updated: 2026-11-12
  estimated_effort: "large"
  tags: [sandbox, microvm, firecracker, libkrun, docker, cloudflare, durable-object, connectrpc, control-plane, valtron, provider]
  stack_files: [rust.md]
  skills: [story-driven-specification, rust-clean-code, rust-valtron-usage, rust-valtron-iterator, test-driven-development]
  tools: []
has_features: true
has_fundamentals: false
related_specs:
  - "specifications/83-agent-rpc-serving-system"
  - "specifications/completed/41-connectrpc"
  - "specifications/completed/28-cloudflare-workers-readiness"
  - "specifications/completed/54-foundation-deployment-docker"
  - "specifications/completed/76-shared-context"
  - "specifications/completed/84-context-injection"
  - "specifications/completed/85-wasm-fetch-transport"
  - "specifications/completed/86-auth-primitives"
  - "specifications/completed/87-mutual-tls"
features:
  completed: 0
  uncompleted: 18
  total: 18
  completion_percentage: 0%
---

# Spec 77: Sandboxes (`foundation_sandbox`)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); remote procedure
call (RPC); virtual machine (VM); virtual machine monitor (VMM); Kernel-based Virtual
Machine (KVM); Hypervisor.framework (HVF); Unix domain socket (UDS); Hypertext Transfer
Protocol (HTTP); JavaScript Object Notation (JSON); process identifier (PID); Durable Object
(DO); WebAssembly (wasm); Open Container Initiative (OCI); Transparent Socket Impersonation
(TSI); behaviour-driven development (BDD); acceptance-test-driven development (ATDD);
deterministic simulation testing (DST); continuous integration (CI); pull request (PR);
minimum viable product (MVP); AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## Glossary

| Term | Meaning |
|---|---|
| sandbox | An isolated Linux environment that runs commands for a caller. |
| substrate | The technology behind one sandbox: a Firecracker microVM, a libkrun microVM, a Docker container, a Docker Sandboxes microVM, a Cloudflare container, or a Cloudflare Durable Object (DO) that serves the slave itself. The last one gives no operating-system isolation (decision D16). |
| Docker Sandboxes | A Docker product. Its program is `sbx`. Each sandbox is a microVM that Docker manages. It is not a Docker container. |
| Cloudflare Sandbox | A Cloudflare product: a Durable Object and the container that it starts, with a helper package (`@cloudflare/sandbox`) and a helper program in the image (`sandbox-shim`). |
| control plane | The code that owns all sandbox set-up: it starts a substrate, waits for the slave, and passes commands to it. The program is `agent-sandbox`. |
| slave | The code that runs inside each sandbox and does what the control plane asks: start a program, move a file, stop. The program is `agent-slave`. |
| driver | The control-plane part for one substrate. It knows how to start and stop that substrate. |
| core | The library code of `foundation_sandbox`. Programs and Workers call it; they add no logic. |
| boundary | A thin wrapper that plugs the core into a place where it runs: a `main` function, a DO class, a `Provider` implementation. |
| `SlaveService` | The RPC interface that the slave serves. |
| `SandboxService` | The RPC interface that the control plane serves. |
| nested sandbox | A microVM that a slave starts inside its own container. |
| sandbox image | The thing that a sandbox boots: a root file system and a manifest (features 12 to 15). |
| `Provider` | The trait in `foundation_deployment_platform/src/providers/mod.rs`. |
| `Ctx` | The shared context of spec 76: the clock, the random number generator and typed entries. |

## 1. Problem

The platform can start Docker containers and QEMU or UTM VMs. It cannot start a small,
isolated sandbox fast and run commands in it. Agent workloads need this: they run code that
the platform does not trust. The agent-rpc system (spec 83) needs a component that owns
sandbox set-up, on a server and on Cloudflare.

Each substrate has a different control model:

| Substrate | Where | How a caller controls it | What runs a command |
|---|---|---|---|
| Firecracker | Linux with KVM | A REST API on a UDS. One process is one VM. | Nothing. The guest needs its own program. |
| libkrun | Linux with KVM, macOS on Apple silicon | A C library that takes over the calling process. | Its built-in init runs one program. |
| Docker container | A Docker daemon | The Docker API (`foundation_deployment_docker`). | `exec` in the Docker API. |
| Cloudflare container | Cloudflare | Only a DO can reach its container, through `fetch` to a port. | A program in the container. |
| Cloudflare DO (model slave) | Cloudflare | The control plane calls the slave DO directly. | The slave DO: the turn runs against OpenRouter or the Cloudflare Model API. |

This spec gives them one model.

## 2. The model

### 2.1 Two roles, one core

```mermaid
graph TD
  subgraph Callers
    A[agent-rpc and other Rust code]
    P[Provider facades in foundation_deployment_platform]
    C[agent-sandbox command line]
  end
  subgraph CP[Control plane: SandboxManager]
    M[registry, life cycle, laws]
    D1[Firecracker driver]
    D2[Cloudflare container driver]
    D3[libkrun driver]
    D4[Docker driver]
  end
  subgraph SB[One sandbox]
    S[Slave: SlaveService<br/>exec, files, shutdown]
    W[user programs]
  end
  A -- library call or SandboxService RPC --> M
  P --> M
  C --> M
  M --> D1
  M --> D2
  M --> D3
  M --> D4
  D1 -. start substrate .-> SB
  M -- SlaveService over Connect RPC --> S
  S --> W
```

_Caption: callers talk to one control plane; the control plane starts a substrate through a driver and then talks to the slave inside it over RPC._

- **The control plane** (`SandboxManager`) owns every sandbox: it validates the request,
  asks a driver to start the substrate, waits until the slave answers, keeps the registry,
  passes commands to the slave, and cleans up. One implementation, in `shared/`.
- **The slave** serves `SlaveService`. It is PID 1 in a microVM, or the main process or a
  sidecar in a container. One implementation of its behaviour.
- **A driver** starts and stops one kind of substrate and says how to reach the slave in
  it. Drivers are the only substrate-specific code in the control plane.
- **A caller** uses the control plane as a library (`SandboxProvider` trait on
  `SandboxManager`), or over RPC (`SandboxService`, with `SandboxRpcClient` that
  implements the same trait). agent-rpc can use either.

### 2.2 The same core in every place

The control plane is library code with no assumption about where it runs. A boundary gives
it what that place has.

| Where the control plane runs | Boundary | Drivers available | How it reaches a slave |
|---|---|---|---|
| A Linux or macOS host (the general case) | the program `agent-sandbox` | Firecracker (Linux), libkrun, Docker, Docker Sandboxes | HTTP on a UDS (vsock behind it) or TCP |
| A Docker container on that host (the operator's choice) | the program `agent-sandbox` in the control-plane image, with the host's Docker socket | Docker | TCP, or a UDS in a shared directory |
| A Cloudflare DO | a `#[durable_object]` class | Cloudflare container | `container.get_tcp_port(port).fetch(..)` |
| A Cloudflare DO driving a slave DO | a `#[durable_object]` class | Cloudflare DO slave (model API) | a service binding (`stub.fetch(..)`) |
| Inside a Docker container (nested) | the program `agent-slave` with the control feature | Firecracker | HTTP on a UDS |

The last row is the nested case: a slave in a container holds the same control-plane core
and can start microVMs inside its container. One container then gives many sandboxes. It
needs `/dev/kvm` in the container. Docker can give that. A Cloudflare container does not
have it today, so there the slave reports that it cannot control, with the same code
(feature 11, section 6.6).

### 2.3 The systems

The owner named six systems. Two of the names are products (owner answer, 2026-11-12).

| System | Meaning in this spec | Feature |
|---|---|---|
| Firecracker | The control plane starts a Firecracker microVM; the slave is PID 1. | 05 |
| Cloudflare container | The control plane, in a DO, starts its container; the slave is the container's main process. | 07, 08 |
| Cloudflare DO slave (model API) | The control plane, in a DO, drives a second DO as the slave. That DO runs the turn against OpenRouter or the Cloudflare Model API. No container, no isolation. | 07, 08 |
| Cloudflare Sandbox (the product) | The same primitive as the row above: a DO and its container. An image made for the product runs under our control plane, and an application of the product can call our control plane. | 17 (on 07, 08) |
| libkrun | The control plane starts a libkrun microVM; the slave is the program of the libkrun init. | 09 |
| Docker container | The control plane talks to the Docker Engine of the host and starts containers that hold the slave. The slave is the entry point or a sidecar process. | 10 |
| Docker Sandboxes (the product) | The control plane runs the `sbx` program; each sandbox is a microVM that Docker manages, with the slave in it. **Draft, not a must have** (decision D14). | 16 (draft) |

Where the control plane runs is a different question from which system it drives:

- **General case:** `agent-sandbox` runs on the host and drives each local system.
- **By choice:** `agent-sandbox` runs in a Docker container with the host's Docker socket
  and starts sibling containers (feature 10, section 6.8).
- **On Cloudflare:** the control plane is the DO. It runs `agent-slave` in its Cloudflare
  container. A Cloudflare container cannot start a container or a microVM, so Cloudflare
  has no nested case (feature 11, section 6.6).

### 2.4 Crates, modules and boundaries

| Path | Kind | Content |
|---|---|---|
| `backends/foundation_sandbox` | library | **All logic.** |
| `.../src/shared/` | all targets | Domain types, laws, the two RPC contracts, `SandboxManager`, `SandboxProvider`, `SlaveClient`, `ContainerControl` and `ContainerHandle`, the slave's request handling that needs no operating system, `Platform` and `PlatformKind` (`shared/platform.rs`). |
| `.../src/shared/cloudflare/` | all targets | Everything of the Cloudflare part that needs no `worker` type: `CloudflareContainerDriver`, `CloudflareContainerControl`, `DoContainerOps`, `ContainerPortTransport`, `DoRecordStore`, `DoControlPlane`, `CloudflarePlatform`, and the model-slave part of decision D16 (`CloudflareDoDriver`, `DoSlaveConnector`, `ModelSlave`, `ModelClient`). `ContainerApi`, `DoStorage`, `DoAlarm` and the service binding are the seams. |
| `.../src/native/` | not wasm | The slave engine (processes, mounts, PID 1), the Firecracker, libkrun, Docker and Docker Sandboxes drivers, the Docker container control, the native side effects, the RPC servers on UDS, vsock and TCP, and the wrappers `NativePlatform` and `DockerPlatform`. |
| `.../src/cloudflare/` | wasm32 with the `cloudflare` feature | The adapters of the traits (feature 07 sections 6.3 and 6.10): `WorkerContainer`, `WorkerStorage`, `WorkerAlarm`, `WorkerModelClient`, and `CloudflareHost::from_state`. |
| `.../src/cli/` | not wasm | The command structures of `agent-sandbox` and `agent-slave` (arguments, sub-commands, `run(ctx, args)`). |
| `.../src/bin/agent_sandbox.rs` | program | `main`: build the `Ctx`, call `cli::sandbox::run`. The control plane is this crate's binary (decision D4). |
| `.../src/bin/agent_slave.rs` | program | `main`: build the `Ctx`, call `cli::slave::run`. The `slave` feature pulls no TLS library and no Docker client, so the binary builds as a static musl program (decision D11). |
| `.../src/providers/` | not wasm | `Provider` facades over `SandboxManager`, behind the `provider` feature. They hold no logic. The `Provider` trait itself stays in `foundation_deployment_platform` (decision D1). |
| `infrastructure/sandbox_worker` | Cloudflare Worker (Rust, wasm) | The `#[durable_object]` class and the Worker entry. They call `foundation_sandbox::cloudflare`. No logic. |

The split `shared` / `native` / `cloudflare` follows `foundation_connectrpc`,
`foundation_http` and `foundation_auth`. A `wasm/` module is added only when a browser-only
part exists; this spec has none (the RPC client is in `shared/`).

Docker capability lives in `foundation_deployment_docker` (the existing Docker client).
`foundation_sandbox` uses it behind the `docker` feature.

Cloudflare deployment lives in `foundation_deployment_cloudflare` (the existing crate, with
its generated Cloudflare types; the old `ewe_deployables::cloudflare` module was removed in spec 70 feature 22). It is a native library,
not wasm. `foundation_sandbox` does not use it; a caller of the deployer does
(feature 08).

### 2.5 The two RPC contracts

Both are Connect RPC services on `foundation_connectrpc`, defined code-first with the
`#[service]` macro, with the JSON codec. Every call is **unary or server-streaming**. The
`fetch` transport of a Worker cannot stream a request body, so no call uses client
streaming, and the same contract works on every transport. Feature 00 has the full
definitions.

| Service | Server | Calls |
|---|---|---|
| `SlaveService` | slave | `Info`, `Prepare`, `Exec` (stream), `ExecInput`, `ExecSignal`, `FileWriteBegin`, `FileWriteChunk`, `FileWriteCommit`, `FileWriteAbort`, `FileRead` (stream), `Shutdown`. Feature 12 adds `UnpackBegin`, `UnpackChunk`, `UnpackCommit`, `UnpackAbort`. |
| `SandboxService` | control plane | `Probe`, `Create` (stream of progress), `Get`, `List`, `Stop`, `Kill`, and the `Exec*` and `File*` calls with a sandbox ID, which the control plane passes to the slave |

How the control plane reaches a slave:

| Substrate | Slave listens on | Control plane connects through |
|---|---|---|
| Firecracker | `AF_VSOCK` port 1024 | The Firecracker UDS, with the line `CONNECT 1024\n` first |
| libkrun | `AF_VSOCK` port 1024 | A UDS that libkrun makes for the port |
| Docker container | TCP port 1024 in the container | The published host port on `127.0.0.1` |
| Cloudflare container | TCP port 1024 in the container | `container.get_tcp_port(1024).fetch(..)` |
| tests | a UDS | the UDS |

## 3. Spec-level stories

- **S1**: "A caller asks the control plane for a sandbox from a root file system and gets a
  handle when it is ready."
- **S2**: "A caller runs a command in a sandbox, sends it input, reads its output while it
  runs, and gets its exit status."
- **S3**: "A caller puts a file into a sandbox and reads a file out."
- **S4**: "A caller stops a sandbox, and nothing of it stays on the host."
- **S5**: "A sandbox fails to start, or its slave stops answering; the caller gets a typed
  reason, and nothing stays."
- **S6**: "The same request works when the control plane is in a host program, in a
  Cloudflare DO, or inside a container."
- **S7**: "agent-rpc uses the control plane through Rust calls or through RPC, with the same
  results."
- **S8**: "A caller with a `Provider` starts a sandbox like any other platform resource."

## 4. Context map

One writer for each piece of data. A feature owns the entities in its row.

| Bounded context | Entities it owns | Feature |
|---|---|---|
| Contract | `SandboxSpec`, `Command`, `ExitStatus`, `SandboxError`, `SandboxLimits`, the two service definitions | 00 |
| Slave | Program, process group, program root, upload, slave state | 01 |
| Control plane | Sandbox record, sandbox state, registry, exec session (control side) | 02 |
| Guest artifacts | Cache entry, artifact spec, initramfs | 03 |
| Firecracker API | Generated types, API client | 04 |
| Firecracker driver | Launch plan, VMM process, run directory | 05 |
| Programs and facades | Command lines, `Provider` facades, `SandboxRpcClient` | 06 |
| Cloudflare control plane | DO registry record, container handle, idle alarm | 07 |
| Cloudflare Worker | Worker entry, DO class, container image | 08 |
| libkrun driver | Runner configuration, runner process | 09 |
| Docker driver | Container record | 10 |
| Nested control | The control plane inside a slave | 11 |
| Images | Image manifest, image store, unpack, build cache | 12 to 15 |

## 5. Spec-level laws

Each feature turns these into its own numbered invariants and tests.

| # | Law | Kind |
|---|---|---|
| L1 | A sandbox is `Ready` only after its slave answered `Info` and `Prepare`, and all of that happens inside one `boot_timeout`. | ordering |
| L2 | A sandbox state moves only along `Creating -> Ready -> Stopping -> Stopped` or `Creating/Ready -> Failed -> Stopped`. | monotone |
| L3 | A failed `Create` leaves nothing: no process, no container, no directory, no registry record in state `Ready`. | atomicity |
| L4 | After `Stopped`, the substrate and every host resource of the sandbox are gone. | conservation |
| L5 | The source root file system of a sandbox never changes. | freeze |
| L6 | A sandbox gets no host environment variable, and no network unless the spec asks for it. | bound |
| L7 | Output events of one command arrive in the order the program wrote them on each stream, and the exit event is last and unique. | ordering |
| L8 | `ExecInput` and `FileWriteChunk` are idempotent on their sequence number or offset: a repeat has no second effect, and a gap is rejected. | idempotence |
| L9 | A file write is atomic: the target path has the old content or the complete new content. | atomicity |
| L10 | `Stop` and `Kill` are idempotent: later calls return the first result. | idempotence |
| L11 | The control plane gives the same outcome for the same request through the library call and through `SandboxService`. | linkage |
| L12 | The core has no logic in a boundary: a program, a DO class and a `Provider` only translate. | linkage |
| L13 | Every wait has a deadline from the injected clock; every size has a limit from configuration; every side effect comes from the `Ctx`. | bound |
| L14 | Nothing is buffered without a bound. Bodies stream. | bound |

## 6. The public API (control plane)

All functions that wait are `async fn` (root rule 6). A sync caller wraps them with
`foundation_core::valtron::block_on_future`. Feature 00 has the types; feature 02 has the
implementation.

### 6.0 The handle doctrine (owner direction 2026-11-15)

Every outside caller reaches this library through a handle that it reads from the `Ctx`, and
never by starting a sandbox itself:

"Whether containers (docker, cloudflare,...etc), everything should be through a handle that
whatever central wrapper over Ctx or exposed via Ctx.get provides and whatever is returned
specifically provides the necessary methods to interact with the relevant underlying
infrastructure."

So there are two levels with the same shape:

| Level | Factory (the control) | Handle (the live object) | Methods of the handle |
|---|---|---|---|
| Sandbox | `SandboxProvider` | `SandboxHandle` | Create, get, list, stop and kill live on the provider. The handle runs commands and moves files. |
| Container | `ContainerControl` | `ContainerHandle` | Start, inspect, exec, signal, fetch a port, wait, stop and destroy one container. |

A sandbox uses a `ContainerHandle` internally. `SandboxHandle::container()` returns it, or
`None` for a microVM.

The handle is the whole interface. A caller does not know whether the work crosses an RPC
boundary (a Cloudflare DO in front of `worker::Container`) or reaches the resource directly
(the Docker Engine API). That choice belongs to the adapter.

The `Ctx` gains no method, and `foundation_context` gains no dependency. Each part of the
platform wraps the `Ctx` in a struct that holds the handles of that part. A caller takes the
concrete type when it needs one part, and `&dyn Platform` when one function serves several
parts (owner direction 2026-11-15, second and third notes):

"we wont go edit foundation_context, but rather wrap Ctx with whatever the cloudflare part
needs that provides the other cloudflare specific handles", "same for others", and "functions for
a given specific platform expect the specific rust type, as far as they implement the trait, we
can use them where using the trait is more suited".

```rust
// foundation_sandbox/src/shared/platform.rs
/// What every platform wrapper gives a caller that does not name the part.
pub trait Platform: Send + Sync + 'static {
    /// The `Ctx` of this run: the clock, the timers, the entropy and the `Env` entry.
    fn ctx(&self) -> &Ctx;
    /// The sandbox control plane. Local (`SandboxManager`) or remote.
    fn sandboxes(&self) -> &Arc<dyn SandboxProvider>;
    /// The container control, when the host has one.
    fn containers(&self) -> Option<&Arc<dyn ContainerControl>>;
    /// Which part this is. The value names the concrete type of the downcast helpers.
    fn kind(&self) -> PlatformKind;
    /// The same object as `dyn Any`, for the downcast helpers.
    fn as_any(&self) -> &dyn Any;
}

/// The parts that ship a wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformKind {
    /// Firecracker and libkrun, in process (features 05 and 09).
    Native,
    /// The Docker Engine (feature 10).
    Docker,
    /// A Cloudflare Worker and its container (features 07 and 08).
    Cloudflare,
}

// In native/platform.rs, which the crate compiles only on a native host:
impl dyn Platform + '_ {
    /// The same object as `NativePlatform`, or `PlatformMismatch`.
    ///
    /// # Errors
    /// [`SandboxError::PlatformMismatch`] when `kind()` is not `PlatformKind::Native`.
    pub fn as_native(&self) -> SandboxResult<&NativePlatform>;

    /// The same object as `DockerPlatform`, or `PlatformMismatch`.
    ///
    /// # Errors
    /// [`SandboxError::PlatformMismatch`] when `kind()` is not `PlatformKind::Docker`.
    pub fn as_docker(&self) -> SandboxResult<&DockerPlatform>;
}

// In shared/cloudflare/platform.rs, which the crate compiles on every target:
impl dyn Platform + '_ {
    /// The same object as `CloudflarePlatform`, or `PlatformMismatch`.
    ///
    /// # Errors
    /// [`SandboxError::PlatformMismatch`] when `kind()` is not `PlatformKind::Cloudflare`.
    pub fn as_cloudflare(&self) -> SandboxResult<&CloudflarePlatform>;
}
```

Each helper sits in the module of its type, so `shared/platform.rs` names no part. A helper
reads `kind()` first, so a caller that passes the wrong part gets a typed error that names both
kinds and not a `None` that hides the mistake. It then calls `as_any().downcast_ref()`.

Each wrapper names the `Ctx` and the common handles as fields, so `cf.ctx` is a field read and
not a trait call:

| Part | Wrapper | It also holds | Owner |
|---|---|---|---|
| Firecracker and libkrun | `NativePlatform` | the local `SandboxManager` as a concrete value (the drivers, the record store and the `ManagerConfig`) | features 05 and 09 |
| Docker | `DockerPlatform` | the `DockerContainerControl` and the Engine connection | feature 10 |
| Cloudflare | `CloudflarePlatform` | the `CloudflareContainerControl` | feature 07 |

```rust
// foundation_sandbox/src/shared/cloudflare/platform.rs — compiled on every target
pub struct CloudflarePlatform {
    /// The `Ctx` of this run.
    pub ctx: Ctx,
    /// The sandbox manager: one sandbox, `SandboxIdPolicy::Fixed`, in a DO.
    pub sandboxes: Arc<dyn SandboxProvider>,
    /// The same object as `control`.
    pub containers: Option<Arc<dyn ContainerControl>>,
    /// The Cloudflare container control (feature 07). It implements `ContainerControl`.
    pub control: Arc<CloudflareContainerControl>,
}
```

`Ctx` is `Arc`-backed and `Clone`, so a handle clones the `ctx` field and reads the clock, the
entropy and the `Env` entry (spec 84) through the same `Ctx` as the host.

**The wrapper carries no HTTP client.** The platform-neutral way to make an HTTP or RPC call
is `foundation_netio`'s `DynNetClient = Arc<dyn NetClient>`. `HttpClientBuilder::build()` gives
the implementation of the target, and `H1Transport` builds on it. A Worker that calls its own
DO holds a `DynNetClient` over the stub (spec 85, section 4.5) and passes it to
`H1Transport::new`. So there is no `RuntimeFetch` type and no `fetch` field: one client
abstraction serves every part, and a target difference never reaches this struct.

An earlier draft of this section added `Option<RuntimeFetch>`. The owner removed it on
2026-11-15: "the core idea is `DynNetClient` is the platform agnostic way, and connectrpc
already builds on it."

**What belongs to which side.** The same API serves both targets. A member that only one side
can have sits behind the one seam that is already there, and no `cfg` decides the shape of a
public struct:

| Member | Both targets | Wasm only | Native only |
|---|---|---|---|
| `CloudflarePlatform`, `Platform` trait, `PlatformKind` | yes | | |
| The HTTP client of a call | `DynNetClient` (`foundation_netio`), on every target | `StubHttpClient` (spec 85, section 4.5) | nothing |
| `CloudflareContainerControl`, `DoContainerOps`, `DoContainerClient` | yes: `ContainerApi` is the seam | | |
| `WorkerContainer`, `WorkerStorage`, `WorkerAlarm`, `CloudflareHost::from_state` | | yes | |
| `FoundationCloudflareDeployer`, `WranglerCloudflareDeployer` | | | yes |

The deployment surface is not platform state. Both deployers apply a `DeploymentPlan`
(feature 08, section 6.8), they live in the native deployment crate, and the deploy host holds
them directly. So `CloudflarePlatform` carries no deployer and needs no `cfg` for one.

The host entry point builds one wrapper and passes it down. A wrapper never goes into the
`Ctx`: it holds the `Ctx`, and an entry lives in the `Ctx`, so the pair would make an `Arc`
cycle.

- `agent-rpc` on Cloudflare (spec 83 feature 06) takes `&CloudflarePlatform`.
- The native host (feature 06) takes `&NativePlatform`.
- A Docker host takes `&DockerPlatform`.
- A function that serves more than one part takes `&dyn Platform`. It calls `kind()` for a
  branch, or one of the `as_*` helpers when it needs the part.
- `agent-slave` takes `&dyn Platform`. In a nested host (feature 11) its `sandboxes()` is the
  inner control plane, so the slave creates child sandboxes without a driver of its own.

They do not build a sandbox, a driver or a container control. Once this library lands, the
host builds the wrapper, and that is all.

```rust
/// The control plane, as a trait. `SandboxManager` (local) and `SandboxRpcClient`
/// (remote) both implement it.
pub trait SandboxProvider: Send + Sync + 'static {
    fn probe(&self) -> BoxFuture<'_, SandboxResult<Vec<HostReport>>>;
    fn create(&self, spec: SandboxSpec) -> BoxFuture<'_, SandboxResult<SandboxHandle>>;
    fn create_with_progress(&self, spec: SandboxSpec) -> CreateStream;
    fn get(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<SandboxHandle>>;
    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<SandboxInfo>>>;
    fn stop(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>>;
    fn kill(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>>;
}

impl SandboxHandle {
    pub fn id(&self) -> &SandboxId;
    pub fn info(&self) -> SandboxInfo;                       // driver, state, created time
    pub fn container(&self) -> Option<ContainerHandle>;      // None for a microVM
    pub async fn exec(&self, command: Command) -> SandboxResult<ExecHandle>;   // streamed
    pub async fn run(&self, command: Command) -> SandboxResult<ExecOutput>;    // bounded capture
    pub async fn write_file(&self, path: &str, mode: u32, data: impl ByteStream) -> SandboxResult<u64>;
    pub async fn read_file(&self, path: &str) -> SandboxResult<FileStream>;
    pub async fn stop(&self) -> SandboxResult<ExitReport>;
    pub async fn kill(&self) -> SandboxResult<ExitReport>;
}
```

### 6.1 `SandboxSpec`

| Field | Type | Default | Notes |
|---|---|---|---|
| `driver` | `Option<DriverId>` | `None` | `None`: the control plane picks the first usable driver in its configured order. |
| `rootfs` | `RootFs` | required | `Directory`, `Ext4Image`, `SquashfsImage`, `ContainerImage(String)`, `Image(ImageRef)`. A driver rejects a kind that it cannot use. |
| `root_mode` | `RootMode` | `Overlay` | `Overlay`: writes go to a memory layer. `ReadOnly`: only `/tmp`, `/run` and the work directory are writable. |
| `vcpus` | `u8` | 1 | |
| `memory_mib` | `u32` | 512 | |
| `network` | `NetworkMode` | `None` | `None` or `Host`. The driver table of each feature says what `Host` means. |
| `dns` | `Vec<String>` | `["1.1.1.1", "8.8.8.8"]` | Used only with `NetworkMode::Host`. |
| `env` | `Vec<(String, String)>` | empty | Base environment of each command. |
| `workdir` | `String` | `/workspace` | The slave makes it. |
| `mounts` | `Vec<HostMount>` | empty | Host directory to guest path. Only drivers with the capability accept it. |
| `kernel` | `Option<KernelSource>` | driver default | Firecracker only. |
| `limits` | `SandboxLimits` | section 6.3 | |

### 6.2 Errors

`SandboxResult<T>` is `Result<T, ErrorTrace<SandboxError>>`. Feature 00 defines every
variant. Over RPC, each variant maps to a Connect code and a `detail` with the variant name
and fields, and the client maps it back, so law L11 holds for errors too.

| Group | Variants |
|---|---|
| Host set-up | `HostNotConfigured`, `NoDriver { reports }`, `DriverUnavailable { driver, reason }`, `BinaryNotFound`, `LibraryNotFound`, `LibraryTooOld`, `NotSigned` |
| Spec | `InvalidSpec { field, reason }`, `UnsupportedRootFs`, `UnsupportedNetwork`, `UnsupportedMount`, `UnsupportedCapability { capability }`, `SocketPathTooLong` |
| Start | `SubstrateExited { code, log_tail }`, `SubstrateApi { status, message }`, `BootTimeout { stage }`, `SlaveHandshake` |
| Use | `NotFound { what }`, `SandboxStopped`, `ExecTimeout`, `SlaveError { code, errno, message }`, `SequenceGap`, `InputQueueFull`, `StreamLost`, `TooManySessions` |
| Artifacts and images | `ArtifactHashMismatch`, `ArtifactUnsafePath`, `ArtifactTooLarge`, `ImageArchMismatch`, `BuildStepFailed` |
| Transport | `Rpc { code, message }`, `Unauthorized`, `Io { op }` |

### 6.3 Limits (root rule 8)

All are fields of `SandboxLimits` with builder methods.

| Field | Default | Source of the default |
|---|---|---|
| `boot_timeout` | 10 s | Firecracker reaches user space in about 125 ms; a cold Cloudflare container takes seconds. 10 s covers both and a slow CI host. |
| `shutdown_grace` | 5 s | Half of the Docker stop default (10 s). |
| `rpc_timeout` | 5 s | One unary call. They are local or one hop. |
| `max_message_bytes` | 4 MiB | The default maximum message size of gRPC. |
| `chunk_bytes` | 64 KiB | The Linux pipe buffer size: one pipe read is one event. |
| `max_exec_sessions` | 128 | The default `SETTINGS_MAX_CONCURRENT_STREAMS` of nginx for HTTP/2. |
| `max_captured_output` | 1 MiB for each stream | The `client_max_body_size` default of nginx. |
| `max_error_message` | 64 KiB | `BodyLimits::max_error_body` in this repository. |
| `output_drain_timeout` | 2 s | Time the slave waits for output after the program exits. |
| `upload_idle_timeout` | 60 s | The `client_body_timeout` default of nginx. |
| `input_retry_timeout` | 30 s | Half of the `send_timeout` default of nginx (60 s). |

`boot_timeout` is one deadline for the whole create: the driver launch, the readiness wait
and `Prepare` (feature 02, section 6.3). The Cloudflare control plane uses its own value,
60 s (feature 07). Features 12 and 14 add limits for images and builds in their own
structs.

### 6.4 Side effects (root rule 11)

- Time and timers: `ctx.clock()`. Sandbox IDs: the `IdSource` of the `Env` entry (spec 84),
  through the `SandboxIdPolicy` of feature 02 section 6.2 (`Random` or `Fixed(id)` for a test).
- Processes, sockets, files, the Docker client and the Cloudflare container handle are
  behind traits. Each role reads one typed entry from the `Ctx`: `SandboxHost` (control
  plane, native), `SlaveHost` (slave) and `CloudflareHost` (control plane in a DO). The
  handles themselves come in the wrapper of section 6.0, which implements the `Platform`
  trait; they are not entries.
- Only a boundary builds these entries and these wrappers. A missing entry or wrapper gives
  `HostNotConfigured`.
- Tests give in-memory entries, a simulated clock and a seeded `Ctx`.

### 6.5 Valtron style

- Logic is native `async fn`. Each start sequence is one `#[task]` state machine that emits
  `Pending(CreateProgress)` and one `Ready(result)`.
- Every fd is non-blocking on the shared reactor (`foundation_nativeapis::native::fd`). No
  code blocks an executor thread or loops on `Wait`.
- The slave uses valtron too (owner decision 2026-11-11). `foundation_nativeapis` with the
  `fd` feature compiles for `x86_64-unknown-linux-musl` (checked 2026-11-11).

### 6.6 Container control

A container is the substrate of a sandbox, and it is also a first-class object. The traits
are in feature 00, section 6.13. This section states the spec-level rules.

```rust
pub trait ContainerControl: Send + Sync + 'static {
    fn start(&self, ctx: Ctx, spec: ContainerSpec) -> BoxFuture<'_, SandboxResult<ContainerHandle>>;
    fn get(&self, ctx: Ctx, id: &ContainerId) -> BoxFuture<'_, SandboxResult<Option<ContainerHandle>>>;
    fn list(&self, ctx: Ctx) -> BoxFuture<'_, SandboxResult<Vec<ContainerInfo>>>;
}
```

- `ContainerControl` is the factory; `ContainerHandle` is the live object. The handle has
  `fetch`, `exec`, `signal`, `wait`, `stop`, `destroy` and `log_tail`.
- A boundary puts the control in the `containers` field of the wrapper (section 6.0), and
  every caller reads it there. On a native host it is `SandboxManager::containers`, the control
  of the first driver in its order that has one. On a Worker it is `DoContainerClient::new`, a
  remote control over the transport of the Worker. A `SandboxHandle` gives the container of one
  sandbox with `SandboxHandle::container()`.
- The Cloudflare adapter is in feature 07; the Docker adapter is in feature 10. Both use
  the same contract.
- `SandboxHandle::container()` returns the handle of a container-backed sandbox, or `None` for a
  microVM. This is the bridge between the two surfaces.
- A method that a runtime cannot support gives `UnsupportedCapability { capability }`.
  `ContainerCapabilities` states what the runtime supports before the call.
- The response body of `fetch` streams; the request body is one bounded message.
- A port that does not answer yet is `SandboxError::Rpc { code: Unavailable }` with the
  platform's text. A caller that waits for a server to listen retries until its deadline.
- A `ContainerHandle` is the only writer of its container. `destroy` is idempotent.

The agent-rpc Cloudflare host (spec 83 feature 06) is the first caller outside this spec. It
takes `&CloudflarePlatform` (section 6.0), reads `control` or `containers`, starts a
container, talks to its port, runs commands through the slave, and stops it.

## 7. Features and the way to the MVP

```mermaid
graph LR
  F00[00 contract] --> F01[01 slave core]
  F00 --> F02[02 control-plane core]
  F00 --> F03[03 guest artifacts]
  F00 --> F04[04 Firecracker API client]
  F01 --> F05[05 Firecracker driver]
  F02 --> F05
  F03 --> F05
  F04 --> F05
  F05 --> F06[06 programs, RPC, Provider, CI]
  F06 --> MA((MVP-A: Firecracker))
  F01 --> F07[07 Cloudflare control plane]
  F02 --> F07
  F07 --> F08[08 Cloudflare Worker and end to end]
  F06 --> F08
  F08 --> MB((MVP-B: Cloudflare))
  F08 --> F18[18 lifetime and stream cost]
  MB --> F09[09 libkrun]
  MB --> F10[10 Docker]
  F10 --> F11[11 nested sandboxes]
  F10 -.-> F16[16 Docker Sandboxes product<br/>draft, not scheduled]
  MB --> F17[17 Cloudflare Sandbox product]
  F09 --> F12[12 images] --> F13[13 container source] --> F14[14 Sandboxfile] --> F15[15 registry pull]
  F10 --> F13
```

_Caption: nine features give the MVP on Firecracker and on Cloudflare; the other substrates and the image tooling come after it and reuse its shape._

| # | Feature | Model | Phase | Done when |
|---|---|---|---|---|
| 00 | [Contract](features/00-contract/feature.md) | sonnet | MVP | The crate, the domain types and both service definitions exist with their law tests. |
| 01 | [Slave core](features/01-slave-core/feature.md) | sonnet | MVP | The slave serves `SlaveService` on a UDS as a normal Linux process and passes its scenarios. |
| 02 | [Control-plane core](features/02-control-plane-core/feature.md) | sonnet | MVP | `SandboxManager` creates, uses and stops sandboxes through a simulated driver and through a real slave process. |
| 03 | [Guest artifacts](features/03-guest-artifacts/feature.md) | sonnet | MVP | Kernel, root image and initramfs are fetched, verified and cached. |
| 04 | [Firecracker API client](features/04-firecracker-api-client/feature.md) | sonnet | MVP | Generated types and the client pass against a mock and the real program. |
| 05 | [Firecracker driver](features/05-firecracker-driver/feature.md) | sonnet | MVP | A command runs in a Firecracker microVM through `SandboxManager`. |
| 06 | [Programs, RPC and Provider](features/06-programs-rpc-provider/feature.md) | sonnet | **MVP-A** | `agent-sandbox` and `agent-slave` exist; `SandboxService` works; the `Provider` facade works; CI runs the microVM tests. |
| 07 | [Cloudflare control plane](features/07-cloudflare-control-plane/feature.md) | sonnet | MVP | The control plane compiles to wasm, drives a container through fakes, exposes `ContainerControl` and `ContainerHandle`, and holds the model-slave substrate of decision D16. |
| 08 | [Cloudflare Worker and deployment](features/08-cloudflare-worker/feature.md) | sonnet | **MVP-B** | A command runs in a Cloudflare container through `SandboxService` on a Worker; both deploy paths apply one plan; the Worker hosts the model slave of decision D16. |
| 09 | [libkrun driver](features/09-libkrun-driver/feature.md) | sonnet | 2 | A command runs in a libkrun microVM on Linux and macOS. |
| 10 | [Docker driver](features/10-docker-driver/feature.md) | sonnet | 2 | A command runs in a Docker container that the control plane starts through the host's Docker Engine; `agent-sandbox` can also run in a container. |
| 11 | [Nested sandboxes](features/11-nested-sandboxes/feature.md) | sonnet | 2 | A slave in a container starts Firecracker microVMs in it. |
| 16 | [Docker Sandboxes product driver](features/16-docker-sandboxes-driver/feature.md) | sonnet | draft, not scheduled | A command runs in a Docker Sandboxes microVM through the `sbx` program. |
| 17 | [Cloudflare Sandbox product support](features/17-cloudflare-sandbox-product/feature.md) | sonnet | 2 | An image made for the Cloudflare Sandbox product runs under the control plane; a plain HTTP caller uses `SandboxService`. |
| 18 | [Sandbox lifetime and stream cost](features/18-sandbox-lifetime-and-stream-cost/feature.md) | sonnet | 2 | A sandbox has a lifetime limit that shares the one DO alarm with the idle deadline; an `Exec` stream stops at `stream_max_duration`. Feature 18 follows the study in `research.md`. |
| 12 | [Sandbox images](features/12-sandbox-images/feature.md) | sonnet | 3 | An image with a manifest boots on Firecracker and libkrun; the slave can unpack a tree into a writable root. |
| 13 | [Container image source](features/13-container-image-source/feature.md) | sonnet | 3 | A Docker image becomes a sandbox image. |
| 14 | [Sandboxfile builder](features/14-sandboxfile-builder/feature.md) | sonnet | 3 | A build file with `FROM`, `ADD`, `FETCH`, `RUN`, `CMD` gives a sandbox image, with a step cache. |
| 15 | [Registry pull](features/15-registry-pull/feature.md) | sonnet | 3 | A container image is pulled from a registry with no Docker daemon. |

### 7.1 What the MVP is

**MVP-A (after feature 06; the longest path is four features deep: 00, then one of 01 to
04, then 05, then 06):** on a Linux host with KVM, `agent-sandbox run --rootfs <image> -- <program>` starts a
Firecracker microVM with `agent-slave` as PID 1 and prints the program's output; Rust code
does the same through `SandboxManager`, `SandboxRpcClient` or a `Provider`.

**MVP-B (after feature 08, two more features):** the same `SandboxService` calls work
against a Cloudflare Worker, where the control plane runs in a DO and the slave runs in the
DO's container. Feature 07 needs only features 01 and 02. Feature 08 also needs feature
06, for the native client (`SandboxRpcClient::connect`), the `Provider` facade and the CI
workflow; so MVP-B comes after MVP-A. Feature 08 also owns the two deployment paths: the
Rust deployer is the primary path, and the `wrangler` adapter is the fallback.

**After MVP-B:** feature 18 adds a lifetime deadline and a bound on the time an `Exec` stream
keeps a DO awake. It reads the study in `research.md`. The Durable Streams project does not
measure its dollar savings; the techniques themselves are tested.

Features 00 to 04 have no dependency on each other after 00, so 01, 02, 03 and 04 run in
parallel. Feature 07 starts as soon as 01 and 02 are done, in parallel with 05 and 06.

### 7.2 Why this order

Firecracker and Cloudflare are the two extremes: a local process with a REST API and a
PID 1 slave, and a `fetch`-only platform with the control plane in wasm. A core that fits
both has the right shape. libkrun, Docker and Docker Sandboxes then add a driver each, with
no change to the core. The Cloudflare Sandbox product adds no driver.

### 7.3 Not in this spec

Each item becomes a GitHub issue when feature 06 lands.

- The Firecracker `jailer`, custom seccomp filters, cgroup limits for a VMM process.
- Guest network devices (tap, `passt`), port forwards and an egress allow list.
- Snapshots, pause and resume.
- Interactive terminals.
- The libkrun 2.0 API. Only the runner program calls libkrun, so a later move is local.
- Windows hosts.
- The Docker Sandboxes cloud (`sbx --cloud`) and the Docker Sandboxes HTTP API (feature 16,
  section 6.9). Both need authentication on the slave link first.
- Authentication on the slave link for a TCP endpoint.
- Cloudflare Sandbox product functions with no call in the Rust `worker` crate: more than
  one image for each class, container snapshots, outbound interception, bucket mounts and
  directory backups (feature 17, section 6.4).
- The agent-rpc Worker, its bindings and its model path. Spec 83 feature 06 builds them on
  this spec's container control and deployer.

## 8. Tests

| Level | Needs | What it proves | Where |
|---|---|---|---|
| Contract | nothing | Message round trips, error mapping, spec validation. | `tests/contract_*.rs` |
| Simulation (DST) | nothing | Control-plane laws with a simulated driver, a scripted slave, a `SimulatedClock` and a seed. | `tests/control_sim_*.rs` |
| Slave process | Linux | The real slave as a normal process on a UDS. | `tests/slave_*.rs` |
| Mock substrate | nothing | Each driver's start sequence against a fake API and fake processes. | `tests/<driver>_sim_*.rs` |
| Container control | nothing (a fake runtime) | Start, fetch, exec, capabilities, stop and idempotent destroy on each adapter. | `tests/contract_container.rs`, `tests/*_container_*.rs` |
| Real substrate | KVM, Docker or `wrangler dev` | The acceptance scenarios end to end. | `tests/e2e_*.rs` |
| Boundaries | as the substrate | Programs, the `Provider` facade, the Worker. | `foundation_deployment_platform/tests/`, `infrastructure/sandbox_worker/test/` |

One file, `tests/support/acceptance.rs`, holds the shared acceptance scenarios (run a
command, exit code, input, files, isolation, stop). Each driver's `e2e_*.rs` calls them with
its own `SandboxProvider`, so every substrate proves the same behaviour, and so does the RPC
path (law L11).

Property tests use `proptest` 1.12, declared once in the root `[workspace.dependencies]` and
taken with `proptest = { workspace = true }`. Each test file sets one `ProptestConfig` with a
fixed seed, so a failure reproduces from the seed that the case prints. A feature names the
property tests it needs in the `Oracle` column: `identity`, `bound`, `metamorphic` or
`stateful model`.

A real substrate test calls `probe()` first. When the host cannot run it, the test prints
the report and returns.

CI (owner note 2026-11-11: the CI already uses KVM): GitHub-hosted `ubuntu-latest` has
`/dev/kvm`; `.github/workflows/platform-images-vm.yaml` uses it today. Feature 06 adds a
job that runs the Firecracker tests there. Operating systems that cannot run a test set opt
out in `ci/packages.toml` with a reason (`.agents` rule 16).

## 9. Rules for every feature

- A feature file follows `.agents/skills/story-driven-specification/skill.md`: story,
  bounded context, rules with numbered invariants, acceptance scenarios, technical
  breakdown, a table from laws to tests.
- Read `learnings.md` first.
- Tests go in `tests/` (root rule 7). One test must fail on the code before the feature.
- No `SystemTime::now()`, `Instant::now()`, `thread_rng()`, `std::process::Command::new`,
  socket bind or connect outside the files that a feature names as its native side-effect
  file and the boundary programs.
- Time types are `foundation_compact::Instant` and `foundation_compact::SystemTime`.
- Register each new crate and program in `ci/packages.toml`.
- Locally: `cargo fmt`, clippy with `--profile uat --no-deps`, and the tests of the changed
  crates.
- Update `progress.md` and `learnings.md` before the PR.
- Each bug or limit that a feature finds becomes a GitHub issue with a severity label.

## 10. Decisions and open questions

### 10.1 Decided by the owner (2026-11-11)

| # | Decision |
|---|---|
| D1 | The `Provider` trait is in `foundation_deployment_platform`, the platform abstraction every deployment uses. The sandbox `Provider` facades are in `foundation_sandbox/src/providers/`, behind the `provider` feature; all logic is in the crate (revised 2026-11-25). |
| D2 | The control plane is `agent-sandbox`; the in-sandbox part is `agent-slave`. They talk over Connect RPC (`foundation_connectrpc`). |
| D3 | Everything is a library split into `shared`, `native` and `cloudflare`. Programs, the DO and the Worker are boundaries. |
| D4 | Both program entry points are `src/bin/` of `foundation_sandbox`: `agent_sandbox.rs` and `agent_slave.rs` (revised 2026-11-25). |
| D5 | The slave uses valtron, not threads. |
| D6 | MVP order: Firecracker, then Cloudflare. libkrun and Docker follow. |
| D7 | The slave can start nested microVMs with the same core. |
| D8 | Image tooling: containers as a source, an own build file, and a registry pull (features 12 to 15). |
| D9 | The work is a specification on a branch, raised as a PR for review. No GitHub issue is filed yet. |
| D10 | (2026-11-12) "Docker sandbox" and "Cloudflare sandbox" are the products. Both are supported: Docker Sandboxes with a driver (feature 16), Cloudflare Sandbox on the Cloudflare container driver (feature 17). |
| D11 | (2026-11-12, superseded 2026-11-25) `agent-slave` first had its own package, because `foundation_deployment_platform` does not build for musl. It now lives in `foundation_sandbox/src/bin/agent_slave.rs`: the `slave` feature alone pulls no TLS library and no Docker client, so the binary builds for musl. Feature 06 section 6.10 keeps an issue open for the exact dependency set. |
| D12 | (2026-11-12) The Docker driver talks to the Docker Engine of the host and starts containers that hold the slave. The general case is a control plane on the host. A control plane in a Docker container is the operator's choice; it uses the host's Docker socket. |
| D13 | (2026-11-12) On Cloudflare the control plane runs `agent-slave` in Cloudflare containers. There is no nested case there. |
| D16 | (2026-11-25) On Cloudflare the slave may also be another Durable Object (DO). That DO answers `SlaveService` with agent turns against OpenRouter or the Cloudflare Model API, so a model runtime is a sandbox with the same RPC surface. The driver id is `cloudflare-do`. It is **not** an isolation boundary; it shares feature 07 (library) and feature 08 (Worker classes) with the Cloudflare container. |
| D14 | (2026-11-12) The Docker Sandboxes product is not ready. Feature 16 is a draft and not a must have. It is not scheduled, and no milestone needs it. |
| D15 | (2026-11-14) Spec 77 owns Cloudflare container control and Cloudflare deployment. `foundation_deployment_cloudflare` is the primary deploy path; the manual `wrangler` adapter is a supported fallback. Both apply one `DeploymentPlan`. Spec 83 feature 06 consumes these surfaces and builds the agent-rpc Worker on them. |
| D16 | (2026-11-15) The target vocabulary is two levels with the same shape: `SandboxProvider` and `SandboxHandle`, `ContainerControl` and `ContainerHandle` (section 6.0). The old control trait is `SandboxProvider`; the old handle struct is `SandboxHandle`; the old id-policy enum of feature 02 is `SandboxIdPolicy`, because `IdSource` is the spec 84 trait. Feature 06's deployment adapter is `SandboxDeploymentProvider` with the handle `SandboxDeploymentHandle`, so it does not clash with the control. Every outside caller takes a reference to the wrapper of section 6.0 (the concrete type, or `&dyn Platform` when a function serves several parts). `foundation_context` gets no change. |
| D17 | (2026-11-15) The secret and the bearer check come from spec 86 (`Secret`, `extract_bearer_token`, `Secret::ct_eq`). This spec defines neither. Mutual TLS comes from spec 87; the native listener may add it, and a Worker uses `CfMtlsAuth`. The wasm gaps (the streaming client, the streaming bridges and sink, `StubHttpClient`) come from spec 85; the transport is `H1Transport` on every host. |

### 10.2 Open, with a recommendation

| # | Question | Recommendation |
|---|---|---|
| Q2 | The owner named `foundation_platform_docker`. The existing Docker client crate is `foundation_deployment_docker`. | (2026-11-24) Decided: use `foundation_deployment_docker`. A rename is a separate change. |
| Q4 | Authentication of `SandboxService`: a bearer token on TCP and on the Worker; file permissions on a UDS. A Worker cannot run its own mTLS: Cloudflare ends TLS. | (2026-11-14) Decided: a signed secret as a bearer credential, or Cloudflare's own mTLS (`request.cf.tlsClientAuth`), through the `ClientAuth` providers of spec 86 and spec 87. The secret and the bearer check are spec 86's `Secret` and `extract_bearer_token`. A direct native listener adds mTLS when spec 87 lands; on a Worker the provider is `CfMtlsAuth` of spec 87. |
| Q5 | The old spec `start.md` files are not in spec 76 or here. | (2026-11-14) Resolved: this spec has a `start.md` at its root and one in each feature directory, as spec 83 does. |
