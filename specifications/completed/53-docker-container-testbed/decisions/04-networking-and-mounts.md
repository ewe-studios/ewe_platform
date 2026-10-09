# 04 — Networking & Volume Mounts

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Use **user-defined Docker bridge networks** for inter-container communication
and **explicit port mappings** for host-to-container access (SSH, RDP, VNC).
Use **bind mounts** (`-v /host/path:/container/path`) for project source code
sharing (replacing 9p/virtiofs) and **named Docker volumes** for persistent
caches (cargo registry, build artifacts, VM disk storage). This eliminates the
two biggest QEMU pain points: fragile filesystem mounts and brittle port
forwarding.

## Table of Contents

1. [Network model](#network-model)
2. [Why user-defined bridge networks](#why-user-defined-bridge-networks)
3. [Network lifecycle](#network-lifecycle)
4. [Port mapping strategy](#port-mapping-strategy)
5. [Volume mount strategy](#volume-mount-strategy)
6. [Comparison: QEMU pain points → Docker solutions](#comparison-qemu-pain-points--docker-solutions)
7. [Multi-service topologies](#multi-service-topologies)

---

## Network model

```
┌─────────────────────────────────────────────────────────────┐
│  Docker Host                                                 │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Network: testbed-net (bridge, 172.20.0.0/16)         │  │
│  │                                                       │  │
│  │  ┌──────────────────┐  ┌──────────────────┐           │  │
│  │  │ linux-build      │  │ postgres-test    │           │  │
│  │  │ IP: 172.20.0.2   │  │ IP: 172.20.0.3   │           │  │
│  │  │ SSH: :22→host:2422│ │ PG: :5432→host:5432│          │  │
│  │  │ Vol: /mnt/project │  │ Vol: pg-data      │           │  │
│  │  └──────────────────┘  └──────────────────┘           │  │
│  │           │                      │                     │  │
│  │           └──────────────────────┘                     │  │
│  │                  internal DNS resolves                  │  │
│  │                  "linux-build" ↔ "postgres-test"        │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Host access:                                               │
│    127.0.0.1:2422 → linux-build:22 (SSH)                    │
│    127.0.0.1:5432 → postgres-test:5432 (PostgreSQL)         │
└─────────────────────────────────────────────────────────────┘
```

---

## Why user-defined bridge networks

Docker's default bridge network does not provide automatic DNS resolution
between containers. User-defined bridge networks do — container names become
DNS hostnames resolvable by all containers on the same network.

This is critical for multi-service test scenarios:

```yaml
# Inside linux-build container:
$ ping postgres-test     # Resolves to 172.20.0.3 via Docker's built-in DNS
$ psql -h postgres-test  # Connects to the test database
```

No port mapping needed for container-to-container communication — only for
host-to-container access (SSH, RDP, VNC).

### Network configuration

```rust
fn ensure_network(&self, name: &str) -> Result<()> {
    let config = bollard::network::CreateNetworkOptions {
        name,
        driver: String::from("bridge"),
        ipam: bollard::models::Ipam {
            driver: Some(String::from("default")),
            config: Some(vec![bollard::models::IpamConfig {
                subnet: Some(String::from("172.20.0.0/16")),
                gateway: Some(String::from("172.20.0.1")),
                ..Default::default()
            }]),
            ..Default::default()
        },
        attachable: true,
        ..Default::default()
    };

    // Idempotent — creates if missing, no-op if exists
    rt().block_on(self.docker.create_network(config))?;
    Ok(())
}
```

### Compose equivalent

If using the Compose path, the network is defined declaratively:

```yaml
networks:
  testbed-net:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1
```

---

## Network lifecycle

| Event | Action |
|-------|--------|
| First `testbed start` | Create `testbed-net` network (idempotent). |
| `testbed start linux-build` | Attach container to `testbed-net`. |
| `testbed start postgres-test` | Attach to same network — can reach `linux-build` by name. |
| `testbed stop linux-build` | Container removed; network persists. |
| All containers stopped | Network persists (cheap, no resources used). |
| `testbed network prune` | Remove the testbed network (explicit cleanup). |

The network is shared across all concurrently running testbed containers. This
is safe because container names are unique per profile — you can't run two
`linux-build` containers at once (same as QEMU today with port conflicts).

---

## Port mapping strategy

| Container port | Host port | Purpose | Notes |
|---------------|-----------|---------|-------|
| 22 | Profile's `ssh_port` (e.g., 2422) | SSH access | Maps 1:1 from QEMU profiles; no change for callers |
| 3389 | Profile's `rdp_port` (e.g., 3389) | RDP (Windows only) | Only mapped for dockurr/windows |
| 5985 | Profile's `winrm_port` | WinRM (Windows only) | Only mapped during bootstrap phase |
| 8006 | Profile-specific | Web viewer (dockurr) | Debug fallback |
| App-specific | Dynamic (bollard auto-assign) | Test application ports | `resolved_ports()` reads back the assignment |

Port conflicts are handled the same way as QEMU today: each profile gets fixed
ports, and `serial_test` prevents concurrent use of the same profile. Docker's
port binding fails with a clear error if the port is taken:

```
Error: port is already allocated
```

This is actually better than QEMU, where `hostfwd` silently binds to a
different interface or fails in confusing ways.

---

## Volume mount strategy

### Three mount types

| Type | Docker mechanism | Use case | Lifecycle |
|------|-----------------|----------|-----------|
| **Bind mount** | `-v /host/path:/container/path` | Project source code, test fixtures | Matches container lifetime |
| **Named volume** | `-v volume-name:/container/path` | Cargo registry, build cache, VM disks | Persists across container restarts |
| **tmpfs** | `--tmpfs /container/path` | Ephemeral test data | Dies with container |

### Bind mount (project source)

Replaces QEMU's 9p/virtiofs mounts. Dramatically simpler:

```rust
// QEMU (before): complex virtfs setup with security model, mount_tag, etc.
// "-virtfs local,path=/host/project,mount_tag=project,security_model=mapped-xattr,..."

// Docker (after): one line
HostConfig {
    binds: Some(vec![
        format!("{}:/mnt/project:ro", project_dir.display()),
    ]),
    ..Default::default()
}
```

Bind mounts use the host kernel's filesystem directly — there is no performance
penalty, no 9p protocol overhead, no cache coherency issues. Changes on the
host are immediately visible in the container and vice versa.

For Windows (dockurr), the project is shared via SMB or virtiofs inside the VM,
same as today. dockurr's `/shared` volume maps a host folder to the Windows
desktop.

### Named volumes (persistent caches)

```rust
// Cargo registry cache — survives container rebuilds
Volume {
    name: "linux-build-cargo",
    mount_point: "/root/.cargo",
}

// Build artifacts — shared across test runs
Volume {
    name: "linux-build-target",
    mount_point: "/mnt/project/target",
}

// Windows VM disk (dockurr)
Volume {
    name: "windows-build-storage",
    mount_point: "/storage",
}
```

Named volumes are created once and reused across container recreations. This
means:
- Cargo doesn't re-download the entire registry on each `testbed start`.
- Incremental builds work — `target/` persists.
- dockurr's Windows disk image persists across restarts.

### Volume lifecycle

```
testbed start linux-build
  → Docker creates container with volumes attached
  → If named volumes don't exist, Docker creates them

testbed stop linux-build
  → Container removed
  → Named volumes PERSIST (cargo cache, VM disk)

testbed start linux-build  (again)
  → Container uses SAME named volumes
  → Cargo cache is warm, build is incremental

testbed cleanup --volumes
  → Explicitly remove named volumes (frees disk space)
```

---

## Comparison: QEMU pain points → Docker solutions

| Pain point (QEMU) | Docker solution |
|-------------------|-----------------|
| 9p/virtiofs mounts: permissions, symlinks, case sensitivity bugs | Bind mounts: kernel-level, no issues |
| User-mode port forwarding: `hostfwd` syntax, no error on conflict | Docker port binding: clear API, clear errors |
| No DNS between VMs: must hardcode IPs | Built-in DNS: container name → IP |
| SMB for Windows file sharing: fragile, requires samba config | dockurr `/shared` volume or virtiofs inside VM |
| `virtiofsd` daemon management: separate process, must coordinate with QEMU | Docker volume driver: managed by daemon |
| QEMU monitor socket for introspection | `docker inspect` for structured state, `docker exec` for commands |
| PID tracking + stale PID detection | Container IDs — always accurate via Docker API |

---

## Multi-service topologies

The network model enables test scenarios that were impractical with QEMU:

### Example: Tauri app + PostgreSQL integration test

```yaml
# compose.yaml
services:
  linux-build:
    image: testbed/linux-build:latest
    ports: ["2422:22"]
    volumes:
      - ${PROJECT_DIR}:/mnt/project
      - linux-build-cargo:/root/.cargo
    networks:
      - testbed-net

  postgres-test:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: test
      POSTGRES_DB: app_test
    networks:
      - testbed-net
    # No host port mapping needed — only linux-build talks to it

networks:
  testbed-net:
    driver: bridge

volumes:
  linux-build-cargo:
```

Inside `linux-build`, the test suite connects to `postgres-test:5432` via
Docker's internal DNS. No IP addresses are hardcoded, no port conflicts with
the host.

### Lifecycle orchestration

```rust
// Start the whole stack
testbed compose up linux-build  // starts linux-build + postgres-test

// Run tests
testbed exec linux-build -- "cargo test -- --test-threads=1"

// Check database
testbed exec postgres-test -- "psql -U postgres -d app_test -c 'SELECT count(*) FROM users;'"

// Tear down
testbed compose down  // stops and removes all services
```
