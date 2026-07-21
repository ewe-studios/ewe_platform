# 03 — Container Lifecycle

**Date:** 2026-07-08
**Status:** Resolved

## Decision

The container lifecycle follows an 8-stage pipeline: **Pull → Create → Start →
Inspect → Wait → Use → Stop → Remove**. `ContainerHandle::start(config)` drives
stages 1–5, the user's function body is stage 6, and `ContainerHandle::Drop`
drives stages 7–8 (even on panic).

## Table of Contents

1. [Lifecycle stages](#lifecycle-stages)
2. [ContainerHandle::start()](#containerhandlestart)
3. [ContainerHandle::Drop](#containerhandledrop)
4. [Port resolution](#port-resolution)
5. [Container naming](#container-naming)
6. [Health and readiness](#health-and-readiness)

---

## Lifecycle stages

```
Pull         create_image() stream — consume all chunks for pull to complete
  │            If image exists locally and always_pull=false, skip.
  ▼
Create       create_container(ContainerCreateBody) — returns container ID
  │            Config: image, ports, env, volumes, network, memory, cpus.
  ▼
Start        start_container(id) — container process begins
  │
  ▼
Inspect      inspect_container(id) — read back resolved host ports,
  │            network IP, container state.
  ▼
Wait         Apply WaitFor strategy — poll until ready or timeout.
  │            Port: TCP connect loop. Stdout: log stream scan.
  │            HTTP: GET loop. Composite: serial AND.
  ▼
Use          User function body runs. ContainerHandle alive in scope.
  │            handle.host_port(6379) → u16. handle.id() → &str.
  ▼
Stop         stop_container(id, {t: stop_timeout}) — SIGTERM, wait, SIGKILL
  │            Best-effort on Drop (errors logged, not propagated).
  ▼
Remove       remove_container(id, {force: true, v: true})
              Best-effort on Drop.
```

---

## ContainerHandle::start()

```rust
impl ContainerHandle {
    /// Start a container from the given configuration. Drives stages 1–5 of
    /// the lifecycle, returning a handle that will clean up on Drop.
    ///
    /// # Lifecycle
    /// 1. Pull image if not cached (or if always_pull)
    /// 2. Create container with the given config
    /// 3. Start the container
    /// 4. Inspect to resolve assigned host ports and network IPs
    /// 5. Apply wait strategy (block until ready or timeout)
    pub async fn start(config: ContainerConfig) -> Result<Self, DockerError>;
}
```

The method is async. Sync callers use `futures_lite::block_on` (re-exported by
`foundation_deployment_platform`) or `.await` inside valtron's `block_on_future`.

### Error handling within start()

If any stage 1–5 fails, the error is returned and **no cleanup is needed** —
the container was either never created or never started. The caller gets the
`DockerError` and can inspect it (e.g., `is_connection_error()` for graceful
skip).

If stages 1–4 succeed but stage 5 (wait) times out, the container is stopped
and removed before returning the error — the partially-started container does
not leak.

---

## ContainerHandle::Drop

```rust
use futures_lite::future::block_on;  // re-exported by foundation_deployment_platform

impl Drop for ContainerHandle {
    fn drop(&mut self) {
        let docker = self.docker.clone();
        let id = self.container_id.clone();
        let timeout = self.stop_timeout;

        // Best-effort: stop with timeout, then force-remove.
        // Errors are logged at warn level, never propagated from Drop.
        // futures_lite::block_on bridges the async bollard calls from sync Drop.
        let _ = block_on(async {
            // 1. Stop (graceful)
            let stop_opts = bollard::container::StopContainerOptions { t: timeout as i64 };
            let _ = docker.stop_container(&id, Some(stop_opts)).await;

            // 2. Remove (force)
            let remove_opts = bollard::container::RemoveContainerOptions {
                force: true,
                v: true,  // also remove anonymous volumes
                ..Default::default()
            };
            let _ = docker.remove_container(&id, Some(remove_opts)).await;
        });
    }
}
```

Key properties:
- **Panic-safe** — Drop runs during unwinding. The container IS cleaned up on
  test failure.
- **Best-effort** — Stop/remove failures are logged (tracing::warn!) not
  propagated. A stuck container is better than a double-panic abort.
- **`futures_lite::block_on`** bridges async bollard calls from the sync Drop
  context. Safe because Drop only fires from sync contexts (stack unwinding,
  explicit drop, end of scope) where no async reactor is mid-poll.

---

## Port resolution

When a container is created with auto-assigned host ports (`host_port: None`,
mapped to `"0"` in the port binding), Docker assigns a random available port.
This port is not known until after `start_container` completes.

`ContainerHandle::start()` calls `inspect_container()` after start to read back
the resolved ports from `NetworkSettings.Ports`:

```rust
fn resolve_ports(
    info: &ContainerInspectResponse,
    requested: &[PortMapping],
) -> HashMap<String, u16> {
    let mut ports = HashMap::new();
    if let Some(settings) = &info.network_settings {
        if let Some(bindings) = &settings.ports {
            for mapping in requested {
                let key = format!("{}/tcp", mapping.container_port);
                if let Some(Some(bindings)) = bindings.get(&key) {
                    if let Some(first) = bindings.first() {
                        if let Some(host_port) = &first.host_port {
                            if let Ok(port) = host_port.parse::<u16>() {
                                ports.insert(key, port);
                            }
                        }
                    }
                }
            }
        }
    }
    ports
}
```

The caller accesses ports via `handle.host_port(6379) -> u16`. This is a simple
lookup on the pre-resolved map — no async, no fallibility.

---

## Container naming

By default, `ContainerConfig` generates a unique container name:

```
testbed-{profile_name}-{random_suffix}
```

For the proc macro, the profile name is the function name (e.g.,
`testbed-test_redis-a3f2`). This avoids name collisions when multiple tests
run in parallel.

Users can override with `name = "my-container"` in the macro attribute. Fixed
names are useful for debugging (`docker ps` shows a human-readable name) but
risk collisions if two tests run concurrently with the same name.

---

## Health and readiness

Docker health checks (`HEALTHCHECK` in Dockerfile) are supported but not
required. The `WaitFor::healthcheck()` variant (future) would poll
`inspect_container() → State.Health.Status == "healthy"`.

For images without built-in health checks (most), the `WaitFor` strategies
(Port, Http, Stdout) provide the readiness signal. See decision 05 for details.
