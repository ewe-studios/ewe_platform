---
feature: "Cloudflare Worker and deployment"
status: "pending"
priority: "high"
model: "sonnet"
phase: "MVP-B"
depends_on: ["06-programs-rpc-provider", "07-cloudflare-control-plane"]
crates: ["sandbox_worker", "foundation_platform_sandbox", "foundation_deployment_platform", "foundation_deployment_cloudflare"]
---

# Feature 08: The Cloudflare Worker, deployment and the end-to-end path (MVP-B)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); remote procedure
call (RPC); Durable Object (DO); WebAssembly (wasm); Hypertext Transfer Protocol (HTTP);
behaviour-driven development (BDD); continuous integration (CI); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Give the Cloudflare control plane its boundary: a Rust Worker crate with two DO classes and
a container image, and prove the whole path with the same acceptance scenarios as
Firecracker.

When this feature lands, a native program does this against the Worker (in `wrangler dev`,
and on Cloudflare after a deploy):

```rust
let control = SandboxRpcClient::connect(ctx.clone(), ConnectAddr::Url(worker_url), limits)?
    .with_token(token);
let sandbox = control.create(SandboxSpec::new(RootFs::ContainerImage("sandbox".into()))).await?;
let out = sandbox.run(Command::new(["echo", "hello"])).await?;
assert_eq!(out.stdout, b"hello\n");
sandbox.stop().await?;
```

This is MVP-B: the control plane runs in a DO, the slave runs in the DO's container, and
the caller uses the same `SandboxControl` API as on a host.

This feature owns the two supported Cloudflare deployment paths. The Rust deployer in
`foundation_deployment_cloudflare` is the primary path. The manual `wrangler` adapter is a
supported fallback. Both apply the same deployment plan. Spec 83 feature 06 uses this
deployment surface; it does not own one.

## 2. Story

- **S1**: "A remote caller creates a sandbox through the Worker; the Worker picks a new DO
  for it."
- **S2**: "The caller uses the sandbox; each call reaches the DO of that sandbox."
- **S3**: "The caller lists the sandboxes of the Worker."
- **S4**: "A caller with no token, or a wrong token, calls the Worker."
- **S5**: "A caller names a sandbox that does not exist."
- **S6**: "An operator deploys the Worker and its container image, by the Rust deployer or
  by `wrangler`."
- **S7**: "A caller with a `Provider` starts a Cloudflare sandbox."
- **S8**: "The two deploy paths apply the same plan and give the same Worker."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Route `Create` | Worker | `SandboxService/Create` | A new ID; the call goes to the DO with that name. |
| U2 | Route a scoped call | Worker | any call with `sandbox_id` | The call goes to that DO. |
| U3 | List | Worker | `SandboxService/List` | The entries of the index DO. |
| U4 | Probe | Worker | `SandboxService/Probe` | One report: the Cloudflare driver. |
| U5 | Authorise | Worker | every call | 401-class error with no token. |
| U6 | Deploy | operator | `CloudflareDeployer::deploy`, or `npx wrangler deploy` | The Worker, the DO classes, the image and the routes exist. |
| U7 | Provider | orchestration code | `SandboxProvider::cloudflare(control)` | A `SandboxHandle`. |
| U8 | Check a deploy | operator | `CloudflareDeployer::status` | The deployment state and the Worker URL. |

## 3. Bounded context

**Inside this feature:** the Worker entry, routing a call to a DO, the token check, the two
DO classes as wrappers, the index, the container image, the deployment plan and its two
adapters, the end-to-end tests, the Cloudflare `Provider` constructor.

**Outside:** everything a DO does with its sandbox (feature 07); the client
(`SandboxRpcClient`, feature 06); the slave (feature 01).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Worker entry | The `fetch` handler. | `infrastructure/sandbox_worker/src/lib.rs` |
| `SandboxDo` | The DO class of one sandbox. Holds a `DoControlPlane`. | `src/sandbox_do.rs` (wrapper only) |
| `SandboxIndexDo` | One DO (name `index`) with the list of sandboxes. | `src/index_do.rs` |
| Index entry | Sandbox ID to `SandboxInfo`. | `SandboxIndexDo` storage |
| Worker token | The secret `SANDBOX_TOKEN`. | the operator |
| Container image | The image with `agent-slave`. | `infrastructure/sandbox_worker/Dockerfile` |
| Deployment plan | The one description of the Worker, its bindings, DO migrations, container classes, routes and secret names. | `foundation_deployment_cloudflare::DeploymentPlan` |
| Deploy path | `FoundationCloudflareDeployer` (primary) or `WranglerCloudflareDeployer` (fallback). | the operator picks one; both read the plan |
| Routing rule | Which DO gets a call. | `foundation_platform_sandbox::shared::cloudflare::routing` (library) |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| authorise | entity | each request | Constant-time token compare. |
| route | entity | each request | Finds the target: new DO, named DO, index DO, or a local answer. |
| forward | entity | after route | Sends the request to the DO stub and returns its response as it is. |
| index put / remove / list | entity | a DO, the Worker | Keeps the list. |
| deploy | story | an operator | Applies one `DeploymentPlan` through the selected adapter. |
| status | entity | an operator | Reads the deployment state and the Worker URL. |
| rollback | story | a failed deploy or an operator | Restores the previous Worker and container tags. |

The routing rule is library code (`shared::cloudflare::routing::route(path, body) ->
RouteTarget`), with native tests. The Worker crate only calls it and talks to the `worker`
types.

## 4. Rules

### 4.1 Law worksheet

```
Entity: Routing
Conserved value:         a response from a DO reaches the caller with no change
Bound:                   the request body read for routing <= max_message_bytes
Inverse pairs:           none
Idempotent verbs:        route (same request, same target)
Ordering pairs:          authorise -> route -> forward
Monotone values:         none
Freeze points:           a sandbox ID maps to the same DO for ever (the DO name is the ID)
Links to other entities: a scoped call's sandbox_id = the DO name

Entity: Index
Conserved value:         entries = sandboxes whose DO has a record (after the calls settle)
Bound:                   none
Inverse pairs:           put / remove
Idempotent verbs:        put of the same info; remove of a missing id
Ordering pairs:          none (last write wins for one id)
Monotone values:         none
Freeze points:           none
Links to other entities: each entry's id is a DO name; List checks a stale entry against its DO

Entity: Worker deployment
Conserved value:         the plan is the same for both paths; the slave in the image and the control plane in the Worker are built from the same crate version
Bound:                   one deploy at a time for one Worker name (the deploy lock)
Inverse pairs:           deploy / rollback
Idempotent verbs:        deploy of the same plan; rollback of a rolled-back deployment
Ordering pairs:          build the slave -> build the image -> deploy
Monotone values:         none
Freeze points:           none
Links to other entities: the deployed Worker's bindings and DO migrations equal the plan
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | Every call needs a valid bearer token. The check is constant-time and runs before any DO is reached. |
| I2 | `Create` goes to a DO whose name is a new `SandboxId`; every scoped call goes to the DO whose name is its `sandbox_id`; `List` goes to the index; `Probe` is answered by the Worker. |
| I3 | A `sandbox_id` that is not a valid `SandboxId` is refused with `InvalidSpec` before any DO is reached. |
| I4 | The Worker does not change a DO's response: status, headers and streamed body pass through. |
| I5 | A scoped call for a sandbox that does not exist gives `NotFound`, and creates no stored state. |
| I6 | `List` returns only sandboxes whose DO still has a record: a stale index entry is removed during the call. |
| I7 | The Worker crate has no sandbox logic: its source has no state check, no retry and no call to `ContainerApi` outside the `worker` trait implementations. |
| I8 | The shared acceptance scenarios pass against the Worker through `SandboxRpcClient`, the same as against Firecracker. |
| I9 | The image has a static `agent-slave` for `linux/amd64` at the configured path, built from the same commit as the Worker. |
| I10 | Both deploy paths consume one `DeploymentPlan`; the deployed Worker has the same name, bindings, DO migrations, container class, routes and secret names. |
| I11 | `FoundationCloudflareDeployer` is the primary path. `WranglerCloudflareDeployer` is a supported fallback. The deploy task selects the foundation path when no adapter is named. |
| I12 | The plan holds secret names and sources, never secret values. A printed plan has no credential. |
| I13 | A failed deploy leaves the previous Worker live. `rollback` is idempotent. |

### 4.3 Must have

- `infrastructure/sandbox_worker` with `SandboxDo`, `SandboxIndexDo`, the entry, the
  `wrangler.toml`, the `Dockerfile`, the README.
- `shared::cloudflare::routing`.
- `ProviderId::CloudflareContainer` and `SandboxProvider::cloudflare`.
- `CloudflareDeployer`, `DeploymentPlan`, `DeploymentOutcome`, `DeploymentStatus`,
  `DeployError`, `FoundationCloudflareDeployer`, `WranglerCloudflareDeployer`.
- The end-to-end test and its script.

### 4.4 Must do

- Read the request body one time, with a size limit, for routing; then forward the same
  bytes.
- Document how to deploy and how to set the token.
- Deploy by the foundation path by default; document the `wrangler` path as the fallback.
- Read every secret from the `SecretSource`; never write a value to the plan or a log.

### 4.5 Must not

- Must not log the token or a request body.
- Must not keep state in the Worker isolate between requests, other than the valtron pool
  and the `Ctx`.
- Must not have a second deployment format for the agent-rpc Worker (spec 83 feature 06).
- Must not bake a secret into an image or a plan.

## 5. Acceptance scenarios

```gherkin
Feature: Cloudflare Worker

  Background:
    Given the Worker runs in wrangler dev with SANDBOX_TOKEN "test"
    And a native SandboxRpcClient with that token

  Scenario: A command runs in a Cloudflare container
    When the caller creates a sandbox with RootFs::ContainerImage("sandbox") and runs ["echo", "hello"]
    Then stdout is "hello\n" and the exit code is 0
    And "uname -s" in the sandbox prints "Linux"

  Scenario: The shared acceptance scenarios pass
    When the shared acceptance scenarios run with this client
    Then each has the same result as on Firecracker

  Scenario: Every call needs the token
    Given a client with no token
    When it calls List
    Then the call fails with Unauthorized
    Given a client with the token "wrong"
    When it calls Create
    Then the call fails with Unauthorized

  Scenario: An unknown sandbox is NotFound
    When the caller calls Get with a valid id that was never created
    Then the call fails with NotFound
    And List does not show that id

  Scenario: A bad sandbox id is refused
    When the caller calls Get with the id "../x"
    Then the call fails with InvalidSpec

  Scenario: List shows live sandboxes only
    Given two created sandboxes
    When the caller stops the first and calls List
    Then List shows only the second

  Scenario: Two sandboxes are separate
    Given two created sandboxes
    When the first writes /tmp/a
    Then the second does not see /tmp/a

  Scenario: No internet unless asked
    Given a sandbox with NetworkMode::None
    When it runs ["sh", "-c", "wget -q -T 3 -O- http://example.com"]
    Then the exit code is not 0

  Scenario: Stop destroys the container
    When the caller stops a sandbox
    Then Get for it fails with NotFound
    And "docker ps" shows no container for it

  Scenario: The slave and the Worker have the same version
    When the caller creates a sandbox
    Then SandboxInfo.slave.slave_version equals the Worker's reported version

  Scenario: A provider starts a Cloudflare sandbox
    Given SandboxProvider::cloudflare on the remote client
    When launch is called with RootFs::ContainerImage("sandbox")
    Then is_running is true
    When stop is called
    Then is_running is false

  Scenario: Both deployers apply one plan
    Given one DeploymentPlan for the sandbox Worker
    When the foundation deployer and the wrangler deployer each plan the deploy
    Then both name the same Worker, bindings, DO migrations, container class and routes

  Scenario: The primary deployer is the Rust path
    When an operator runs the deploy task with no adapter selected
    Then FoundationCloudflareDeployer runs

  Scenario: A secret never enters the plan
    Given a plan with a SecretRef
    When the plan is printed
    Then the secret value is absent
```

## 6. Technical breakdown

### 6.1 The routing rule (library)

`src/shared/cloudflare/routing.rs` in `foundation_platform_sandbox`:

```rust
pub enum RouteTarget {
    Probe,                              // the Worker answers
    List,                               // the index DO
    NewSandbox,                         // Create: a new id, then that DO
    Sandbox(SandboxId),                 // the DO with this name
}
/// `path` is the request path. `body` is the JSON request message.
pub fn route(path: &str, body: &[u8]) -> SandboxResult<RouteTarget>;
```

| Path (`/ewe.sandbox.control.v1.SandboxService/<Method>`) | Target |
|---|---|
| `Probe` | `Probe` |
| `List` | `List` |
| `Create` | `NewSandbox` |
| every other method | Read `sandbox_id` from the JSON body (a small struct with only that field). Parse it as `SandboxId` (`InvalidSpec { field: "sandbox_id" }` on a failure). `Sandbox(id)`. |
| a path that is not a method of the service | `NotFound { what: "method" }` |

The token check is `check_bearer` of feature 02, section 6.10. It is in `shared/`, so the
native server and the Worker use one function.

### 6.2 The Worker crate

`infrastructure/sandbox_worker/` is its own workspace root, like `examples/cf-agent-loop`
(it is in the root `Cargo.toml` `exclude` list by the `infrastructure/*` pattern or an
explicit line; add the line if the pattern does not cover it).

| File | Content |
|---|---|
| `Cargo.toml` | `crate-type = ["cdylib", "rlib"]`; `worker = "0.8"`; `foundation_platform_sandbox` with `default-features = false, features = ["control", "cloudflare"]`; `foundation_http` and `foundation_core` with the wasm features that `examples/cf-agent-loop/Cargo.toml` uses. |
| `src/lib.rs` | The `fetch` entry. |
| `src/sandbox_do.rs` | `#[durable_object] pub struct SandboxDo`. |
| `src/index_do.rs` | `#[durable_object] pub struct SandboxIndexDo`. |
| `wrangler.toml` | Section 6.5. |
| `Dockerfile` | Section 6.6. |
| `README.md` | Deploy and token steps. |
| `test/run.sh` | Starts `wrangler dev`, saves its PID, waits for readiness, runs the tests, stops the process by that PID. `--keep-running` leaves it up and prints the PID. |

**Entry (`src/lib.rs`).** One time for each isolate: start the valtron pool and build the
`Ctx` (`Ctx::host()`; the entropy seed works on wasm since spec 76). For each request:

1. `check_bearer(header("authorization"), env.secret("SANDBOX_TOKEN"))`. A failure answers
   with the Connect error for `Unauthorized`.
2. Read the body, at most `max_message_bytes` (a larger body answers `InvalidSpec`).
3. `route(path, body)`.
4. `Probe`: answer with one `HostReport` (driver `cloudflare-container`, usable, the
   capabilities of feature 07, section 6.4). `List`: forward to the index DO
   (`env.durable_object("SANDBOX_INDEX")`, name `index`). `NewSandbox`:
   `SandboxId::generate(&ctx)`, then forward to `env.durable_object("SANDBOX")` with
   `id_from_name(id)`. `Sandbox(id)`: forward to that DO.
5. Forward: build a new request with the same method, path, headers and the body bytes,
   call `stub.fetch_with_request`, and return the response as it is (the streamed body
   passes through).

Errors from steps 1 to 3 are written with the error writer of `foundation_connectrpc`
(`shared/error_writer.rs`) so the client sees a normal Connect error.

**`SandboxDo`.** Holds `state`, `env` and a `OnceCell<DoControlPlane>`. On the first
request it builds `CloudflareHost::from_state(&state, index)` (feature 07,
`src/cloudflare/`), where `index` is a `SandboxIndex` that calls the index DO, and
`DoControlPlane::open(ctx, host, Some(id), limits)` with `id` from the request header
below. `fetch` converts
the `worker::Request` to the request type of `foundation_http::wasm`, calls
`control.handle(..)`, and converts the response back. `alarm` calls `DoControlPlane::open(ctx, host, None, limits)` when the cell is empty (the
ID comes from storage; feature 07, section 6.6), then `on_alarm()`.
Nothing else.

The DO name is not in `worker::State` directly; the Worker entry sends it in the header
`x-ewe-sandbox-id` on each forwarded request, and the DO checks that it parses as a
`SandboxId`.

**`SandboxIndexDo`.** Storage keys `sbx:<id>` with the JSON `SandboxInfo`. It serves three
internal paths (not part of `SandboxService`): `PUT /index/<id>` (body: `SandboxInfo`),
`DELETE /index/<id>`, and the `List` RPC path. For `List` it reads every `sbx:` key and
returns `ListResponse`.

Stale entries: for each entry, `List` in the Worker entry does not call every DO (that
costs one sub-request for each sandbox). The rule is: an entry is removed when its DO calls
`index.remove` (at `Stopped`, feature 07), and when a scoped call to that DO answers
`NotFound` (the Worker entry then calls `DELETE /index/<id>` before it returns the answer).
So an entry of a sandbox that died while nobody called it stays until the first call to
it. `List` therefore means "sandboxes that were not seen to end". The README says so.

### 6.3 Fault table: a request in the Worker

| Step that fails | What the Worker does | What the caller sees | What is left behind |
|---|---|---|---|
| Token check | Stops. | `Unauthorized` | nothing |
| Body too large or not JSON | Stops. | `InvalidSpec` | nothing |
| Unknown method | Stops. | `NotFound { what: "method" }` | nothing |
| Bad `sandbox_id` | Stops. | `InvalidSpec { field: "sandbox_id" }` | nothing |
| The DO stub `fetch` throws | Stops. | `Rpc { code: Unavailable, message }` | what the DO did before the error; the DO's own fault table (feature 07, section 6.9) covers it |
| The DO answers `NotFound` for a scoped call | Removes the index entry. | `NotFound` | nothing |

A DO that was created by a scoped call for an ID that never had a sandbox has no stored
state: `restore` finds no record, the call answers `NotFound`, and Cloudflare removes a DO
with no storage.

### 6.4 The `Provider`

`foundation_deployment_platform`: `ProviderId::CloudflareContainer`
(`cloudflare-container`); `SandboxProvider::cloudflare(control)` is
`SandboxProvider::new(control, DriverId::CloudflareContainer)`. No other code: the facade
of feature 06 works on any `SandboxControl`.

### 6.5 `wrangler.toml`

```toml
name = "ewe-sandbox"
main = "build/index.js"
compatibility_date = "2026-05-12"

[build]
command = "worker-build --release"

[[durable_objects.bindings]]
name = "SANDBOX"
class_name = "SandboxDo"

[[durable_objects.bindings]]
name = "SANDBOX_INDEX"
class_name = "SandboxIndexDo"

[[migrations]]
tag = "v1"
new_sqlite_classes = ["SandboxDo", "SandboxIndexDo"]

[[containers]]
class_name = "SandboxDo"
name = "ewe-sandbox"
image = "./Dockerfile"
max_instances = 10
instance_type = "lite"
```

The `containers` keys are the ones of `cloudflare/containers`,
`examples/core-tests/wrangler.jsonc` (`image`, `class_name`, `name`, `max_instances`). The
build command is the one that the skill `rust-valtron-usage` names for Workers
(`worker-build`). `max_instances = 10` is a starting value for the operator to change; the
README says where. It is the most sandboxes that the Worker can have at one time: a
`Create` above it fails in `container.start`, and the caller gets `SubstrateApi` with the
platform's message. `instance_type` sets the CPU and memory of each sandbox (`"lite"` is
the value of the Cloudflare minimal example, `instance: "lite"`); `SandboxSpec.vcpus` and
`memory_mib` are not used on Cloudflare.

The token: `npx wrangler secret put SANDBOX_TOKEN`; for `wrangler dev`, a `.dev.vars` file
that is in `.gitignore`.

### 6.6 The image

```dockerfile
FROM alpine:3.22
COPY agent-slave /usr/local/bin/agent-slave
RUN mkdir -p /workspace
CMD ["/usr/local/bin/agent-slave", "--listen", "tcp:0.0.0.0:1024"]
```

The driver sets the same entry point at start (feature 07, invariant I2), so the `CMD` is
only the default for a person who runs the image by hand.

Cloudflare containers are `linux/amd64`. A `mise` task `sandbox:worker-image` runs
`mise run sandbox:slave` and copies `target/sandbox/agent-slave-x86_64` to
`infrastructure/sandbox_worker/agent-slave`. That copy is in `.gitignore`.

A user who wants other programs in the sandbox makes an image `FROM` their own base with
the same `COPY` line, and points `image` at it. Sandbox images for Cloudflare are container
images; the image features 12 to 15 are for the microVM drivers.

### 6.7 Deploy

The Worker, its two DO classes, its container and its routes come from one `DeploymentPlan`.
Two adapters apply that plan:

| Adapter | How it works | When to use it |
|---|---|---|
| `FoundationCloudflareDeployer` | The Rust path. It uses `foundation_deployment_cloudflare` and `ewe_deployables::cloudflare` (the generated Worker, Durable Object, D1/KV/R2 and container types). It builds the Worker, builds and pushes the image, uploads a Worker version with the bindings and DO migrations, sets the secrets and routes, and promotes the version. | The default. CI and an operator with the Rust toolchain. |
| `WranglerCloudflareDeployer` | The manual path. It writes the same plan as a temporary `wrangler.toml`, copies the Dockerfile, runs `npx wrangler deploy`, and reads the Worker URL from the output. It is native-only and uses an injected `CommandRunner`. | A fallback when the foundation crate is not available, and the documented manual procedure. |

The foundation path is the priority. A deploy run with no adapter selected uses it. The
`wrangler` path is supported and tested; it is not a second design. Both adapters read the
same plan, and a test compares the plan they receive (invariant I10).

The manual steps in the README remain correct: install `wrangler` and `worker-build`; run
`mise run sandbox:worker-image`; set `SANDBOX_TOKEN`; run `npx wrangler deploy`. They are
the `WranglerCloudflareDeployer` path. The README also gives the one-command foundation
path and marks it as the primary one.

### 6.8 The deployment plan and the two adapters

`foundation_deployment_cloudflare` gets the deployment surface. It is a native library; it
does not compile to wasm. The plan types are plain data and have no side effect.

```rust
pub struct DeploymentPlan {
    pub worker: WorkerDeployment,
    pub bindings: Vec<Binding>,
    pub migrations: Vec<DoMigration>,
    pub containers: Vec<ContainerDeployment>,
    pub routes: Vec<Route>,
    pub secrets: Vec<SecretRef>,
}
pub struct WorkerDeployment {
    pub name: String,                  // "ewe-sandbox"
    pub main: PathBuf,                 // the worker-build output, or a wasm module
    pub compatibility_date: String,    // "2026-05-12"
}
pub enum Binding {
    DurableObject { name: String, class_name: String },
    D1 { name: String, database_id: String },
    Kv { name: String, namespace_id: String },
    R2 { name: String, bucket_name: String },
    Ai { name: String },
    Service { name: String, service: String },
    PlainText { name: String, value: String },   // never a secret
}
pub struct DoMigration { pub tag: String, pub new_sqlite_classes: Vec<String> }
pub struct ContainerDeployment {
    pub class_name: String,
    pub name: String,
    pub image: ContainerImage,
    pub max_instances: u32,
    pub instance_type: String,          // "lite", "basic", ...
}
pub enum ContainerImage {
    Dockerfile(PathBuf),                // build and push from this file
    Registry { tag: String, digest: Option<String> },   // CI built it
}
pub struct Route { pub pattern: String, pub zone: Option<String> }
pub struct SecretRef { pub name: String, pub source: SecretSource }
pub enum SecretSource { WorkerSecret, SecretsStore { store: String } }

pub trait CloudflareDeployer: Send + Sync + 'static {
    fn deploy(&self, ctx: Ctx, plan: DeploymentPlan)
        -> BoxFuture<'_, Result<DeploymentOutcome, DeployError>>;
    fn status(&self, ctx: Ctx, id: &DeploymentId)
        -> BoxFuture<'_, Result<DeploymentStatus, DeployError>>;
    fn rollback(&self, ctx: Ctx, id: &DeploymentId)
        -> BoxFuture<'_, Result<(), DeployError>>;
}
pub struct DeploymentOutcome {
    pub id: DeploymentId,
    pub worker_url: String,
    pub container_tags: Vec<String>,
    pub deployed_unix_ms: u64,
}
pub enum DeploymentStatus { Queued, Building, Uploading, Live, Failed, RolledBack }
pub struct DeploymentId(String);
pub enum DeployError {
    HostNotConfigured { entry: String },
    InvalidPlan { field: String, reason: String },
    Auth { message: String },
    Build { step: String, code: Option<i32>, message: String },
    Push { image: String, message: String },
    Api { status: u16, message: String },
    Conflict { message: String },
    RolloutTimeout { deployment: String },
    Io { op: String, message: String },
    Unsupported { what: String },
}
```

**`FoundationCloudflareDeployer`** (primary) runs these steps:

1. Validate the plan. One Worker name, one `main`, one binding name, one migration tag.
   Every `ContainerDeployment.class_name` has a `Binding::DurableObject` with the same
   class name, and every DO class is in a migration `new_sqlite_classes`. A failure is
   `InvalidPlan`; nothing reaches Cloudflare.
2. Build the Worker: run `worker-build --release` in the Worker directory, or use the wasm
   module that the plan names. A build failure is `Build`.
3. For each `ContainerDeployment`: build the image from the Dockerfile with the Docker
   Engine (or use the registry tag), then push it. A failure is `Push`. Cloudflare
   containers are `linux/amd64`.
4. Create or update the container application and the Worker's container class through the
   generated Cloudflare API types (root rule 5). A missing generated type is a generator
   defect; fix the generator, do not hand-roll JSON.
5. Upload a new Worker version with the bindings and the Durable Object migrations. The
   upload is one API call; the body streams from the file. The new version is not live
   yet.
6. Set each secret on the new version: read the value from the `SecretSource` through the
   injected secret store, then write it. The value is never in the plan or a log.
7. Set the routes.
8. Promote the new version. This is the traffic switch. A failure before this step leaves
   the previous version live.
9. Return `DeploymentOutcome { id, worker_url, container_tags, deployed_unix_ms }`.

**`WranglerCloudflareDeployer`** (fallback) runs these steps:

1. Validate the plan with the same function.
2. Write `wrangler.toml` from the plan into a temporary directory; copy or link the
   `Dockerfile` and the Worker `main`.
3. Run `npx wrangler deploy` with the injected `CommandRunner` (native only). The command
   builds the Worker and the image, and pushes them. Capture the output to find the Worker
   URL.
4. For each `SecretRef`: run `npx wrangler secret put <name>` and write the value to its
   standard input. `WorkerSecret` and `SecretsStore` both have a wrangler command; the
   adapter uses the one that the source names.
5. Return the same `DeploymentOutcome` shape. The CLI uploads the Worker as one atomic
   change, so a failed command leaves the previous Worker live.

Both adapters use the injected `Ctx` entries: a `CloudflareDeployHost` with the API client,
the Docker client, the command runner (wrangler only), the secret store and the clock. A
missing entry is `HostNotConfigured`. Tests use fakes.

**The sandbox plan.** `DeploymentPlan::sandbox_worker()` returns the plan of sections 6.5
and 6.6: Worker `ewe-sandbox`, bindings `SANDBOX` and `SANDBOX_INDEX`, the two DO classes,
the `containers` entry with class `SandboxDo`, image `./Dockerfile`, `max_instances 10`,
`instance_type "lite"`, and the `SANDBOX_TOKEN` secret. `wrangler.toml` is a view of this
plan for the manual path; the plan is the source of truth.

**The agent-rpc plan.** Spec 83 feature 06 builds its own `DeploymentPlan` with its Worker
name, bindings, DO migrations and container class, and calls the same `CloudflareDeployer`.
The deployer does not know agent-rpc. This is the boundary: spec 77 owns the deploy
mechanism; a caller owns its plan.

**Status and rollback.** `status` reads the Cloudflare deployment state. `rollback` applies
the previous `DeploymentOutcome` (the previous Worker version and container tags). A failed
deploy does not change the live Worker. `rollback` is idempotent.

## 7. Tests

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| I2, I3, the routing table | `route_table` (each method), `route_bad_id`, `route_unknown_method`; `route_property` (any valid id in a scoped body routes to that id) | `foundation_platform_sandbox/tests/cloudflare_routing.rs` (native) | example, identity |
| I1 | `check_bearer_cases` (missing, wrong scheme, wrong token, right token) | the same | expected outcome |
| I7 | `worker_has_no_logic` (a source scan of `infrastructure/sandbox_worker/src` for `SandboxState`, `boot_timeout`, `retry`) | `foundation_platform_sandbox/tests/boundary_scan.rs` | example |
| I8, "A command runs ...", "The shared acceptance scenarios pass" | `e2e_cloudflare_acceptance` | `foundation_platform_sandbox/tests/e2e_cloudflare.rs` | the shared scenarios |
| I1, "Every call needs the token" | `e2e_cloudflare_token` | the same | expected outcome |
| I5, "An unknown sandbox is NotFound", "A bad sandbox id ..." | `e2e_cloudflare_not_found`, `e2e_cloudflare_bad_id` | the same | expected outcome |
| I6, "List shows live sandboxes only" | `e2e_cloudflare_list` | the same | expected outcome |
| stale index entry | `e2e_cloudflare_stale_index`: create a sandbox; kill its container with `docker kill` (not through the API); `Get` answers `NotFound`; `List` then does not show it | the same | expected outcome |
| index DO paths | `index_put_delete_list` (through `wrangler dev`: the internal paths are reachable only from the Worker; the test checks the effect through `List` after a create and a stop, and that `PUT /index/x` from outside is `NotFound { what: "method" }`) | the same | expected outcome |
| "Two sandboxes are separate" | `e2e_cloudflare_two_sandboxes` | the same | example |
| "No internet unless asked" | `e2e_cloudflare_no_internet`, and with `NetworkMode::Host` the same command exits 0 when the test machine has a network (the test returns with a message when it has none) | the same | expected outcome |
| I4 | `e2e_cloudflare_large_stream` (20 MB of output arrives in order with flat client memory) | the same | identity |
| "Stop destroys the container" | `e2e_cloudflare_stop` (checks `docker ps` for the container that `wrangler dev` made) | the same | conservation |
| I9, "The slave and the Worker have the same version" | `e2e_cloudflare_versions` | the same | identity |
| restart of a DO | `e2e_cloudflare_restart`: create; stop and start `wrangler dev` with the same state directory (`--persist-to`); then `Get`. The test reads `docker ps` first to know which case it is in: the container survived (then `Get` must give `Ready`, and a new command must work, with no second `Prepare` in the slave) or it did not (then `Get` must give `NotFound`, and `List` must be empty). Each case has one exact expected result. | the same | expected outcome |
| "A provider starts a Cloudflare sandbox" | `provider_e2e_cloudflare` | `foundation_deployment_platform/tests/sandbox_provider_tests.rs` | example |
| I10, "Both deployers apply one plan" | `deploy_plan_parity` (both adapters receive the same `DeploymentPlan`; their recorded calls name the same Worker, bindings, migrations, container class and routes) | `foundation_deployment_cloudflare/tests/deploy_plan.rs` | identity |
| I11, "The primary deployer is the Rust path" | `default_deployer_is_foundation` | the same | example |
| I12, "A secret never enters the plan" | `deploy_plan_has_no_secret_value`, `deploy_sets_secret_from_source` (a fake secret store; the value reaches the API and never the plan or the log) | the same | expected outcome, conservation |
| I13 | `deploy_failure_keeps_previous`, `rollback_idempotent` (a fake API fails at each step; the live version stays; a second rollback is `Ok`) | the same | fault, idempotence |
| plan validation | `deploy_plan_invalid` (two mains, a duplicate binding, a missing migration tag) | the same | expected outcome |
| foundation adapter | `foundation_deploy_mock` (a fake API and Docker client; each step in order) | the same | example |
| wrangler adapter | `wrangler_deploy_command_lines` (a fake `CommandRunner`; the generated `wrangler.toml`, the deploy command and the secret commands) | the same | example |

The `e2e_cloudflare` tests need `wrangler`, `worker-build` and Docker. They read
`EWE_SANDBOX_WORKER_URL` and `EWE_SANDBOX_WORKER_TOKEN`; when the URL is not set they print
the reason and return. `test/run.sh` sets them. This is the real local form of the service
(specifications rule 3).

CI: a job `sandbox-cloudflare` in `.github/workflows/sandbox-e2e.yaml` on `ubuntu-latest`
(it has Docker): install Node and `wrangler`, `cargo install worker-build` (pinned
version in `ci/sandbox.toml`), `mise run sandbox:worker-image`,
`infrastructure/sandbox_worker/test/run.sh`. Same triggers and concurrency group as the
Firecracker job, plus `infrastructure/sandbox_worker/**`. `EWE_SANDBOX_E2E_REQUIRED=1`
makes a missing tool a failure.

A deploy to the real Cloudflare network is a manual step before the MVP-B release:
`test/run.sh --url <deployed url>` runs the same tests against it. Record the date and the
result in `progress.md`.

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features testing --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features testing --profile uat cloudflare_routing boundary_scan
( cd infrastructure/sandbox_worker && cargo fmt --check && cargo clippy --target wasm32-unknown-unknown -- -D warnings )
mise run sandbox:worker-image
infrastructure/sandbox_worker/test/run.sh
cargo test -p foundation_deployment_platform --features sandbox --profile uat provider_e2e_cloudflare
cargo test -p foundation_deployment_cloudflare --profile uat deploy_
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes in `wrangler dev`, and the CI
  job `sandbox-cloudflare` is green.
- The shared acceptance scenarios give the same results on Firecracker and on Cloudflare.
- The Worker crate has only the entry, the two DO wrappers and the `worker` type
  conversions.
- The README tells an operator how to deploy and how to set the token.
- Both deploy paths apply one plan. The foundation path is the default; the `wrangler` path
  is a tested fallback.
- `progress.md` records MVP-B and the manual deploy result.
