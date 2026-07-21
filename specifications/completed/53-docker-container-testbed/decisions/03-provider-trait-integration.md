# 03 — Provider Trait Integration & Crate Architecture

**Date:** 2026-07-09
**Status:** Resolved

## Decision

Extract all platform abstraction logic from `foundation_testbed` into a new
crate **`foundation_deployment_platform`** — a sibling of `foundation_deployment`.
This crate owns the `Provider` trait (redesigned with associated types), all
provider implementations (`QemuProvider`, `UtmProvider`, `DockerProvider`), SSH
and WinRM communication, guest bootstrap, image management, state persistence,
and the platform CLI. The Docker runtime (`ContainerHandle`, `ContainerConfig`,
`WaitFor`, `DockerClient`) lives as a `docker/` module within this crate — no
separate `foundation_deployment_platform` crate. `foundation_testbed` becomes a
thin consumer.

## Table of Contents

1. [Why the Provider trait needs associated types](#why-the-provider-trait-needs-associated-types)
2. [Redesigned Provider trait](#redesigned-provider-trait)
3. [Why VmProfile doesn't work for Docker](#why-vmprofile-doesnt-work-for-docker)
4. [ContainerServiceDefinition vs VmProfile](#containerservicedefinition-vs-vmprofile)
5. [Dispatch via PlatformHandle](#dispatch-via-platformhandle)
6. [DockerProvider implementation](#dockerprovider-implementation)
7. [Crate dependency graph](#crate-dependency-graph)
8. [What moves where](#what-moves-where)
9. [Colima support on macOS](#colima-support-on-macos)

---

## Why the Provider trait needs associated types

The current `Provider` trait is QEMU-shaped:

```rust
// Current — QEMU-specific:
fn launch(&self, profile: &VmProfile, display: DisplayMode) -> Result<VmHandle>;
fn monitor_command(&self, handle: &VmHandle, cmd: &str) -> Result<String>;
fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf>;
```

Problems:
1. **`VmProfile`** has `image_name` (qcow2 filename), `vnc_port`, `memory_mib`,
   `disk_gb`, `prebaked_url` — all QEMU concepts. Docker uses image tags, env
   vars, and string-based limits. See [VmProfile analysis](#why-vmprofile-doesnt-work-for-docker).
2. **`VmHandle`** holds a PID + monitor socket. Docker holds a container ID.
3. **`DisplayMode`** is VNC/SPICE/GTK vs headless — QEMU display backends.
   Docker containers don't have displays. dockurr web viewers are just ports.
4. **`monitor_command`** is QEMU Monitor Protocol (QMP). Docker uses
   `docker exec` or bollard's `create_exec`.
5. **`ensure_image`** returns `PathBuf` to a qcow2 on disk. Docker images are
   managed by the daemon, not files on disk.

The fix: **associated types on the Provider trait**. Each provider defines its
own handle type and config type:

```rust
pub trait Provider: Send + Sync {
    /// The handle this provider returns on launch.
    /// QemuProvider/UtmProvider → VmHandle (owns child process + monitor socket)
    /// DockerProvider → ContainerHandle (owns bollard container lifecycle)
    type Handle;

    /// The configuration this provider accepts.
    /// QemuProvider/UtmProvider → VmProfile (qcow2, VNC, MiB, etc.)
    /// DockerProvider → ContainerServiceDefinition (image tag, WaitFor, etc.)
    type Config;

    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    fn launch(&self, config: &Self::Config) -> Result<Self::Handle>;
    fn stop(&self, handle: &Self::Handle) -> Result<()>;
    fn is_running(&self, handle: &Self::Handle) -> bool;
    fn resolved_ports(&self, handle: &Self::Handle) -> Result<ResolvedPorts>;
    fn host_health(&self) -> HostHealth;
}
```

What was **removed** from the trait:
- **`monitor_command`** — QEMU-specific. Callers that need QMP access go
  through the concrete `QemuVm` type, not the trait.
- **`ensure_image`** — Each provider handles image provisioning internally
  as part of `launch()`. QEMU downloads qcow2; Docker pulls from registry.
- **`DisplayMode`** — Set on the provider at construction time
  (`QemuProvider::new().display(Headless)`), not per-launch. Docker doesn't
  have a display mode.

What was **kept**:
- **`launch` / `stop` / `is_running`** — universal lifecycle
- **`resolved_ports`** — all providers resolve host↔guest port mappings
- **`host_health`** — all providers check prerequisites (KVM, Docker socket, etc.)

---

## Why VmProfile doesn't work for Docker

`VmProfile` is currently 14 fields. Here's which ones are QEMU-specific
and have no Docker equivalent:

| Field | QEMU meaning | Docker equivalent | Works? |
|-------|-------------|-------------------|--------|
| `name` | Profile name (`"linux-build"`) | Same | ✅ |
| `os` | GuestOs enum | `ContainerServiceDefinition` has no OS — the image defines it | Partial |
| `image_name` | qcow2 filename (`"ubuntu-24.04-x86_64.qcow2"`) | Docker image tag (`"testbed/linux-build:latest"`) | ❌ Different type |
| `image_cache_path()` | `~/.cache/.../images/<name>.qcow2` | Docker daemon manages images internally | ❌ Not applicable |
| `ssh_port` | Host port → guest :22 | Same port mapping | ✅ |
| `rdp_port` | Host port → guest :3389 | Same port mapping (dockurr Windows) | ✅ |
| `winrm_port` | Host port → guest :5985 | Same port mapping (Windows bootstrap) | ✅ |
| `vnc_port` | VNC display offset (5900 + N) | Docker has no VNC. dockurr exposes web viewer (8006) or VNC (5900) directly as ports | ❌ Different concept |
| `memory_mib` | u32 MiB | Docker uses string (`"4G"`, `"512m"`) in HostConfig | ❌ Different type/unit |
| `cpu_cores` | u32 | Docker uses `HostConfig.nano_cpus` or `--cpus` | Partial |
| `disk_gb` | Pre-allocated qcow2 size | Docker doesn't pre-allocate. dockurr uses `DISK_SIZE` env var | ❌ Not applicable |
| `prebaked_url` | qcow2 download URL | Docker image pull from registry | ❌ Different mechanism |
| `bootstrap` | WinRM→SSH bootstrap mode | Docker bakes provisioning into Dockerfile (not a runtime flag) | ❌ Different model |
| `user` / `pass` | Guest credentials | Env vars in ContainerServiceDefinition | Partial |

**Only 3 of 14 fields map cleanly** (`name`, `ssh_port`, `rdp_port`). The rest
are QEMU concepts with no Docker equivalent, or fundamentally different types.

---

## ContainerServiceDefinition vs VmProfile

These are **sibling types**, not a subtype relationship. Each provider
accepts the config that matches its isolation model:

```rust
// ── VM isolation (QEMU/UTM) ──
pub struct VmProfile {
    pub name: String,
    pub os: GuestOs,
    pub image_name: String,       // qcow2 filename
    pub ssh_port: u16,
    pub rdp_port: Option<u16>,
    pub winrm_port: Option<u16>,
    pub vnc_port: u16,            // display offset
    pub user: String,
    pub pass: String,
    pub bootstrap: BootstrapMode,
    pub memory_mib: u32,
    pub cpu_cores: u32,
    pub disk_gb: u32,
    pub prebaked_url: Option<String>,
}

// ── Container isolation (Docker) ──
pub struct ContainerServiceDefinition {
    pub name: String,
    pub image: String,                   // Docker image tag
    pub ports: Vec<PortMapping>,
    pub env: Vec<(String, String)>,
    pub volumes: Vec<VolumeMount>,
    pub network: Option<String>,
    pub network_aliases: Vec<String>,
    pub wait: WaitFor,
    pub stop_timeout: u64,
    pub memory: Option<String>,          // "4G", "512m"
    pub cpus: Option<u32>,
    pub always_pull: bool,
    pub command: Option<Vec<String>>,
    pub devices: Vec<DeviceMapping>,
    pub cap_add: Vec<String>,
    pub log_output: LogOutput,
    pub required: bool,
}
```

### Conversion between profiles

For the CLI and build pipeline, a `VmProfile` can be converted to a
`ContainerServiceDefinition` for the Docker provider:

```rust
impl ContainerServiceDefinition {
    /// Convert a VmProfile to a ContainerServiceDefinition for the Docker
    /// provider. This is a best-effort mapping — some VmProfile fields have
    /// no Docker equivalent and are ignored (vnc_port, disk_gb, prebaked_url).
    pub fn from_vm_profile(profile: &VmProfile) -> Self {
        let image = match profile.os {
            GuestOs::Linux => "testbed/linux-build:latest",
            GuestOs::Windows => "dockurr/windows:5.15",
            GuestOs::MacOS => "dockurr/macos:latest",
        };
        ContainerServiceDefinition::new(image)
            .name(&profile.name)
            .port_mapped(22, profile.ssh_port)
            .memory(format!("{}M", profile.memory_mib))
            .cpus(profile.cpu_cores)
            .env("USERNAME", &profile.user)
            .env("PASSWORD", &profile.pass)
        // vnc_port, disk_gb, prebaked_url, bootstrap — not mapped
    }
}
```

This conversion is a **convenience for the CLI path** — it lets users type
`testbed start linux-build` and have it work with either QEMU or Docker
depending on what's available. The full expressiveness of
`ContainerServiceDefinition` (WaitFor, LogOutput, devices, cap_add, etc.) is
available when constructing the profile programmatically.

---

## Dispatch via PlatformHandle

Callers that need to work with any provider use the enum-based dispatch:

```rust
/// A handle from any provider. Wraps the concrete handle type.
pub enum PlatformHandle {
    Qemu(VmHandle),
    Utm(VmHandle),       // UTM uses the same VmHandle shape
    Docker(ContainerHandle),
}

impl PlatformHandle {
    pub fn id(&self) -> &str {
        match self {
            PlatformHandle::Qemu(h) => &h.internal_id,
            PlatformHandle::Utm(h) => &h.internal_id,
            PlatformHandle::Docker(h) => h.id(),
        }
    }

    pub fn host_port(&self, container_port: u16) -> u16 {
        match self {
            PlatformHandle::Qemu(h) => h.resolved_ports.ssh_port,
            PlatformHandle::Utm(h) => h.resolved_ports.ssh_port,
            PlatformHandle::Docker(h) => h.host_port(container_port),
        }
    }
}

/// A profile for any provider.
pub enum PlatformProfile {
    Vm(VmProfile),
    Container(ContainerServiceDefinition),
}
```

The CLI dispatch layer resolves `PlatformProfile` → `PlatformHandle`:

```rust
pub fn launch(profile: &PlatformProfile) -> Result<PlatformHandle> {
    match profile {
        PlatformProfile::Container(def) => {
            let provider = DockerProvider::local()?;
            Ok(PlatformHandle::Docker(provider.launch(def)?))
        }
        PlatformProfile::Vm(vm_profile) => {
            // Try Docker first for Linux guests, fall back to QEMU
            if vm_profile.os == GuestOs::Linux && docker_available() {
                let def = ContainerServiceDefinition::from_vm_profile(vm_profile);
                let provider = DockerProvider::local()?;
                return Ok(PlatformHandle::Docker(provider.launch(&def)?));
            }
            // Fall back to QEMU/UTM
            let provider = QemuProvider::new();
            Ok(PlatformHandle::Qemu(provider.launch(vm_profile)?))
        }
    }
}
```

---

## DockerProvider implementation

The `Provider` trait is sync — QEMU/UTM are sync by nature (child process spawn,
AppleScript). `DockerProvider` internally bridges bollard's async via
`futures_lite::block_on` (re-exported from `foundation_deployment_platform`).
Callers using the core `ContainerHandle` API directly (not through the Provider
trait) work with `async fn` natively via `.await`.

```rust
// In foundation_deployment_platform/src/providers/docker/mod.rs
use crate::docker::{ContainerHandle, ContainerServiceDefinition, WaitFor, block_on};

pub struct DockerProvider {
    client: DockerClient,
}

impl Provider for DockerProvider {
    type Handle = ContainerHandle;
    type Config = ContainerServiceDefinition;

    fn name(&self) -> &'static str { "docker" }
    fn id(&self) -> ProviderId { ProviderId::Docker }

    fn launch(&self, config: &ContainerServiceDefinition) -> Result<ContainerHandle> {
        block_on(ContainerHandle::start(config.clone()))
    }

    fn stop(&self, handle: &ContainerHandle) -> Result<()> {
        block_on(handle.shutdown())
    }

    fn is_running(&self, handle: &ContainerHandle) -> bool {
        block_on(handle.is_running()).unwrap_or(false)
    }

    fn resolved_ports(&self, handle: &ContainerHandle) -> Result<ResolvedPorts> {
        let ports = handle.host_ports();
        Ok(ResolvedPorts {
            ssh_port: ports.get("22/tcp").copied().unwrap_or(0),
            winrm_port: ports.get("5985/tcp").copied(),
            rdp_port: ports.get("3389/tcp").copied(),
            vnc_port: ports.get("5900/tcp").copied().unwrap_or(0),
        })
    }

    fn host_health(&self) -> HostHealth {
        let mut health = HostHealth::new("docker");
        health.check("docker_socket", || self.client.is_available());
        health.check("docker_daemon", || block_on(self.client.ping()));
        health.check("kvm_for_dockurr", || Path::new("/dev/kvm").exists());
        health
    }
}
```

The `block_on` here is `futures_lite::future::block_on` — re-exported, not
custom. It bridges the Provider trait's sync contract to the async bollard
calls. The same pattern applies to sync convenience wrappers on
`ContainerHandle` / `ContainerGroup` for callers that need them.

---

## Crate dependency graph

```
foundation_macros                ← #[docker_container] proc macro
  │                                 generates code referencing
  │                                 foundation_deployment_platform::docker
  │
  ▼
foundation_deployment_platform   ← Provider trait + all backends + guest infra
  │                                 + docker/ module (ContainerHandle, WaitFor, etc.)
  │                                 Depends: bollard, tokio, ssh2, foundation_netio
  │
  ├── foundation_testbed         ← thin consumer: wasm harness + CLI wrappers
  │                                 Depends: foundation_deployment_platform
  │
  └── foundation_deployment      ← cloud deployment providers (unchanged)
                                    Depends: foundation_deployment_platform (for
                                    remote Docker via bollard SSH, SSH via sshkit)
```

**No `foundation_deployment_platform` crate exists.** The Docker runtime lives
at `foundation_deployment_platform::docker`. This avoids:
- A proc-macro crate dependency tangle (Docker runtime is not a proc-macro crate)
- A three-crate chain (platform → docker → macros would be circular)
- Extra crate overhead for ~8 source files

---

## What moves where

| Current location | Moves to |
|-----------------|----------|
| `foundation_testbed/src/vms/` (everything) | `foundation_deployment_platform/src/` |
| `foundation_testbed/src/vms/providers/` | `foundation_deployment_platform/src/providers/` |
| `foundation_testbed/src/vms/ssh/` | `foundation_deployment_platform/src/ssh/` |
| `foundation_testbed/src/vms/winrm/` | `foundation_deployment_platform/src/winrm/` |
| `foundation_testbed/src/vms/bootstrap/` | `foundation_deployment_platform/src/bootstrap/` |
| `foundation_testbed/src/vms/qemu/` | `foundation_deployment_platform/src/qemu/` |
| `foundation_testbed/src/vms/config.rs` | `foundation_deployment_platform/src/config.rs` |
| `foundation_testbed/src/vms/cli/` | `foundation_deployment_platform/src/cli/` |
| `foundation_testbed/src/bin/testbed.rs` | `foundation_deployment_platform/src/bin/platform.rs` |
| `foundation_testbed/scripts/` | `foundation_deployment_platform/scripts/` |
| `foundation_testbed/tests/` (platform tests) | `foundation_deployment_platform/tests/` |
| **New:** `src/docker/` | `foundation_deployment_platform/src/docker/` (8 files) |
| **New:** `src/providers/docker/` | `foundation_deployment_platform/src/providers/docker/mod.rs` |
| `foundation_testbed/src/wasm/` | **Stays** in foundation_testbed |
| `foundation_testbed/tests/e2e_tauri.rs` | **Stays** in foundation_testbed |

---

## Colima support on macOS

On macOS, the Docker daemon can be provided by several backends:

| Backend | Cost | Notes |
|---------|------|-------|
| **Colima** | Free, OSS | QEMU/Lima under the hood. Sets `DOCKER_HOST` automatically. |
| **Docker Desktop** | Free/Paid | Official Docker Inc. product. |
| **OrbStack** | Free/Paid | Fast, native ARM64. |

All three expose a Docker socket that bollard connects to via
`connect_with_local_defaults()`. The `DockerProvider` doesn't care which
backend provides the socket — it just needs a working Docker daemon.

dockurr runs **on top of** Docker (whichever backend). It does not replace
Docker — it's a set of Docker images that wrap QEMU to run Windows/macOS.
dockurr needs two things:
1. A working Docker daemon (Colima, Docker Desktop, OrbStack, or native Linux)
2. `/dev/kvm` passthrough (for hardware acceleration; TCG fallback on Windows only)

### Platform dispatch on macOS

```
Docker daemon available (Colima/Docker Desktop/OrbStack)?
  ├── Yes → DockerProvider
  │         ├── Linux container test → native container (fast, ARM64)
  │         ├── Windows test → dockurr/windows container (needs /dev/kvm)
  │         └── macOS test → dockurr/macos container (needs /dev/kvm)
  │              If /dev/kvm unavailable → error for macOS, TCG fallback for Windows
  └── No  → UtmProvider (native Apple HVF)
              └── macOS guest → native macOS VM
              └── Linux guest → QEMU VM via UTM
              └── Windows guest → QEMU VM via UTM
```

The key insight: Colima/Docker Desktop provide the Docker runtime that
dockurr containers run on. They don't replace dockurr — they're the
infrastructure dockurr needs. On macOS without any Docker daemon, UTM is
the fallback for all guest types.
