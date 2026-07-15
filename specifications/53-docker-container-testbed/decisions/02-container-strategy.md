# 02 — Container Strategy per Platform

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Use **native Docker containers** for Linux test environments, **dockurr/windows**
as a Docker-wrapped QEMU escape hatch for Windows testing, and **retain the
existing QEMU provider** for macOS guests. Each platform gets its own
`ContainerProfile` that maps to the existing `VmProfile` concept, with a Docker
image (or Dockerfile build context) as the backing artifact instead of a qcow2
disk.

_OSX support expanded in section 4 below — dockurr/macos provides a Docker interface for macOS guests on KVM-capable hosts._

## Table of Contents

1. [Platform matrix](#platform-matrix)
2. [Linux: native Docker containers](#linux-native-docker-containers)
3. [Windows: dockurr/windows as a Docker interface](#windows-dockurrwindows-as-a-docker-interface)
4. [macOS: retained QEMU/UTM](#macos-retained-qemuutm)
5. [Container profile design](#container-profile-design)
6. [dockurr internals and limitations](#dockurr-internals-and-limitations)
7. [Why not dockurr for Linux](#why-not-dockurr-for-linux)

---

## Platform matrix

| Platform | Backend | Isolation | Requires KVM? | Startup time | Image size |
|----------|---------|-----------|---------------|-------------|------------|
| **Linux** (build + test) | Native Docker container | cgroups + namespaces | No | ~1-3 seconds | ~500 MB-2 GB (Docker image) |
| **Windows** (build + test) | dockurr/windows (QEMU in Docker) | Full VM inside container | Yes | ~30-90 seconds | ~6-8 GB (qcow2 in /storage) |
| **macOS** (build + test) | dockurr/macos (QEMU in Docker) | Full VM inside container | Yes | ~30-90 seconds | ~15-25 GB (qcow2 in /storage) |
| **macOS** (fallback, Apple Silicon) | UTM / QEMU (raw) | Full VM | Yes (HVF) | ~30-90 seconds | ~15-25 GB (qcow2) |

Key insight: native Docker containers are 10-30× faster to start than QEMU VMs
and require no KVM. This alone makes Linux the "fast path" for iterative
development — the test cycle drops from minutes to seconds.

---

## Linux: native Docker containers

Linux testing is the primary use case and the biggest win. We define a set of
Docker images (built from Dockerfiles in the crate) that match the existing
VM profiles:

### Profiles → Docker Images

| Profile | Base Image | Purpose | Key packages |
|---------|-----------|---------|-------------|
| `linux-build` | `ubuntu:24.04` | Full build environment | build-essential, curl, pkg-config, libssl-dev, libgtk-3-dev, libwebkit2gtk-4.1-dev, libayatana-appindicator3-dev, librsvg2-dev |
| `linux-test` | `ubuntu:24.04` (slim) | Binary execution + validation | Minimal; just the runtime deps of the built binary |

### Container configuration (equivalent to QEMU args)

```yaml
# What QEMU does vs what Docker does for Linux

QEMU:                                Docker equivalent:
─────────────────────────────────────────────────────────────
-enable-kvm                          (not needed; native)
-cpu host                            (not needed; native)
-m 4G                                --memory=4g
-smp 4                               --cpus=4
-drive file=disk.qcow2               FROM ubuntu:24.04 (image layers)
-netdev user,hostfwd=...:22          -p 2222:22
-virtfs .../mnt/project              -v ${PWD}:/mnt/project
-vnc :0                              (no display; headless by default)
```

### SSH access

We run an OpenSSH server inside the container so the existing SSH-based workflow
(ssh2 crate → exec commands, scp files) works unchanged:

```dockerfile
RUN apt-get install -y openssh-server \
    && mkdir /run/sshd \
    && echo 'PermitRootLogin yes' >> /etc/ssh/sshd_config \
    && echo 'PasswordAuthentication yes' >> /etc/ssh/sshd_config
# Inject the testbed SSH public key
COPY testbed_key.pub /root/.ssh/authorized_keys
CMD ["/usr/sbin/sshd", "-D"]
```

This preserves the SSH exec pattern from the QEMU provider — no API changes
for callers.

### Why SSH inside containers rather than `docker exec`

The existing `Provider` trait and all callers (bootstrap, build, runner,
screenshot) use SSH to communicate with guests. Replacing SSH with
`docker exec` would require rewriting every guest-interaction module. Running
SSH inside the container is a deliberate compatibility shim:

- **Keep existing code working** — `src/vms/ssh/`, `src/vms/bootstrap/`,
  `src/vms/build/`, `src/vms/runner/` all use `ssh2` and don't need to change.
- **Unified interface** — Whether the "guest" is a QEMU VM or a Docker
  container, it's always reachable via SSH on a known port.
- **SSH key-based auth** — The same keypair works across all providers.

The container's SSH port is mapped to a host port (e.g., 2422 for linux-build),
exactly as QEMU's `hostfwd` does today.

---

## Windows: dockurr/windows as a Docker interface

dockurr/windows runs a full Windows VM inside a Docker container using QEMU
internally. It still requires `/dev/kvm`, but it provides a **Docker interface**
for managing the VM — environment variables for configuration, `/storage`
volume for the disk, port 8006 for web access, port 3389 for RDP.

### What dockurr gives us that raw QEMU doesn't

1. **Pre-configured QEMU args** — No need to figure out OVMF firmware paths,
   `-cpu host`, SATA vs virtio, Apple SMC, etc. dockurr has tested these
   across hundreds of thousands of pulls and dozens of Windows versions.
2. **ISO auto-download** — Set `VERSION=11` and dockurr downloads the correct
   Windows ISO from Microsoft's servers. No Vagrant Cloud dependency. The ISO
   is stored in the `/storage` volume (persistent host mount), making it a
   one-time cost per host — subsequent starts reuse the cached download.
3. **Automatic driver installation** — VirtIO drivers, networking, RDP are
   installed during Windows setup automatically.
4. **Web viewer on port 8006** — For debugging when SSH/WinRM isn't working.
5. **Standard Docker volume for disk** — `-v ./windows:/storage` persists the
   VM disk. Much simpler than figuring out qcow2 paths.

### Integration approach

For the bollard path (via `ContainerServiceDefinition` — the Rust-native API):

```rust
let windows = ContainerServiceDefinition::new("dockurr/windows:5.15")
    .env("VERSION", "11")
    .env("RAM_SIZE", "12G")
    .env("CPU_CORES", "4")
    .env("DISK_SIZE", "80G")
    .env("USERNAME", "vagrant")
    .env("PASSWORD", "vagrant")
    .device("/dev/kvm")
    .device("/dev/net/tun")
    .cap_add("NET_ADMIN")
    .port_mapped(3389, 3389)    // RDP
    .port_mapped(8006, 8006)    // noVNC viewer
    .volume("/var/lib/testbed/windows", "/storage")
    .stop_timeout_secs(120)
    .required();                 // no graceful skip — KVM is required
```

The `ContainerServiceDefinition` is the primary API — users should not need to
construct raw `bollard::container::Config` in common cases. For advanced
scenarios not covered by the builder, `ContainerServiceDefinition` exposes:

```rust
/// Escape hatch: access the underlying bollard Config for modifications
/// not expressible through the high-level builder.
pub fn with_raw_config(mut self, f: impl FnOnce(&mut bollard::container::Config<String>)) -> Self;
```

This preserves the 90% ergonomic case while providing a back door for the 10%.

### Custom Dockerfiles extending base images

vm-uncloud's `dev-windows` recipe demonstrates the pattern: extend a base
dockurr image with a custom Dockerfile that bakes OEM provisioning scripts,
then use the built image. We provide the same via `DockerfileConfig`:

```rust
/// A Dockerfile definition that can be built into an image.
pub struct DockerfileConfig {
    /// Dockerfile content (use include_str!("Dockerfile") or a dynamic string)
    pub dockerfile: String,
    /// Build context directory for COPY/ADD. Defaults to crate root.
    pub context: PathBuf,
    /// Build args
    pub build_args: Vec<(String, String)>,
    /// Tag for the built image (e.g., "testbed/dev-windows:latest")
    pub tag: String,
    /// Platform (e.g., "linux/amd64"). Auto-detected if None.
    pub platform: Option<String>,
}

/// Result of building a Dockerfile, ready for ContainerServiceDefinition.
pub struct ImageBuildResult {
    pub image_tag: String,      // e.g., "testbed/dev-windows:latest"
    pub sha256: String,         // SHA256 digest (for pinning)
    pub was_cached: bool,       // reused existing image (no rebuild)
}
```

**`build_once()`** — atomic build-or-reuse: checks if `tag` already exists
locally (via `DockerClient::image_exists`). If yes, returns `ImageBuildResult`
with `was_cached: true` — the built image is reused. If no, builds the
Dockerfile via bollard's `build_image()`, tags the result, and returns the
SHA. Subsequent calls reuse the cached image indefinitely.

**`ContainerServiceDefinition::from_build(result)`** — consumes the build
result, pointing the container to the built image instead of a registry pull:

```rust
let result = DockerfileConfig::new(include_str!("Dockerfile.dev-windows"))
    .arg("DEV_USER", "vagrant")
    .tag("testbed/dev-windows:latest")
    .build_once()?;

let windows = ContainerServiceDefinition::from_build(result)
    .env("VERSION", "11")
    .env("RAM_SIZE", "12G")
    .device("/dev/kvm")
    .port_mapped(3389, 3389);
```

### Platform-specific convenience functions

For the common case — dockurr base images with baked provisioning scripts —
we provide convenience functions that return pre-configured
`ContainerServiceDefinition` values with the script injected:

**`docker_windows(script: &str, tag: &str) -> ContainerServiceDefinition`**
Extends `dockurr/windows:5.15` with a script written to `/oem/install.bat`
(baked into the image at build time, not mounted at runtime):

```rust
fn docker_windows(script: &str, tag: &str) -> ContainerServiceDefinition {
    let dockerfile = format!(
        "FROM dockurr/windows:5.15\n\
         RUN mkdir -p /oem && echo '{}' > /oem/install.bat\n",
        script
    );
    let result = DockerfileConfig::inline(dockerfile).tag(tag).build_once()
        .expect("failed to build custom Windows image");
    ContainerServiceDefinition::from_build(result)
        .env("VERSION", "11")
        .env("RAM_SIZE", "12G")
        .device("/dev/kvm")
        .cap_add("NET_ADMIN")
        .port_mapped(3389, 3389)
}
```

**`docker_macos(version: &str, tag: &str) -> ContainerServiceDefinition`**
Same pattern, extends `dockurr/macos:latest`.

**`docker_linux(script: &str, tag: &str) -> ContainerServiceDefinition`**
Extends `ubuntu:24.04` with SSH setup + an init script at
`/usr/local/bin/testbed-init.sh`, executed on startup:

```rust
fn docker_linux(script: &str, tag: &str) -> ContainerServiceDefinition {
    let dockerfile = format!(
        "FROM ubuntu:24.04\n\
         RUN apt-get update && apt-get install -y openssh-server && mkdir /run/sshd\n\
         RUN echo 'PermitRootLogin yes' >> /etc/ssh/sshd_config\n\
         COPY testbed_key.pub /root/.ssh/authorized_keys\n\
         RUN echo '{}' > /usr/local/bin/testbed-init.sh && chmod +x /usr/local/bin/testbed-init.sh\n\
         CMD [\"/usr/sbin/sshd\", \"-D\"]\n",
        script
    );
    let result = DockerfileConfig::inline(dockerfile).tag(tag).build_once()
        .expect("failed to build custom Linux image");
    ContainerServiceDefinition::from_build(result)
        .port(22)
}
```

### Key guarantees

1. **Atomic build** — `build_once()` checks for an existing image by tag.
   If found, returns it immediately (zero overhead). If not, builds exactly
   once. No "build every test run" penalty.
2. **Script baked, not mounted** — The provisioning script lives in the image
   layer, not in a volume mount. This makes the image self-contained and
   reproducible across hosts. Same as vm-uncloud's `dev-windows` pattern
   (`/oem/install.bat` baked via `COPY` in the Dockerfile).
3. **SHA pinning** — `ImageBuildResult.sha256` is available for deterministic
   pinning. Pass it to CI for reproducible builds, or ignore it for dev.
4. **Applies to all three platforms** — Windows, macOS, and Linux each get
   a `docker_<platform>()` function with baked-in provisioning. Users provide
   the script via `include_str!()` at compile time.

For the Compose path:

```yaml
services:
  windows:
    image: dockurr/windows
    container_name: windows-build
    environment:
      VERSION: "11"
      RAM_SIZE: "12G"
      CPU_CORES: "4"
      DISK_SIZE: "80G"
      USERNAME: "vagrant"
      PASSWORD: "vagrant"
    devices:
      - /dev/kvm
      - /dev/net/tun
    cap_add:
      - NET_ADMIN
    ports:
      - "3389:3389"
      - "8006:8006"
    volumes:
      - ./windows:/storage
    stop_grace_period: 2m
```

### SSH into dockurr Windows

Once the dockurr Windows VM boots, we need SSH access (matching the existing
`VmProfile` port-forwarding pattern). dockurr doesn't expose SSH natively, so
we use a two-phase approach:

1. **First boot**: dockurr starts the Windows VM. We connect via RDP (port 3389)
   or the web viewer (port 8006) as a fallback for debugging.
2. **Bootstrap phase**: Once RDP is available, we use the existing WinRM
   bootstrap flow (install OpenSSH, configure keys) — exactly as the current
   Windows QEMU bootstrap does. This works because dockurr's Windows is a real
   Windows VM, reachable on the host's ports.

   The custom-Dockerfile pattern from the Integration Approach section above
   (`DockerfileConfig` + `build_once()` + `docker_windows()`) handles script
   injection at image build time — scripts baked via `/oem/install.bat` rather
   than mounted at runtime, making the image self-contained and reproducible.

This means the **existing Windows bootstrap code is fully reused** — only the
VM launch mechanism changes (QEMU args → dockurr container config).

### dockurr limitations

- **Requires KVM** — Does not eliminate the KVM dependency for Windows testing.
  This is a fundamental constraint of running Windows on Linux.
- **No SSH by default** — Still needs the two-phase WinRM→SSH bootstrap.
  This matches vm-uncloud's approach: the default Windows recipe uses RDP
  (port 3389) as the primary authenticated entry, with the noVNC viewer
  (port 8006) reachable only over SSH tunnel for debugging. There is no
  SSH-in-the-box at launch — SSH is installed as part of bootstrap, same
  as our two-phase flow. The `dev-windows` variant experiments with baking
  OpenSSH into a custom Dockerfile via `/oem/install.bat`, which is a
  future optimisation we can adopt.
---

## macOS: dockurr/macos as a Docker interface

dockurr/macos runs a full macOS VM inside a Docker container using QEMU
internally, following the same pattern as dockurr/windows. It provides a
**Docker interface** for managing the VM — environment variables for version
selection, `/storage` volume for the disk, port 8006 for noVNC web access,
and port 5900 for VNC. This was validated in the `vm-uncloud` project via the
`macos-kvm` recipe (`recipes/macos-kvm/compose.yaml`).

### Supported macOS versions

| VERSION | Name |
|---------|------|
| `15` | Sequoia |
| `14` | Sonoma |
| `13` | Ventura |
| `12` | Monterey |
| `11` | Big Sur |

### Docker interface

```yaml
services:
  macos:
    image: dockurr/macos:latest
    environment:
      VERSION: "14"          # Sonoma
      RAM_SIZE: "12G"
      CPU_CORES: "6"
      DISK_SIZE: "96G"       # macOS needs more headroom than Windows
    cap_add:
      - NET_ADMIN
    devices:
      - /dev/kvm             # required — no TCG fallback for macOS
    volumes:
      - macos_storage:/storage
      # noVNC :8006 — NO auth; reach via SSH tunnel, not published
    restart: always
```

### Access model

- **VNC on port 5900** — primary remote access for GUI interaction.
- **noVNC on port 8006** — web-based viewer. No authentication; vm-uncloud
  explicitly does NOT publish this port. Reach it via SSH tunnel only
  (`ssh -L 8006:localhost:8006 root@<node>`), never expose publicly.
- **SSH** — Not exposed by default. Must be enabled inside the guest via
  System Settings → General → Sharing → Remote Login after first boot
  through the VNC viewer. Once enabled, the SSH port can be mapped for
  headless build/test automation.

### Caveats and limitations

1. **Requires KVM** — macOS on dockurr has no TCG fallback. Unlike
   dockurr/windows (which runs on TCG at ~5-10x slowdown), dockurr/macos
   requires hardware acceleration. Needs bare metal (Vultr BM, Hetzner
   Robot) or a local Linux host with KVM.

2. **Licensing** — Running macOS on non-Apple hardware violates Apple's
   End User License Agreement. Fine for personal/experimental use; review
   the posture before relying on it in shared or public CI. The
   `vm-uncloud` recipe explicitly flags this.

3. **No RDP** — macOS has no native RDP. VNC is the primary remote
   desktop protocol. For headless build/test, SSH must be manually
   enabled via the VNC viewer after first boot.

4. **Large disk** — macOS disk images are 15-25 GB, larger than Windows.

5. **AMD core limitation** — On AMD systems, multiple cores may decrease
   performance or cause crashes until after installation. dockurr recommends
   single-core for the initial setup.

### SSH bootstrap for headless use

Following vm-uncloud's patterns, the macOS bootstrap flow is:

1. **First boot** — Launch dockurr/macos with KVM. Connect via VNC to
   complete the initial setup assistant and enable Remote Login (SSH).
2. **SSH key injection** — Once SSH is enabled, inject the testbed public
   key via VNC terminal or `docker exec`.
3. **Subsequent boots** — SSH on the mapped host port works immediately.
   No VNC needed after the one-time setup.

This matches the two-phase Windows bootstrap (first boot interactive,
subsequent boots headless via SSH), just using VNC instead of RDP.

### Provider selection

The `DockerProvider` selects the macOS backend based on host capabilities:

| Host | Provider | Rationale |
|------|----------|-----------|
| Linux with KVM | dockurr/macos via Docker | Same Docker interface as Linux and Windows |
| Apple Silicon Mac | UTM (existing `UtmProvider`) | Native HVF, no Docker KVM passthrough required |
| Linux without KVM | Error — macOS cannot run without KVM | TCG not supported for macOS |
| Hetzner Cloud | Error — no `/dev/kvm` | TCG not supported; need bare metal (Hetzner Robot, Vultr BM) |

---

## Container profile design

`ContainerProfile` maps 1:1 to the existing `VmProfile` concept:

```rust
/// Mirrors VmProfile but for Docker containers.
pub struct ContainerProfile {
    /// Profile name (e.g. "linux-build", "windows-build")
    pub name: String,
    /// Target guest OS
    pub guest_os: GuestOs,
    /// Docker image (e.g. "testbed/linux-build:latest" or "dockurr/windows")
    pub image: String,
    /// Or: path to a Dockerfile build context
    pub dockerfile: Option<PathBuf>,
    /// Memory limit
    pub memory: ByteSize,
    /// CPU count
    pub cpus: u32,
    /// SSH port on the host
    pub ssh_port: u16,
    /// Additional port mappings
    pub ports: Vec<PortMapping>,
    /// Volume mounts (host → container)
    pub volumes: Vec<VolumeMount>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Requires KVM (/dev/kvm device)?
    pub needs_kvm: bool,
    /// Docker network to attach to
    pub network: Option<String>,
    /// Bootstrap mode
    pub bootstrap_mode: BootstrapMode,
}
```

### Profile definitions (embedded in code, overridable via testbed.toml):

```rust
const LINUX_BUILD_PROFILE: ContainerProfile = ContainerProfile {
    name: "linux-build",
    guest_os: GuestOs::Linux,
    image: "testbed/linux-build:latest",  // built from Dockerfile.linux-build
    dockerfile: Some("docker/linux-build/Dockerfile"),
    memory: ByteSize::gb(4),
    cpus: 4,
    ssh_port: 2422,
    ports: vec![("2422", "22")],
    volumes: vec![
        ("${PROJECT_DIR}", "/mnt/project"),  // bind mount (replaces 9p)
        ("linux-build-cargo", "/root/.cargo"),  // named volume (persistent cache)
    ],
    env: vec![],
    needs_kvm: false,
    network: Some("testbed-net"),
    bootstrap_mode: BootstrapMode::SshOnly,
};
```

---

## Why not dockurr for Linux

dockurr does not provide a Linux image. Their focus is OSes that cannot run
natively in containers (Windows, macOS, ChromeOS). For Linux, a native Docker
container is strictly superior:

| Property | dockurr Linux (hypothetical) | Native Docker container |
|----------|------------------------------|------------------------|
| Isolation | QEMU VM | cgroups + namespaces |
| KVM required | Yes | No |
| Startup time | 30-90 seconds | 1-3 seconds |
| Memory overhead | Full VM (~1 GB base) | Process-level (~50 MB base) |
| Filesystem sharing | 9p/virtiofs (brittle) | bind mounts (kernel-level, reliable) |
| Networking | User-mode port forwarding | Native bridge networking |
| Image size | Full disk image (multi-GB) | Layered Docker image (MBs of delta) |

The whole point of this spec is to move away from QEMU's overhead for Linux
testing. Wrapping QEMU in Docker (which is what dockurr does) doesn't solve
that for Linux — it adds another layer.
