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

Decision: the host connects (the slave listens on port 1024 and serves RPC). The control
plane writes `CONNECT 1024\n` after each new connection and reads the `OK` line. When the
slave does not listen yet, Firecracker closes the connection; the control plane's
readiness wait tries again.

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

### 2.7 Things we do not use in this spec

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
- With RPC the slave is the server, so we use `krun_add_vsock_port2(ctx, 1024, path, true)`:
  the VMM listens on `path`, and a host connection goes to the guest port. No text
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
  with `TsiFlags`, port forward to a UDS. A port of `src/bin/sandbox_krun.rs` is small.

### 3.7 Security notes

Source: `docs/security-model.md`.

- The VM boundary does not limit what a device gives to the guest. virtio-fs gives the
  guest every file that the VMM process can read under the shared path. TSI gives the
  guest the network of the VMM process.
- The document tells embedders to run the VMM in host namespaces with its own user ID.
  That hardening is out of scope for 0.1 and is in the backlog (requirements section 2.2).
- TSI with `KRUN_TSI_HIJACK_UNIX` lets the guest connect to host UDS paths. We never set
  this bit.

### 3.8 More facts from the libkrun and libkrunfw sources (2026-11-11)

- **`krun_set_exec` arguments.** `argv` does not include the program name:
  `examples/chroot_vm.c` line 316 passes `guest_argv[0]` as `exec_path` and
  `&guest_argv[1]` as `argv`.
- **The built-in init and a read-only root.** `src/init_blob/init/init.c`,
  `mount_filesystems` (line 536): it calls `mkdir` for `/dev`, `/proc`, `/sys` and accepts
  `EEXIST`; it mounts `devtmpfs`, `proc`, `sysfs`, `cgroup2`; it makes `/dev/pts` and
  `/dev/shm` inside the `devtmpfs` mount. So a read-only root boots when the three
  directories exist.
- **Guest kernel (`libkrunfw`, kernel 6.12.109).** `config-libkrunfw_x86_64` and
  `config-libkrunfw_aarch64` both have `OVERLAY_FS`, `TMPFS`, `TMPFS_XATTR`, `EXT4_FS`,
  `VIRTIO_BLK`, `VIRTIO_FS`, `FUSE_FS`, `EROFS_FS`, `BLK_DEV_LOOP` built in. **x86_64 has
  no squashfs** (`# CONFIG_SQUASHFS is not set`); aarch64 has it. The Firecracker CI
  kernels have squashfs and ext4, and no EROFS. So ext4 is the one image format that every
  microVM kernel of this spec mounts.
- **Block support is a build option.** `Makefile` line 39: `BLK=1` adds `--features blk`.
  `krun_has_feature(KRUN_FEATURE_BLK)` reports it (`src/libkrun/src/lib.rs` line 2052).
  Homebrew builds with `BLK=1 NET=1 GPU=1 TIMESYNC=1` (`homebrew-core`
  `Formula/lib/libkrun.rb` line 74; `slp/homebrew-krun` `Formula/libkrun.rb` line 25). The
  Nix package has `withBlk ? false` (`pkgs/by-name/li/libkrun/package.nix` line 19). The
  Fedora and Arch package files could not be read from this session.
- **File owners on virtio-fs.** macOS: the passthrough stores owner and mode in the
  extended attribute `user.containers.override_stat` (`macos/passthrough.rs` line 37). The
  Linux passthrough has no such override: the guest sees the real host owner. So a host
  directory is not a portable way to keep file owners; an ext4 image is.
- **Disk images.** `krun_add_disk2(ctx, block_id, path, format, read_only)` (header line
  226). The header warns against opening a non-raw image from a source that is not
  trusted, and against format probing. We pass `KRUN_DISK_FORMAT_RAW` always.

## 4. Cloudflare

### 4.1 Model

Sources: `cloudflare/sandbox-sdk` at `61cac81` (`README.md`, `docs/architecture.md`,
`examples/`), `cloudflare/containers` at `a17055c` (`src/lib/container.ts`).

- A sandbox is a Durable Object (DO) and its container. The DO calls `this.ctx.container`.
- Calls seen: `running`, `start({ image, instance, enableInternet, env, entrypoint, labels })`,
  `setInactivityTimeout(ms)`, `monitor()`, `signal(n)`, `destroy()`,
  `getTcpPort(port).fetch(request)`, `exec(argv, options)`, `snapshotContainer({})`.
- Every use of `getTcpPort` in both repositories calls `.fetch`. The architecture document
  says that it streams bodies and carries WebSockets. No source shows a raw TCP socket
  from a DO to a container port.
- A `fetch` to a port with no listener throws an error whose text contains
  `the container is not listening` (`container.ts` line 27, `NOT_LISTENING_ERROR`); the
  library retries on it (lines 927 to 950).
- The image of a container class is in the Worker configuration:
  `containers: [{ image: "./Dockerfile", class_name, name, max_instances }]`, with a DO
  binding and a `new_sqlite_classes` migration
  (`containers/examples/core-tests/wrangler.jsonc`).
- `wrangler dev` runs the container with Docker on the local machine.
- Since version 1.0, `@cloudflare/sandbox` has only file helpers that work through
  `exec()` of a static helper program. The DO class is application code.

### 4.2 The Rust `worker` crate (0.8.5 in the lock; 0.8.7 is the target)

Observed on 0.8.5. Re-checked against `worker-0.8.7` on 2026-11-14. `container.rs` and
`fetcher.rs` have the same lines in both versions. `State::container()` moved from line 258 to
line 281.

- `State::container() -> Option<Container>` (`src/durable.rs` line 281 in 0.8.7; line 258 in
  0.8.5).
- `Container`: `running()`, `start(Option<ContainerStartupOptions>)`, `wait_for_exit()`,
  `destroy(Option<&str>)`, `signal(i32)`, `get_tcp_port(u16) -> Result<Fetcher>`
  (`src/container.rs` lines 13 to 41).
- `ContainerStartupOptions`: `entrypoint`, `enable_internet`, `env`. No image, no instance
  type, no inactivity time-out (lines 86 to 112).
- `Fetcher::fetch` and `fetch_request` (`src/fetcher.rs` lines 23 and 48).
- So a Rust DO can start its container and call a port with `fetch`. This is why the
  control plane can run in a DO with no TypeScript.

### 4.3 This repository

- A Rust DO exists: `backends/foundation_keychain/src/server/signalr_do.rs`, on helpers in
  `backends/foundation_deployment_cloudflare/src/workers/`.
- A Worker serves HTTP through `foundation_http::wasm` (`CfHttpAppSingleton`,
  `serve_cf::CfServe`, `dispatch`); `examples/cf-agent-loop/src/lib.rs` is the pattern,
  with its own workspace root and `worker-build`.
- `foundation_connectrpc`: the router and the client are in `shared/` (all targets). The
  `fetch` client transport is only a capability constant today
  (`shared/transport/wasm.rs`: no request streaming, no full duplex). Spec 83 feature 05
  plans a general one.
- `backends/foundation_cloudflare_workers` has generated calls for
  `GET /accounts/{account_id}/containers` and the registry credentials call. They list and
  authorise; they do not start a container.

### 4.4 The Cloudflare Sandbox product (read again on 2026-11-12)

Source: `cloudflare/sandbox-sdk` at `61cac81` (`README.md`, `docs/architecture.md`,
`images/sandbox-tools/Dockerfile`, `examples/artifact-workspace/Dockerfile`).

- The product is not a service with its own API. "A sandbox is a Durable Object and its
  container." The application writes the DO class and calls `this.ctx.container`.
- The package has no `Sandbox` base class. It does not start, stop, wake or destroy a
  container.
- The package adds `Files`, `S3Mount` and `DirectoryBackup`. Each `Files` call starts one
  short `sandbox-shim` process with `exec()`. The shim is not a daemon.
- `sandbox-shim` is a static program for `linux/amd64`. The donor image is
  `docker.io/cloudflare/sandbox:<version>`. An application image copies the file to
  `/usr/local/bin/sandbox-shim`. Its tag must be equal to the package version.
- The example images end with `CMD ["sleep", "infinity"]`. The DO starts programs.
- Result for this spec: the product is the same primitive that feature 07 uses. Support
  for it is image compatibility and a plain HTTP way in (feature 17). No driver is added.

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
| A fd that a valtron task parks on | `foundation_nativeapis::native::fd::RegisteredFd` implements `EventReadiness`; a task returns `TaskStatus::Depends(fd)` (`native/fd/mod.rs`, module documentation). Shared reactor: `Reactor::get()?.registry()`. |
| A Connect RPC service, code-first | `#[service(package = "...", codecs(json))]` on a trait: `backends/foundation_connectrpc/examples/code_first_streaming/main.rs`. |
| An RPC server on a custom listener | `HttpServer::serve_with_acceptor` (`foundation_http/src/native/server/mod.rs` line 539) with a `foundation_netio::native::connection::Acceptor`. |
| An RPC client transport | The `Transport` trait, `foundation_connectrpc/src/shared/transport/base.rs` line 161; `H1Transport::new(DynNetClient)`. |
| An in-process RPC pair for tests | `PipeClientConn` and `PipeHandlerConn`, `foundation_connectrpc/src/shared/transport/conn.rs`. |
| Signals in a native program | `foundation_nativeapis`, features `signal` and `signal-unix`. |
| KVM in CI | GitHub-hosted `ubuntu-latest` has `/dev/kvm`; `.github/workflows/platform-images-vm.yaml` uses it. |

## 6. Measured on 2026-11-11

| Command | Result |
|---|---|
| `cargo check -p foundation_nativeapis --no-default-features --features fd --target x86_64-unknown-linux-musl --profile uat` | Passes. The readiness layer builds for a static slave. |
| `cargo check -p foundation_deployment_platform --target x86_64-unknown-linux-musl --profile uat` | Fails: `ring` (pulled by `foundation_netio` with `ssl`) needs a musl C compiler. So `agent-slave` is its own package (feature 06, section 6.1). |
| `cargo tree -p foundation_deployment --depth 1 -e normal` | Depends on `foundation_db`. So the generated Firecracker client's common types move to a small crate before the slave can hold the Firecracker driver (feature 11, section 6.5). |
| `ls /dev/kvm` in the specification session | Not present. No microVM was run while this spec was written; every microVM fact is from source code and documents. |

## 7. Design dead ends

| Idea | Why we did not use it |
|---|---|
| Call libkrun in the caller process | `krun_start_enter` calls `exit()`. It would stop the caller. |
| Link `krun-sys` | Every build would need libkrun, `pkg-config` and `clang`. |
| Put the slave in the user rootfs image | It changes user data, and each image needs a rebuild when the slave changes. |
| An own frame protocol with channels and credit windows (first design of this spec) | The repository has Connect RPC. A Worker can only `fetch`, which fits unary and server-streaming calls, and each call's HTTP stream gives the back pressure. One contract then works on a Unix socket, on vsock and on `fetch`. |
| A TypeScript bridge Worker that pipes a WebSocket to the container (first design) | The Rust `worker` crate has the container API, so the control plane itself runs in the DO, in Rust, with the same core. |
| The guest connects to the host on vsock (first design) | With RPC the slave is the server. The control plane dials and retries until `Info` answers; the same loop serves every substrate. |
| A slave on `std` threads (first design) | The owner wants valtron everywhere; the readiness layer builds for musl, so there is no reason for an exception. |
| A root directory as the image form on libkrun | File owners do not survive on a Linux host (section 3.8). |
| squashfs as the common image form | The `libkrunfw` x86_64 kernel has no squashfs (section 3.8). |
| `mkfs.ext4 -d <directory>` to fill an image | It copies the host owner of each file. The guest fills the image as root (feature 12). |
| Firecracker `--config-file --no-api` | No typed error for each field, and no path to pause or snapshot. |
| The Cloudflare `Files` helper | A second file protocol for one substrate only. |
| A serial console protocol for exit codes | Not reliable: kernel messages mix with program output on the same stream. |

**Wrong reading of two system names (2026-11-12).**

The first drafts read "Docker sandbox" and "Cloudflare sandbox" as "the control plane runs
in Docker or on Cloudflare". The owner meant the two products. Root cause: the spec did
not look for products with those names before it chose a meaning. The fix is requirements
section 2.3 and features 16 and 17.

## 8. Docker Sandboxes (the product, read on 2026-11-12)

Source: the `docker/docs` repository (a sparse clone; `docs.docker.com` was not reachable
from the session). Paths are under `content/manuals/ai/sandboxes/` unless they start with
`data/` or `content/reference/`.

### 8.1 The product

- The program is `sbx`. The older `docker sandbox` command is deprecated. The daemon is
  `sandboxd` (`release-notes.md`, `mcp-gateway.md`).
- `sbx` needs no Docker Desktop and no Docker Engine. Local sandboxes need KVM on Linux
  (Ubuntu 24.04 or later, user in the `kvm` group), macOS 14 on Apple silicon, or
  Windows 11 with the Windows Hypervisor Platform (`install.md`).
- Each sandbox is a microVM with its own Docker daemon, image cache and packages. It
  stays until `sbx rm` (`architecture.md`).
- All outbound traffic goes through a proxy on the host. The proxy applies the network
  policy and puts credentials into requests (`architecture.md`).
- A sign-in is necessary. In CI: `sbx login --username <id> --password-stdin` with a
  personal access token (`workflows/automation.md`).

### 8.2 The commands that the driver uses

- `sbx create AGENT|KIT [PATH...]` with `--name` (2 to 63 characters: letters, numbers,
  hyphens, periods), `--cpus`, `--memory` (minimum 512 MiB), `--template` (an image),
  `--publish`, `--env`, `--no-progress` (`data/sbx_cli/sbx_create.yaml`).
- From version 0.42.0 a create with no path gives a sandbox with no host directory. The
  work directory is the image's `WORKDIR` (`architecture.md`).
- The agent `shell` has no agent program; the sandbox runs `bash -l` (`agents/shell.md`).
- `sbx exec [-i] [-t] [-u user] [-w dir] [-e K=V] SANDBOX COMMAND...`. No detach flag.
  It returns the command's exit status; 125 when `sbx` cannot run it; 127 for a missing
  program (`data/sbx_cli/sbx_exec.yaml`, `workflows/automation.md`).
- `sbx cp SRC DST`; the sandbox side is `NAME:/absolute/path` (`usage.md`, line 421).
- `sbx ports NAME --publish [[HOST_IP:]HOST_PORT:]SANDBOX_PORT`; with only the sandbox
  port, the system picks the host port. `sbx ls` shows `127.0.0.1:8080->3000/tcp4`
  (`usage.md`, lines 439 to 483).
- From version 0.48.0 the structured output changed: `sbx ls --json` is an array; port
  rows have `host_ip`, `host_port`, `sandbox_port`, `protocol` (a cloud row has
  `sandbox_port` and `url`); `sbx rm` and `sbx stop` give an array of results with
  `action`, `resource`, `name`, `id`, `error`. Names are snake case. Results go to
  `stdout`; errors and progress go to `stderr` (`workflows/automation.md`).
- A confirmation needs a terminal. `--force` skips it (`workflows/automation.md`).
- The Docker images run as the user `agent` with `sudo`
  (`customize/author/base-images.md`).

### 8.3 The HTTP API (not used)

Source: `content/reference/api/sandboxes/api.yaml` (OpenAPI 3.2, "Docker Sandboxes API",
version v1; its home is `docker/sandboxes-api`).

- The management address is `https://connect.docker.com/sandboxes` (the Docker cloud),
  with a bearer token.
- Process and file calls go to the sandbox endpoint (`core.endpoint.uri`) with a
  short-lived credential from `createEndpointCredential`. The protocol of an endpoint is
  `http` or `unixSocket`; a local socket uses file permissions and no token.
- Calls: `createSandbox`, `getSandbox`, `deleteSandbox`, `stop`, `start`, `exec` (runs
  to the end, output in the answer), `createProcess`, `interact` (a WebSocket),
  `readOutput` (pages by a sequence number), `signal`, file read, write and upload,
  ports, snapshots, volumes.
- The documents give no path for a local socket. So the driver uses the `sbx` program.
  A cloud sandbox publishes a port as a public HTTPS address; the slave link has no
  authentication, so the cloud is not in this spec.

### 8.4 Not known until a real run

- Whether the process that `sbx exec` started stays when the `sbx exec` client stops.
  The design of feature 16 is correct for both answers (`restore` checks `Info`).
- Which template name is the best default. Candidates from the documents:
  `docker.io/sbx/shell-image:latest` and `docker/sandbox-templates:claude-code-minimal`.

## 9. The Cloudflare merge (2026-11-14)

### 9.1 The conflict

Spec 83 feature 06 (then "Cloudflare Deployment") and this spec said opposite things:

| Question | Spec 77 | Spec 83 feature 06 |
|---|---|---|
| Who owns Cloudflare deployment? | The manual `npx wrangler deploy` of the operator (feature 08, section 6.7). A deploy from Rust is out of scope. | `foundation_deployment_cloudflare` and its generated types (`ewe_deployables::cloudflare` was removed in spec 70 feature 22). |
| Who owns the container surface? | The private `ContainerApi` of the Cloudflare DO (feature 07, section 6). | A public `ContainersDeploy` in `foundation_deployment_cloudflare`. |
| Is there a handle for the container? | No. The `SandboxHandle` is the only handle. | No. `ContainersDeploy` takes a `ContainerSpec` with undefined fields. |

Both texts were correct about one artifact and wrong about the other. The owner merged them.

### 9.2 What the Cloudflare Rust API can and cannot do

Checked against `worker` 0.8.7 in the `cloudflare/workers-rs` source (2026-11-13):

- `State::container()` returns a `Container` (`durable.rs:281`).
- `Container` has `running`, `start`, `wait_for_exit`, `destroy`, `signal` and
  `get_tcp_port` (`container.rs:13-41`).
- `ContainerStartOptions` has `entrypoint`, `enable_internet` and `env`
  (`container.rs:86-112`). It has no image field and no instance type.
- The image comes from `wrangler.toml` `containers[].image`. So a deploy from Rust must
  write the Worker configuration, not call a container "create" API.
- A port fetch before the server listens fails with text that contains
  `the container is not listening`.
- The container architecture is `linux/amd64`.
- There is no exit code and no log call through `Container`.

Consequence: the container handle on Cloudflare is truthful about its limits. It reports
`exec: true` only when the sandbox slave endpoint exists. It reports `logs: false`. It
cannot fetch a port by itself, because the slave owns that path.

### 9.3 Why one handle, not two

The `SandboxHandle` is the unit that runs programs. A container is a possible substrate under it.
Two unconnected handles would let a caller start a container with no sandbox record, or
destroy the substrate under a live sandbox. So the two are joined:

- `ContainerControl::start` creates a container and returns a handle. It creates no sandbox
  record. A caller that only needs a container uses this.
- `SandboxHandle::container()` and `SubstrateHandle::container()` return the container of a live
  sandbox, or `None`. A sandbox caller never builds a second handle.
- `ContainerHandle` methods that change state go through `ContainerOps`. On Cloudflare, the
  DO is the single writer (invariant I12). `destroy` on the substrate handle and on the
  sandbox handle reach the same container, and the second call is idempotent.

So there is one container type and one set of capability rules. The sandbox is the owner
when a sandbox exists; the standalone control is the path when it does not.

### 9.4 Why both deploy paths stay

The Rust deployer is the primary path because it is typed, testable and it records the
plan. The `wrangler` path stays supported because:

- The Cloudflare Worker configuration holds fields that a new API version can change. The
  `wrangler` program follows the API.
- An operator can deploy by hand when the Rust client is out of date or when a CI job has
  no credentials.
- The Cloudflare Sandbox product documents its own `wrangler.product.toml`. Feature 17
  keeps that file as the manual adapter's view.

Both adapters take the same `DeploymentPlan`, so a test proves parity: the plan that
`FoundationCloudflareDeployer` applies and the `wrangler.toml` that
`WranglerCloudflareDeployer` writes must carry the same bindings, migrations, container
class, routes and secret names.

### 9.5 Dead end: a second container service

The first design gave the Cloudflare container handle its own HTTP service in the Worker.
The DO already runs the control service that the sandbox slave talks to. A second service
would add a second token, a second route and a second place to start a container. The
feature uses the existing service with new methods instead (invariant I14).


## 10. The handle doctrine and the six-conflict review (2026-11-15)

The owner reviewed six conflicts across the plan and gave one direction for each. This section
records the two rules that the review set for this specification.

### 10.1 Every outside caller uses a handle from a wrapper that holds the `Ctx`

Owner direction: "Whether containers (docker, cloudflare,...etc), everything should be through
a handle that whatever central wrapper over Ctx or exposed via Ctx.get provides and whatever is
returned specifically provides the necessary methods to interact with the relevant underlying
infrastructure... whether its using RPC internally or directly talks to the resource is not our
concern. The benefit of this is, once sandbox is build, we just provide the Arc<SandboxProvider>
to agent-rpc/agent-slave and it uses that and thats it, same for Containers and others."

So two levels have the same shape:

| Level | Factory | Handle |
|---|---|---|
| Sandbox | `SandboxProvider` | `SandboxHandle` |
| Container | `ContainerControl` | `ContainerHandle` |

The handles come in a wrapper value, not in a `Ctx` entry and not in a new `Ctx` method. The
owner corrected the first design on 2026-11-15: "we wont go edit foundation_context, but rather
wrap Ctx with whatever the cloudflare part needs that provides the other cloudflare specific
handles", and "same for others".

So each part has one wrapper struct that holds the `Ctx` and the handles of that part:
`NativePlatform` (Firecracker and libkrun), `DockerPlatform` and `CloudflarePlatform`. They
share the `Platform` trait (`ctx`, `sandboxes`, `containers`, `kind`, `as_any`) and a
`PlatformKind` enum, with `as_native`, `as_docker` and `as_cloudflare` helpers for the downcast.
A function for one part takes the concrete type; a function that serves several takes
`&dyn Platform`. A boundary builds the wrapper; `agent-rpc` and `agent-slave` take a reference
to it. They do not start a sandbox, pick a driver or build a container control.

The owner added a third note: "just wrap the js with a SendWrapper to make it look Send, wasm is
non-send anyway". For one pass that note applied to a `RuntimeFetch` field on
`CloudflarePlatform`. The owner then replaced that whole direction (section 10.4): the wrapper
carries no HTTP client at all, because `DynNetClient` and `H1Transport` already work on both
targets. The note still governs the `worker` adapters of feature 07.

The two mistakes that pass made, and the owner caught both:

- `Arc<dyn Fetcher>` does not compile, because `Fetcher` is not `Send`. A JS value crosses a
  `Send` slot only inside `foundation_compact::SendWrapper`.
- The type was first made wasm-only, "because `cloudflare/` is". That is wrong:
  `CloudflareContainerControl` is generic over `ContainerApi` and lives in
  `shared/cloudflare/`, so only `WorkerContainer` and its neighbours are wasm. A native host
  that manages a Cloudflare container needs the same wrapper. Owner rule: "this should properly
  separate whats wasm and native, so the same API services both".
- The deployer was in the wrapper for one pass. It does not belong there: both deployers are
  native values in the deployment crate, and deployment touches no sandbox or container handle.

The lasting rule from that review, and the one a new part wrapper must follow: a `cfg` belongs
on a private inner type, a method, a module or a dependency. It never changes the shape of a
public type.

The same seam applies to every wasm adapter of a `Send + Sync` trait. `ContainerApi`,
`DoStorage` and `DoAlarm` all require both bounds, and their `worker` values are `!Send`, so
`WorkerContainer`, `WorkerStorage` and `WorkerAlarm` hold their values in `SendWrapper` too
(feature 07 section 6.3). Keep one rule: a JavaScript value or future crosses a `Send` slot
only inside `SendWrapper`.

**The same class of error elsewhere (audit 2026-11-15).** The owner asked: "can you check where
else you have made similar mistake". The two shapes of the mistake are (a) a public type whose
shape or bounds change per target, and (b) a `!Send` value in a `Send + Sync` slot. The audit
found and fixed these:

| Place | Defect | Fix |
|---|---|---|
| Spec 77 requirements section 2.4 | `src/cloudflare/` was listed with the driver, the control, the transport, the store and the request handler. Feature 07 puts all of them in `shared/cloudflare/` (`all`). | The row now lists only `WorkerContainer`, `WorkerStorage`, `WorkerAlarm` and `CloudflareHost::from_state`; a new row describes `shared/cloudflare/`. |
| Spec 77 feature 00 | The `cloudflare` feature said "wasm32 only". | It gates `shared/cloudflare/` on every target and the `worker` adapters on wasm only. |
| Spec 85 section 4 (first draft) | `FetchTransport` held `fetcher: Rc<F>` while `Transport` requires `Send + Sync + 'static`, and the whole type repeated `H1Transport`. | The type is gone (2026-11-15). Spec 85 fills the gaps inside `HttpClient` and `H1Transport`, which are `Send + Sync` on both targets. |
| Spec 81 section 4.4 | `KeySource::File(PathBuf)` was a `cfg(not(wasm))` variant, so the public enum changed shape per target. | The variant exists on every target, and a wasm host returns the new `KeyLoadError::NoFileSystem`. |
| Spec 80 feature 01 | `MigrationSource::Directory(PathBuf)` was a `cfg(not(wasm))` variant. | The variant exists everywhere; `plan` reports the new `MigrationProblem::NoFileSystem` on wasm; the readers stay native-only. |
| Spec 83 feature 08 | `SealFuture` was a `Send` alias on native and a `!Send` alias on wasm. | One `Send` alias on every target; a wasm sealer wraps its future in `SendWrapper`. |
| Spec 83 architecture, section 3.2 | `foundation_core` owned the "`Env` bundle" and `Ctx`; both are in `foundation_context`. The `foundation_auth` row called `client_auth` target-neutral, but `client_auth::cloudflare` is wasm and `client_auth::mtls` is native. | Both rows are corrected, and `foundation_context` has its own row. |

Two more bounds were missing rather than wrong, and both now name the bound: `DockerApi` is
`Send + Sync + 'static` because `DockerPlatform` holds it in an `Arc<dyn ..>`, and
`ContainerApi`/`DoStorage`/`DoAlarm` adapters show the wrapper in feature 07 section 6.3.

The lesson: a `cfg` belongs on an inner type, a method, a module or a dependency, never on the
shape of a public type. When a target cannot have a value, make the value-unconstructible inner
type private, keep the outer type the same, and name the missing capability in a typed error.

The reason is a boundary rule, not a style rule. Without a shared value, every consumer repeats
the host detection, the driver order and the container control construction, and each copy can
pick a different driver for the same host. With the wrapper, the boundary makes that decision
once.

Two dead ends, recorded so a later pass does not repeat them:

- A `Platform` entry in the `Ctx` (`Ctx::insert::<Platform>(..)`) needs an extension trait for
  the read side, because `foundation_context` must not depend on this crate. The trait works,
  but it puts a look-up that can fail in the path of every handle use, and it invites a second
  entry for each part. The wrapper is a value the compiler checks.
- Putting the `Ctx` inside the entry makes a cycle: the entry lives in the `Ctx`. The wrapper
  holds the `Ctx`, so the wrapper cannot be an entry.

### 10.2 One owner for each shared block

The review found three blocks planned in two or three specifications at once: the wasm fetch
pump, the secret and bearer check, and mutual TLS. Each block is now in one specification that
lands before its consumers:

| Block | Owner | Consumers here |
|---|---|---|
| The wasm gaps: the streaming wasm client, the streaming bridges and sink, `StubHttpClient` | Spec 85 | Features 07 and 08 |
| `Secret`, `constant_time_eq`, the bearer parser | Spec 86 | Features 00 and 02 |
| The CA, the trust store, the verifiers, the mTLS providers | Spec 87 | Feature 06 (native listener), question Q4 |
| The `Env` entry and the `IdSource` trait | Spec 84 | Feature 02 (the id policy) |

The lesson repeats the one from the `Env` split: a shared block that two specifications plan
is a block that two implementations will write. One owner with a date is cheaper than a
reconcile.

### 10.3 Why the names changed

| Old | New | Reason |
|---|---|---|
| `SandboxControl` | `SandboxProvider` | "Control" names the lower level in the container rows (`ContainerControl`). The sandbox level is the provider of sandboxes. |
| `Sandbox` | `SandboxHandle` | The struct is the live object, not the concept. The handle doctrine names it that way. |
| `SandboxProvider` (feature 06) | `SandboxDeploymentProvider` | It would clash with the control trait. It is a `Provider` facade for deployment, and its handle is `SandboxDeploymentHandle`. |
| `IdSource` (feature 02 enum) | `SandboxIdPolicy` | `IdSource` is the spec 84 trait. Two types with one name across specifications is the defect the review removed. |
| `check_bearer` | `authorize` over spec 86's `Secret` | The bearer parse and the constant-time compare are primitives, not sandbox code. |

### 10.4 The HTTP client is `DynNetClient`, and nothing else (2026-11-15)

The owner asked three times, in stronger words each time: "if it is using API should it not use
our Http api that properly abstract out wasm/web fetch vs native http as well? the `DynHttpClient`
trait from foundation_netio or is this something else?", then "there is a `DynHttpClient` which
encapsulates http across native, wasm, use that", then "the core idea is `DynNetClient` is the
platform agnostic way", and "and connectrpc already builds on it".

The final answer is that the workspace has one client abstraction, and this specification must
hold it rather than wrap it:

| Layer | Type | File |
|---|---|---|
| The trait | `HttpClient: Send + Sync` | `foundation_netio/src/shared/client/http_client.rs:65` |
| The handle | `DynNetClient = Arc<dyn NetClient>` | `foundation_netio/src/network_client.rs:41` |
| The choice per target | `HttpClientBuilder::build()` | `foundation_netio/src/network_client.rs:440` |
| The RPC transport | `H1Transport`, in `shared/`, on both targets | `foundation_connectrpc/src/shared/transport/h1.rs:61` |

There is no `DynHttpClient` type. The names are `HttpClient` and `DynNetClient`; the owner means
the abstraction, and it is these two.

The first answer here was wrong. It said `RuntimeFetch` held the raw JavaScript `fetch` because
`HttpClient` buffers a wasm response (`wasm/client/client.rs:334`) and so cannot carry a streamed
RPC response. That observation is true and it is issue #557, but the conclusion was not: the fix
belongs inside `FetchHttpClient`, which is where every wasm caller already goes. Adding a second
client above it gave a caller two ways to send a request and a second place to keep honest.

So, on 2026-11-15, `RuntimeFetch` is removed:

- `CloudflarePlatform` holds the `Ctx`, `sandboxes`, `containers` and `control`. It carries no
  HTTP client and no `fetch` field.
- A caller that sends a request holds a `DynNetClient`. On wasm that is `FetchHttpClient`, from
  `HttpClientBuilder::build()`.
- An RPC call is `H1Transport::new(client)`, on every host.
- `ContainerPortTransport` is a thin wrapper over `H1Transport` over a port `NetClient`
  (feature 07 section 6.5).
- The Worker entry is `foundation_http`'s `HttpApp<Arc<dyn CfServe>>` (feature 08).
- The Worker's calls to a Durable Object are `H1Transport::new(StubHttpClient)` (spec 85
  section 4.5).

The same mistake had a second form, in this specification's own first draft: it planned a body
pump of about 80 lines for the container port, with the sentence "It does not wait for it". That
sentence is the defect. `H1Transport` already streams a request body on wasm and a response body
after spec 85 T1, so the pump has nothing left to do.

The lesson, for this spec and for the next one: when a plan is about to add a "platform seam",
first list the types the workspace already has for that job. Here every row of that list already
existed, and three specifications had planned the same pump. The remaining real work was four
small gaps inside `foundation_netio`, `foundation_http` and `foundation_connectrpc`, which is
what spec 85 now contains.

The two defects of the client itself are recorded, not hidden:

- Issue #557 (high): the wasm `HttpClient` reads whole bodies. Spec 85 T1 fixes it.
- Issue #558 (medium): `WASM_FETCH_CAPABILITIES` says the wasm client cannot stream a request
  body, but it can. Spec 85 T2 corrects the constant.

## 11. Feature 00 implementation findings (2026-11-24)

- A trait object of `StreamIterator` needs the supertrait associated type, so
  `Box<dyn StreamIterator<D = D, P = P>>` does not compile. `StreamIterator` has a
  blanket implementation for every `Iterator<Item = Stream<D, P>>`. The boxed form is
  `Box<dyn Iterator<Item = Stream<D, P>> + Send + 'static>`. Features 02 and 05 use this
  form.
- Two `#[service]` traits in one module both emit a `procedure` module. Each service
  lives in its own module (`rpc/slave.rs` and `rpc/sandbox.rs`), and `rpc/mod.rs`
  re-exports the names.
- The service macro writes default method bodies that ignore their parameters. A
  module-level `#![allow(unused_variables, reason = "...")]` in each service module keeps
  the `-D warnings` build clean.
- `std::os::fd::OwnedFd` does not exist on `wasm32-unknown-unknown`. The process type set
  and the `SandboxHost.processes` field are `#[cfg(unix)]`.
- `proptest` is one workspace dependency. Each property test file uses one
  `ProptestConfig` with `RngSeed::Fixed(seed)`, so a failure reproduces from its seed.

## Owner corrections (2026-11-25)

- **One crate for the sandbox work.** The owner moved the programs and the provider facades
  into `foundation_sandbox`. The earlier decision (D11) kept `agent-slave` in its own
  `bin/agent_slave` package because `foundation_deployment_platform` does not build for
  musl. That reason is gone: the `slave` feature of `foundation_sandbox` pulls neither a
  TLS library nor the Docker client.
- **A model runtime is a sandbox substrate.** On Cloudflare the slave can be another DO that
  answers `SlaveService` with turns against `OpenRouter` or the Cloudflare Model API
  (decision D16). State its limit in every document: it gives no isolation.
- **A crate rename touches every spec.** 70 files named the old crate. Search the whole
  tree, including the completed specifications, before the first commit.

## 12. Feature 01 implementation findings (2026-11-25)

### 12.1 The TLS-free build opened three dependency layers

The slave builds as a static musl program, so the `slave` feature must not link a C
TLS library. The measurement on 2026-11-11 was correct about `ring`, but four crates
forced it into the graph, not one:

| Crate | What forced TLS | Fix |
|---|---|---|
| `foundation_http` | its `foundation_netio` dependency used the netio default features, and `default` was empty | `netio-core` (no TLS) and `netio-defaults` (TLS); `default = ["netio-defaults"]` |
| `foundation_connectrpc` | the native `foundation_http` and `foundation_netio` dependencies used their defaults | the same two features; `rpc` enables `netio-defaults` |
| `foundation_iogate` | the `foundation_netio` dependency used the defaults | `default-features = false` with `wire-native`, and the same two features |
| `foundation_auth` | the native `foundation_netio` dependency used the defaults, and the spec 87 modules (`rcgen`, `rustls`, `x509-parser`) were unconditional | a `netio-ssl` feature, and a new `mtls` feature that names the spec 87 modules; both are in the default set |
| `foundation_db` | the `foundation_netio` dependency asked for `ssl` by name | a `netio-ssl` feature, enabled by `d1`, `r2` and the default set |

A consumer that must stay free of C code takes each crate with `default-features = false`
and the `netio-core` feature. Every existing consumer keeps today's set, because the new
features are in each crate's default set.

Two more gaps appeared only in the TLS-free build:

- `foundation_netio`'s `http` module had never compiled without a TLS provider: it
  imports `netcap::ssl::SSLConnector`, and `native/ssl/nossl.rs` was an empty file. The
  file now defines a `SSLConnector` type that fails at use, and `http/mod.rs` gates the
  `tls_task` module and its re-export.
- `foundation_connectrpc`'s native modules need netio's `http` module, so the native
  build always turns `netio-core` on. The wasm build must not: `netio-core` enables
  `multi`, which turns the single-threaded wasm netio onto valtron's `Send` path. So the
  `foundation_sandbox` dependency on `foundation_connectrpc` is split into one native
  table and one wasm table.

### 12.2 What `foundation_netio` and `foundation_http` did not have

- `Connection` had no constructor from an accepted descriptor of another address
  family. `Connection::from_stream_fd(OwnedFd)` is new; a vsock stream socket is a
  stream descriptor like a Unix domain socket.
- `Acceptor` is implemented for `TcpListener` only. The slave adds a `UnixAcceptor` in
  `serve.rs`; `local_addr` returns an error for a Unix socket, and the server then uses
  its bind label.
- `HttpServer::serve_with_acceptor` is the path for UDS, TCP and vsock.

### 12.3 The fork must not cross an `await`

A `Vec<*const c_char>` is not `Send`. A future that builds the `execve` pointer lists and
then awaits (the start-status wait) is not `Send`, and the generated service trait needs
`Send`. The fix is a synchronous `fork_process` helper: the pointer lists live across the
`fork` and nowhere else, so the async part holds no raw pointer.

### 12.4 The wake pattern

Every parked task uses one rule: register the waker, then check the condition. This is
the only pattern that loses no wake with a `Mutex<Vec<Waker>>` notifier. The pumps park on
`FdReadiness` (`clear`, check, `register_waker`, check) and on the same notifier for the
"stop" flag. A close of the sink or of an input queue wakes every parked sender and
receiver.

### 12.4.1 The drain wait must not wake on its own condition

The first exit watch put `exit.is_some()` in the condition of its wait, and then
wrapped that wait in a `select` with the drain timer. After the exit the condition was
true at every poll, so the `select` returned at once and the task spun until the
deadline. The condition now names the state that must wake it: before the deadline is
set, the exit; after it, the deadline plus room in the event sink. A receiver that
takes an event notifies the session's change signal through the sink, so the room check
has a wake and is not a poll.

### 12.5 Limits and the server bind

`SlaveLimits` (the `Prepare` message) has no `max_error_message` field, so the slave cuts
an error message with `SandboxLimits::default().max_error_message`. The HTTP/1.1 server
binds before `Prepare`, so its request body limit is the default `max_message_bytes`; the
service checks the limits of `Prepare` for the input chunk size.

`libc::open_how` is `#[non_exhaustive]`, so `sys.rs` defines the three-field kernel layout
itself and passes `RESOLVE_IN_ROOT` to `openat2`.

### 12.6 Test layout and CI

- The crate has one test binary (`tests/mod.rs`). The slave tests are the module
  `tests/slave/` (`support`, `info`, `exec`, `input`, `files`, `root`, `lifecycle`), not
  the `tests/slave_*.rs` files of the feature text, which would be a second test binary
  and would restart the whole harness for each group.
- The child slave process is the same test binary started with
  `--exact slave::support::slave_process_entry --test-threads=1`. One thread is necessary:
  `SIGCHLD` must be blocked on the process's only thread before the pool starts, and the
  libtest harness runs a test in a new thread unless it runs one test at a time.
- `ci/packages.toml` runs `["", "slave"]` on Linux. macOS and Windows run `[""]` through
  `macos-runs` and `windows-runs` rather than `macos-tests = "none"`: the contract tests
  still run there, and the reason "the slave is a Linux guest program" is in the comment.

### 12.7 Dead ends

- A stub `SSLConnector` was not enough for the netio `http` module; the `tls_task` module
  also had to be gated. Do not try to compile that module without a provider.
- `std::fs::read_dir("/proc")` for the zombie check needs no `CONFIG_PROC_CHILDREN`; the
  `stat` file of every process is enough. `/proc/<pid>/task/<tid>/children` is not
  portable across the CI kernels.
- `Slave::arc_self` (an `Arc` pointer cast from `&self`) was wrong: an `Arc<'T>` value's
  address is not the address of its data. The shutdown handler captures the pieces it
  needs (`Ctx`, session table, file state, shutdown signal) instead of the slave.

### 12.8 The version bump of the changed libraries

Rule 12 of the root `AGENTS.md` moves the version of a crate that gains public API, and of
every workspace consumer of that crate, in the same change. This feature changes seven
crates:

| Crate | Old | New | Why |
|---|---|---|---|
| `foundation_netio` | 0.0.17 | 0.0.18 | new `Connection::from_stream_fd`, a new `nossl::SSLConnector` type, the `http` module without TLS |
| `foundation_http` | 0.0.15 | 0.0.16 | the `netio-core` and `netio-defaults` features |
| `foundation_connectrpc` | 0.0.16 | 0.0.17 | the same two features |
| `foundation_iogate` | 0.0.6 | 0.0.7 | the same two features |
| `foundation_auth` | 0.0.15 | 0.0.16 | the `mtls` and `netio-ssl` features |
| `foundation_db` | 0.0.10 | 0.0.11 | the `netio-ssl` feature |
| `foundation_sandbox` | 0.0.2 | 0.0.3 | the `native::slave` module and the `SlaveHost` bundle |

`foundation_netio` is not the only crate that needs the new source: `foundation_sandbox`
0.0.3 names the `netio-core` features of `foundation_connectrpc`, `foundation_http` and
`foundation_iogate`, and builds `foundation_auth` and `foundation_db` without their TLS
dependencies. A packaged consumer that resolved the old registry version of any of them
would fail the musl build. So all seven move together, and 77 manifests move their version
requirement. This is a wide but mechanical diff; the pull request's CI covers the affected
packages.

### 12.9 The musl check found two portability facts

- `libc::ioctl` takes an unsigned long request number on the GNU C library and an int
  on musl. `sys.rs` names the type in one place, `IoctlRequest`, with a `target_env`
  branch. A cast at each call is not portable and a truncating cast fails clippy.
- `cargo check` for a musl target prints `dropping unsupported crate type cdylib` for
  `foundation_nativeapis`, which declares `["rlib", "cdylib"]`. The warning is old and
  does not affect the slave, which links the rlib. The feature's musl check is the first
  build that shows it.

### 12.10 The `unsafe` placement rule

Feature section 6.2 allows `unsafe` only in `sys.rs` and the `fork` child block of
`exec.rs`. The first commit also held raw `libc` calls in `serve.rs` (vsock socket, bind,
listen, `accept4` and `fcntl`) and in `pid1.rs` (the console write of a failed power off).
They are now `sys::vsock_listen`, `sys::accept_stream`, `sys::set_nonblocking_mode` and
`sys::write`. A review found this. The check is `grep -rn unsafe src/native/slave/`: only
`sys.rs` and the fork child block of `exec.rs` may appear.

## 13. Feature 02 implementation findings (2026-11-25)

### 13.1 The task context is not the manager context

The `#[task]` transition receives the context of the executor that drives it, not
the context of the manager. `execute(task, None)` seeds the task from the pool root,
which has a real clock and a real timer. A create that read `ctx.clock()` or
`ctx.require_env()` from that argument waited on real time and gave a real creation
time, so the deterministic tests could not control it. The transition now ignores
the argument and reads `task.inner.ctx`. The same rule applies to the remote create
task in `service.rs`: it reads `task.ctx`. A later task that needs effects takes its
context from the state, not from the argument.

### 13.2 Unary deadlines do not belong on a stream

The first version gave `Exec` and `FileRead` the `rpc_timeout` deadline, as a unary
call has. The deadline is absolute on the injected clock, so a test that moved the
clock past `rpc_timeout` to fire a command timeout also ended the event stream with
`DeadlineExceeded`, and `events()` reported `StreamLost`. A stream has no unary
deadline: its caller ends it, or the slave does. `SlaveClient` and the scoped client
use a context without a deadline for `Exec` and `FileRead`.

### 13.3 The in-memory transport must stream

`ConnectRpcHandler::dispatch` collects a streaming response in memory. A
server-streaming call whose handler waits for a signal then never completes inside
`dispatch`, so the client never receives the first event and the test deadlocks. The
test transport calls `dispatch_to` with a `ResponseSink` that writes the head and
each frame into the response pipe as the handler produces it. The request body is a
`RequestBody` over the request pipe. The same transport now serves unary and
streaming calls.

### 13.4 The Connect client dropped error details (issue #850)

The unary client rebuilt a server error from `WireError.code` and `WireError.message`
and dropped `details`. The streaming path did the same in `wire_error_to_trace`. A
typed `SandboxError` therefore arrived as `SandboxError::Rpc`, which breaks law L11.
Both paths now rebuild each detail with `WireErrorDetail::to_detail()` and add it to
the error. The fix is in `foundation_connectrpc`; the control-plane test
`rpc_error_variants` covers the unary case.

### 13.5 A timer is the `Env` timer, not `valtron::sleep_until`

`foundation_core::valtron::sleep_until` is re-exported from the executor module: it
returns a `Result<DrivenStreamIterator<SleepingTask>>`, not a future, and the
`timer` module is native-only. The control plane waits on
`ctx.require_env()?.timer().sleep_until(ctx.clone(), deadline)`, which is the same
injected timer on native and on wasm and never reads the real clock. A missing
`Env` is `HostNotConfigured`.

### 13.6 Send on wasm

The connectrpc client futures and `ServerStream` are `!Send` on single-threaded
wasm when the `multi` feature is off. The crate keeps the `Send` bound on every
public future and stream. On wasm it wraps the future in
`foundation_compact::SendWrapper` (`send_box`) and the stream in `SendStream`. On
native the wrapper is transparent, so the value must be `Send` already. Native
builds enable `foundation_core/multi` in a target-specific dependency table; the
wasm build keeps the one-thread executor.

### 13.7 The pool is shared and a leaked task starves it

The test binary starts one valtron pool with four workers. A gated `Exec` stream
whose gate is never released, or a create task that waits for an `Info` that never
answers, holds a worker forever. The tests release each gate, end each waiting
create with a clock advance, and use a streaming transport that ends its dispatch
task. `cargo test` must run the control tests with `--test-threads=1`: the
`ControlService` create stream drives a blocking iterator, and several such calls
at once can exhaust the pool.

**Measured at review (2026-11-25).** Dropping the `CreateStream` receiver does not
drop the `#[task]` state machine. `execute` schedules the task on the pool and
returns a receiver; dropping the receiver closes the result queue and leaves the
task to run. A create over a slave that never answers `Info` keeps one worker until
the boot deadline, and a substrate that the create already launched stays alive.
Issue #852 (severity: high) records this. The fix is a cancellation signal that the
receiver owns, or a guard on the receiver that starts the kill and the removal on
drop. The feature test `rpc_create_dropped_cleans_up` waits for that fix.

The review also added a small stateful model test. It draws a random op list, runs
it against one manager, and compares each step with the state table: `Exec` only
while `Ready`; `Stop` and `Kill` idempotent and the first report wins; a substrate
exit moves the record to `Stopped` through the watcher; `get` and `list` follow the
registry. The test keeps the value of a stateful oracle without a second model type.

**Fixed in this change (2026-11-25, issue #852).** The fix is option 1, a
receiver-owned guard, because it keeps the change in `foundation_sandbox` and
does not change `foundation_core`'s public behaviour. `CreateGuard` holds the
cancel flag, the wakers and the record slot. The `GuardedCreateIterator` that
the caller receives and the `create_ready` future hold an `Arc<CreateGuard>`
and call `cancel()` on drop. `cancel()` marks the create cancelled, wakes the
task's cancellation future, and moves the record to a background `kill_record`.
`AwaitSlave` and `Prepare` select on that future, so the worker leaves its wait
at once. `CreateTask::drop` also calls `cancel`. The final result calls
`finish()`, which disarms the guard for a later drop. The tests
`drop_create_receiver_cancels` and `drop_create_future_cancels` fail on
`befb4397d` and pass after the fix.

## 13. The Linux test failures of feature 01 (2026-11-26)

The first Linux run of the slave tests failed in three ways. Each has one cause, and two
of them are in the feature's own code.

### 13.1 Typed errors were lost on the client

The failing run was `3c3eb07fa`, which does not contain #850. That change rebuilds the
wire details of a server error on the unary path and in the stream terminator. The
message in the test (`Rpc { code: "not_found", message: "slave error not_found: ..." }`)
proves the server sent the typed error and the client dropped its detail. The branch now
carries #850; `exec_not_found`, `exec_not_executable` and `exec_id_conflict` pass without
a change to the slave.

### 13.2 The test process kept `SIGCHLD` unblocked on its main thread

A libtest test cannot be the slave's process. `/proc/<pid>/task/<tid>/status` showed
`SigBlk: 0000000000000000` on the process main thread and `0000000000010000` (SIGCHLD)
on the test thread and every pool thread. The test ran on a spawned harness thread, so
`block_sigchld` in the test entry set the mask of the wrong thread. `SIGCHLD` is
process-directed and the unblocked main thread took it, so the `signalfd` never became
readable and the reaper never collected a child. A zombie stayed under the slave and the
`Exit` event never came.

The slave process of the tests is now the program `tests/support/slave_main.rs`, a
`[[bin]]` target (`test = false`, `required-features = ["slave"]`). Its `main` blocks
`SIGCHLD` before any thread starts, and the tests start it through
`CARGO_BIN_EXE_slave_test_driver`.

### 13.3 The readiness helper cleared before the wait

`wait::ready` called `FdReadiness::clear()` and then checked `is_ready`. The shared
selector registers with `EPOLLET`, and `rearm_readable` is a no-op on epoll. A clear
before the wait therefore erased the edge the wait existed to return; a level condition
that emits no new edge (a pending `SIGCHLD`, end of file on a pipe that never carried
data) then never woke the task again. The first run showed the stdout pump reading and
the stderr pump and the reaper parked for ever.

The clear now happens in the caller, immediately before the I/O attempt: `ready()`,
then `readiness.clear()`, then `read` or `write`. The output pump also drains its pipe to
`WouldBlock` before it parks, because a partial read on an edge-triggered selector leaves
data that emits no new edge. The signs of the old bug were a stalled `head` in the
test's pipeline and a full pipe no task was reading.

### 13.4 The exit watch spun on a finished stream

`wait_for_exit` woke on `streams_done` even when the exit status was absent. A stream
that ends before the reaper reports the child is common, so the condition was true at
every poll and the task spun at 100 percent of one pool worker until the call deadline.
The wait now acts only on the exit status before the deadline, and on the finished
streams or the deadline after it.

### 13.5 The open wake race

The `exec_output_property` and `input_model` cases can park the HTTP/1.1 connection
task with frames waiting and no wake. The same class of run passed with extra timing
(`P:` trace prints) and failed without them. The fixes above removed the deterministic
causes (the two named stalls and every typed-error check). This one is a wake race in
the shared HTTP/1.1 streaming path. Issue #866 (severity: medium) records it, with the
failing cases, the observed trace and the suspected files. The branch does not close it.

### 13.6 `ExecSignal` before `Prepare` returned `NotFound`

The invariant I1 says that every call but `Info`, `Prepare` and `Shutdown` fails with
`NotPrepared` before `Prepare`. `exec_signal_inner` looked up the session first, so an
unknown `exec_id` gave `NotFound`. `not_prepared` now checks `self.prepared()` first.

A signal that arrives just after `Exit` must be accepted. The exit watch remembers the
`exec_id` **before** it removes the session, so the two states overlap and a signal can
never find neither. The branch head had lost the branch of `exec_signal_inner` that reads
the remembered set, so `signal_after_exit` got `NotFound`; it is back. `signal_bad_number`
drops its stream: it ran a program that sleeps for a minute and then drained it.

### 13.7 This host's container has no `openat2`

The Linux image on this host (OrbStack) answers `openat2` with `ENOSYS` (errno 38), so
every file test fails here with `open the parent directory: Function not implemented`.
The GitHub `ubuntu-latest` runner has a 6.x kernel and the earlier job reached the file
code, so the tests run there. The guest kernels of this spec are 6.1 and 6.12, which
have `openat2`. The feature states that limit.

### 13.8 The check for the next feature

`grep -rn unsafe src/native/slave/` must show only `sys.rs` and the fork child block of
`exec.rs`. A libtest test must not be the slave process. A readiness helper clears in
the caller, before the attempt, never before the wait.

### 13.9 A dropped response body now ends the exchange

`stream_drop_kills` failed: after the caller dropped the `Exec` stream, the program was
still alive after one second. The server's connection task had not dropped the handler
stream, so the `Drop` that sends `SIGKILL` to the process group never ran.

The cause is on the client side, in `foundation_connectrpc`. The HTTP/1.1 transport
spawns one drive task for the exchange (`h1.rs`). The task owns the exchange, and with
it the connection. Dropping the caller's response body closed only the body observer's
queue: `SplitCollectorMapContinuation` treats a closed queue as an accepted push,
because the head observer closes the same way. The drive task therefore stayed parked
on the next exchange item, the socket stayed open, and the server never saw a hangup.

The fix keeps the split as it is. The transport now wraps the body observer in
`StopOnDrop`, which sets a `ResumeSignal` when the caller drops it, and the drive task
in `StopDrive`, which ends when the signal is set and adds the signal to every
`Depends` wait, so a parked drive task wakes at once. The exchange then drops, the
connection closes, the server's hangup watcher fires, and the handler stream drops.

Fail-then-pass, on the same tree:
`stream_drop_kills` failed in 1.67 s without the `h1.rs` change and passed in 0.42 s
with it (`cargo test -p foundation_sandbox --features slave,testing --test mod`, Linux
container). `foundation_connectrpc` goes from 0.0.18 to 0.0.19, because the transport
behaviour changes.

### 13.10 The full test run with one thread hangs at the first slave test

`cargo test -p foundation_sandbox --features slave,testing -- --test-threads=1`
runs the 86 control and contract tests, then stops at `slave::exec::drain_timeout`
for ever. The test passes alone in 0.87 s. Every thread of the test process waits
on a `futex`, and the process has no `reactor-drain` thread; the slave driver is
idle and its reactor waits in `epoll_pwait`.

The hang is not from this feature. It also happens on `8ee13b8db` without the
`h1.rs` change. The control tests run first in the same process and use the global
valtron pool; #852 records that a dropped `CreateStream` receiver leaves its task on
the pool. Issue #867 (severity: medium) records the hang, the two trees, the
observations and the likely fix. CI runs the same command without
`--test-threads=1`, so the current job does not show it.

`gdb` cannot attach on this host: OrbStack answers `Couldn't get registers:
Input/output error`, so the trace above is from `/proc/<tid>/wchan` and the process
trees.

### 13.11 The source scan rejected a method name, not a blocking call

`slave::lifecycle::no_blocking_calls` scans the slave source text for `.read(` and
`.write(`. `Files::read`, the file-read RPC handler, matched `.read(`. That method
opens the file through `sys::open_in_root` and returns a stream, so the scan found
a false positive. The failure was deterministic but stayed hidden until the earlier
stalls were fixed, because no run had reached `slave::lifecycle` before.

The method is now `Files::read_file`, with a doc comment that names the scan. The
scan reads the raw text, so the comment names the pattern in words and does not
write it. The scan keeps its meaning: a blocking call with the `read` or `write`
name in the slave sources is still rejected. Issue #868 (severity: high) records
it.

### 13.12 The two input failures of the real CI runner

The Linux job (`cargo test --profile uat -p foundation_sandbox --no-fail-fast
--features slave -- --nocapture`, run 45676172618) failed on two input tests that
pass on this host.

**`input_model`.** The smallest counterexample was `ops = [(1, [], false)]`, seed
`b1264a50226f1c7b2d9f66fee316ce396e7f3056f652576ef9e08fb7bb94e125`, with
`next_seq` 1 against the model's 0. The model built a repeat as
`next.saturating_sub(1)`. Before the first chunk `next` is 0, so the request was
`seq 0`, which is the expected number and not a repeat: the slave applies it and
answers 1, exactly as `Sequence::accept` says (`seq < next` is a repeat,
`seq == next` applies). The slave was right and the model was wrong. The model now
treats that one case as the first chunk, and the root case has its own test,
`input_repeat_before_the_first_chunk`. The seed is committed in
`tests/slave/input.proptest-regressions`; with the old model the file reproduces
the exact failure (left 1, right 0) and the new model passes.

**`input_queue_full_then_retry`.** The test ran `sh -c "sleep 1; cat >/dev/null"`
and sent chunks until the queue refused one. On the runner the program started to
read before the queue filled, so no call was refused. The test depended on the
race between a one-second sleep and the send loop.

The fix adds `SlaveLimits::input_pipe_bytes` (default 64 KiB, the Linux default of
16 pages) and applies it with `F_SETPIPE_SZ` before the fork, so the pipe bound is
configuration. The test then stops the reader with `SIGSTOP` and sends five 4 KiB
chunks against a 4 KiB pipe and an 8 KiB queue. Five chunks are exactly one more
than the storage holds: one chunk in the pipe, two in the queue, and one that the
input pump popped and holds while the pipe is full. The fifth call must fail. The
test continues the program with `SIGCONT` and retries; every refusal is followed by
one `Stdout` event of `cat`, which proves that the program consumed bytes and the
queue has room. No sleep and no timing assumption remains. Five runs pass in 0.78
to 0.84 s each.


## 14. Feature 03 implementation findings (2026-11-25)

### 14.1 The pinned values and how each hash was made

One manual session on 2026-11-25 set every value in `native/artifacts/manifest.rs`.
The code lists no bucket at run time.

| Artifact | Where the value came from |
|---|---|
| Alpine 3.22 patch | `https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/<arch>/` listed once by hand. The newest patch is `3.22.8`. |
| Alpine SHA-256 | The published file `alpine-minirootfs-3.22.8-<arch>.tar.gz.sha256` beside the archive. x86_64: `2d7f46af…96e5`; aarch64: `36091efa…bca4`. |
| Alpine size | `Content-Length` of `curl -sSIL` (3 647 469 bytes for x86_64, 3 971 704 for aarch64). |
| Firecracker CI prefix | One S3 list call, paged with the continuation token: `https://s3.amazonaws.com/spec.ccfc.min?list-type=2&prefix=firecracker-ci/v1&delimiter=/`. The newest stable prefix is `v1.15`. |
| Firecracker files | `firecracker-ci/v1.15/<arch>/vmlinux-6.1.155` and `firecracker-ci/v1.15/<arch>/ubuntu-24.04.squashfs`. |
| Firecracker SHA-256 | `curl -sS <url> \| shasum -a 256` for each of the four files. |
| Firecracker size | `Content-Length` of the same URLs (kernel 44 279 576 bytes on x86_64 and 17 111 552 on aarch64; image 107 986 944 and 105 332 736). |
| File kinds | The kernel starts with `7f 45 4c 46` (ELF) on x86_64; the image starts with `hsqs`. |

The prefix layout in section 2.6 (`<YYYYMMDD>-<hash>/`) is old. The bucket now
uses one directory for each Firecracker version.

The real x86_64 Alpine 3.22.8 archive (SHA-256 `2d7f46af…96e5`) unpacked through the store on
2026-11-25 from a local copy: the hash matched the pinned value, `bin/busybox` is a regular
file with mode `0755`, and `usr/bin/yes` is a symbolic link to `/bin/busybox`. The tree has
520 entries.

### 14.2 The feature text names a download example that does not match the code

Feature section 3.2 says: "Read how
`backends/foundation_deployment_platform/src/qemu/download.rs` downloads images, and use the
same client calls." That file does not use `foundation_netio`. It starts `curl` with
`std::process::Command`, writes `dest.partial` and renames it. The same section and the
spec rules say the HTTP client is `foundation_netio`, and root rule 11 forbids
`std::process::Command` in core logic. The implementation follows the explicit client rule.
Issue #855 records the stale reference (severity: low).

### 14.3 The real archive must accept two shapes

- **The archive root entry.** The Alpine archive starts with `./`. A `..`-and-absolute
  check that also rejects the root would fail the real tree with `ArtifactUnsafePath`.
  `checked_relative` gives `None` for the root entry, and the unpack applies its mode to
  the target directory.
- **Absolute symbolic links.** The Alpine minirootfs has 335 symbolic links, and most have
  the target `/bin/busybox`. An unpack that rejects an absolute link target cannot produce
  a working tree, and a symbolic link is a path string and not a write. The unpack allows
  the target, records each created link, and rejects a later entry whose path passes
  through a link. A `link -> /tmp` entry followed by `link/x` gives
  `ArtifactUnsafePath`; without that check the second write would follow the link.

### 14.4 The TLS-free slave build stays TLS-free

`foundation_netio` holds the base feature set
(`standard`, `std`, `wire-native`, `multi`) in the native dependency table, and the
`artifacts` feature adds the provider through the weak feature `foundation_netio?/ssl`.
So `cargo build --no-default-features --features slave` for musl still links no C TLS
library, and the default native build downloads the `https://` URLs.

`HttpClientBuilder::build()` calls `unimplemented!` when the native build lacks the
`multi` feature. A consumer of the artifact store must keep `multi` on.

### 14.5 The host file system has no mode or remove call

Feature 00 names two `HostFs` additions: `rename` and `create_file`. Tar entries carry a
mode, and the temporary archive must leave the entry after the unpack. The unpack calls
`std::fs::set_permissions` (Unix) and `std::fs::remove_file` directly. A fake `HostFs`
therefore does not see the modes or the archive removal. Issue #854 (severity: low)
records the missing `HostFs::set_mode` and `HostFs::remove_file`.

### 14.6 The test layout

The feature text names `tests/artifacts_*.rs`. That form would add a second test binary and
restart the harness for each group. The crate keeps one binary (`tests/mod.rs`), so the
tests are the module `tests/artifacts/` (`support`, `cpio`, `fetch`, `store`, `tarball`,
`net`), as in section 12.6. The golden file is
`tests/fixtures/artifacts/initramfs-golden.newc`; `cpio -tv` listed its five entries one
time by hand.

### 14.7 The concurrency and the deadline

- Two builders draw two values from `ctx.rng()` and never share a temporary directory. The
  rename is the commit. A builder whose rename fails finds the `.ready` marker of the
  winner and returns the winner's path.
- One idle deadline covers each body read. It comes from `ctx.require_env()?.timer()`, so a
  stalled body fails on the injected clock and never on real time. A context without an
  `Env` gives `HostNotConfigured`.
- The stream of `ensure_with_progress` runs the same `Job` steps as `ensure`. The two
  functions cannot drift.

### 14.8 The download uses the streamed exchange, not `send_async`

Two defects in `foundation_netio` appeared with the real `https://` URLs (macOS, 2026-11-25).

1. **`send_async` ended a large HTTPS body early.** The head and `Content-Length` were
   correct, and the body stopped after 16898 bytes of a 65536-byte range
   (`https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.15/aarch64/vmlinux-6.1.155`
   with `Range: bytes=0-65535`). A 4096-byte range worked, and the sync `send` read the same
   URL in full. The same URL through `open_exchange` also read all 65536 bytes, so the
   defect is the `AsyncSendSafeBody` wake path of `send_async`. The store now uses
   `open_exchange` and the body arrives as `HttpExchange::BodyChunk` items, driven by
   `execute(...).into_next_stream()`. Issue #861 records the `send_async` defect (severity:
   high); the store does not use that path.
2. **The exchange task pooled a `Connection: close` socket.** The next request on the same
   client checked out the dead socket, waited for the head until the head deadline and
   failed with `Timeout occurred, please investigate`. A fresh client succeeded. The fix is
   in `http_exchange_task.rs` in this change: the body stream remembers whether the head
   allows another request, and a failed, stalled or unsupported-mode body drops its
   connection instead of pooling it. `store_is_atomic` and `fetch_checks_hash` cover the
   second request after a failure. The existing test
   `test_connection_returned_to_pool_after_exchange` expected a `Connection: close` socket
   in the pool, so it now uses a keep-alive server; a new test
   `test_connection_close_is_not_pooled` proves the close case. Issue #860 records it
   (severity: high). A server that closes an idle keep-alive socket without a header still
   leaves a dead pooled socket; issue #862 records that remaining limit (severity: medium).

The `execute` call needs the valtron pool. The boundary program starts it; a context without
a pool gives the text of `execute`'s error in `SandboxError::Io`. The artifact tests start
the pool once for the whole test binary (`tests/pool/mod.rs`), because `initialize_pool`
blocks until the previous guard drops and the test binary keeps its guard.

### 14.9 The network tests skip a link that is too slow

On 2026-11-25 this host reached `dl-cdn.alpinelinux.org` and `s3.amazonaws.com` at 13 to
17 KiB/s. The Firecracker root file system is 105 MiB, so `curl` and the store both needed
hours. Each network test now asks for a bounded prefix of its own pinned URL (256 KiB for
Alpine, 1 MiB for the root file system) on a thread with a five-second window. A link that
cannot deliver the prefix in that window gives the same skip message as a missing link. A CI
runner passes the probe in well under a second, so the full file and its hash check still
run there.

### 14.10 One test pool for the whole binary

The control tests and the artifact tests each started the pool in their own support module.
`initialize_pool` takes a process-wide FIFO gate (the previous `PoolGuard` must drop), and
both modules `std::mem::forget` their guard. The second start blocked forever, and the
control tests waited on their serial mutex behind it. `tests/pool/mod.rs` now owns the one
start, and both support modules call it.

## 15. Feature 04 implementation findings (2026-11-26)

### 15.1 The unified generator had no inline-enum support

`foundation_openapi::spec::Schema` parses `enum_values`, but no generator code read it.
Every property with an inline `enum` became `String`, so `InstanceInfo.state` could not be
`Not started`, and `MachineConfiguration.huge_pages` could not be `2M`. This is a real
generator gap, not a Firecracker peculiarity.

The fix, in `foundation_openapi/src/unified/`:

- `emit.rs`: `TypeBody::Enum { derives, variants }` and `EnumVariant { doc, name, value }`.
  The renderer writes `#[serde(rename = "...")]` when the variant name differs from the
  value, and `#[default]` on the first variant, because a generated struct that holds the
  enum in a required field derives `Default`.
- `generator.rs`: `inline_enum_type` names an enum after its parent and property
  (`InstanceInfoState`), so two groups that share a schema each get a local enum and need no
  cross-module import. Variant names come from `variant_ident`: `Not started` ->
  `NotStarted`, `2M` -> `Variant2M`, `Self` -> `SelfValue`, an empty value -> `Empty`.
  A duplicate mapped name gets a number suffix.

The rule is opt-in (`with_inline_enums`) because changing the default rewrites the
committed types of every provider in one go. Spec 77 feature 04 turns it on.

### 15.2 Optional fields serialized as `null`

`field_of` emitted `Option<T>` with no `skip_serializing_if`, and serde writes `None` as
`null`. Firecracker rejects a `null` for several fields, and invariant I2 forbids it. The
opt-in `with_skip_optional_none` adds
`#[serde(skip_serializing_if = "Option::is_none")]` to every optional field. The request
body tests assert that the bodies carry no `null`.

### 15.3 Error-response schemas were never generated

The type closure starts at each endpoint's request and success response types. The
extractor also collects `error_types` (status -> schema name), but the generator never read
them, so `Error { fault_message }` did not exist. The opt-in `with_error_types` adds those
names to the closure. The generated `Error` aliases a shared `FaultMessageObject` shape, and
the client parses the bounded error body with it. The feature's "Error body" generator point
now holds.

### 15.4 Shape hoisting and enum fields

The generator hoists identical structs whose fields name no generated type into `shared`
(`MachineConfigurationShape`) and keeps the public name as an alias. A struct with an enum
field no longer qualifies, so `MachineConfiguration` stays a real struct in each module that
uses it (`machine_config`, `config`). Each copy has its own local enum. That is correct and
needs no cross-module import. Do not try to force such a struct back into `shared` without
also hoisting its enum.

### 15.5 `JsonHash` expands in the deriving crate

`#[derive(JsonHash)]` expands to code that calls `base85rs::encode` and `sha2::Sha256`, so
the crate that derives it must name both. `foundation_sandbox` now has
`base85rs = "0.1"` and `sha2` under the `firecracker` feature. Without them the generated
code fails with 109 `unresolved import base85rs` errors. `foundation_deployment_docker`
already names both for the same reason.

### 15.6 The build script is the generator; the CLI cannot target a submodule

The feature names `gen_api generate --output-dir backends/foundation_sandbox/src/native/firecracker`.
For a provider not in `SPLIT_OUT_PROVIDERS`, the CLI writes
`<output-dir>/<provider>/generated/`, so that command would create
`src/native/firecracker/firecracker/generated/`. The `foundation_codegentools::Codegen`
builder supports the layout with `.output_dir(crate).provider_dir(crate/src/native/firecracker)`,
so the build script is the declaration and the generator (root rule 5).

`scripts/sandbox/check-firecracker-gen.sh` proves invariant I5: it hashes the generated
tree, touches `build.rs` so Cargo runs the build script again, fails if the script reports
a regeneration, and compares the hash. It passed with hash
`a3b158e6798543fd82977f398f37e5947dc46a859c21d494480bab8969e7bceb`.

### 15.7 The feature text names a specification file that the CLI does not discover

Feature section 3.2 names `artefacts/cloud_providers/firecracker/firecracker-v1.19.json`.
`gen_api generate --provider firecracker` discovers only `openapi.json` or
`firecracker.json` in that directory. The converted file is
`artefacts/cloud_providers/firecracker/openapi.json` (the repository convention), and the
version pin lives in the build script (`spec_version("1.19.0-dev")`) and `specs/README.md`.
A `firecracker-v1.19.json` copy would be a second copy of the same input.

### 15.8 A 204 answer is `()`, not `Empty`

The feature's generator table expects a 204 PUT to return `Empty`. The generator returns
`ApiResponse<()>` and calls `send_without_body`, which never reads the body. This is the
existing convention (Docker's PUTs use `()`), and `()` is the accurate type for "no body".
Changing it would regenerate every provider's committed output. The client accepts `()`.
The table row is a documentation mismatch; the functional requirement (a typed empty
success) holds.

### 15.9 The time-out test and the injected timer

`ValtronTimer` compares the deadline with the clock of the `Ctx` it receives, and
`SimulatedClock::advance` nudges the timer thread. A simulated clock therefore ends a
waiting call with no real sleep. Two details matter:

- `HttpClientBuilder`'s Unix read timeout defaults to 5 s, the same as the default
  `rpc_timeout`. A real transport timeout can then race the injected one and give `Io`
  instead of `ApiTimeout`. The client sets `transport_timeout` (default 60 s, nginx's
  `send_timeout`) so the injected timer ends a silent call first.
- The test advances the clock only after the mock server has read the request. The `call`
  helper computes the deadline and creates the timer before it polls the request, so after
  the server has the request the deadline is fixed; advancing earlier can move `now` past
  the deadline before it is computed.

### 15.10 Socket path length and the test socket

`UnixListener::bind` rejects a path at or over `SUN_LEN` (104 bytes on macOS, 108 on Linux
including the NUL). The macOS temporary directory plus a descriptive test name exceeds that,
and the first test run failed with `path must be shorter than SUN_LEN`. The test helper uses
`/tmp`. The client rejects a path over `max_socket_path` (default 107, the Linux
`sockaddr_un.sun_path` 108 less the NUL) with `SocketPathTooLong`.

### 15.11 The `ApiTimeout` error variant

No existing `SandboxError` variant meant "the API did not answer". `BootTimeout` names a
boot stage, and `ExecTimeout` names a command. `ApiTimeout { timeout_ms }` maps to
`Code::DeadlineExceeded`. The contract wire test and the code-table test include it.

### 15.12 Version bumps

Rule 12 moved three crates and their consumers: `foundation_openapi` 0.0.8 to 0.0.9,
`foundation_codegentools` 0.1.7 to 0.1.8, `foundation_sandbox` 0.0.5 to 0.0.6. The
consumers are `bin/platform` and the three deployment crates whose build scripts use the
generator.

### 15.13 Platform

All feature 04 tests need a Unix domain socket. `types_match_real_firecracker` also needs
the `firecracker` program and Linux `/dev/kvm`; on a host without them it prints a skip
message. On Windows the generated module compiles, but `client.rs` is `#[cfg(unix)]`, so
the client is absent. This is correct: Firecracker's API is a Unix socket.

### 15.14 The Docker artefact was stale before the shared script

Moving the conversion to `scripts/openapi/swagger2openapi.py` is behaviour-preserving: the
old script from `HEAD`, run on the current YAML, gives the new output byte for byte. The
committed `artefacts/cloud_providers/docker/openapi.json` differs in one value (the `t`
query parameter of `POST /build` is `array` in the file and `string` in the YAML). Issue
[#864](https://github.com/ewe-studios/ewe_platform/issues/864) (severity: low) records it.


## 16. Feature 05 implementation findings (2026-11-26)

### 16.1 The launch races the API call with the process and the deadline

`AwaitApi` must not `await` one `describe_instance()` call on its own. The real client has
an `rpc_timeout`, but the create deadline can be shorter, and a fake API in a test can
never answer. The state now `select_all`s three futures: the API call, `vmm.wait()` and the
deadline timer on the injected clock. The first one that is ready decides the next state.

`futures::future::select_all` needs one future type. Three `async` blocks are three
different types, so each is boxed as `Pin<Box<dyn Future<Output = ApiWait> + Send>>`. The
same rule as the control plane: a deadline that is not raced is not a deadline.

### 16.2 The `#[task]` macro needs the full `TaskStatus` type

The macro checks the last path segment of the second tuple element. A local type alias
`type Status = TaskStatus<..>` failed with "#[task] second tuple element must be
`TaskStatus<R, P>`". The return type is written in full in the transition only; the helper
functions may use the alias.

### 16.3 A fake process must answer a signal

The clean-up of a failed launch sends `SIGKILL` and then waits for the exit. A fake child
that records the signal but stays alive makes the launch hang. The fake child therefore
records the exit on `SIGKILL` and `SIGTERM`. A fake that does not do this hides a leak: the
real code waits for a process that never ends.

### 16.4 `ProcessSpec` cannot be cloned

`ProcessSpec.extra_fds` holds `OwnedFd` values, which are not `Clone`. The fake spawner
therefore records a `ProcessRecord` with the fields that a test reads (program, args, env,
working directory, log paths and the parent-death flag). The spec moves into the spawn; it is
never copied.

### 16.5 The wait thread receives the child through a channel

`std::process::Child` must move into the helper thread that calls `wait()`. If the thread
cannot start, the child is already inside the failed closure and cannot be killed. The
helper thread now receives the child through a channel after the spawn succeeds, so the
error path still owns the child and kills it.

### 16.6 The parent-death signal follows the thread, so one thread spawns

`prctl(PR_SET_PDEATHSIG, SIGKILL)` names the thread that made the child. `NativeProcessSpawner`
starts one dedicated thread at construction and sends every `ProcessSpec` to it. The
`spawn_kill_with_parent` test starts a helper process that makes a `sleep` through the real
spawner and is then killed; the `sleep` is gone in less than 2 seconds.
`spawn_survives_pool_thread_exit` makes a child from a short-lived thread through the same
spawner and proves that the child lives after that thread ends.

### 16.7 The slave transport disables `Expect: 100-continue`

A local slave answers a unary call at once. The connector still sent the default
`Expect: 100-continue` header, and a simple test server that writes its answer after the head
then closed the socket before the body. `NativeConnector` sets `expect_continue(false)` on
every client. The Firecracker API client already did this for the same reason.

### 16.8 The vsock preamble belongs to the connection

`HttpClientBuilder::unix_socket_preamble(preamble, expect, max_reply)` records the bytes in
the `ClientConfig`. The UDS connect path (`create_connection_unix_with_preamble`) writes
them and reads one reply line before the stream is wrapped. A Unix connection is never
pooled, so every new connection gets the preamble and a kept connection needs no second
one. A closed socket, a long line or a different prefix is a connection error, which the
readiness wait treats as "not ready".

### 16.9 `HostConfig::discover` needed a user identifier

The last state-directory fallback is `/tmp/ewe-sbx-<uid>`. The uid comes from the `UID`
value of the environment reader when it is there, and from `getuid(2)` otherwise. `libc`
moved from a Linux optional dependency to a non-optional Unix dependency, so `native/host.rs`
has it on Linux and macOS. The `slave` feature no longer lists `dep:libc`.

### 16.10 `ci/sandbox.toml` did not exist

Feature 05 names `ci/sandbox.toml` as the source of `min_firecracker_version`. The file was
absent. This feature creates it with `1.15.0`, the version of the Firecracker continuous
integration (CI) artifact prefix, and documents that only a CI run changes the value. The build script reads the key with a line parser (no TOML build dependency) and sets
`FOUNDATION_SANDBOX_MIN_FIRECRACKER_VERSION`. A packaged crate has no file and accepts every
version; the API fields that the driver sends are old.

### 16.11 The release-tree script is not on the feature branch

`scripts/ci/sync_release_tree.py` exists on `origin/master` and not on
`origin/spec77/f04-firecracker-api-client`. The master copy was run against this tree
(`--check` exits 0). It does not rewrite `[target.'cfg(..)'.dependencies.<name>]` tables, so
three Firecracker consumers (`foundation_vectors` twice and
`foundation_deployment_cloudflare`) were updated by hand to `foundation_netio` 0.0.19.

### 16.12 The generic spec validation does not check `max_vcpus`

Feature 05 section 6.6 says that "the manager's `validate` knows only `max_vcpus`".
`SandboxSpec::validate` checks `vcpus >= 1` and nothing else about the count, so a spec
with more vCPUs than the driver allows passes the manager. The Firecracker driver rejects
it in `Prepare`. Issue [#872](https://github.com/ewe-studios/ewe_platform/issues/872)
(severity: medium) records the gap and the likely fix.

### 16.13 Test layout and the platform gate

The driver is `cfg(target_os = "linux")`, so the macOS host does not compile it. During
development it was temporarily gated `cfg(unix)` and its 23 simulation tests passed on
macOS: the API order and bodies, the back-off waits 5, 10, 20 and 40 ms, a pending API, an
early process exit, a rejected drive, the four spec rejections, the probe checks, `attach`,
the two sweeps, the state model of the handle and the source scan of invariant I12. The
final gate is Linux, and the Linux continuous integration (CI) image is the check for it.
The native side-effect tests (7 on macOS) run on both Unix systems. The real microVM tests
are Linux with Kernel-based Virtual Machine (KVM) only.

## 17. The decision-16 documents did not agree (2026-11-26)

Decision D16 puts the model slave in the Cloudflare part. Requirements section 8 and
decision D16 name "feature 07 (library) and feature 08 (Worker classes)". Feature 07 gained
section 6.10 on 2026-11-25. Feature 08 was not updated: its goal, its context table, its
`wrangler.toml`, its deployment plan and its tests named two DO classes only.

A reader of feature 08 would have built a Worker with no `SLAVE` class and no binding for
it. The plan had a hole, not only a wording fault.

### 17.1 Root cause

The decision was recorded in two files that describe the same fact at different levels
(requirements and feature 07), and the third file that depends on it (feature 08) was left
alone. A cross-document check was missing. The owner asked for the check ("review to
validate"); it found the gap.

### 17.2 What the correction adds

- A table in feature 07 section 6.10 that names the file of each type. Before, it named the
  types only.
- `SlaveEndpoint::CloudflareDo { binding, name }`. The name is necessary: a service binding
  names a class, and `id_from_name` gives the instance. Without the name, every sandbox
  would reach one slave DO.
- Two drivers and one endpoint-dispatching connector in `DoControlPlane::open`. The first
  text named one driver and `PortConnector`.
- `ModelSlaveDo`, `WorkerModelClient`, the `SLAVE` and `AI` bindings, `Binding::Ai` in the
  plan, and the model rows of feature 08's test table.
- The `CloudflareAi` backend uses the `AI` binding, so it needs no key. Only `OpenRouter`
  reads a key. This keeps one secret out of the Worker.

### 17.3 Size

Feature 07 is 794 lines and feature 08 is 728 lines, over the 700-line review line of
`specifications/CLAUDE.md`. Both files grew by the D16 part. A later pass may split the D16
library into its own feature; requirements section 8 would then need a new number and a new
edge in the feature graph. The current order keeps 07 and 08 as the owner's decision D16
states.

## 18. Feature 06 implementation findings (2026-11-27)

### 18.1 The default binary needs the driver

The first commit of feature 06 built `agent-sandbox` without the `firecracker` feature, so
`run` had no driver on a Linux host. Commit `22cff605a` gives the default feature set the
Firecracker driver. A binary that ships a default action needs the default provider of that
action.

### 18.2 Two crates became non-optional for the native build

`foundation_http` and `foundation_netio` are now non-optional native dependencies. Both were
already in every native graph through the slave and the artifact store. The two musl checks
show that the slave binary still links no TLS library.

### 18.3 The I12 source scan needs a boundary list

The scan of invariant I12 looks for sandbox logic in the program crates. It must exempt
`src/cli/`, because `cli::sandbox::run` holds the body of the `agent-sandbox` main function.
A scan without that exemption reports the program's own entry point as a violation. The test
names the exemption and its reason.

### 18.4 The test binary owns the CLI tests

The feature named `agent_sandbox_cli.rs` as the test file. The crate has one test binary
(`tests/mod.rs`), so the CLI tests are modules of it. This follows root rule 7 and keeps one
binary for the whole crate.

### 18.5 What a macOS host cannot prove

The static-link check, the real Unix domain socket cases of the create and input paths, and
every microVM case need Linux. The macOS run proves the types, the parsing and the wasm and
musl checks. The pull request's CI job is the proof for the rest.

## 19. The Durable Streams study (2026-11-27)

### 19.1 A Durable Object has one alarm

The study project sets one alarm for each DO, at an absolute time. A new `setAlarm` replaces
the pending one. Our idle deadline already uses that single slot. A lifetime limit therefore
cannot have its own timer. The control plane computes the earlier of the two deadlines and
sets one alarm. Feature 18 records the rule.

### 19.2 A model is not a measurement

The study project publishes large savings: about 100 times less for hibernation, and a
total in the tens of thousands for each month. Every figure is an arithmetic model of
assumed counts and list prices. The measured inputs are hit rates only. A reader must not
treat a percentage in a README as a bill.

We record our own limits as configuration with a default (root rule 8), and we say in
`research.md` which number is measured and which is a model.

### 19.3 Hibernation needs a different protocol

A hibernatable WebSocket lets a DO sleep between writes. `SandboxService` is RPC over
HTTP. An open response stream is an open request, and the DO stays awake for it. We bound
the wake time with `stream_max_duration` instead of changing the protocol. A later feature
may move large output to R2 behind a signed URL, which removes the long stream.

## 20. A Linux compile check on the macOS host (2026-11-27)

A `cfg(target_os = "linux")` function is not compiled on the macOS host. Feature 06 had two
Linux-only compile errors that macOS could not see, and each one cost a full continuous
integration (CI) round.

**The tool.** Zig supplies a C compiler and a linker for a Linux target, with no virtual
machine. The CC environment variable of the C compiler wrapper crate needs the target name
`x86_64-unknown-linux-gnu`, and Zig accepts only `x86_64-linux-gnu`. A three-line wrapper
maps the name (it replaces `--target=x86_64-unknown-linux-gnu` with
`--target=x86_64-linux-gnu` before it calls `zig cc`).

**The command.**

```bash
export CC_x86_64_unknown_linux_gnu=/tmp/zigcc
export AR_x86_64_unknown_linux_gnu="/tmp/zig/zig ar"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/tmp/zigcc
cargo check -p foundation_sandbox --target x86_64-unknown-linux-gnu \
  --features "cli testing slave" --profile uat
```

**What it proves.** Every Linux-only path in the crate compiles: the Firecracker driver, the
slave engine, the CLI's Linux blocks and their tests.

**What it does not prove.** Linking, the run of a test, the Kernel-based Virtual Machine
(KVM), real sockets and the generated code at run time. The `sandbox-e2e` job proves those.
Run this check before a push that changes a Linux-gated file.

## 21. A blocked test thread cannot advance a simulated clock (2026-11-27)

The port timeout test of feature 07 did not finish. The cause was not the transport: the test
blocked on the future while the deadline lived on the simulated clock. A simulated clock moves
only when the test moves it, and a blocked thread cannot. The deadline never fired, and the
call waited for ever.

**Rule for a test.** If a test blocks on a future whose deadline must fire, use the host clock
(the real time source), or run the runtime in a loop that also advances the simulated clock.
A test of a real-time limit uses real time. A deterministic simulation test (DST) advances the
clock itself and never blocks on the future.

**Rule for the code.** The transport was correct: the 50 ms deadline dropped the pending port
exchange and returned `DeadlineExceeded`. The defect was in the test, and the fix stayed in the
test.

## 22. A Durable Object must stream with `dispatch_to` (2026-11-27)

`ConnectRpcHandler::dispatch` collects a streaming response and returns it when the stream
ends. Its own documentation says that a server must use `dispatch_to` to send frames as they
are produced. Feature 07 used `dispatch` first. An `Exec` through a Durable Object would then
arrive only at its end: the stream guard never held, and the idle alarm could stop a sandbox
while a command ran.

**Rule.** A server writes with `dispatch_to` and a `ResponseSink`. The only use of `dispatch`
is a test or an in-process caller that needs the whole answer.

The response of the DO handler is `HttpResponseParts`: a status, flat headers and a
`BoxByteStream`. A Worker maps it to the wasm response and holds the stream for the life of
the response. The request body is bounded by `max_message_bytes`; the response streams.

## 23. Parallel tests cannot share one valtron pool with a simulated clock (2026-11-27)

`cargo test -p foundation_sandbox --features "testing cloudflare"` hung with the default
thread count and passed with `--test-threads=1`. The cause was the test harness: it created
one process-wide valtron pool that every test shared. A test that parked a worker on a
simulated clock held the pool, and the next test waited for a worker that never came free.
The same suite then timed out in continuous integration.

**The fix.** Each test thread takes its own pool lease. A global gate serializes the creation
and the drop of the pool, so only one pool exists at a time, and a test that starts its own
threads releases the lease before it joins them (`pool::release()`).

**The rules.**

1. A test that advances a simulated clock owns its pool.
2. A test that runs on the host clock still takes a lease; the gate keeps one pool.
3. A test that spawns threads which drive their own futures releases the lease first.
4. The acceptance for a suite is the plain command, with no thread flag. A suite that needs
   `--test-threads=1` hides a shared-state defect.

## 24. Two clocks meet in the Durable Object (2026-11-27)

The idle alarm and the lifetime deadline are wall-clock times, stored in milliseconds. The
stream guard's touch timer runs on the monotonic clock, because a timer measures an interval.
A test that advances one clock without the other sees a deadline that does not move.

**The rule.** A simulation test advances the wall clock first and the monotonic clock second,
in that order, so a guard that reads the wall clock sees the new time when it wakes. In
production both clocks move together, so the order never matters there.

The test `an_open_stream_holds_the_sandbox_alive` documents this: it advances 30 minutes of
wall time over three idle periods, sees the stored idle deadline move to `4 * idle_timeout`,
fires the alarm, and checks that the sandbox still runs. Then it advances past the one-hour
cap and expects the `deadline_exceeded` terminal frame.

## 25. The artifact TLS feature put `ring` into the static slave (2026-11-27)

Feature 11 needed the static musl build of `agent-slave` with the `control` and `firecracker`
features. The build failed in the build script of `ring`, which wants a C compiler. The
`firecracker` feature enables `artifacts`, and `artifacts` enabled `foundation_netio/ssl`,
which links `ring` and its C code.

**Rule.** The artifact store does not need TLS to read its cache. The feature split is:
`artifacts` has no TLS, and `artifacts-remote` adds it. The default feature set has
`artifacts-remote`; the static slave build has `artifacts` only.

**What it proves.** `cargo build -p foundation_sandbox --no-default-features --features
"slave control firecracker cli" --bin agent-slave --target x86_64-unknown-linux-musl` gives a
static-pie program of 4.9 MB.

**Finding.** `ContainerHostConfig` flattens a `HostConfig` and a `Resources` that both name
`Devices`. A `Devices` value on the `HostConfig` alone is written as null, because the later
flattened field wins. The Docker driver sets both fields (issue #966).

**Finding.** The Firecracker probe of a nested host reports `kvm` false, and
`SlaveInfo.can_control` is then false. A child create fails with `DriverUnavailable`, as
invariant I2 says. No second code path is needed.
