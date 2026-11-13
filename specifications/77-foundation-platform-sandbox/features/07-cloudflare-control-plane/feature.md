---
feature: "Cloudflare control plane"
status: "pending"
priority: "high"
model: "sonnet"
phase: "MVP"
depends_on: ["01-slave-core", "02-control-plane-core"]
crates: ["foundation_platform_sandbox"]
---

# Feature 07: The control plane on Cloudflare (library part)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); remote procedure
call (RPC); Durable Object (DO); WebAssembly (wasm); Hypertext Transfer Protocol (HTTP);
Transmission Control Protocol (TCP); behaviour-driven development (BDD); continuous
integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner
of the STE standard ASD-STE100.

## 1. Goal

Make the control-plane core run in a Cloudflare DO and drive that DO's container, with the
slave as the container's main process. This feature is library code only:

1. `CloudflareContainerDriver`: a `SandboxDriver` on a small trait for the container.
2. `ContainerPortTransport`: an RPC transport that sends each call with the container's
   port `fetch`.
3. `DoRecordStore` and the idle alarm logic.
4. `DoControlPlane`: the object that a DO class holds; it builds the `SandboxManager` and
   handles each request.
5. In `src/cloudflare/`: the implementations of the traits on the `worker` crate types.

The logic of items 1 to 4 depends only on traits, so it compiles and is tested on a native
host with fakes. Feature 08 adds the Worker crate and the end-to-end test.

## 2. Story

- **S1**: "A DO gets a `Create` call; it starts its container, waits for the slave in it,
  prepares it, and answers with a ready sandbox."
- **S2**: "A caller runs commands and moves files in that container through the DO."
- **S3**: "The DO is removed from memory while its container still runs; the next call
  finds the sandbox again."
- **S4**: "Nobody uses the sandbox for some time; it stops by itself."
- **S5**: "The container is not listening yet, crashes, or was destroyed by the platform."
- **S6**: "A caller stops the sandbox; the container is destroyed and the record is gone."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Launch | control plane | `driver.launch` | The container starts with the slave as entry point; the endpoint is `CloudflarePort { 1024 }`. |
| U2 | Call the slave | control plane | any `SlaveService` call | One `fetch` to port 1024 for each call. |
| U3 | Restore | DO | first request after a restart of the DO | The manager has the sandbox again, or the record is removed. |
| U4 | Idle stop | alarm | the idle deadline passed | `stop()`. |
| U5 | Handle a request | DO class | an HTTP request | The `SandboxService` answer. |

## 3. Bounded context

**Inside this feature:** the Cloudflare driver, the port transport, the DO record store,
the idle rule, the request handler of one DO.

**Outside:** `SandboxManager` and its laws (feature 02); the slave (feature 01); the Worker
entry, routing to a DO, the token check and the container image (feature 08); the
Cloudflare runtime.

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Container | The one container of one DO. | the Cloudflare runtime, through `ContainerApi` |
| `ContainerApi` | The trait for the container: running, start, destroy, signal, wait, port fetch. | defined in `shared::cloudflare`; implemented in `cloudflare/` and by a fake |
| DO storage | The key-value store of one DO. | `DoStorage` trait; the DO |
| Stored record | The sandbox record under the key `sandbox`. | `DoRecordStore` |
| Idle deadline | The time after which an unused sandbox stops. Key `idle_deadline_ms`. | `DoControlPlane` |
| Alarm | The DO's one timer. | `DoAlarm` trait |
| Index | The list of sandboxes of the Worker. | feature 08 (an index DO); written here through `SandboxIndex` trait |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| launch | story | `create` | Start the container; return the substrate. |
| attach | entity | restore | Give a substrate for a running container, or none. |
| fetch call | entity | each RPC | One port `fetch`; maps "not listening" to a transport error. |
| restore | story | first request of a DO instance | Load the record; attach; rebuild the manager state. |
| touch | entity | each request | Move the idle deadline; set the alarm. |
| on alarm | story | the alarm | Stop when idle; else set the alarm again. |
| handle | entity | a request | Route to `ControlService`. |

**Sessions and DO memory.** A running `Exec` stream is an open request to the DO, and the
runtime keeps a DO in memory while a request is open. `ExecInput` and `ExecSignal` are
separate requests to the same DO and reach the same slave session, because the session
lives in the slave, not in the DO (the service passes calls through; feature 02, section
6.10). If the DO is restarted anyway, the open stream ends; the caller sees `StreamLost`,
and the slave kills that program (feature 01, invariant I8).

**Words.** *One DO, one sandbox*: a DO instance holds a `SandboxManager` with room for one
sandbox, and its ID is the DO's name. *Not listening*: the container runs and no process
accepts on the port yet.

## 4. Rules

### 4.1 Law worksheet

```
Entity: Container (through the driver)
Conserved value:         one container for each DO; after cleanup, running is false
Bound:                   none
Inverse pairs:           start / destroy
Idempotent verbs:        destroy on a container that does not run; cleanup; attach
Ordering pairs:          start -> fetch (enabling); destroy / fetch (competing: after destroy, fetch fails)
Monotone values:         none
Freeze points:           entry point, environment and internet flag at start
Links to other entities: stored record exists <=> the sandbox is Creating, Ready or Stopping

Entity: Stored record
Conserved value:         the record in storage = the manager's record, after each state change
Bound:                   one record for each DO
Inverse pairs:           put / remove
Idempotent verbs:        remove of a missing record
Ordering pairs:          put(Creating) -> put(Ready) -> remove
Monotone values:         state, as in feature 02
Freeze points:           id (= the DO name), spec
Links to other entities: the index has an entry <=> the record exists

Entity: Idle deadline
Conserved value:         none
Bound:                   deadline = last request time + idle_timeout
Inverse pairs:           none
Idempotent verbs:        on alarm, when the deadline is in the future (only sets the alarm again)
Ordering pairs:          touch / on alarm (competing: a touch before the alarm moves the deadline)
Monotone values:         the deadline never moves back
Freeze points:           none
Links to other entities: an in-flight streaming call keeps the deadline in the future

Entity: Restore (story)
Conserved value:         after restore: the manager has the sandbox <=> the container runs and the slave answers Info with prepared true
Bound:                   time <= rpc_timeout
Inverse pairs:           none
Idempotent verbs:        restore (a second call does nothing)
Ordering pairs:          restore -> every request (enabling)
Monotone values:         none
Freeze points:           none
Links to other entities: a record with no running container is removed from storage and from the index
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | One DO holds at most one sandbox, and its ID is the DO name. A second `Create` fails with `TooManySessions`. |
| I2 | The container starts with the entry point `["/usr/local/bin/agent-slave", "--listen", "tcp:0.0.0.0:1024"]`, with only the environment that the driver sets, and with `enable_internet` equal to `network == Host`. |
| I3 | Every slave call is one port `fetch`. No call needs a streamed request body. |
| I4 | A `fetch` that fails during the readiness wait is a transport error, so the wait of feature 02 tries again until the deadline. The last error text is in the `BootTimeout` error; it is not hidden. |
| I5 | A failed `create` destroys the container and leaves no record in storage and no index entry. |
| I6 | After `Stopped`: the container does not run, storage has no record, the index has no entry, and no alarm is set. |
| I7 | After a DO restart, the first request restores the sandbox when the container runs and the slave is prepared; if not, it removes the record and answers `NotFound`. |
| I11 | An alarm on a DO that was removed from memory still stops an idle sandbox: the DO reads its sandbox ID from storage and restores first. |
| I8 | A sandbox with no request for `idle_timeout` stops. A request before that moves the deadline. A running streaming call counts as use. |
| I9 | The logic of this feature (driver, transport, store, idle, handler) compiles and passes its tests on a native host with fakes; only `src/cloudflare/` names a `worker` type. |
| I10 | The driver accepts only `RootFs::ContainerImage(name)` with `name` equal to the configured image name, no mounts, and `RootMode::Overlay`. Each other value is rejected before the container starts. `vcpus` and `memory_mib` are ignored (the Worker configuration sets the instance type). |

### 4.3 Must have

- `ContainerApi`, `DoStorage`, `DoAlarm`, `SandboxIndex` traits and `CloudflareHost`.
- `CloudflareContainerDriver`, `ContainerPortTransport`, `DoRecordStore`, `DoControlPlane`.
- The `worker` implementations in `src/cloudflare/`.
- The `attach` implementation of this driver, and the use of `restore` and `IdSource`
  (section 6.7).

### 4.4 Must do

- Report the container's exit through `SubstrateHandle::exited` (`wait_for_exit`).
- Keep the stored record equal to the manager's record on each state change.

### 4.5 Must not

- Must not use a Cloudflare call that is not in the `worker` crate version of this
  workspace (section 6.1).
- Must not keep sandbox state only in DO memory.
- Must not start a container in `restore`.

## 5. Acceptance scenarios

All run on a native host with `FakeContainer`, `MemoryDoStorage`, `FakeAlarm`, a
`SimulatedClock` and a `ScriptedSlave` behind the fake container's port.

```gherkin
Feature: Cloudflare control plane

  Scenario: Create starts the container with the slave
    When the DO control plane handles Create with RootFs::ContainerImage("sandbox")
    Then the fake container got start with the slave entry point and enable_internet false
    And the progress values are Preparing, SubstrateStarted, SlaveConnected, SlaveReady
    And storage has the record with state Ready
    And the index has the sandbox

  Scenario: The slave is reached with port fetches
    Given a ready sandbox
    When the caller runs ["echo", "hi"]
    Then every slave call was a fetch to port 1024
    And the result is stdout "hi\n", code 0

  Scenario: Not listening means try again
    Given a container whose port answers "the container is not listening" 3 times and then works
    When the DO control plane handles Create
    Then Create succeeds
    And Info was fetched 4 times, with waits of 5, 10 and 20 ms

  Scenario: Another fetch error is not hidden
    Given a container whose port fetch fails with "internal error"
    When the clock passes boot_timeout
    Then the result is BootTimeout { stage: "slave" }
    And the container got destroy
    And storage has no record and the index has no entry

  Scenario: A wrong image name is refused before start
    When the DO control plane handles Create with RootFs::ContainerImage("other")
    Then the result is UnsupportedRootFs
    And the container did not start

  Scenario: Host network turns the internet flag on
    When the DO control plane handles Create with NetworkMode::Host
    Then the fake container got start with enable_internet true

  Scenario: One DO holds one sandbox
    Given a ready sandbox
    When the DO control plane handles Create again
    Then the result is TooManySessions

  Scenario: A restarted DO finds its sandbox
    Given a ready sandbox
    When the DO control plane object is dropped and a new one is built on the same storage and container
    And the caller runs ["echo", "again"]
    Then the result is stdout "again\n"
    And the container did not start a second time
    And the slave did not get a second Prepare

  Scenario: A restarted DO with a dead container forgets the sandbox
    Given a stored record with state Ready and a container that does not run
    When a new DO control plane handles Get
    Then the result is NotFound
    And storage has no record and the index has no entry

  Scenario: An idle sandbox stops
    Given a ready sandbox and idle_timeout 10 minutes
    When the clock moves 10 minutes and the alarm fires
    Then the sandbox is stopped and the container got destroy
    And no alarm is set

  Scenario: An alarm on a fresh DO object still stops the sandbox
    Given a ready sandbox and idle_timeout 10 minutes
    When the DO control plane object is dropped
    And the clock moves 10 minutes and a new object handles the alarm with no request before it
    Then the sandbox is stopped and the container got destroy

  Scenario: A request moves the idle deadline
    Given a ready sandbox and idle_timeout 10 minutes
    When a request arrives after 9 minutes and the alarm fires at 10 minutes
    Then the sandbox is still Ready
    And the alarm is set for minute 19

  Scenario: A streaming call keeps the sandbox alive
    Given a command that runs for 15 minutes with an open event stream
    When the alarm fires at 10 minutes
    Then the sandbox is still Ready

  Scenario: Stop removes everything
    When the caller stops a ready sandbox
    Then the slave got Shutdown
    And the container got destroy
    And storage has no record, the index has no entry, and no alarm is set

  Scenario: A container that exits makes the sandbox Failed
    Given a ready sandbox
    When the fake container's wait_for_exit completes
    Then the state becomes Failed and then Stopped
    And storage has no record
```

## 6. Technical breakdown

### 6.1 Facts

| Fact | Source |
|---|---|
| The workspace uses `worker` 0.8.5 (Rust Workers). | `Cargo.lock` |
| `State::container() -> Option<Container>`. | `worker-0.8.5/src/durable.rs`, line 258 |
| `Container::running() -> bool`, `start(Option<ContainerStartupOptions>) -> Result<()>`, `wait_for_exit()`, `destroy(Option<&str>)`, `signal(i32)`, `get_tcp_port(u16) -> Result<Fetcher>`. | `worker-0.8.5/src/container.rs`, lines 13 to 41 |
| `ContainerStartupOptions` has `entrypoint`, `enable_internet`, `env`. It has no image and no instance type. | the same file, lines 86 to 112 |
| `Fetcher::fetch` and `fetch_request` send an HTTP request and give a response with a streamed body. | `worker-0.8.5/src/fetcher.rs`, lines 23 and 48 |
| A port `fetch` before a process listens fails with an error whose text contains `the container is not listening`. | `cloudflare/containers`, `src/lib/container.ts`, line 27 |
| The image of a container class is set in the Worker configuration (`containers[].image`), not at start. | `cloudflare/containers`, `examples/core-tests/wrangler.jsonc` |
| The repository serves HTTP in a Worker with `foundation_http::wasm` (`CfHttpAppSingleton`, `serve_cf::CfServe`, `dispatch`), and has a Rust DO (`foundation_keychain/src/server/signalr_do.rs`). | those files; `examples/cf-agent-loop/src/lib.rs` |
| The Connect client transport for `fetch` is a capability constant only today (`WASM_FETCH_CAPABILITIES`: no request streaming, no full duplex). | `foundation_connectrpc/src/shared/transport/wasm.rs` |

### 6.2 Files

| File | Target | Content |
|---|---|---|
| `src/shared/cloudflare/mod.rs` | all | The traits, `CloudflareHost`, `CloudflareConfig`. |
| `src/shared/cloudflare/driver.rs` | all | `CloudflareContainerDriver`, its handle. |
| `src/shared/cloudflare/transport.rs` | all | `ContainerPortTransport`, `PortConnector`. |
| `src/shared/cloudflare/store.rs` | all | `DoRecordStore`. |
| `src/shared/cloudflare/control.rs` | all | `DoControlPlane`: restore, touch, alarm, request handling. |
| `src/shared/cloudflare/testing.rs` | feature `testing` | `FakeContainer`, `MemoryDoStorage`, `FakeAlarm`, `MemoryIndex`. |
| `src/cloudflare/mod.rs` | wasm32, feature `cloudflare` | `WorkerContainer`, `WorkerStorage`, `WorkerAlarm`, `CloudflareHost::from_state(&state, index)`. |

The `shared/cloudflare/` code has no target gate (`.agents` rule: platform-agnostic code is
in `shared/`). It uses only the traits below.

### 6.3 The traits

```rust
/// The container of one DO. Mirrors `worker::Container`.
pub trait ContainerApi: Send + Sync + 'static {
    fn running(&self) -> bool;
    fn start(&self, options: ContainerStart) -> SandboxResult<()>;
    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>>;
    fn signal(&self, signo: i32) -> SandboxResult<()>;
    fn wait_for_exit(&self) -> BoxFuture<'_, SandboxResult<()>>;
    fn fetch(&self, port: u16, request: PortRequest) -> BoxFuture<'_, Result<PortResponse, PortError>>;
}
pub struct ContainerStart { pub entrypoint: Vec<String>, pub enable_internet: bool, pub env: Vec<(String, String)> }
pub struct PortRequest { pub method: String, pub path: String, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
pub struct PortResponse { pub status: u16, pub headers: Vec<(String, String)>, pub body: BoxByteStream }
pub enum PortError { NotListening, Other(String) }   // Other text is cut to max_error_message

pub trait DoStorage: Send + Sync + 'static {
    fn get(&self, key: &str) -> BoxFuture<'_, SandboxResult<Option<Vec<u8>>>>;
    fn put(&self, key: &str, value: Vec<u8>) -> BoxFuture<'_, SandboxResult<()>>;
    fn delete(&self, key: &str) -> BoxFuture<'_, SandboxResult<()>>;
}
pub trait DoAlarm: Send + Sync + 'static {
    fn set(&self, at_unix_ms: u64) -> BoxFuture<'_, SandboxResult<()>>;
    fn clear(&self) -> BoxFuture<'_, SandboxResult<()>>;
}
/// The Worker-wide list of sandboxes. Feature 08 implements it on an index DO.
pub trait SandboxIndex: Send + Sync + 'static {
    fn put(&self, info: SandboxInfo) -> BoxFuture<'_, SandboxResult<()>>;
    fn remove(&self, id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>>;
}

#[derive(Clone)]
pub struct CloudflareHost {
    pub container: Arc<dyn ContainerApi>,
    pub storage: Arc<dyn DoStorage>,
    pub alarm: Arc<dyn DoAlarm>,
    pub index: Arc<dyn SandboxIndex>,
    pub config: Arc<CloudflareConfig>,
}
pub struct CloudflareConfig {
    pub image_name: String,              // default "sandbox": the name a spec must use
    pub slave_port: u16,                 // default 1024
    pub slave_path: String,              // default "/usr/local/bin/agent-slave"
    pub idle_timeout: Duration,          // default 10 minutes: the default sleepAfter of @cloudflare/containers
    pub boot_timeout: Duration,          // default 60 s; used in place of SandboxLimits.boot_timeout.
                                         // A cold container start with a first image pull takes much longer than a microVM boot.
}
```

`WorkerContainer::fetch` maps an error whose text contains `the container is not listening`
to `PortError::NotListening`, and every other error to `PortError::Other`. That text match
is in one function with a test. The text is from the Cloudflare library source (section
6.1), not from a contract. To not depend on it for correctness, the readiness wait treats
**both** `PortError` kinds as "try again" until the deadline; the kind only decides the
error at the deadline: `BootTimeout { stage: "slave" }` with the last `PortError` text.
So a changed text costs nothing but a less exact message.

### 6.4 `CloudflareContainerDriver`

Capabilities: `rootfs_container_image: true`, the other `rootfs_*` false,
`root_modes: [Overlay]`, `host_mounts: false`, `network_host: true`, `max_vcpus: 1`.

`probe`: one check, `container_binding`: ok when the DO has a container
(`CloudflareHost` exists). No call to the platform.

`launch(ctx, id, spec)` is a small `#[task]`:

| State | Work | Emits | Next |
|---|---|---|---|
| `Check` | `RootFs::ContainerImage(name)` with `name == config.image_name` (`UnsupportedRootFs`). `container.running()` must be false (`TooManySessions`). `vcpus` and `memory_mib` are not checked and not used: the instance type in the Worker configuration decides them, and the `Capabilities` documentation of this driver says so. | `Pending(Preparing)` | `Start` |
| `Start` | `container.start(ContainerStart { entrypoint: [slave_path, "--listen", "tcp:0.0.0.0:<slave_port>"], enable_internet: spec.network == Host, env: [] })`. | `Pending(SubstrateStarted)`, then `Ready(Ok(substrate))` | `Done` |

The substrate: `endpoint: CloudflarePort { port }`,
`root: RootSetup { source: Plain, mode: Overlay }`, no mounts, and a handle:

| Handle method | Work |
|---|---|
| `exited()` | `container.wait_for_exit()`; `SubstrateExit { code: None, signal: None, forced: false }` (the platform gives no code). |
| `terminate(grace)` | Await the first of the exit and a timer of `grace`; then `destroy()`. `forced` is true when the timer won. |
| `kill()` | `destroy()`; `forced: true`. |
| `log_tail(max)` | Empty. The platform gives no log through this API; the error message says "see the Worker logs". |
| `cleanup()` | `destroy()` when `running()`. |

A failed `start` returns `SubstrateApi { status: 0, message }` with the platform's text.

`attach(ctx, record)` (section 6.7): `Ok(Some(substrate))` when `container.running()`, with
the same endpoint and handle; `Ok(None)` when it does not run.

### 6.5 `ContainerPortTransport`

Implements `foundation_connectrpc::shared::transport::Transport`
(`shared/transport/base.rs`):

- `capabilities()` returns `WASM_FETCH_CAPABILITIES`. So the client refuses a call that
  needs a streamed request, before it sends (`check_compatible`).
- `open(request)`: collect the request body from `send_body` (the calls of both services
  send one message, at most `max_message_bytes`); call
  `container.fetch(port, PortRequest { method: "POST", path, headers, body })`; publish the
  response head from `status` and `headers`; pump `body` chunks to `recv_body`; no
  trailers. The pump is a task started with `execute(from_future(..))`.
- `PortError::NotListening` and `PortError::Other` both become a `TransportError` before
  the head; the first has the text `not listening`.
- A unary call has the time-out `rpc_timeout` from the `Ctx` clock (a race with a timer).

`PortConnector` is the `SlaveConnector` for a DO: `CloudflarePort { port }` gives a
`ContainerPortTransport`; every other endpoint gives `InvalidSpec { field: "endpoint" }`.

If feature 05 of spec 65 lands a general `WasmFetchTransport` in `foundation_connectrpc`
first, this type uses its body pump. It does not wait for it: the pump is about 80 lines.

### 6.6 `DoRecordStore` and `DoControlPlane`

`DoRecordStore` implements `RecordStore` (feature 02, section 6.8): `put` writes the record
as JSON under the key `sandbox` and calls `index.put(info)`; `remove` deletes the key,
calls `index.remove(id)` and `alarm.clear()`; `list` returns the one record or none. An
index error is logged and does not fail the call (the index is a view; section 6.8).

```rust
pub struct DoControlPlane { /* ctx, host, manager, service, restored flag */ }

impl DoControlPlane {
    /// Builds a SandboxManager with one driver, PortConnector, DoRecordStore,
    /// max_sandboxes 1, IdSource::Fixed(id) and DropPolicy::Detach.
    /// `id`: `Some` on a request (the DO name, from the request); `None` on an alarm.
    /// The ID is stored under the key `sandbox_id` the first time it is known, and read
    /// from there when `id` is `None`. With no stored ID and `None`, there is nothing to
    /// do, and `on_alarm` clears the alarm.
    pub async fn open(ctx: Ctx, host: CloudflareHost, id: Option<SandboxId>, limits: SandboxLimits)
        -> SandboxResult<Option<Self>>;

    /// Handles one HTTP request to this DO: restore (first time), touch, then the
    /// SandboxService router.
    pub async fn handle(&self, request: HttpRequestParts) -> HttpResponseParts;

    /// The DO alarm.
    pub async fn on_alarm(&self);
}
```

`handle`:

1. `restore()` when it did not run yet in this object (section 6.7).
2. `touch()`: `deadline = now + idle_timeout`; `storage.put("idle_deadline_ms", ..)`;
   `alarm.set(deadline)`. A streaming response holds a guard; while a guard exists, a timer
   task calls `touch()` every `idle_timeout / 2`.
3. Route the request to `ControlService` (feature 06) through a
   `foundation_connectrpc` `Router`. `HttpRequestParts` and `HttpResponseParts` are the
   request and response types that `foundation_http::wasm` uses for a Worker
   (`examples/cf-agent-loop/src/lib.rs` shows the wiring); on a native host the tests call
   the router through the pipe connection types.

The manager's `boot_timeout` is `CloudflareConfig.boot_timeout`, not the value of
`SandboxLimits`.

`on_alarm`: `restore()` first (an alarm can wake a DO that has no object in memory; the
manager must know the sandbox before it can stop it). Then read `idle_deadline_ms`. `now >= deadline` and a sandbox exists: `stop()`.
`now < deadline`: `alarm.set(deadline)`. No sandbox: `alarm.clear()`.

### 6.7 What this feature uses from features 00 and 02

- `SandboxDriver::attach` (feature 00, section 6.8): this driver returns
  `Ok(Some(substrate))` when `container.running()`, else `Ok(None)`.
- `IdSource::Fixed(id)` and `max_sandboxes: 1` (feature 02, section 6.2): the DO's manager
  holds one sandbox, named as the DO.
- `SandboxManager::restore` (feature 02, section 6.2): `DoControlPlane::handle` calls it
  one time for each DO object, before the first request.

### 6.8 The index is a view

The index (feature 08) holds a copy of each sandbox's `SandboxInfo` for `List`. The truth
is each DO's own record. An index entry with no live DO is possible when an index call
failed; `List` in feature 08 removes an entry whose DO answers `NotFound`.

### 6.9 Fault table: `Create` in a DO

| Step that fails | What the control plane does | What the caller sees | What is left behind |
|---|---|---|---|
| Driver check | Nothing started. | `UnsupportedRootFs`, `InvalidSpec`, `TooManySessions` | nothing |
| `storage.put(Creating)` | Removes the manager record. | `Io { op: "do storage" }` | nothing |
| `container.start` | Removes the record. | `SubstrateApi` | nothing |
| The slave does not answer until `boot_timeout` | `destroy`; removes the record. | `BootTimeout { stage: "slave" }` | nothing |
| The container exits during the wait | Removes the record. | `SubstrateExited` | nothing |
| `Prepare` fails | `destroy`; removes the record. | the `SlaveError` | nothing |
| The DO is removed from memory during `Create` | The next request runs `restore`: a `Creating` record is cleaned up (`destroy`) and removed. | the first caller's stream ends with a transport error; a later `Get` gives `NotFound` | nothing |

## 7. Tests

`backends/foundation_platform_sandbox/tests/cloudflare_*.rs`, native, features `testing`.
Each scenario of section 5 is one test.

| Law or scenario | Test | Oracle |
|---|---|---|
| I2, "Create starts the container with the slave", "Host network ..." | `create_starts_container` | example |
| I3, "The slave is reached with port fetches" | `calls_are_port_fetches`; the shared acceptance scenarios with a real slave process behind the fake container's port (`FakeContainer` forwards `fetch` to a UDS HTTP client; Linux only) | example, identity |
| I4, the two readiness scenarios | `not_listening_retries`, `other_error_times_out`, `not_listening_text_match` | expected outcome |
| I5, the fault table of section 6.9 | `create_fault_matrix` | fault |
| I1, "One DO holds one sandbox" | `one_sandbox_per_do` | expected outcome |
| I10, "A wrong image name ..." | `driver_rejects` (image name, mounts, `ReadOnly`), `driver_ignores_cpu_and_memory` | expected outcome |
| I7, the two restart scenarios | `restore_live`, `restore_dead_container`, `restore_unprepared_slave`, `restore_creating_record` | expected outcome |
| I11, "An alarm on a fresh DO object ..." | `alarm_on_fresh_object_stops` | expected outcome |
| transport time-out | `port_call_times_out` (a port that never answers a unary call: the call fails after `rpc_timeout` on the simulated clock) | expected outcome |
| touch guard | `stream_guard_touches` (a stream open for 3 half-periods makes 3 touches) | example |
| index failure | `index_error_does_not_fail_create` (a failing `SandboxIndex`; the create succeeds; a warning is logged) | expected outcome |
| `attach` | `attach_running`, `attach_not_running` | expected outcome |
| I8, the three idle scenarios | `idle_stops`, `touch_moves_deadline`, `stream_keeps_alive`; `idle_model` (stateful: random requests, alarms and clock steps against a model with one deadline) | stateful model |
| I6, "Stop removes everything" | `stop_removes_all` | conservation |
| "A container that exits ..." | `container_exit_fails_sandbox` | fault |
| transport | `transport_refuses_request_streaming` (a client-streaming call fails before a fetch), `transport_streams_response` (a 10 MiB server stream arrives in chunks; the fake counts that it never holds more than 2 chunks) | expected outcome, bound |
| I9 | the native test run itself, and the wasm check in section 8 | build |

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features testing --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "testing slave" --profile uat cloudflare_ control_sim
cargo check -p foundation_platform_sandbox --no-default-features --features "control cloudflare" --target wasm32-unknown-unknown --profile uat
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes on a native host.
- The `cloudflare` feature compiles for wasm32.
- Only `src/cloudflare/` names a `worker` type.
- A restarted DO control plane finds its sandbox again (invariant I7).
