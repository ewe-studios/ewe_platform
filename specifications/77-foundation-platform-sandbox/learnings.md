# Spec 77 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); virtual machine
(VM); virtual machine monitor (VMM); Kernel-based Virtual Machine (KVM); Hypervisor.framework
(HVF); Unix domain socket (UDS); Transparent Socket Impersonation (TSI); representational
state transfer (REST); process identifier (PID); Durable Object (DO); continuous integration
(CI); context identifier (CID); AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

This file records what the research found, with the source of each fact. A feature agent
reads it before it starts. Add new facts, dead ends and root causes below the matching
heading.

## 1. Sources read (2026-11-11)

| Source | Revision | Used for |
|---|---|---|
| `github.com/firecracker-microvm/firecracker` | `778a26c` (2026-11-10), API `1.19.0-dev` | Sections 2 and 5 |
| `github.com/containers/libkrun`, branch `stable-1.19.x` | `c4d283c`, version 1.19.8 | Section 3 |
| `github.com/containers/libkrun`, branch `main` | `c9327bd`, version `2.0.0-dev` | Section 3.6 |
| `github.com/cloudflare/sandbox-sdk` | `61cac81` (2026-11-02), package 1.0.3 | Section 4 |
| `github.com/cloudflare/containers` | `a17055c` (2026-08-23) | Section 4 |

To read a source again: `git clone --depth 1 [-b <branch>] <url>`.

## 2. Firecracker

### 2.1 Process and command line

Source: `src/firecracker/src/main.rs`, lines 158 to 282.

| Flag | Meaning |
|---|---|
| `--api-sock <path>` | UDS for the REST API. |
| `--id <id>` | MicroVM identifier. |
| `--config-file <path>` | Full configuration as one JSON file. With `--no-api`, no API socket exists. |
| `--log-path <path>`, `--level <level>` | Logger output. The file or pipe must exist first. |
| `--no-seccomp`, `--seccomp-filter <path>` | Seccomp control. The default filter is on. Keep the default. |
| `--boot-timer` | Logs the guest boot time. |
| `--http-api-max-payload-size <bytes>` | Maximum API request size. |
| `--enable-pci` | PCIe support. Not used in 0.1. |

Decision: use the REST API, not `--config-file`. Reasons: the API returns a `fault_message`
for each bad field, the generated client gives typed requests (root rule 5), and pause and
snapshot need the API later.

### 2.2 REST API

Source: `src/firecracker/swagger/firecracker.yaml` (Swagger 2.0, 2259 lines).

Endpoints that 0.1 uses:

| Call | Body type | Notes |
|---|---|---|
| `GET /` | returns `InstanceInfo` | `state` is `Not started`, `Running` or `Paused`. Used to know that the API is up. |
| `PUT /machine-config` | `MachineConfiguration` | Required: `vcpu_count` (1 or an even number), `mem_size_mib`. Set `smt: false`. |
| `PUT /boot-source` | `BootSource` | Required: `kernel_image_path`. Optional: `initrd_path`, `boot_args`. |
| `PUT /drives/{drive_id}` | `Drive` | Required: `drive_id`, `is_root_device`. For virtio-block: `path_on_host`, `is_read_only`. |
| `PUT /vsock` | `Vsock` | Required: `guest_cid` (3 or more), `uds_path`. |
| `PUT /actions` | `InstanceActionInfo` | `action_type`: `InstanceStart`, `SendCtrlAltDel` (x86_64 only) or `FlushMetrics`. |
| `GET /version` | returns `FirecrackerVersion` | Used by the host check. |

A failed call returns status 400 with `Error { fault_message }`. A good `PUT` returns 204
with no body.

Endpoints that exist and that 0.1 does not use: `/balloon`, `/cpu-config`, `/enclave`,
`/pmem`, `/logger`, `/metrics`, `/mmds`, `/entropy`, `/serial`, `/hotplug/memory`,
`/network-interfaces/{id}`, `/snapshot/create`, `/snapshot/load`, `/vm` (pause and resume),
`/vm/config`.

`/enclave` belongs to "Nitro Sandboxes", a new mode that does not use KVM
(`docs/nitro-sandboxes/`). It is out of scope.

### 2.3 Guest boot

- Kernel: an uncompressed ELF `vmlinux` on x86_64, a `PE` `Image` on aarch64
  (`docs/rootfs-and-kernel-setup.md`).
- Recommended boot arguments: `console=ttyS0 reboot=k panic=1`
  (`docs/getting-started.md` line 268; `docs/kernel-policy.md` lines 193 to 196).
  `reboot=k` makes a guest reboot stop the VM. `panic=1` makes a kernel panic reboot after
  one second.
- initramfs (`docs/initrd.md`): a `cpio` archive in `newc` format with the init program at
  `/init`. Notes from that file:
  - Do not set `is_root_device: true` on a drive when an initramfs is in use.
  - The init program cannot `pivot_root`, because the initramfs is the kernel rootfs. It
    must use the `switch_root` method: move the new root mount to `/` and `chroot`.
- Guest stop: the vCPU loop handles `KVM_SYSTEM_EVENT_RESET` and
  `KVM_SYSTEM_EVENT_SHUTDOWN` (`src/vmm/src/vstate/vcpu.rs` line 480). So a guest
  `reboot()` or power-off stops Firecracker on aarch64 too.
- Process exit codes (`src/vmm/src/lib.rs` line 183, `FcExitCode`): 0 success, 1 generic
  error, 148 bad system call (seccomp), 152 bad configuration, 153 bad arguments, 149 to
  157 for fatal signals. The guest cannot set this code.

### 2.4 vsock

Source: `docs/vsock.md`.

- The document lists the host kernel option `CONFIG_VHOST_VSOCK`. Its design section says
  that Firecracker implements the device in user space and bypasses the vhost code of the
  host. So we do not make this option a host check. The guest needs
  `CONFIG_VIRTIO_VSOCKETS=y`.
- Guest to host: the guest connects to CID 2 and port `P`. Firecracker connects to the UDS
  `<uds_path>_P`. The host must listen there before the guest connects. If nothing
  listens, the guest gets a reset.
- Host to guest: the host connects to `<uds_path>`, writes `CONNECT P\n`, and reads
  `OK <port>\n`.
- Ports from 2^30 to 2^31 are reserved. Do not listen on `<uds_path>_P` for such a `P`.
- Firecracker creates `<uds_path>` when the VM starts. The path must not exist before.
- A snapshot restore resets vsock connections. This matters when snapshots arrive later:
  the agent must connect again.

Decision: the guest connects (port 1024). Reasons: the accept is the ready signal, there is
no text handshake, and libkrun supports the same direction.

### 2.5 CI kernel configuration

Source: `resources/guest_configs/microvm-kernel-ci-x86_64-6.1.config`. These are built in
(`=y`): `VSOCKETS`, `VIRTIO_VSOCKETS`, `BLK_DEV_INITRD`, `RD_GZIP`, `VIRTIO_BLK`,
`VIRTIO_NET`, `EXT4_FS`, `SQUASHFS`, `OVERLAY_FS`, `TMPFS`, `DEVTMPFS`, `DEVTMPFS_MOUNT`,
`SERIAL_8250_CONSOLE`, `FUSE_FS`, `IP_PNP`. Configs exist for kernels 5.10, 6.1 and 6.18 on
both architectures.

### 2.6 Where the CI kernel and rootfs come from

Source: `docs/getting-started.md`, lines 108 to 140. The bucket is
`https://s3.amazonaws.com/spec.ccfc.min`. The prefix is
`firecracker-ci/<YYYYMMDD>-<hash>/<arch>/`. Files: `vmlinux-<version>` and
`ubuntu-<version>.squashfs`. The document finds the newest prefix with a bucket list call.
Feature 03 pins one prefix and one SHA-256 for each file; it does not list the bucket at run
time.

### 2.7 Things we do not use in 0.1

- `jailer` (`docs/jailer.md`): needs root. It gives a chroot, cgroups and namespaces to the
  VMM process.
- Network (`docs/network-setup.md`): needs a tap device, thus root or `CAP_NET_ADMIN`.

## 3. libkrun 1.x

### 3.1 The C API

Source: `include/libkrun.h` on `stable-1.19.x` (1457 lines). The line numbers below are from
that file.

| Function | Line | Use in this spec |
|---|---|---|
| `krun_set_log_level(level)` | 28 | Runner. |
| `krun_create_ctx()` | 74 | Returns a context ID, or a negative `errno`. |
| `krun_set_vm_config(ctx, vcpus: u8, ram_mib: u32)` | 98 | |
| `KRUN_FS_ROOT_TAG` = `"/dev/root"` | 104 | The tag of the root virtio-fs device. |
| `krun_set_root(ctx, path)` | 119 | Not used. We use `krun_add_virtiofs3` with the root tag to set the read-only flag. |
| `krun_add_virtiofs3(ctx, tag, path, shm_size, read_only)` | 364 | Root and extra host directories. |
| `KRUN_TSI_HIJACK_INET`, `KRUN_TSI_HIJACK_UNIX` | 395 | TSI feature bits. |
| `krun_set_port_map(ctx, port_map[])` | 605 | `NULL` exposes every guest listening port on the host. An empty list exposes none. We pass an empty list. |
| `krun_set_workdir(ctx, path)` | 833 | Not used. The agent sets the work directory for each command. |
| `krun_set_exec(ctx, exec_path, argv[], envp[])` | 851 | Starts the agent. `envp = NULL` copies the host environment. We never pass `NULL`. |
| `krun_add_vsock_port(ctx, port, path)` | 929 | Guest connect on `port` goes to the host UDS `path`. |
| `krun_add_vsock_port2(ctx, port, path, listen)` | 943 | `listen = true`: the VMM listens on `path`, and the host connects. |
| `krun_add_vsock(ctx, tsi_features)` | 966 | Needs `krun_disable_implicit_vsock` first. `0` means no TSI. |
| `krun_get_shutdown_eventfd(ctx)` | 980 | macOS on aarch64 only. Not used. |
| `krun_set_console_output(ctx, path)` | 996 | Writes the implicit console to a file. |
| `krun_has_feature(feature)` | 1132 | Host check. |
| `krun_get_max_vcpus()` | 1140 | Host check. |
| `krun_fs_add_overlay_file(ctx, fs_tag, path, data, len, mode, one_shot)` | 1238 | Puts the agent in the root file system. The data is not copied. |
| `krun_disable_implicit_vsock(ctx)` | 1279 | |
| `krun_start_enter(ctx)` | 1451 | Never returns on success. |

All functions return 0 or a positive value on success and a negative `errno` on failure.

### 3.2 `krun_start_enter`

Source: header lines 1425 to 1451.

- It returns only on a configuration error (`-EINVAL`).
- On success the VMM calls `exit()` with the exit code of the guest program.
- Reserved exit codes from the built-in init: 125 (init cannot set up the guest), 126 (the
  program exists and cannot run), 127 (the program does not exist).
- The VMM takes over standard input and output.

### 3.3 vsock port forward

Source: `src/devices/src/virtio/vsock/muxer.rs`, lines 540 to 575, and
`muxer_thread.rs`, lines 172 to 200.

- A guest connection request to a port in the map with `listen = false` makes a
  `UnixProxy` that connects to the host path.
- A port with `listen = true` gets a reset when the guest connects to it. The VMM made a
  listening socket on the host path at start (`create_lisening_ipc_sockets`).
- So `krun_add_vsock_port(ctx, 1024, path)` gives the direction that we want. No text
  handshake exists on this path.

### 3.4 Built-in init and standard streams

Source: header lines 1291 to 1340.

- libkrun injects its own init (`/init.krun`) as PID 1. It mounts `/proc`, `/sys`, `/dev`
  and starts the program from `krun_set_exec`.
- When the standard file descriptors of the VMM process are not terminals, the console
  device gets extra ports (`krun-stdin`, `krun-stdout`, `krun-stderr`), and the init
  connects the program to them.
- So the standard output and standard error of the agent arrive on the standard output and
  standard error of the runner process. The backend sends both to `console.log`.

### 3.5 Build and install

Source: `README.md`, `docs/building.md` on `main`; the 1.x flow is the same.

- Linux: needs `libkrunfw` (the guest kernel library). Fedora has packages `libkrun` and
  `libkrunfw`. Other distributions build from source: `make && sudo make install`.
- macOS: needs macOS 14 or newer on Apple silicon. Homebrew tap `slp/krun` has `libkrun`
  and `libkrunfw`. Libraries are in `/opt/homebrew/lib`.
- macOS: the program that loads libkrun must be signed with the entitlement
  `com.apple.security.hypervisor` (`hvf-entitlements.plist`). Ad hoc signing is enough:
  `codesign --entitlements <plist> --force -s - <program>`.
- The crate `krun-sys` (in the 1.x tree, version 1.11.1) uses `bindgen` and `pkg-config`
  at build time. We do not use it: it makes every build need libkrun and `clang`. The
  runner loads the library at run time with `libloading`.

### 3.6 libkrun 2.0 (`main`)

Source: `README.md`, `docs/vm-lifecycle.md`, `docs/workloads.md`, `docs/init.md`.

- The README says that `main` is libkrun 2.0, that it is not compatible with 1.x, and
  that the API can still change. It tells production users to use a `stable-*` branch.
- New model: `Payload` (kernel source), `krun_init_blob::Config` (init arguments),
  `FsOverlay`, `MmioDeviceManager` (devices), `VmmBuilder`, then `Vmm::run()`.
- New: `Vmm::handle()` gives a `VmmHandle` for another thread. Pause and resume work on
  macOS. Shutdown works on macOS aarch64. These return `FeatureDisabled` on Linux.
- New: a Windows backend (`src/whp`) is "upcoming".
- The same concepts that we use exist in 2.0: `FsDevice` with an overlay, `VsockDevice`
  with `TsiFlags`, port forward to a UDS. A port of `bin/sandbox_krun` is small.

### 3.7 Security notes

Source: `docs/security-model.md`.

- The VM boundary does not limit what a device gives to the guest. virtio-fs gives the
  guest every file that the VMM process can read under the shared path. TSI gives the
  guest the network of the VMM process.
- The document tells embedders to run the VMM in host namespaces with its own user ID.
  That hardening is out of scope for 0.1 and is in the backlog (requirements section 2.2).
- TSI with `KRUN_TSI_HIJACK_UNIX` lets the guest connect to host UDS paths. We never set
  this bit.

## 4. Cloudflare

### 4.1 Model

Source: `sandbox-sdk/README.md`, `docs/architecture.md`, `examples/minimal/`.

- A sandbox is a DO and its container. The DO calls `this.ctx.container`.
- Calls seen in the examples: `container.running`, `container.start({ image, enableInternet })`,
  `container.images.<name>`, `container.exec(argv, { cwd, signal, stderr })`,
  `process.output()`, `container.getTcpPort(port).fetch()`, `container.destroy()`,
  `container.snapshotContainer({})`.
- `wrangler.jsonc` declares the container: `containers[].class_name`,
  `scheduling_policy: "durable_object"`, `images.<name>.dockerfile`, and a DO binding.
- Version 1.0 of `@cloudflare/sandbox` has only `Files`, `S3Mount` and `DirectoryBackup`.
  Version 0.x had a `Sandbox` class with a server in the container; that design is gone.
- `Files` works by `exec()` of a static helper (`sandbox-shim`) for each operation. The
  frame format is in `docs/shim-protocol.md`. We do not use it: our agent does file
  transfer on all backends.

### 4.2 Facts to measure at the start of feature 08

The examples prove `getTcpPort(port).fetch()`. They do not prove a raw TCP `connect()` from
the DO to a container port. Feature 08 measures this first and has a written fall-back.

### 4.3 This repository

- `backends/foundation_cloudflare_workers` has generated calls for
  `GET /accounts/{account_id}/containers` and the registry credentials call (lines 8469
  to 8550 of `src/workers/mod.rs`). They list and authorise; they do not start a
  container.
- `backends/foundation_deployment_cloudflare` deploys Workers.

## 5. This repository: patterns to follow

| Need | Where to look |
|---|---|
| HTTP on a UDS | `foundation_netio::HttpClientBuilder::new().unix_socket(path).build()` gives a `DynNetClient` (`foundation_deployment_docker/src/client/mod.rs`, line 11). |
| Generated request functions | `foundation_deployment_docker/src/generated/`, made by `gen_api` (`specifications/completed/54-foundation-deployment-docker/decisions/03-type-replication-strategy.md`). |
| Swagger 2.0 to OpenAPI 3 | `foundation_deployment_docker/specs/swagger2openapi.py`. The Firecracker file is also Swagger 2.0. |
| Limits as configuration | `DockerLimits` in `foundation_deployment_docker/src/client/mod.rs`, lines 55 to 140. |
| A frame decoder with a cap | `foundation_deployment_docker/src/streaming/decoder.rs` (`LogFrameDecoder`). |
| UDS listener and stream with readiness | `foundation_nativeapis/src/native/net/unix.rs`; `foundation_netio/src/native/connection.rs` (`Listener`, `Connection::connect_unix`, `register_waker`). |
| A `Provider` implementation | `foundation_deployment_platform/src/providers/docker/mod.rs`. |
| A sync wrapper on an async call | `foundation_core::valtron::block_on_future` (`executors/sendables.rs`, line 1004). |
| `#[task]` state machines | `.agents/skills/rust-valtron-usage/skill.md`, section "State Tasks". |

## 6. Design dead ends

| Idea | Why we did not use it |
|---|---|
| Call libkrun in the caller process | `krun_start_enter` calls `exit()`. It would stop the caller. |
| Link `krun-sys` | Every build would need libkrun, `pkg-config` and `clang`. |
| Put the agent in the user rootfs image | It changes user data, and each image needs a rebuild when the agent changes. |
| Host connects to the guest on vsock | It needs a retry loop until the guest listens, and the two VMMs use different handshakes. |
| Firecracker `--config-file --no-api` | No typed error for each field, and no path to pause or snapshot. |
| Use the Cloudflare `Files` shim | It is a second file protocol for one backend only. |
| A serial console protocol for exit codes | Not reliable: kernel messages mix with program output on the same stream. |
