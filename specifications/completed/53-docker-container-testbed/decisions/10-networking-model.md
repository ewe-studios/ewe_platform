# 04 — Networking Model

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Use **user-defined Docker bridge networks** for inter-container communication.
Containers on the same network resolve each other by container name via Docker's
built-in DNS (127.0.0.11). `NetworkHandle` provides RAII network management —
create on first use (idempotent), connect containers, optionally remove on drop.

## Table of Contents

1. [Why user-defined bridge networks](#why-user-defined-bridge-networks)
2. [NetworkHandle API](#networkhandle-api)
3. [DNS-based service discovery](#dns-based-service-discovery)
4. [Network lifecycle](#network-lifecycle)
5. [Port exposure](#port-exposure)

---

## Why user-defined bridge networks

Docker's default bridge network does **not** provide automatic DNS resolution
between containers. User-defined bridge networks do — container names become
DNS hostnames.

Example: On a user-defined network `testbed-net`, container `postgres-test`
is reachable from container `linux-build` as simply `postgres-test`:

```bash
# Inside linux-build:
$ ping postgres-test  # resolves to 172.20.0.3
$ psql -h postgres-test -U test -d testdb  # no IP hardcoding
```

This is critical for multi-service test scenarios (app + database + cache).

---

## NetworkHandle API

```rust
/// RAII guard for a Docker network. On Drop, optionally disconnects all
/// containers and removes the network.
///
/// Networks are cheap (no resources consumed when empty), so the default
/// is to NOT remove on drop — the network persists for the test session.
pub struct NetworkHandle {
    client: DockerClient,
    network_id: String,
    name: String,
    remove_on_drop: bool,
}

impl NetworkHandle {
    /// Create or find a bridge network. Idempotent — returns the existing
    /// network handle if one with the same name already exists.
    ///
    /// `subnet` is optional (e.g., "172.20.0.0/16"). If None, Docker
    /// auto-assigns a subnet.
    pub async fn create_or_find(
        client: &DockerClient,
        name: &str,
        subnet: Option<&str>,
    ) -> Result<Self, DockerError>;

    /// Find an existing network by name. Returns None if not found.
    pub async fn find(
        client: &DockerClient,
        name: &str,
    ) -> Result<Option<Self>, DockerError>;

    /// Connect a container to this network with optional DNS aliases.
    /// Aliases are additional hostnames the container answers to.
    pub async fn connect(
        &self,
        container_id: &str,
        aliases: &[&str],
    ) -> Result<(), DockerError>;

    /// The network's Docker ID.
    pub fn id(&self) -> &str;

    /// The network name.
    pub fn name(&self) -> &str;

    /// Mark the network for removal on Drop (off by default).
    pub fn remove_on_drop(mut self) -> Self;
}
```

### Usage from the proc macro

```rust
#[docker_container(
    image = "postgres:16",
    port = 5432,
    network = "testbed-net",
    env = [("POSTGRES_PASSWORD", "test")]
)]
#[valtron_test]
fn test_with_db() {
    // postgres is reachable at localhost:5432 (host port)
    // AND at container-name "testbed-test_with_db" on testbed-net
}
```

The `network = "testbed-net"` attribute tells the macro to:
1. Call `NetworkHandle::create_or_find("testbed-net", None)` before creating
   the container.
2. Pass the network name in `ContainerConfig::network("testbed-net")`.
3. This attaches the container to the network, enabling DNS resolution.

---

## DNS-based service discovery

Docker's embedded DNS (at 127.0.0.11 inside each container) resolves:

- **Container names** → container IP on the same network
- **Network aliases** → set via `--network-alias`, additional names
- **Service names** (Compose) → resolved to container IPs

For the `#[docker_container]` macro, the container name is the function name
(plus random suffix for uniqueness), so:

```rust
#[docker_container(image = "postgres:16", port = 5432, network = "testbed-net")]
fn test_app() {
    // From another container on testbed-net:
    // → resolves "testbed-test_app-a3f2" to the postgres container's IP
    // → connects on port 5432 (container port, no host mapping needed)
}
```

For predictable names, use `name = "mydb"` in the attribute, then other
containers can use `mydb` as the hostname.

---

## Network lifecycle

| Event | Action |
|-------|--------|
| First `#[docker_container(..., network = "X")]` | `create_or_find("X")` — creates the network (idempotent) |
| Subsequent containers on same network | `find("X")` — finds the existing network |
| Container start | `connect_container_to_network(id, network_id)` |
| Container stop (Drop) | Container removed from network automatically by Docker |
| All containers stopped | Network persists (empty, consumes no resources) |
| `NetworkHandle::drop()` with `remove_on_drop` | Network removed |

Default: networks persist across test runs. This means the first test run
creates the network, subsequent runs reuse it. Manual cleanup via
`docker network prune` or `NetworkHandle::remove_on_drop()`.

---

## Port exposure

Two distinct purposes for port mappings:

1. **Host access** — `port = 5432` maps container port 5432 to a host port
   (auto-assigned or explicit). Used when the test body needs to connect from
   the host (e.g., a Rust PostgreSQL client connecting to `localhost:<port>`).

2. **Inter-container access** — No port mapping needed. Containers on the same
   network connect directly on the container's port. Docker's internal DNS and
   routing handle this.

For the proc macro, `port = 5432` always creates a host mapping (needed for the
test body). Inter-container access works automatically when both containers
specify the same `network = "..."`.
