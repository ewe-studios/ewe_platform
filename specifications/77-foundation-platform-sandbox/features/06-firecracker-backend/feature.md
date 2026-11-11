---
feature: "Firecracker backend"
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["01-guest-agent", "02-host-core", "03-guest-artifacts", "05-firecracker-api-client"]
crates: ["foundation_platform_sandbox"]
---

# Feature 06: The Firecracker backend (milestone M2)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); virtual machine
(VM); virtual machine monitor (VMM); Kernel-based Virtual Machine (KVM); Unix domain socket
(UDS); process identifier (PID); context identifier (CID); virtual CPU (vCPU); continuous
integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

## 1. Goal

Start a Firecracker microVM with our agent as its init program, on Linux. When this feature
lands, this code works on a Linux host with KVM and the `firecracker` program:

```rust
let store = ArtifactStore::new(ctx.clone(), SandboxHost::from_ctx(&ctx)?);
let rootfs = store.firecracker_ci_rootfs(GuestArch::host()).await?;

let sandbox = FirecrackerBackend::new()
    .create(ctx, SandboxSpec::new(RootFs::SquashfsImage(rootfs)))
    .await?;
let out = sandbox.run(Command::new(["echo", "hello"])).await?;
assert_eq!(out.stdout, b"hello\n");
sandbox.stop().await?;
```

The kernel and the agent initramfs come from the artifact store by default.

## A. Story

- **S1**: "A caller on Linux starts a sandbox from a root file system image and runs a
  command in it."
- **S2**: "Many sandboxes use one image file at the same time, and the file never changes."
- **S3**: "The host is not ready: no KVM access, no `firecracker` program."
- **S4**: "The VM does not come up: the process exits, the API rejects a setting, the kernel
  panics, the agent never connects."
- **S5**: "The host program dies; the VM does not stay."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Check the host | caller, doctor | `FirecrackerBackend::probe` | A report and the capabilities. |
| U2 | Create | caller | `create(ctx, spec)` | A `Ready` sandbox, or a typed error and no leftovers. |
| U3 | Watch progress | caller | `create_with_progress` | `Preparing`, `VmmSpawned`, `VmmConfigured`, `Booting`, `AgentConnected`, then the result. |
| U4 | Stop | caller | `stop()` | The guest powers off; the process exits with code 0. |

## B. Bounded context

**Inside this feature:** the Firecracker start state machine, the plan of one VM, the boot
arguments, the Firecracker host checks.

**Outside:** the `firecracker` program (installed); the API client (feature 05); the agent
as PID 1 (feature 01); `AgentSandbox` (feature 02); kernel, initramfs and image files
(feature 03).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| `FcPlan` | Every resolved path and value of one VM. | the `Prepare` state; later states only read |
| VMM process | One `firecracker` child. | `ProcessSpawner` (spawn); backend (signal) |
| API socket | `fc.sock`. | Firecracker creates it |
| Agent listener | `v.sock_1024`. | backend |
| Root image | The caller's ext4 or squashfs file. Attached read-only. | the caller; nobody writes it |
| Launch state | The enum `FcLaunch`. | the `fc_launch` task |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| probe | entity | caller | Reads the host; starts nothing. |
| prepare | entity | `create` | Resolves files; makes the run directory and the listener. |
| spawn | entity | after prepare | Starts `firecracker`. |
| await API | entity | after spawn | Polls `GET /` with back-off until it answers. |
| configure | story | API is up | Five calls in a fixed order; the last one starts the VM. |
| await agent | entity | after start | Waits for accept, process exit or the deadline. |
| create | story | caller | All of the above plus the handshake; cleans up on a failed step. |

## C. Rules

### Law worksheet

```
Entity: Launch (the create story)
Conserved value:         a failed create leaves 0 processes and 0 run directories; a good one leaves exactly 1 of each
Bound:                   total time <= boot_timeout (+ clean-up); back-off wait <= 100 ms
Inverse pairs:           create / stop
Idempotent verbs:        probe
Ordering pairs:          listen -> InstanceStart (enabling); machine-config, boot-source, drive, vsock -> InstanceStart (enabling);
                         accept / process exit / deadline (competing)
Monotone values:         launch state; ConfigStep; progress values
Freeze points:           FcPlan after Prepare; boot arguments
Links to other entities: Vsock.uds_path + "_1024" = the listener path; drive path = spec image; initramfs = hash of the agent file

Entity: Root image file
Conserved value:         its bytes before create = its bytes after stop
Bound:                   none
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          none
Monotone values:         none
Freeze points:           the whole file, for the life of every sandbox that uses it
Links to other entities: attached as drive "rootfs", is_read_only true, is_root_device false
```

### Invariants

| # | Invariant |
|---|---|
| I1 | The listener on `v.sock_1024` exists before `InstanceStart` is sent. |
| I2 | The API calls go in the order machine-config, boot-source, drive, vsock, start. A failed call stops the sequence; no later call is sent. |
| I3 | The root image is attached with `is_read_only: true` and `is_root_device: false`. Its bytes never change. |
| I4 | The default seccomp filter stays on. `--no-seccomp` is never passed. |
| I5 | The `firecracker` process gets an empty environment. |
| I6 | A failed `create` leaves no `firecracker` process and no run directory. |
| I7 | Every wait uses the injected clock; the back-off is 5, 10, 20, 40, 80, 100, 100 ... ms. |
| I8 | When the host process dies, the `firecracker` process dies (`PR_SET_PDEATHSIG`). |
| I9 | The agent is PID 1 in the guest, and a `Shutdown` ends the process with code 0. |
| I10 | Two sandboxes on one image do not see each other's writes. |

### Must have

- `FirecrackerBackend`, the `FirecrackerApi` trait, `FcPlan`, the boot argument builder.

### Must do

- Put the tail of `console.log` and `vmm.log` in each start error.
- Reject, before any process starts, a spec that Firecracker cannot run (directory root,
  odd vCPU count above 1, host network, host mounts).

### Must not

- Must not add `root=` to the boot arguments.
- Must not write to the image, the kernel or the initramfs.
- Must not use the `jailer` or a network device in this feature.

## D. Acceptance scenarios

```gherkin
Feature: Firecracker backend

  Scenario: A command runs in a microVM
    Given a usable Firecracker host and the CI squashfs image
    When the caller creates a sandbox and runs ["echo", "hello"]
    Then stdout is "hello\n" and the exit code is 0

  Scenario: The agent is the first process
    When the sandbox reports HelloAck
    Then is_pid1 is true
    And "cat /proc/1/comm" prints the agent name

  Scenario: The image never changes
    Given the SHA-256 of the image file
    When a sandbox runs ["sh", "-c", "echo x > /root/f"] and stops
    Then the SHA-256 of the image file is the same

  Scenario: Two sandboxes share one image and not their writes
    Given two sandboxes on one image file
    When the first writes /tmp/a
    Then the second does not see /tmp/a

  Scenario: Read-only mode allows only the named places
    Given a spec with RootMode::ReadOnly
    When the sandbox runs ["sh", "-c", "touch /etc/x"]
    Then the exit code is not 0
    When the sandbox runs ["sh", "-c", "touch /tmp/x && touch x"]
    Then the exit code is 0

  Scenario: The API calls come in order
    Given a fake API that records calls
    When create runs
    Then the calls are describe_instance, put_machine_config, put_boot_source, put_drive, put_vsock, instance_start
    And the listener existed when instance_start arrived

  Scenario: A rejected setting stops the launch
    Given a fake API where put_drive returns 400 "bad drive"
    When create runs
    Then the result is VmmApi { status: 400, fault_message: "bad drive" }
    And instance_start was never called
    And the process got SIGKILL and no run directory is left

  Scenario: The API back-off uses the clock
    Given a fake API that fails to connect 4 times
    When create runs on a simulated clock
    Then the waits are 5, 10, 20 and 40 ms

  Scenario: A guest that cannot start is reported with its console
    Given boot arguments with ewe.transport=bad
    When create runs
    Then the result is BootTimeout or VmmExited
    And the error has the console tail
    And no process and no run directory are left

  Scenario: A wrong root kind is refused before any process
    When create runs with RootFs::Directory
    Then the result is UnsupportedRootFs
    And no process was spawned

  Scenario: Stop powers the guest off
    When the caller stops a ready sandbox
    Then the firecracker process exits with code 0
    And ExitReport.killed is false
```

## 2. How the guest is put together

| Part | Source | How Firecracker gets it |
|---|---|---|
| Kernel | `spec.kernel`, or `store.firecracker_kernel(arch)` | `BootSource.kernel_image_path` |
| Init program | The agent, in an initramfs from `store.agent_initramfs(agent_path)` | `BootSource.initrd_path` |
| User root file system | `RootFs::Ext4Image` or `RootFs::SquashfsImage` | A drive with `drive_id = "rootfs"`, `is_root_device = false`, `is_read_only = true` |
| Host link | vsock | `Vsock { guest_cid, uds_path }` |

The user image is always attached read-only. The agent mounts it as the lower layer and
gives programs a root as `RootMode` says (feature 01, section 5). So many sandboxes can use
one image file at the same time, and no copy is made.

`is_root_device` is false because Firecracker rejects a root drive together with an
initramfs (`docs/initrd.md`). The agent finds the drive at `/dev/vda`: it is the first
virtio-block device.

### 2.1 Boot arguments

```
console=ttyS0 reboot=k panic=1 quiet ewe.transport=vsock:2:1024
```

On x86_64 add `pci=off i8042.noaux i8042.nomux i8042.dumbkbd`.

| Argument | Reason |
|---|---|
| `console=ttyS0` | Kernel and agent messages go to the serial port, thus to the standard output of `firecracker`, thus to `console.log`. |
| `reboot=k` | A guest reboot stops the VM. The agent uses this to power off. |
| `panic=1` | A kernel panic reboots after 1 s, which stops the VM. A hung guest does not stay. |
| `quiet` | Less console text, a faster boot. |
| `pci=off` | x86_64: no PCI scan. The devices are on the MMIO bus. |
| `i8042.*` | x86_64: skips keyboard controller probes, and keeps the reset line that `reboot=k` uses. |
| `ewe.transport=...` | The agent reads it from `/proc/cmdline` (feature 01, section 3). |

Do not add `root=`. The kernel uses the initramfs as its root and starts `/init`.

## 3. Start state machine

File `backends/foundation_platform_sandbox/src/firecracker/mod.rs`, behind the feature
`firecracker` and `cfg(target_os = "linux")`.

```mermaid
sequenceDiagram
  participant B as FirecrackerBackend
  participant F as firecracker process
  participant A as sandbox-agent (PID 1)
  B->>B: Prepare: artifacts, run directory, listen on v.sock_1024
  B->>F: spawn firecracker --api-sock fc.sock --id <id>
  loop until the API answers or the deadline
    B->>F: GET /
  end
  B->>F: PUT /machine-config
  B->>F: PUT /boot-source
  B->>F: PUT /drives/rootfs
  B->>F: PUT /vsock
  B->>F: PUT /actions InstanceStart
  F->>A: kernel boots, unpacks initramfs, runs /init
  A->>F: vsock connect CID 2 port 1024
  F->>B: connect to v.sock_1024
  B->>A: preface, Hello (BlockDevice /dev/vda)
  A->>A: mount image, build root
  A->>B: HelloAck
  Note over B: Sandbox is Ready
```

_Caption: the backend configures the VM with five API calls, starts it, and waits for the agent to connect to a socket that already listens._

```rust
pub enum FcLaunch {
    Prepare    { ctx: Ctx, spec: SandboxSpec },
    Spawn      { ctx: Ctx, spec: SandboxSpec, plan: FcPlan, listener: Box<dyn StreamListener> },
    AwaitApi   { ctx: Ctx, spec: SandboxSpec, plan: FcPlan, listener: Box<dyn StreamListener>,
                 vmm: Box<dyn ChildProcess>, api: Arc<dyn FirecrackerApi>,
                 deadline: foundation_compact::Instant, attempt: u32 },
    Configure  { /* the same fields */ step: ConfigStep },
    AwaitAgent { ctx: Ctx, spec: SandboxSpec, plan: FcPlan, listener: Box<dyn StreamListener>,
                 vmm: Box<dyn ChildProcess>, deadline: foundation_compact::Instant },
    Handshake  { ctx: Ctx, spec: SandboxSpec, plan: FcPlan, vmm: Box<dyn ChildProcess>,
                 stream: Box<dyn ByteDuplex>, deadline: foundation_compact::Instant },
    Done,
}

pub enum ConfigStep { MachineConfig, BootSource, Drive, Vsock, Start }

/// Everything that `Prepare` resolved.
pub struct FcPlan {
    pub id: SandboxId,
    pub run: RunDir,
    pub firecracker: PathBuf,
    pub kernel: PathBuf,
    pub initramfs: PathBuf,
    pub rootfs: PathBuf,
    pub rootfs_type: FsType,
    pub guest_cid: u32,
    pub boot_args: String,
}

#[task]
async fn fc_launch(ctx: Ctx, state: FcLaunch)
    -> Option<(FcLaunch, TaskStatus<SandboxResult<Sandbox>, CreateProgress>)>;
```

| State | Work | Emits | Next |
|---|---|---|---|
| `Prepare` | Validate the spec. Resolve the `firecracker` path, the kernel, the initramfs and the image (await the artifact store). Make the ID and the run directory. Check the socket path length for `v.sock_1024`, the longest one. Create an empty `vmm.log`. Pick `guest_cid` (section 4.2). Bind the listener at `<run_dir>/v.sock_1024`. | `Pending(Preparing)` | `Spawn` |
| `Spawn` | Spawn `firecracker` (section 4.1). Set `deadline = now + boot_timeout`. | `Pending(VmmSpawned)` | `AwaitApi` |
| `AwaitApi` | If `fc.sock` does not exist yet, or `describe_instance()` fails to connect: await a timer of `min(5 ms * 2^attempt, 100 ms)` from the clock, then the same state with `attempt + 1`. Also check `vmm.try_wait()`. On a good answer with state `Not started`, go on. | nothing (`Ignore`) while it waits | `Configure { MachineConfig }` |
| `Configure` | One API call for each step, in the order of `ConfigStep`. Section 4.3 has the bodies. | `Pending(VmmConfigured)` after `Vsock`; `Pending(Booting)` after `Start` | the next step, then `AwaitAgent` |
| `AwaitAgent` | Await the first of: `listener.accept()`, `vmm.wait()`, the deadline timer. | `Pending(AgentConnected)` | `Handshake` |
| `Handshake` | `AgentClient::handshake` with `RootSource::BlockDevice { device: "/dev/vda", fstype }` and an empty `dns` list (Firecracker has no guest network in 0.1). Build `AgentSandbox`. | `Ready(Ok(sandbox))` | `Done` |
| `Done` | | `None` | |

Failure handling is the same as for libkrun (feature 04, section 4.1): kill the VMM, read
the tail of `console.log` and `vmm.log` (at most `max_error_message` bytes together),
remove the run directory, emit `Ready(Err(..))`, go to `Done`.

| Failure | Error |
|---|---|
| The `firecracker` process exits in `AwaitApi`, `Configure` or `AwaitAgent` | `VmmExited { code, log_tail }` |
| An API call returns a status that is not 2xx | `VmmApi { status, fault_message }` |
| The deadline passes in `AwaitApi` | `BootTimeout` with the stage `api` |
| The deadline passes in `AwaitAgent` | `BootTimeout` with the stage `agent` and the console tail. The usual causes are a kernel with no vsock, or a kernel panic. |
| `Hello` set-up fails in the guest | `AgentError { code: RootSetupFailed, .. }` |

### 3.1 `FirecrackerApi`

The state machine calls the API through a trait, so the simulation tests can replace it:

```rust
pub trait FirecrackerApi: Send + Sync + 'static {
    fn describe_instance(&self) -> BoxFuture<'_, SandboxResult<InstanceInfo>>;
    fn put_machine_config<'a>(&'a self, body: &'a MachineConfiguration) -> BoxFuture<'a, SandboxResult<()>>;
    fn put_boot_source<'a>(&'a self, body: &'a BootSource) -> BoxFuture<'a, SandboxResult<()>>;
    fn put_drive<'a>(&'a self, body: &'a Drive) -> BoxFuture<'a, SandboxResult<()>>;
    fn put_vsock<'a>(&'a self, body: &'a Vsock) -> BoxFuture<'a, SandboxResult<()>>;
    fn instance_start(&self) -> BoxFuture<'_, SandboxResult<()>>;
}
impl FirecrackerApi for FirecrackerClient { /* delegates */ }
```

`SandboxHost` gets one more field for this: `firecracker_api: Arc<dyn Fn(&Ctx, &Path, &SandboxLimits) -> SandboxResult<Arc<dyn FirecrackerApi>> + Send + Sync>`.
`SandboxHost::native()` sets it to the real client. The body types are the generated types
of feature 05.

## 4. Details

### 4.1 The `firecracker` process

```
firecracker --api-sock <run_dir>/fc.sock --id <sandbox_id> --log-path <run_dir>/vmm.log --level Warning
```

- Standard output and standard error go to `<run_dir>/console.log`. Standard input is
  `/dev/null`.
- The environment is empty. The work directory is the run directory.
- `kill_with_parent = true`.
- The default seccomp filter stays on. Never pass `--no-seccomp`.
- `--log-path` needs a file that exists; `Prepare` creates it.

### 4.2 Run directory and the guest CID

`<state_dir>/<sandbox_id>/`, mode `0700`:

| File | Made by | Content |
|---|---|---|
| `fc.sock` | Firecracker | The API socket. |
| `v.sock` | Firecracker, at `InstanceStart` | The vsock socket for host-started connections. We do not use it, but Firecracker makes it. The path must not exist before. |
| `v.sock_1024` | backend | The listening socket for the agent. |
| `console.log` | Firecracker output | Serial console. |
| `vmm.log` | Firecracker | VMM log. |

`guest_cid`: each Firecracker process has its own vsock device in user space, so two VMs can
use the same CID with no conflict. Still, pick a value from `ctx.rng()` in the range 3 to
`2^31 - 1`, so that logs show which VM a line is from.

### 4.3 API bodies

| Step | Call | Body |
|---|---|---|
| `MachineConfig` | `put_machine_config` | `vcpu_count = spec.vcpus`, `mem_size_mib = spec.memory_mib`, `smt = false`. Other fields `None`. |
| `BootSource` | `put_boot_source` | `kernel_image_path`, `initrd_path`, `boot_args` from the plan. |
| `Drive` | `put_drive` | `drive_id = "rootfs"`, `path_on_host`, `is_root_device = false`, `is_read_only = true`. Other fields `None`. |
| `Vsock` | `put_vsock` | `guest_cid`, `uds_path = <run_dir>/v.sock`. |
| `Start` | `instance_start` | |

`spec.vcpus` must be 1 or an even number. `FirecrackerBackend` adds this check to
`validate` and returns `InvalidSpec { field: "vcpus", .. }`.

### 4.4 Stop

`AgentSandbox::stop()` does the work (feature 02, section 3.2): `Shutdown` to the agent, the
agent calls `reboot()`, Firecracker exits with code 0. No API call is necessary. On x86_64
`SendCtrlAltDel` exists, but it needs a guest that handles it, and aarch64 does not have it;
we do not use it.

### 4.5 Capabilities

`rootfs_directory: false`, `rootfs_ext4: true`, `rootfs_squashfs: true`,
`host_mounts: false`, `network_host: false`.

`RootMode::Overlay` is the default. The Firecracker CI kernels have overlayfs and tmpfs
(`learnings.md` section 2.5). A user kernel with no overlayfs gets the agent error
`RootSetupFailed` with a message that names overlayfs; the user then picks `ReadOnly`.

## 5. Host check (`probe`)

| Check | How |
|---|---|
| `kvm` | `/dev/kvm` exists and opens for read and write. The message for a permission error tells the user to join the `kvm` group or to set an access control entry (`setfacl -m u:$USER:rw /dev/kvm`). |
| `firecracker` | The program exists at the resolved path. `firecracker --version` runs and prints a version. Report the version. |
| `firecracker_version` | The version is 1.10 or newer. Record in `learnings.md` the oldest version that the real test passed on, and set the check to that. |
| `agent` | The agent file exists for the guest architecture. |

## 6. Tests

`backends/foundation_platform_sandbox/tests/`. Each scenario of section D is one test
below. Each invariant of section C has a test that names it in a comment.

No VM (simulation host, `SimulatedClock`, a `FakeFirecrackerApi` that records calls):

| Test | Proves |
|---|---|
| `fc_launch_sim_ok` | The calls arrive in the order of section 4.3 with the bodies of section 4.3, and the progress values arrive in order. |
| `fc_boot_args_x86_64`, `fc_boot_args_aarch64` | The string of section 2.1 for each architecture. |
| `fc_await_api_backs_off` | `describe_instance` fails 4 times, then answers. The timer waits are 5, 10, 20, 40 ms on the simulated clock. No real sleep. |
| `fc_api_timeout` | The API never answers: `BootTimeout` with the stage `api`. |
| `fc_api_error_stops_launch` | `put_drive` returns 400: the result is `VmmApi`, `instance_start` is never called, the fake child got `SIGKILL`, the run directory is gone. |
| `fc_vmm_exit_during_boot` | The fake child exits with 152 before a connect: `VmmExited { code: 152 }` with the log tail. |
| `fc_agent_timeout` | No connect: `BootTimeout` with the stage `agent`. |
| `fc_rejects_directory_rootfs` | `RootFs::Directory` gives `UnsupportedRootFs`. |
| `fc_rejects_odd_vcpus` | `vcpus = 3` gives `InvalidSpec`. |
| `fc_rejects_host_network` | `NetworkMode::Host` gives `UnsupportedNetwork`. |
| `fc_listener_bound_before_start` | The fake API checks in `instance_start` that `v.sock_1024` exists. |

Real microVM (`e2e_firecracker.rs`). Each test calls `probe()` first and returns with the
report when `usable` is false.

| Test | Proves |
|---|---|
| `e2e_echo` | The code of section 1, with the CI squashfs image. |
| `e2e_ext4_root` | The same with an ext4 image. The test makes it with `mkfs.ext4 -q -F -d <Alpine directory> <file> 64M` and returns with a message when that program is not found. File owners in this image are wrong (feature 03, section 4.2); the test only proves that the ext4 mount path works. |
| `e2e_exit_code`, `e2e_write_and_read_file`, `e2e_no_network_by_default`, `e2e_stop_is_clean`, `e2e_kill_parent_kills_vm` | The same as the libkrun tests of feature 04. Put the shared body in `tests/support/e2e_cases.rs` and call it from both files. |
| `e2e_image_not_changed` | The SHA-256 of the image file is the same before `create` and after `stop`, after a command that wrote to `/`. |
| `e2e_two_sandboxes_one_image` | Two sandboxes from one image file run at the same time. A file that one writes is not in the other. |
| `e2e_readonly_mode` | With `RootMode::ReadOnly`: a write to `/etc` fails, a write to `/tmp` and to the work directory succeeds. |
| `e2e_agent_is_pid1` | `HelloAck.is_pid1` is true, and `cat /proc/1/comm` in the guest shows the agent. |
| `e2e_orphans_are_reaped` | `sh -c "sleep 0.1 & exit 0"` 50 times; then no zombie process is in `ps` output. |
| `e2e_kernel_panic_is_reported` | Boot with a kernel command line that makes `init` fail (a test-only extra boot argument `ewe.transport=bad`): the result is `BootTimeout` or `VmmExited` with the console tail in the error, in less than `boot_timeout` + 2 s. |
| `e2e_boot_time` | Prints the time from `create` to `Ready`. The value goes to `progress.md`. |

## 7. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --profile uat fc_
mise run sandbox:agent
EWE_SANDBOX_AGENT=<agent path> EWE_FIRECRACKER=<firecracker path> \
  cargo test -p foundation_platform_sandbox --profile uat e2e_firecracker -- --nocapture
```

The CI job of feature 07 runs the last command on `ubuntu-latest` (x86_64, with `/dev/kvm`).
Until that job exists, run it on a Linux x86_64 host with KVM and paste the output in the
report. aarch64 has no CI runner with KVM: the backend compiles and runs its tests with no
VM there, and the first person with an aarch64 KVM host runs the real tests and records the
result in `progress.md`. The README lists aarch64 Firecracker as "built, not verified"
until then.

## 8. Done when

- The tests with no VM pass in CI.
- The real microVM tests pass on a Linux x86_64 host with KVM, and `progress.md` has the
  date, the host, the Firecracker version and the boot time.
- The agent is PID 1 in the guest, mounts the image read-only, and powers the VM off on
  `Shutdown`.
- A failed start leaves no `firecracker` process and no run directory.
