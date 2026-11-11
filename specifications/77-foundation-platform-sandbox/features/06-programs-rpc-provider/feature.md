---
feature: "Provider and doctor"
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["04-libkrun-backend", "06-firecracker-backend"]
crates: ["foundation_deployment_platform", "foundation_platform_sandbox"]
---

# Feature 07: `Provider` implementations and host checks (version 0.1)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); virtual machine
(VM); Kernel-based Virtual Machine (KVM); Hypervisor.framework (HVF); behaviour-driven development (BDD); continuous integration
(CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

## 1. Goal

Make the two local backends available through the `Provider` trait of
`foundation_deployment_platform`, add a default backend choice, and add one example program.
This feature completes version 0.1.

## A. Story

- **S1**: "A caller that uses the platform `Provider` trait starts and stops a sandbox the
  same way as a Docker container or a QEMU VM."
- **S2**: "A caller asks which sandbox backend this machine can run, and why the others
  cannot."
- **S3**: "A new user runs one example program and sees a command run in a sandbox."
- **S4**: "No backend is usable on this machine."
- **S5**: "The real microVM tests run in CI on a runner with KVM."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Launch through a provider | orchestration code | `Provider::launch(&spec)` | A `SandboxHandle`. |
| U2 | Stop through a provider | orchestration code | `Provider::stop(&handle)` | The sandbox is stopped. |
| U3 | Health | doctor, caller | `Provider::host_health()`, `sandbox_host_reports` | Each check with its message. |
| U4 | Pick a backend | caller | `default_sandbox_backend(ctx)` | A backend, or `NoBackend` with every report. |
| U5 | Try it | new user | `sandbox_run -- echo hello` | `hello`, and the program's exit code. |

## B. Bounded context

**Inside this feature:** the `Provider` implementations in `foundation_deployment_platform`
(the central place where the platform brings its backends together), the default choice,
the example, the documentation, the CI job.

**Outside:** all sandbox logic. It lives in `foundation_platform_sandbox` (features 00 to
06). A provider adapts; it adds no behaviour of its own.

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| `ProviderId` | The identifier of a provider kind. | `foundation_deployment_platform::providers` |
| `FirecrackerProvider`, `LibkrunProvider` | A `Ctx` and a backend, behind the sync `Provider` trait. | `providers::sandbox` |
| `SandboxHandle` | A `Sandbox` and its `ProviderId`. | `providers::sandbox` |
| `HostReport` | The result of a backend `probe`. | each backend (read here) |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| launch | entity (adapter) | caller | Blocks on `backend.create`. |
| stop | entity (adapter) | caller | Blocks on `sandbox.stop`. |
| is_running | entity (adapter) | caller | Reads the sandbox state. |
| host_health | entity (adapter) | caller | Maps `probe` checks to tuples. |
| choose default | entity | caller | Probes in a fixed order; returns the first usable backend. |

**The driver rule.** A provider translates. It does not decide. A provider that validates a
spec, retries a start or hides an error moves a rule out of the sandbox crate, where the
tests are.

## C. Rules

### Law worksheet

```
Entity: Provider (adapter)
Conserved value:         launch(spec) gives the same result as block_on(backend.create(ctx, spec))
Bound:                   none of its own
Inverse pairs:           launch / stop
Idempotent verbs:        stop (as the sandbox: the second call is Ok); is_running; host_health
Ordering pairs:          launch -> stop (enabling)
Monotone values:         none
Freeze points:           none
Links to other entities: is_running(handle) == (handle.sandbox.state() == Ready)

Entity: Default choice
Conserved value:         none
Bound:                   none
Inverse pairs:           none
Idempotent verbs:        default_sandbox_backend (on an unchanged host)
Ordering pairs:          Linux: Firecracker before libkrun; macOS: libkrun only
Monotone values:         none
Freeze points:           none
Links to other entities: the result is usable <=> its probe report says usable
```

### Invariants

| # | Invariant |
|---|---|
| I1 | A provider method has the same outcome as the sandbox call that it wraps. The error is the same typed error. |
| I2 | `is_running(handle)` is true exactly when the sandbox state is `Ready`. |
| I3 | `default_sandbox_backend` returns a backend only when its `probe` says `usable`. When none is, the error holds every report. |
| I4 | The provider crate has no sandbox logic: no spec validation, no retry, no time-out of its own. |
| I5 | Without the `sandbox` feature, `foundation_deployment_platform` builds and tests as before. |
| I6 | The only blocking calls of this spec are in the `Provider` methods and in example `main` functions. |

### Must have

- `ProviderId::Firecracker` and `ProviderId::Libkrun`.
- The two providers, `SandboxHandle`, `default_sandbox_backend`, `sandbox_host_reports`.
- The example `sandbox_run`, the README, `docs/sandbox.md`, the CI job.

### Must do

- Bump the crate versions and every consumer (`.agents` reminder 12).
- Run the real microVM tests on Linux in CI.
- Declare in `ci/packages.toml` which operating systems run which tests, with a reason for
  each opt-out (`.agents` rule 16).

### Must not

- Must not put the sandbox checks behind the `vms` feature.
- Must not call `Ctx::host()` in library code.

## D. Acceptance scenarios

```gherkin
Feature: Provider and doctor

  Scenario: A provider launches and stops a sandbox
    Given a provider on the simulation host
    When launch is called with a valid spec
    Then a handle is returned and is_running is true
    When stop is called
    Then the result is Ok and is_running is false

  Scenario: A provider passes errors through
    When launch is called with a root kind that the backend cannot use
    Then the error context is UnsupportedRootFs

  Scenario: Health mirrors the probe
    Given a backend whose probe has the checks ("kvm", ok) and ("agent", not ok, "missing")
    When host_health is called
    Then it returns exactly those two tuples

  Scenario: Linux prefers Firecracker
    Given a Linux host where both backends are usable
    When default_sandbox_backend is called
    Then the backend id is firecracker

  Scenario: Linux falls back to libkrun
    Given a Linux host where only libkrun is usable
    When default_sandbox_backend is called
    Then the backend id is libkrun

  Scenario: No backend is a full answer
    Given a host where no backend is usable
    When default_sandbox_backend is called
    Then the result is NoBackend with one report for each backend

  Scenario: The example runs a program
    Given a usable host
    When "sandbox_run -- sh -c 'echo hello; exit 3'" runs
    Then its standard output is "hello\n" and its exit code is 3

  Scenario: The handle gives the sandbox API
    Given a launched handle on a usable host
    When handle.run(["echo", "ok"]) is called through block_on_future
    Then stdout is "ok\n"
```

## 2. Measured facts

File `backends/foundation_deployment_platform/src/providers/mod.rs`:

```rust
pub trait Provider: Send + Sync {
    type Handle;
    type Config;
    type Error: std::fmt::Debug + std::fmt::Display;
    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    fn launch(&self, config: &Self::Config) -> Result<Self::Handle, Self::Error>;
    fn stop(&self, handle: &Self::Handle) -> Result<(), Self::Error>;
    fn is_running(&self, handle: &Self::Handle) -> bool;
    fn resolved_ports(&self, handle: &Self::Handle) -> Result<ResolvedPorts, Self::Error>;
    fn host_health(&self) -> Vec<(String, bool, String)>;
}
```

- The trait is sync. `DockerProvider` (`providers/docker/mod.rs`) wraps async calls with
  sync functions.
- `ProviderId` has `Qemu`, `Utm` and `Docker`, with a `Display` implementation.
- `ResolvedPorts { ssh_port: u16, winrm_port: Option<u16>, rdp_port: Option<u16>, vnc_port: u16 }`.
  A sandbox has no such ports. `DockerProvider` returns 0 for a port that it does not have.
- `foundation_deployment_platform` version is `0.1.6`. A new public API needs a version
  bump and an update of each workspace consumer (`.agents` reminder 12).

## 3. Work

### 3.1 `ProviderId`

Add the variants `Firecracker` and `Libkrun`, with the `Display` strings `firecracker` and
`libkrun`. Search the workspace for each `match` on `ProviderId`
(`grep -rn "ProviderId::" --include=*.rs .`) and update it.

### 3.2 The providers

New Cargo feature `sandbox` in `foundation_deployment_platform`, off by default:
`sandbox = ["dep:foundation_platform_sandbox"]`. Add it to `all-providers`.

New module `src/providers/sandbox/mod.rs`, behind the feature:

```rust
/// A running sandbox, as a `Provider` handle.
pub struct SandboxHandle {
    pub sandbox: Sandbox,
    pub provider_id: ProviderId,
}

pub struct FirecrackerProvider { ctx: Ctx, backend: FirecrackerBackend }   // cfg(target_os = "linux")
pub struct LibkrunProvider     { ctx: Ctx, backend: LibkrunBackend }       // cfg(linux or macos)

impl FirecrackerProvider { pub fn new(ctx: Ctx) -> Self; }
impl LibkrunProvider     { pub fn new(ctx: Ctx) -> Self; }
```

Both implement `Provider` with `Handle = SandboxHandle`, `Config = SandboxSpec`,
`Error = ErrorTrace<SandboxError>`. One generic helper holds the body, so the two types do
not repeat it:

| Method | Body |
|---|---|
| `launch(config)` | `block_on_future(self.backend.create(self.ctx.clone(), config.clone()))`, then wrap in `SandboxHandle`. |
| `stop(handle)` | `block_on_future(handle.sandbox.stop())`, drop the `ExitReport`. |
| `is_running(handle)` | `handle.sandbox.state() == SandboxState::Ready` |
| `resolved_ports(handle)` | `Ok(ResolvedPorts { ssh_port: 0, winrm_port: None, rdp_port: None, vnc_port: 0 })` |
| `host_health()` | `self.backend.probe(&self.ctx).checks`, mapped to `(name, ok, message)`. |

`block_on_future` is `foundation_core::valtron::block_on_future`
(`executors/sendables.rs`, line 1004). Read its doc comment for where a call is legal. The
`Provider` methods are a sync boundary; they are the only place in this spec that blocks.
The doc comment of each provider says: "Async callers use `Sandbox` and `SandboxBackend`
directly."

`SandboxHandle` implements `Deref<Target = Sandbox>`, so a caller that has a handle can call
`run` and `exec`.

### 3.3 Default choice

```rust
/// The best local sandbox backend for this host.
pub fn default_sandbox_backend(ctx: &Ctx) -> SandboxResult<Arc<dyn SandboxBackend>>;
```

| Host | Order |
|---|---|
| Linux | Firecracker when its `probe()` is usable; if not, libkrun when usable. |
| macOS | libkrun when usable. |
| other | `SandboxError::NoBackend` |

When no backend is usable, the error is `SandboxError::NoBackend { reports }` with each
`HostReport`, so the caller sees why. The function lives in
`foundation_platform_sandbox` (`src/lib.rs`), not in the provider crate, so async callers
get it too. Firecracker is first on Linux because it has the smaller device set and its own
seccomp filter.

### 3.4 Doctor

`foundation_deployment_platform` has a `doctor` module behind the `vms` feature. Do not put
sandbox checks behind `vms`. Add a function in the provider module:

```rust
/// One report for each sandbox backend that this build has.
pub fn sandbox_host_reports(ctx: &Ctx) -> Vec<HostReport>;
```

If the `testbed` command-line program has a `doctor` command
(`backends/foundation_deployment_platform/src/cli/`), add the sandbox reports to its output
when both the `cli` and `sandbox` features are on.

### 3.5 Example

`backends/foundation_platform_sandbox/examples/sandbox_run.rs`:

```
sandbox_run [--backend firecracker|libkrun] [--rootfs <path>] -- <program> [args...]
```

- Builds `Ctx::host()` and stores `SandboxHost::native(..)`. This is a host entry point, so
  these calls are legal here.
- With no `--rootfs`: libkrun uses the Alpine directory, and Firecracker uses the CI
  squashfs image (feature 03).
- Prints progress lines from `create_with_progress` to standard error.
- Streams the program output to its own standard output and standard error with
  `exec().events()`. Exits with the exit code of the program.

### 3.6 Documentation

- `backends/foundation_platform_sandbox/README.md`: what the crate is, the host
  requirements for each backend, how to build the agent and the runner, the example, the
  security notes of section 4.
- `docs/`: one page, `docs/sandbox.md`, with the architecture diagram of the requirements
  and a "first sandbox" guide for Linux and for macOS.

### 3.7 Issues to file

File these GitHub issues in this change (root rule 9). Requirements section 2.2 lists the
first group.

| Issue | Label |
|---|---|
| One for each item of requirements section 2.2 | `enhancement` |
| `ResolvedPorts`: `ssh_port` and `vnc_port` are `u16`, a provider with no such port returns 0 | `severity: low` |
| Embed `sandbox-agent` in the host crate with `include_bytes!` | `enhancement` |
| Each finding from features 00 to 06 that has no issue yet | its severity |

## 4. Security notes for the documentation

State these limits plainly in the README:

- Version 0.1 gives the isolation of the VM boundary. It does not confine the VMM process
  on the host. The VMM runs with the rights of the caller.
- libkrun with `NetworkMode::Host` gives the guest the network of the host process.
- A host directory mounted with `HostMount` is visible to the guest through the VMM. Mount
  only what the guest needs.
- The Firecracker `jailer` and VMM process confinement are planned, and the issue numbers
  are in the README.

## 5. Tests

`backends/foundation_deployment_platform/tests/sandbox_provider_tests.rs`:

| Test | Needs | Proves |
|---|---|---|
| `provider_ids_display` | nothing | `firecracker` and `libkrun`. |
| `provider_launch_stop_sim` | nothing | With the simulation host: `launch` returns a handle, `is_running` is true, `stop` returns `Ok`, `is_running` is false. Proves that `block_on_future` works with the launch task. |
| `provider_host_health_maps_checks` | nothing | A fake report with two checks gives two tuples. |
| `provider_launch_error_is_typed` | nothing | A spec with the wrong `RootFs` kind gives an error whose `current_context()` is `UnsupportedRootFs`. |
| `default_backend_no_backend` | nothing | With a simulation host where each probe fails, the result is `NoBackend` with the reports. |
| `provider_e2e_libkrun`, `provider_e2e_firecracker` | KVM or HVF | `launch`, `handle.run(["echo", "ok"])` through `block_on_future`, `stop`. Returns with a message when the host is not usable. |

The simulation host is in `foundation_platform_sandbox/tests/support/`. The provider tests
need it too. Move it to a public module `foundation_platform_sandbox::testing`, behind a
Cargo feature `testing`, with a doc comment that it gives no isolation and is for tests
only. `foundation_deployment_platform` enables that feature in `dev-dependencies`.

## 6. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_deployment_platform --features sandbox --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_deployment_platform --features sandbox --profile uat sandbox_provider
cargo test -p foundation_deployment_platform --profile uat          # default features still pass
cargo tree -i foundation_deployment_platform --workspace --depth 1  # then test each consumer
cargo run -p foundation_platform_sandbox --example sandbox_run --profile uat -- -- echo hello
python3 scripts/ci/package_workspace.py
```

Add the run `sandbox` to the `runs` and `clippy-runs` lists of
`[packages.foundation_deployment_platform]` in `ci/packages.toml`.

## 7. Done when

- Both providers implement `Provider`, and the tests of section 5 pass.
- The example prints `hello` on a Linux host with KVM (both backends) and on a macOS host
  with Apple silicon (libkrun).
- The versions of `foundation_deployment_platform` and `foundation_platform_sandbox` are
  bumped, and each consumer uses the new version.
- The README, `docs/sandbox.md` and the issues of section 3.7 exist.
- `requirements.md` frontmatter counts are current, and `progress.md` records 0.1.
