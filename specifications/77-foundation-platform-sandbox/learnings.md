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

### 4.2 The Rust `worker` crate (version 0.8.5, in this workspace's `Cargo.lock`)

- `State::container() -> Option<Container>` (`src/durable.rs` line 258).
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
  (`shared/transport/wasm.rs`: no request streaming, no full duplex). Spec 65 feature 05
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

