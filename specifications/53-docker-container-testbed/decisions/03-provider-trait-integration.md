# 03 — Provider Trait Integration

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Add `DockerProvider` as a new implementation of the existing `Provider` trait,
sitting alongside `QemuProvider` and `UtmProvider`. The trait surface is already
a good fit: `launch`, `stop`, `is_running`, `resolved_ports`, `ensure_image`,
`host_health` all map cleanly to Docker operations. `default_provider()` changes
to prefer Docker on Linux hosts when Docker is available, falling back to QEMU
when it's not (no Docker socket, or KVM required for non-Linux guests).

## Table of Contents

1. [Trait surface mapping](#trait-surface-mapping)
2. [DockerProvider implementation](#dockerprovider-implementation)
3. [VmHandle for Docker containers](#vmhandle-for-docker-containers)
4. [Provider selection logic](#provider-selection-logic)
5. [Profile dispatch](#profile-dispatch)
6. [What changes in callers](#what-changes-in-callers)

---

## Trait surface mapping

The existing `Provider` trait (simplified):

```rust
pub trait Provider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn launch(&self, profile: &VmProfile, display: DisplayMode) -> Result<VmHandle>;
    fn stop(&self, handle: &VmHandle) -> Result<()>;
    fn is_running(&self, handle: &VmHandle) -> Result<bool>;
    fn resolved_ports(&self, handle: &VmHandle) -> Result<HashMap<String, u16>>;
    fn monitor_command(&self, handle: &VmHandle, cmd: &str) -> Result<String>;
    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf>;
    fn host_health(&self) -> Result<Vec<HealthCheck>>;
}
```

### How each method maps to Docker:

| Trait method | Docker equivalent | Notes |
|-------------|-------------------|-------|
| `launch()` | `docker create` + `docker start`, or `docker compose up -d` | Returns a `VmHandle` with container ID |
| `stop()` | `docker stop` + `docker rm` | Graceful (SIGTERM) then force (SIGKILL) |
| `is_running()` | `docker inspect` → check `State.Running` | Fast, single API call |
| `resolved_ports()` | `docker inspect` → read `NetworkSettings.Ports` | Returns host ports mapped to container ports |
| `monitor_command()` | `docker exec container cmd` | Maps to container exec for QEMU-monitor-like introspection |
| `ensure_image()` | `docker pull` or `docker build` | Pull from registry or build from Dockerfile |
| `host_health()` | Check Docker socket exists + daemon reachable + disk space | Replaces KVM/QEMU binary checks |

---

## DockerProvider implementation

```rust
pub struct DockerProvider {
    /// Bollard client connected to the local Docker daemon.
    docker: Docker,
    /// Path to Compose files directory (optional).
    compose_dir: Option<PathBuf>,
}

impl DockerProvider {
    /// Connect to the local Docker daemon (Unix socket or named pipe).
    pub fn local() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        Ok(Self { docker, compose_dir: None })
    }

    /// Connect to a remote Docker daemon over SSH.
    pub fn remote_ssh(host: &str, key_path: &Path, user: &str, port: u16) -> Result<Self> {
        let docker = Docker::connect_with_ssh(host, &[key_path], user, port)?;
        Ok(Self { docker, compose_dir: None })
    }

    /// Use Compose files for launch/stop (escapes to CLI for `up`/`down`).
    pub fn with_compose(mut self, dir: PathBuf) -> Self {
        self.compose_dir = Some(dir);
        self
    }
}
```

### launch() implementation

```rust
impl Provider for DockerProvider {
    fn launch(&self, profile: &VmProfile, display: DisplayMode) -> Result<VmHandle> {
        let container_profile = ContainerProfile::from_vm_profile(profile)?;

        if let Some(ref compose_dir) = self.compose_dir {
            // Compose path: shell out to docker compose up -d
            self.launch_via_compose(compose_dir, &container_profile)
        } else {
            // Bollard path: programmatic container creation
            self.launch_via_bollard(&container_profile)
        }
    }
}

impl DockerProvider {
    fn launch_via_bollard(&self, profile: &ContainerProfile) -> Result<VmHandle> {
        // 1. Pull image if not present
        self.ensure_image_bollard(profile)?;

        // 2. Create network if specified
        if let Some(ref network) = profile.network {
            self.ensure_network(network)?;
        }

        // 3. Build container config
        let config = self.build_container_config(profile);

        // 4. Create container
        let create_result = rt().block_on(self.docker.create_container(
            Some(bollard::container::CreateContainerOptions {
                name: &profile.name,
                ..Default::default()
            }),
            config,
        ))?;

        // 5. Start container
        rt().block_on(self.docker.start_container(
            &create_result.id,
            None::<bollard::container::StartContainerOptions<String>>,
        ))?;

        // 6. Wait for health check or SSH port
        self.wait_for_ready(&create_result.id, profile)?;

        Ok(VmHandle {
            provider: ProviderId::Docker,
            id: create_result.id,
            vm_name: profile.name.clone(),
            ports: profile.port_map(),
            state_path: profile.state_dir(),
        })
    }

    fn launch_via_compose(&self, dir: &Path, profile: &ContainerProfile) -> Result<VmHandle> {
        // Shell out: docker compose -f <dir>/compose.yaml up -d
        let output = Command::new("docker")
            .args(["compose", "-f", &dir.join("compose.yaml").to_string_lossy(), "up", "-d"])
            .output()?;

        if !output.status.success() {
            return Err(TestbedError::LaunchFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        // Inspect the started container to get its ID
        let container_id = self.resolve_compose_container(&profile.name)?;
        // … same as bollard path from here
    }
}
```

### ensure_image() mapping

In the QEMU world, `ensure_image()` downloads a qcow2 disk from Vagrant Cloud
or a URL. In Docker:

| Profile type | ensure_image() behavior |
|-------------|------------------------|
| Pre-built image from registry | `docker pull <image>:<tag>` via bollard's `create_image()` |
| Dockerfile on disk | `docker build -t <tag> -f <dockerfile> <context>` (shell out; bollard's BuildKit support is behind a feature flag and less tested) |
| dockurr image | `docker pull dockurr/windows` — no custom build needed |

### stop() implementation

```rust
fn stop(&self, handle: &VmHandle) -> Result<()> {
    // Graceful stop (SIGTERM, 10s timeout)
    let stop_opts = bollard::container::StopContainerOptions { t: 10 };
    let _ = rt().block_on(self.docker.stop_container(&handle.id, Some(stop_opts)));

    // Remove container (and associated anonymous volumes)
    let remove_opts = bollard::container::RemoveContainerOptions {
        force: true,
        v: true,  // remove anonymous volumes
        ..Default::default()
    };
    rt().block_on(self.docker.remove_container(&handle.id, Some(remove_opts)))?;

    Ok(())
}
```

### is_running() implementation

```rust
fn is_running(&self, handle: &VmHandle) -> Result<bool> {
    let inspect = rt().block_on(self.docker.inspect_container(&handle.id, None))?;
    Ok(inspect.state.and_then(|s| s.running).unwrap_or(false))
}
```

### host_health() — replaces KVM checks

```rust
fn host_health(&self) -> Result<Vec<HealthCheck>> {
    let mut checks = Vec::new();

    // Docker socket accessible?
    checks.push(HealthCheck::new("docker_socket")
        .check(|| Docker::connect_with_local_defaults().is_ok()));

    // Docker daemon responding?
    checks.push(HealthCheck::new("docker_ping")
        .check(|| rt().block_on(self.docker.ping()).is_ok()));

    // Disk space for images?
    let info = rt().block_on(self.docker.system_info())?;
    // …

    Ok(checks)
}
```

---

## VmHandle for Docker containers

The existing `VmHandle` already carries a string `id` (QEMU PID) and provider
identifier. For Docker, `id` becomes the container ID (64-char hex string),
and `provider` becomes `ProviderId::Docker`.

```rust
pub enum ProviderId {
    Qemu,
    Utm,
    Docker,  // ← NEW
}
```

The `VmHandle` state file path (`.testbed/<name>/state/`) is unchanged —
Docker containers don't have a persistent PID on the host, but we store the
container ID and can always `docker inspect <id>` to check liveness.

---

## Provider selection logic

```rust
pub fn default_provider() -> Box<dyn Provider> {
    // macOS: UTM remains default (Docker Desktop is optional)
    if cfg!(target_os = "macos") {
        return Box::new(UtmProvider::new());
    }

    // Linux: prefer Docker if available, fall back to QEMU
    if cfg!(target_os = "linux") {
        if let Ok(docker) = DockerProvider::local() {
            // Docker is available — use it for Linux profiles,
            // but we still need QEMU for Windows/macOS profiles
            return Box::new(docker);
        }
        return Box::new(QemuProvider::new());
    }

    // Windows: QEMU (Windows containers are a different beast)
    Box::new(QemuProvider::new())
}
```

Note: `default_provider()` returns a single provider, but the provider may
need to delegate to another for profiles it can't handle. For example,
`DockerProvider` handles `GuestOs::Linux` natively but delegates
`GuestOs::Windows` to `QemuProvider` (or to itself via the dockurr path).

This delegation happens inside `DockerProvider::launch()` based on
`profile.guest_os`:

```rust
fn launch(&self, profile: &VmProfile, display: DisplayMode) -> Result<VmHandle> {
    match profile.guest_os {
        GuestOs::Linux => self.launch_linux_container(profile),
        GuestOs::Windows => self.launch_windows_dockurr(profile),
        GuestOs::MacOs => {
            // Delegate to QEMU — DockerProvider can't do macOS
            let qemu = QemuProvider::new();
            qemu.launch(profile, display)
        }
    }
}
```

---

## Profile dispatch

The `VmProfile` → `ContainerProfile` mapping happens at launch time:

```rust
impl ContainerProfile {
    pub fn from_vm_profile(vm_profile: &VmProfile) -> Result<Self> {
        let base = match vm_profile.guest_os {
            GuestOs::Linux => CONTAINER_PROFILES.linux_build.clone(),
            GuestOs::Windows => CONTAINER_PROFILES.windows_build.clone(),
            GuestOs::MacOs => return Err(TestbedError::UnsupportedOs(
                "macOS containers are not supported; use QEMU/UTM".into()
            )),
        };

        // Override with user config from testbed.toml
        let overrides = load_container_overrides(&vm_profile.name)?;
        Ok(base.apply(overrides))
    }
}
```

---

## What changes in callers

**Zero changes** to callers that use the `Provider` trait. The trait surface is
unchanged. Callers already get a `Box<dyn Provider>` from `default_provider()`
and call `.launch()`, `.stop()`, etc.

The only behavioral difference is that `default_provider()` now returns a
`DockerProvider` instead of a `QemuProvider` on Linux hosts with Docker.

**SSH and bootstrap code is unchanged** — the Docker container runs an SSH
daemon on port 22, mapped to the same host port the QEMU profile used. The
`ssh2` client connects to `127.0.0.1:<ssh_port>` exactly as before.

**CLI is unchanged** — `testbed start linux-build` works regardless of whether
the provider is QEMU or Docker. Users can force a specific provider:

```bash
testbed start linux-build --provider qemu    # Force QEMU
testbed start linux-build --provider docker  # Force Docker
```
