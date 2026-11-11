---
feature: "Contract"
status: "pending"
priority: "high"
model: "sonnet"
phase: "MVP"
depends_on: []
crates: ["foundation_platform_sandbox"]
---

# Feature 00: The contract (domain types and the two RPC services)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); remote procedure
call (RPC); JavaScript Object Notation (JSON); Unix domain socket (UDS); process identifier
(PID); WebAssembly (wasm); behaviour-driven development (BDD); continuous integration (CI);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

Create `backends/foundation_platform_sandbox` with everything that the other features agree
on: the domain types, the error type, the limits, the two RPC service definitions, and the
traits that drivers, side effects and the control plane implement. No behaviour that needs
an operating system. The crate compiles for native targets and for
`wasm32-unknown-unknown`.

After this feature, features 01, 02, 03 and 04 start in parallel.

## 2. Story

- **S1**: "A control plane and a slave agree on the messages of each call, on every
  transport."
- **S2**: "A caller describes the sandbox that it wants, and gets an error that names the
  wrong field before anything starts."
- **S3**: "An error that starts in the slave or in a driver reaches a remote caller as the
  same typed error."
- **S4**: "A message is larger than the limit, or a sequence number is not the next one."
- **S5**: "Library code asks for its side effects and none were given."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Encode and decode a call | any peer | an RPC | The same message on both sides. |
| U2 | Validate a spec | control plane | `SandboxSpec::validate(caps)` | `Ok`, or the first failed rule. |
| U3 | Carry an error | any server | a `SandboxError` | A Connect error with a code and a detail; the client gets the same variant back. |
| U4 | Read the side effects | library code | `SandboxHost::from_ctx(ctx)` | The bundle, or `HostNotConfigured`. |
| U5 | Check a sequence | slave, control plane | `Sequence::accept(n)` | `Apply`, `Duplicate` or `Gap`. |

## 3. Bounded context

**Inside this feature:** value types and interfaces. Nothing here starts, connects, reads a
clock or touches a file.

**Outside:** the slave behaviour (feature 01); the control-plane behaviour (feature 02);
the drivers (05, 07, 09, 10); `foundation_connectrpc` (the RPC engine, used as it is).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| `SandboxSpec` | What the caller wants. A value. | `shared::spec` |
| `SandboxLimits` | Every size and time limit. | `shared::limits` |
| `Command`, `ExitStatus`, `ExecEvent` | One program run and its results. | `shared::command` |
| `SandboxError`, `SandboxErrorWire` | Every failure, and its form on the wire. | `shared::error` |
| `SlaveService`, `SandboxService` | The two RPC interfaces and their messages. | `shared::rpc` |
| `Sequence` | A counter that accepts only the next number. | `shared::sequence` |
| `SandboxId`, `ExecId`, `UploadId`, `DriverId` | Identifiers. | `shared::ids` |
| `Capabilities`, `HostReport` | What a driver can do here. | each driver (defined here) |
| `SandboxDriver`, `SubstrateHandle`, `SlaveEndpoint` | The driver interface. | defined here; implemented in 05, 07, 09, 10 |
| `SandboxControl` | The control-plane interface. | defined here; implemented in 02 and 06 |
| `SandboxHost`, `SlaveHost` | The side-effect bundles. | defined here; built only by boundaries |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| validate | entity | a spec and capabilities | Returns the first failed rule; changes nothing. |
| to wire / from wire | entity | an error | Converts between `SandboxError` and a Connect error. |
| accept | entity | a sequence number | Advances the counter, or reports a duplicate or a gap. |
| from_ctx | entity | a `Ctx` | Returns a bundle or `HostNotConfigured`. |

**Words.** *Substrate*: the isolation technology of one sandbox. *Endpoint*: how to reach
the slave of a substrate. *Session*: one running `Exec` call. *Upload*: one file write in
progress. *Chunk*: a piece of bytes of at most `chunk_bytes`.

## 4. Rules

### 4.1 Law worksheet

```
Entity: Message (every request and response type)
Conserved value:         decode(encode(m)) = m
Bound:                   encoded size <= max_message_bytes; a data field <= chunk_bytes
Inverse pairs:           encode / decode
Idempotent verbs:        none
Ordering pairs:          none
Monotone values:         none
Freeze points:           field names and JSON shapes of version v1
Links to other entities: a scoped SandboxService message = sandbox_id + the SlaveService message

Entity: SandboxSpec
Conserved value:         none
Bound:                   vcpus >= 1; memory_mib >= 64; every limit > 0
Inverse pairs:           none
Idempotent verbs:        validate
Ordering pairs:          the rules run in the fixed order of section 6.3
Monotone values:         none
Freeze points:           a spec does not change after create takes it
Links to other entities: rootfs kind, network mode and mounts must be in Capabilities

Entity: SandboxError
Conserved value:         from_wire(to_wire(e)) = e for every variant
Bound:                   every text field <= max_error_message
Inverse pairs:           to_wire / from_wire
Idempotent verbs:        none
Ordering pairs:          none
Monotone values:         none
Freeze points:           variant names and the code of each variant
Links to other entities: each variant has exactly one Connect code

Entity: Sequence
Conserved value:         next = count of accepted numbers
Bound:                   next <= u64::MAX
Inverse pairs:           none
Idempotent verbs:        accept(n) for n < next (reports Duplicate, changes nothing)
Ordering pairs:          accept(n) enables accept(n + 1)
Monotone values:         next
Freeze points:           none
Links to other entities: ExecInput.seq and FileWriteChunk.offset are checked by it
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | Every message type round-trips through its JSON codec with no change. |
| I2 | Bytes in a message are base64 text in JSON, and decode to the same bytes. |
| I3 | Every call of both services is unary or server-streaming. No call needs a streamed request. |
| I4 | `validate` returns the first failed rule in the order of section 6.3 and changes nothing. |
| I5 | Every `SandboxError` variant maps to one Connect code and back to the same variant with the same fields. |
| I6 | `Sequence::accept(n)`: `n == next` gives `Apply` and adds 1; `n < next` gives `Duplicate` and changes nothing; `n > next` gives `Gap` and changes nothing. |
| I7 | A missing side-effect bundle gives `HostNotConfigured`. Nothing falls back to the real system. |
| I8 | Every limit has a default from `SandboxLimits::default()`, and no code path holds a limit as a `const`. |
| I9 | Two `Ctx` values with the same seed give the same sequence of `SandboxId`, `ExecId` and `UploadId`. |
| I10 | The crate compiles for `wasm32-unknown-unknown` with `--no-default-features`. |

### 4.3 Must have

- The module tree `shared/`, `native/`, `cloudflare/`, `cli/` (the last three empty except
  for their `mod.rs` and target gates).
- Both service traits with the `#[service]` macro.
- Every `SandboxError` variant of requirements section 6.2.

### 4.4 Must do

- Document each limit default with its source.
- Give each message field a doc comment that says its unit and its bound.

### 4.5 Must not

- Must not read a clock, a random source, a file, a socket or an environment variable.
- Must not put a driver or slave behaviour in this feature.
- Must not add a call that streams its request.

## 5. Acceptance scenarios

```gherkin
Feature: Contract

  Scenario: A message survives a round trip
    Given one value of each request and response type, with data fields of 0, 1 and 65536 bytes
    When each is encoded to JSON and decoded
    Then the decoded value equals the first value

  Scenario: A spec with a wrong root kind is refused
    Given capabilities with rootfs_squashfs false
    When a spec with RootFs::SquashfsImage is validated
    Then the result is UnsupportedRootFs

  Scenario: The first failed rule wins
    Given a spec with vcpus 0 and a relative workdir
    When it is validated
    Then the result is InvalidSpec { field: "vcpus" }

  Scenario: An error keeps its type across RPC
    Given the error SlaveError { code: NotFound, errno: Some(2), message: "no such file" }
    When it is converted to a Connect error and back
    Then the result equals the first error
    And the Connect code is NotFound

  Scenario: An unknown remote error is still an error
    Given a Connect error with code Internal and no detail
    When it is converted to a SandboxError
    Then the result is Rpc { code: Internal, message }

  Scenario: The next sequence number is applied
    Given a new Sequence
    When accept(0) then accept(1) are called
    Then both return Apply and next is 2

  Scenario: A repeated sequence number has no second effect
    Given a Sequence with next 2
    When accept(1) is called
    Then it returns Duplicate and next is 2

  Scenario: A gap is refused
    Given a Sequence with next 2
    When accept(3) is called
    Then it returns Gap and next is 2

  Scenario: No side-effect bundle, no fall-back
    Given a Ctx with no SandboxHost
    When SandboxHost::from_ctx is called
    Then the result is HostNotConfigured

  Scenario: A socket path that is too long is refused
    When check_socket_path is called with a path of 104 bytes
    Then the result is SocketPathTooLong
```

## 6. Technical breakdown

### 6.1 Crate

`backends/foundation_platform_sandbox`, version `0.0.1`, workspace edition, lints and
licence.

| Cargo feature | Adds | Default |
|---|---|---|
| `slave` | The slave engine (feature 01). Native only. | off |
| `control` | `SandboxManager` (feature 02). | on |
| `firecracker` | The Firecracker driver (features 04, 05). Linux only. | on |
| `libkrun` | The libkrun driver (feature 09). Linux and macOS. | off |
| `docker` | The Docker driver (feature 10). | off |
| `cloudflare` | The Cloudflare modules (feature 07). wasm32 only. | off |
| `cli` | The command structures (feature 06). Native only. | off |
| `testing` | The simulation helpers as a public module (feature 02). | off |

Dependencies of this feature: `foundation_context`, `foundation_core`, `foundation_compact`
(`default-features = false`), `foundation_errstacks`, `foundation_connectrpc`,
`foundation_macros`, `serde`, `serde_json`, `base64`, `derive_more`, `futures`, `tracing`.

```
src/
  lib.rs
  shared/mod.rs
  shared/spec.rs        SandboxSpec, RootFs, RootMode, NetworkMode, HostMount, KernelSource, validate
  shared/limits.rs      SandboxLimits
  shared/command.rs     Command, StdinMode, ExitStatus, ExecEvent, ExecOutput
  shared/error.rs       SandboxError, SandboxResult, SandboxErrorWire, the code table
  shared/ids.rs         SandboxId, ExecId, UploadId, DriverId, check_socket_path
  shared/sequence.rs    Sequence
  shared/rpc/mod.rs     the two #[service] traits
  shared/rpc/messages.rs  every request and response type
  shared/driver.rs      SandboxDriver, Substrate, SubstrateHandle, SlaveEndpoint, Capabilities, HostReport, CreateProgress
  shared/control.rs     SandboxControl, Sandbox (the handle type), SandboxInfo, SandboxState, ExitReport
  shared/host.rs        SandboxHost, SlaveHost and their traits
  native/mod.rs         #[cfg(not(target_family = "wasm"))]
  cloudflare/mod.rs     #[cfg(all(target_family = "wasm", feature = "cloudflare"))]
  cli/mod.rs            #[cfg(all(not(target_family = "wasm"), feature = "cli"))]
```

### 6.2 Domain types

Requirements section 6.1 has the `SandboxSpec` field table. The types that it names:

```rust
pub enum RootFs {
    Directory(PathBuf),          // a directory on the control plane's host
    Ext4Image(PathBuf),          // a raw ext4 image file on the control plane's host
    SquashfsImage(PathBuf),
    ContainerImage(String),      // a name that the driver understands (Docker tag, Cloudflare image)
    Image(ImageRef),             // a sandbox image of feature 12
}
pub enum RootMode { Overlay, ReadOnly }          // feature 12 adds Writable
pub enum NetworkMode { None, Host }
pub struct HostMount { pub host_path: PathBuf, pub guest_path: String, pub read_only: bool }
pub enum KernelSource { Default, Path(PathBuf) }

pub struct Command {
    pub argv: Vec<String>,                 // argv[0] is the program; no shell
    pub env: Vec<(String, String)>,        // added over the sandbox env; a later key wins
    pub cwd: Option<String>,               // None = the sandbox workdir
    pub stdin: StdinMode,                  // Null | Piped | Bytes(Vec<u8>)
    pub timeout: Option<Duration>,
}
pub struct ExitStatus { pub code: Option<i32>, pub signal: Option<i32> }   // exactly one is Some
pub enum ExecEvent { Stdout(Vec<u8>), Stderr(Vec<u8>), Exit(ExitStatus) }
pub struct ExecOutput {
    pub stdout: Vec<u8>, pub stderr: Vec<u8>, pub status: ExitStatus,
    pub stdout_truncated: bool, pub stderr_truncated: bool,
}
```

A path in `RootFs` and `HostMount` is a path on the machine where the control plane runs.
A remote caller of `SandboxService` names paths of that machine.

Identifiers: `SandboxId` is 12 lower-case hex characters, `ExecId` and `UploadId` are 16.
Each has `fn generate(ctx: &Ctx) -> Self` that reads `ctx.rng()`, and a `FromStr` that
accepts only that shape. `DriverId` is an enum: `Firecracker`, `Libkrun`, `Docker`,
`CloudflareContainer`, with the display strings `firecracker`, `libkrun`, `docker`,
`cloudflare-container`.

### 6.3 `SandboxSpec::validate`

`validate(&self, caps: &Capabilities) -> SandboxResult<()>` checks, in this order, and
returns at the first failure:

1. `vcpus >= 1` (`InvalidSpec { field: "vcpus" }`); `memory_mib >= 64`
   (`InvalidSpec { field: "memory_mib" }`).
2. `workdir` and each `HostMount.guest_path` are absolute and have no `..` part
   (`InvalidSpec`).
3. Each `env` key is not empty and has no `=` or zero byte (`InvalidSpec { field: "env" }`).
4. Each `dns` entry parses as an IP address (`InvalidSpec { field: "dns" }`).
5. The `rootfs` kind is in `caps` (`UnsupportedRootFs`).
6. `root_mode` is in `caps.root_modes` (`InvalidSpec { field: "root_mode" }`).
7. `network == Host` needs `caps.network_host` (`UnsupportedNetwork`).
8. `mounts` not empty needs `caps.host_mounts` (`UnsupportedMount`).
9. Each limit is more than zero; `chunk_bytes <= max_message_bytes / 2`
   (`InvalidSpec { field: "limits" }`).

```rust
pub struct Capabilities {
    pub rootfs_directory: bool,
    pub rootfs_ext4: bool,
    pub rootfs_squashfs: bool,
    pub rootfs_container_image: bool,
    pub root_modes: &'static [RootMode],
    pub host_mounts: bool,
    pub network_host: bool,
    pub max_vcpus: u8,
}
```

### 6.4 `SlaveService`

```rust
#[service(package = "ewe.sandbox.slave.v1", codecs(json))]
pub trait SlaveService {
    async fn info(&self, ctx: RpcCtx, req: Request<InfoRequest>) -> ConnectResult<Response<SlaveInfo>>;
    async fn prepare(&self, ctx: RpcCtx, req: Request<PrepareRequest>) -> ConnectResult<Response<PrepareResponse>>;
    async fn exec(&self, ctx: RpcCtx, req: Request<ExecRequest>)
        -> ConnectResult<impl Stream<Item = ConnectResult<ExecEventMsg>>>;
    async fn exec_input(&self, ctx: RpcCtx, req: Request<ExecInputRequest>) -> ConnectResult<Response<ExecInputResponse>>;
    async fn exec_signal(&self, ctx: RpcCtx, req: Request<ExecSignalRequest>) -> ConnectResult<Response<Empty>>;
    async fn file_write_begin(&self, ctx: RpcCtx, req: Request<FileWriteBeginRequest>) -> ConnectResult<Response<FileWriteBeginResponse>>;
    async fn file_write_chunk(&self, ctx: RpcCtx, req: Request<FileWriteChunkRequest>) -> ConnectResult<Response<FileWriteChunkResponse>>;
    async fn file_write_commit(&self, ctx: RpcCtx, req: Request<FileWriteCommitRequest>) -> ConnectResult<Response<FileWriteCommitResponse>>;
    async fn file_write_abort(&self, ctx: RpcCtx, req: Request<FileWriteAbortRequest>) -> ConnectResult<Response<Empty>>;
    async fn file_read(&self, ctx: RpcCtx, req: Request<FileReadRequest>)
        -> ConnectResult<impl Stream<Item = ConnectResult<FileChunk>>>;
    async fn shutdown(&self, ctx: RpcCtx, req: Request<ShutdownRequest>) -> ConnectResult<Response<Empty>>;
}
```

The macro form is the one of `backends/foundation_connectrpc/examples/code_first_streaming/main.rs`.

Messages (all `Clone + Debug + PartialEq + Serialize + Deserialize`; `Vec<u8>` fields use a
base64 `serde` helper):

```rust
pub struct InfoRequest {}
pub struct SlaveInfo {
    pub slave_version: String,
    pub arch: String,                 // "x86_64" | "aarch64"
    pub kernel: String,               // uname release
    pub is_pid1: bool,
    pub prepared: bool,
    pub can_control: bool,            // true when the slave also serves SandboxService (feature 11)
}

pub struct PrepareRequest {
    pub root: RootSetup,
    pub workdir: String,
    pub env: Vec<(String, String)>,
    pub mounts: Vec<GuestMount>,      // { tag, guest_path, read_only }
    pub dns: Vec<String>,             // empty = do not touch /etc/resolv.conf
    pub limits: SlaveLimits,
}
pub struct RootSetup { pub source: RootSource, pub mode: RootMode }
pub enum RootSource {
    BlockDevice { device: String, fstype: FsType },   // FsType: Ext4 | Squashfs
    VirtioFs { tag: String },
    Plain,                                            // use the slave's own root as it is
}
pub struct SlaveLimits {
    pub chunk_bytes: u32,
    pub max_exec_sessions: u32,
    pub output_drain_timeout_ms: u32,
    pub upload_idle_timeout_ms: u32,
    pub input_queue_bytes: u32,       // chunk_bytes * 4
}
pub struct PrepareResponse { pub root_mode: RootMode }

pub struct ExecRequest {
    pub exec_id: String,              // chosen by the caller; ExecId shape
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
    pub stdin: bool,                  // false = /dev/null
}
pub struct ExecEventMsg { pub seq: u64, pub event: ExecEventKind }
pub enum ExecEventKind {
    Started { pid: u32 },             // always seq 0
    Stdout { data: Vec<u8> },
    Stderr { data: Vec<u8> },
    Exit { code: Option<i32>, signal: Option<i32> },   // always last
}
pub struct ExecInputRequest { pub exec_id: String, pub seq: u64, pub data: Vec<u8>, pub eof: bool }
pub struct ExecInputResponse { pub next_seq: u64 }
pub struct ExecSignalRequest { pub exec_id: String, pub signal: i32 }

pub struct FileWriteBeginRequest { pub path: String, pub mode: u32 }
pub struct FileWriteBeginResponse { pub upload_id: String }
pub struct FileWriteChunkRequest { pub upload_id: String, pub offset: u64, pub data: Vec<u8> }
pub struct FileWriteChunkResponse { pub next_offset: u64 }
pub struct FileWriteCommitRequest { pub upload_id: String, pub size: u64 }
pub struct FileWriteCommitResponse { pub size: u64 }
pub struct FileWriteAbortRequest { pub upload_id: String }
pub struct FileReadRequest { pub path: String }
pub struct FileChunk { pub offset: u64, pub data: Vec<u8>, pub eof: bool }

pub struct ShutdownRequest { pub grace_ms: u32 }
pub struct Empty {}
```

Feature 01 defines what each call does. This feature defines only the shapes.

### 6.5 `SandboxService`

```rust
#[service(package = "ewe.sandbox.control.v1", codecs(json))]
pub trait SandboxService {
    async fn probe(&self, ctx: RpcCtx, req: Request<Empty>) -> ConnectResult<Response<ProbeResponse>>;
    async fn create(&self, ctx: RpcCtx, req: Request<CreateRequest>)
        -> ConnectResult<impl Stream<Item = ConnectResult<CreateEvent>>>;
    async fn get(&self, ctx: RpcCtx, req: Request<SandboxRef>) -> ConnectResult<Response<SandboxInfo>>;
    async fn list(&self, ctx: RpcCtx, req: Request<Empty>) -> ConnectResult<Response<ListResponse>>;
    async fn stop(&self, ctx: RpcCtx, req: Request<SandboxRef>) -> ConnectResult<Response<ExitReport>>;
    async fn kill(&self, ctx: RpcCtx, req: Request<SandboxRef>) -> ConnectResult<Response<ExitReport>>;
    async fn exec(&self, ctx: RpcCtx, req: Request<Scoped<ExecRequest>>)
        -> ConnectResult<impl Stream<Item = ConnectResult<ExecEventMsg>>>;
    async fn exec_input(&self, ctx: RpcCtx, req: Request<Scoped<ExecInputRequest>>) -> ConnectResult<Response<ExecInputResponse>>;
    async fn exec_signal(&self, ctx: RpcCtx, req: Request<Scoped<ExecSignalRequest>>) -> ConnectResult<Response<Empty>>;
    async fn file_write_begin(&self, ctx: RpcCtx, req: Request<Scoped<FileWriteBeginRequest>>) -> ConnectResult<Response<FileWriteBeginResponse>>;
    async fn file_write_chunk(&self, ctx: RpcCtx, req: Request<Scoped<FileWriteChunkRequest>>) -> ConnectResult<Response<FileWriteChunkResponse>>;
    async fn file_write_commit(&self, ctx: RpcCtx, req: Request<Scoped<FileWriteCommitRequest>>) -> ConnectResult<Response<FileWriteCommitResponse>>;
    async fn file_write_abort(&self, ctx: RpcCtx, req: Request<Scoped<FileWriteAbortRequest>>) -> ConnectResult<Response<Empty>>;
    async fn file_read(&self, ctx: RpcCtx, req: Request<Scoped<FileReadRequest>>)
        -> ConnectResult<impl Stream<Item = ConnectResult<FileChunk>>>;
}

pub struct Scoped<T> { pub sandbox_id: String, #[serde(flatten)] pub inner: T }
pub struct SandboxRef { pub sandbox_id: String }
pub struct CreateRequest { pub spec: SandboxSpec }
pub enum CreateEvent { Progress(CreateProgress), Created(SandboxInfo) }   // Created is last
pub struct ProbeResponse { pub reports: Vec<HostReport> }
pub struct ListResponse { pub sandboxes: Vec<SandboxInfo> }
pub struct SandboxInfo {
    pub id: String, pub driver: DriverId, pub state: SandboxState,
    pub created_unix_ms: u64, pub slave: Option<SlaveInfo>,
}
pub enum SandboxState { Creating, Ready, Stopping, Stopped, Failed }
pub struct ExitReport { pub substrate_exit_code: Option<i32>, pub killed: bool }
pub enum CreateProgress { Preparing, SubstrateStarted, SubstrateConfigured, Booting, SlaveConnected, SlaveReady }
```

`SandboxSpec` and its parts derive `Serialize` and `Deserialize`. `Duration` fields
serialize as integer milliseconds.

### 6.6 Errors on the wire

```rust
/// The JSON body of the Connect error detail. Type name: "ewe.sandbox.v1.SandboxError".
pub struct SandboxErrorWire { pub variant: String, pub fields: serde_json::Value }

impl SandboxError {
    pub fn to_connect(&self, max_message: usize) -> ConnectError;     // code from the table + one detail
    pub fn from_connect(err: &ConnectError) -> SandboxError;          // no detail => Rpc { code, message }
}
```

| Variant | Connect code |
|---|---|
| `InvalidSpec`, `UnsupportedRootFs`, `UnsupportedNetwork`, `UnsupportedMount`, `SocketPathTooLong` | `InvalidArgument` |
| `NotFound`, `SlaveError` with code `NotFound` | `NotFound` |
| `SandboxStopped`, `HostNotConfigured`, `NoDriver`, `DriverUnavailable`, `BinaryNotFound`, `LibraryNotFound`, `LibraryTooOld`, `NotSigned` | `FailedPrecondition` |
| `BootTimeout`, `ExecTimeout` | `DeadlineExceeded` |
| `TooManySessions`, `InputQueueFull` | `ResourceExhausted` |
| `SequenceGap` | `OutOfRange` |
| `StreamLost` | `Aborted` |
| `Unauthorized` | `Unauthenticated` |
| `SlaveError` with code `PermissionDenied` | `PermissionDenied` |
| `SubstrateExited`, `SubstrateApi`, `SlaveHandshake`, other `SlaveError`, `Io`, artifact and image variants | `Internal` |
| `Rpc { code, .. }` | that code |

`SlaveError.code` is `SlaveErrorCode`: `BadRequest`, `NotFound`, `PermissionDenied`,
`SpawnFailed`, `RootSetupFailed`, `MountFailed`, `Io`, `AlreadyExists`, `NotPrepared`,
`Internal`. Text fields are cut to `max_error_message` in `to_connect`.

### 6.7 `Sequence`

```rust
#[derive(Debug, Default)]
pub struct Sequence { next: u64 }
pub enum SequenceCheck { Apply, Duplicate, Gap }
impl Sequence {
    pub fn next(&self) -> u64;
    /// For a counter that grows by 1 (ExecInput.seq).
    pub fn accept(&mut self, n: u64) -> SequenceCheck;
    /// For a byte offset (FileWriteChunk.offset): `len` is added on Apply.
    pub fn accept_span(&mut self, offset: u64, len: u64) -> SequenceCheck;
}
```

`accept_span(offset, len)`: `offset == next` gives `Apply` and `next += len`;
`offset + len <= next` gives `Duplicate`; every other case gives `Gap` (this includes a
chunk that overlaps the end).

### 6.8 Driver interface

```rust
pub trait SandboxDriver: Send + Sync + 'static {
    fn id(&self) -> DriverId;
    /// Checks the host. Starts nothing. Ends in rpc_timeout or sooner.
    fn probe(&self, ctx: Ctx) -> BoxFuture<'static, HostReport>;
    /// Starts one substrate. Emits progress, then one result.
    fn launch(&self, ctx: Ctx, id: SandboxId, spec: SandboxSpec) -> LaunchStream;
    /// Finds the substrate of a stored record again, after the control plane restarted.
    /// `Ok(None)`: the substrate does not exist any more. A driver whose substrates die
    /// with the control plane's process returns `Ok(None)` and removes what is left of
    /// that sandbox on the host.
    fn attach(&self, ctx: Ctx, record: &StoredRecord) -> BoxFuture<'_, SandboxResult<Option<Substrate>>>;
}

/// What a control plane keeps about one sandbox outside its memory.
pub struct StoredRecord {
    pub id: SandboxId, pub driver: DriverId, pub spec: SandboxSpec,
    pub state: SandboxState, pub created_unix_ms: u64, pub endpoint: SlaveEndpoint,
}
pub type LaunchStream =
    Box<dyn StreamIterator<D = SandboxResult<Substrate>, P = CreateProgress> + Send + 'static>;

/// A started substrate. The slave in it may not answer yet.
pub struct Substrate {
    pub endpoint: SlaveEndpoint,
    pub root: RootSetup,                  // what the control plane sends in Prepare
    pub mounts: Vec<GuestMount>,
    pub handle: Box<dyn SubstrateHandle>,
}
pub enum SlaveEndpoint {
    Unix { path: PathBuf },
    FirecrackerVsock { uds: PathBuf, port: u32 },
    Tcp { host: String, port: u16 },
    CloudflarePort { port: u16 },
}
pub trait SubstrateHandle: Send + Sync + 'static {
    /// Completes when the substrate stops by itself.
    fn exited(&self) -> BoxFuture<'_, SubstrateExit>;
    /// Asks for an exit and waits up to `grace`; then forces it.
    fn terminate(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>>;
    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>>;
    /// The last bytes of the substrate logs, for error messages.
    fn log_tail(&self, max: usize) -> BoxFuture<'_, Vec<u8>>;
    /// Removes every host resource. Safe to call two times.
    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>>;
    /// The error for a substrate that stopped during a start. The default is
    /// `SubstrateExited { code, log_tail }`. A driver whose exit codes have a meaning
    /// (libkrun, feature 09) gives a more exact error.
    fn exit_error(&self, exit: &SubstrateExit, log_tail: Vec<u8>) -> SandboxError {
        SandboxError::SubstrateExited { code: exit.code, log_tail }
    }
}
pub struct SubstrateExit { pub code: Option<i32>, pub signal: Option<i32>, pub forced: bool }
pub struct HostReport { pub driver: DriverId, pub usable: bool, pub checks: Vec<HostCheck>, pub capabilities: Capabilities }
pub struct HostCheck { pub name: String, pub ok: bool, pub message: String }
```

The readiness wait (call `Info` until it answers) and `Prepare` are in the control plane
(feature 02), not in a driver. A driver only starts and stops.

### 6.9 Side-effect bundles

```rust
/// Control plane, native.
#[derive(Clone)]
pub struct SandboxHost {
    pub processes: Arc<dyn ProcessSpawner>,
    pub fs: Arc<dyn HostFs>,
    pub connector: Arc<dyn SlaveConnector>,
    pub config: Arc<HostConfig>,
}
/// Turns an endpoint into an RPC transport.
pub trait SlaveConnector: Send + Sync + 'static {
    fn connect(&self, ctx: &Ctx, endpoint: &SlaveEndpoint, limits: &SandboxLimits)
        -> SandboxResult<Arc<dyn foundation_connectrpc::shared::transport::Transport>>;
}
pub struct HostConfig {
    pub state_dir: PathBuf,              // run directories; short (socket path limit)
    pub cache_dir: PathBuf,
    pub slave_path: Option<PathBuf>,     // agent-slave for the guest architecture
    pub firecracker_path: Option<PathBuf>,
    pub krun_runner_path: Option<PathBuf>,
    pub libkrun_path: Option<PathBuf>,
    pub driver_order: Vec<DriverId>,     // default: firecracker, libkrun, docker
}
pub trait ProcessSpawner: Send + Sync + 'static {
    fn spawn(&self, spec: ProcessSpec) -> SandboxResult<Box<dyn ChildProcess>>;
}
pub struct ProcessSpec {
    pub program: PathBuf, pub args: Vec<String>,
    pub env: Vec<(String, String)>,      // the full environment; nothing is inherited
    pub cwd: PathBuf,
    pub stdout: PathBuf, pub stderr: PathBuf,   // files, opened for append
    pub kill_with_parent: bool,
    /// File descriptors that the child inherits, as (number in the child, descriptor).
    /// Empty except for the libkrun lifeline on macOS (feature 09).
    pub extra_fds: Vec<(i32, std::os::fd::OwnedFd)>,
}
pub trait ChildProcess: Send + Sync + 'static {
    fn pid(&self) -> u32;
    fn signal(&self, signal: i32) -> SandboxResult<()>;
    fn wait(&self) -> BoxFuture<'_, SandboxResult<ProcessExit>>;   // can be awaited more than once
    fn try_wait(&self) -> SandboxResult<Option<ProcessExit>>;
}
pub trait HostFs: Send + Sync + 'static {
    fn create_dir_all(&self, path: &Path, mode: u32) -> SandboxResult<()>;
    fn remove_dir_all(&self, path: &Path) -> SandboxResult<()>;
    fn write(&self, path: &Path, data: &[u8]) -> SandboxResult<()>;
    fn read(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>>;
    fn tail(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>>;
    fn exists(&self, path: &Path) -> bool;
}
```

`SlaveHost` (the slave's bundle) is defined in feature 01, in `shared/host.rs`, as a trait
set for processes, mounts and files in the guest. `CloudflareHost` is defined in feature 07.
Each has `from_ctx` that returns `HostNotConfigured` when the entry is missing.

`check_socket_path(path)`: a UDS path has a limit of 104 bytes on macOS and 108 on Linux,
with the end zero byte. The function returns `SocketPathTooLong { path, max: 103 }` when the
path is longer than 103 bytes.

### 6.10 `SandboxControl` and `Sandbox`

Requirements section 6 has the trait. `Sandbox` is a struct that holds an
`Arc<dyn SandboxOps>`:

```rust
pub trait SandboxOps: Send + Sync + 'static {
    fn info(&self) -> SandboxInfo;
    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>>;
    fn write_file(&self, path: String, mode: u32, data: BoxByteStream) -> BoxFuture<'_, SandboxResult<u64>>;
    fn read_file(&self, path: String) -> BoxFuture<'_, SandboxResult<FileStream>>;
    fn stop(&self) -> BoxFuture<'_, SandboxResult<ExitReport>>;
    fn kill(&self) -> BoxFuture<'_, SandboxResult<ExitReport>>;
}
```

`ExecHandle` and `FileStream` get their bodies in feature 02. `Sandbox::run` is a provided
function on top of `exec` (feature 02).

## 7. Tests

`backends/foundation_platform_sandbox/tests/contract_*.rs`. Property tests use the
property-test crate that `foundation_core` already has in `dev-dependencies`, with a fixed
seed.

| Law or scenario | Test | Oracle |
|---|---|---|
| I1, I2, "A message survives a round trip" | `message_roundtrip` (property: generated values of each type) | identity |
| I3 | `no_request_streaming` (the generated service descriptors have only unary and server-streaming kinds) | example |
| I4, "A spec with a wrong root kind is refused", "The first failed rule wins" | `spec_validate` (one case for each rule), `spec_validate_first_rule_wins` | expected outcome |
| I5, "An error keeps its type across RPC" | `error_wire_roundtrip` (property: every variant with generated fields), `error_code_table` (each variant, its code) | identity, example |
| "An unknown remote error is still an error" | `error_without_detail` | expected outcome |
| I6, the three sequence scenarios | `sequence_accept` (property: any series of numbers against a model that is a plain counter), `sequence_span` | stateful model |
| I7, "No side-effect bundle, no fall-back" | `host_not_configured` | expected outcome |
| I8 | `limits_defaults` | example |
| I9 | `ids_are_deterministic` | identity |
| I10 | the verify command below | build |
| "A socket path that is too long is refused" | `socket_path_limit` (103 passes, 104 fails) | bound |

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --profile uat contract
cargo check -p foundation_platform_sandbox --no-default-features --target wasm32-unknown-unknown --profile uat
cargo check -p foundation_platform_sandbox --no-default-features --target x86_64-unknown-linux-musl --profile uat
```

## 9. Done when

- The crate exists, is in `ci/packages.toml`, and builds for native, wasm32 and musl with
  no default feature.
- Each scenario of section 5 and each row of section 7 passes.
- Both service traits exist, and every call is unary or server-streaming.
- No limit is a `const` in a code path.
