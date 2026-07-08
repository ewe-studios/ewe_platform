# 06 — Cloud Deployment

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Adopt the **cloud-init + remote Docker** pattern from `vm-uncloud` for deploying
the testbed to cloud providers (Hetzner, AWS, GCP). The core insight from
vm-uncloud is: (1) cloud-init provisions the bare minimum (curl + ca-certificates),
(2) a bootstrap phase installs Docker and the testbed binary over SSH, and
(3) all subsequent operations go through bollard's SSH transport or the testbed
CLI over SSH. This replaces the current `HETZNER.md` pattern of installing QEMU
via apt and managing VMs inside a cloud VM.

## Table of Contents

1. [vm-uncloud patterns we adopt](#vm-uncloud-patterns-we-adopt)
2. [Cloud-init template](#cloud-init-template)
3. [Provisioning flow](#provisioning-flow)
4. [Remote Docker via bollard SSH](#remote-docker-via-bollard-ssh)
5. [Hetzner-specific configuration](#hetzner-specific-configuration)
6. [Multi-provider abstractions](#multi-provider-abstractions)
7. [What we don't adopt from vm-uncloud](#what-we-dont-adopt-from-vm-uncloud)

---

## vm-uncloud patterns we adopt

vm-uncloud's architecture has several patterns that apply directly to the
testbed cloud deployment:

### 1. Minimal cloud-init

vm-uncloud's `cloud-init/uncloud.yaml` installs only `curl` and
`ca-certificates`. It deliberately does NOT install Docker or the application
— those are installed by a subsequent SSH-based bootstrap step (`uc machine init`).

We apply the same principle:

```yaml
#cloud-config
package_update: true
package_upgrade: false
packages:
  - curl
  - ca-certificates
```

Rationale: cloud-init runs once at VM creation. If Docker's apt repository
changes, a cloud-init that installs Docker would fail on every new VM until
the template is updated. By deferring Docker installation to a script that
runs over SSH, we can update the install logic without rebuilding VM images.

### 2. Remote-exec provisioner for readiness

vm-uncloud uses an OpenTofu `remote-exec` provisioner that blocks on
`cloud-init status --wait` before proceeding. This ensures the host is ready
before any SSH commands run.

### 3. Single wildcard DNS (optional, for multi-node)

vm-uncloud uses a single `*.domain.com` wildcard DNS record for all services,
with Caddy providing TLS termination. This simplifies DNS management to one
record. For the testbed, this is only relevant if we deploy a Caddy ingress
or need HTTPS for web-based test viewers.

### 4. Secrets never on disk

vm-uncloud uses the macOS keychain (via `fnox`) for API tokens. We follow the
same principle: Hetzner tokens, registry credentials, and SSH keys come from
environment variables or a local secrets store, never from files committed to
the repo.

---

## Cloud-init template

```yaml
# testbed_cloud_init.yaml
#cloud-config
package_update: true
package_upgrade: false

packages:
  - curl
  - ca-certificates
  - ufw

# Basic firewall: allow SSH only
write_files:
  - path: /etc/ufw/user.rules
    permissions: '0640'
    content: |
      *filter
      :INPUT DROP [0:0]
      :FORWARD DROP [0:0]
      :OUTPUT ACCEPT [0:0]
      -A INPUT -i lo -j ACCEPT
      -A INPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
      -A INPUT -p tcp --dport 22 -j ACCEPT
      # Allow Docker bridge network traffic
      -A INPUT -s 172.20.0.0/16 -j ACCEPT
      COMMIT

runcmd:
  - ufw --force enable
  - cloud-init status --wait  # signal to provisioner that we're done
```

Key points:
- **No Docker install in cloud-init** — Docker is installed by the bootstrap
  script over SSH, ensuring the latest version and correct repository setup.
- **UFW firewall** — Blocks everything except SSH (port 22) and internal
  Docker bridge traffic. Docker ports (SSH into containers) are not exposed
  to the internet; they're reached through the wireguard mesh or SSH tunnel.
- **`cloud-init status --wait`** — The signal that the provisioner blocks on.

---

## Provisioning flow

```
┌─────────────────────────────────────────────────────────────────┐
│  Step 1: Create cloud VM                                        │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  testbed cloud create --provider hetzner                   │  │
│  │    → Creates Hetzner CX42 (4 vCPU, 16 GB)                  │  │
│  │    → Attaches cloud-init user_data (testbed_cloud_init)     │  │
│  │    → Creates firewall rules (SSH + WireGuard)               │  │
│  │    → Blocks until cloud-init completes                      │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│                              ▼                                   │
│  Step 2: Bootstrap the host                                     │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  testbed cloud bootstrap <host>                             │  │
│  │    → SSH into the host                                      │  │
│  │    → curl -fsSL https://get.docker.com | sh                 │  │
│  │    → Install Docker Compose plugin                          │  │
│  │    → Pull testbed Docker images (linux-build, etc.)          │  │
│  │    → Verify Docker daemon is running                        │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│                              ▼                                   │
│  Step 3: Run tests                                              │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Option A: Local bollard → SSH → remote Docker              │  │
│  │    testbed start linux-build --remote ssh://hetzner-box     │  │
│  │                                                             │  │
│  │  Option B: SSH into host, run testbed CLI there              │  │
│  │    ssh hetzner-box "testbed start linux-build"               │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│                              ▼                                   │
│  Step 4: Tear down                                              │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  testbed cloud destroy <host>                               │  │
│  │    → Stops all running containers                           │  │
│  │    → Destroys the cloud VM                                  │  │
│  │    → Removes firewall rules                                 │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Remote Docker via bollard SSH

Bollard supports connecting to a remote Docker daemon over SSH without
exposing the Docker socket over TCP:

```rust
use bollard::Docker;

/// Connect to Docker on a remote host over SSH.
///
/// This tunnels the Docker API through an SSH connection — the Docker
/// socket is never exposed to the network. Authentication is handled
/// by SSH keys, not TLS certificates.
pub fn connect_remote(host: &str) -> Result<Docker> {
    Docker::connect_with_ssh(
        host,                                    // e.g., "hetzner-box.example.com"
        &["/home/user/.ssh/id_ed25519"],         // SSH key path(s)
        "root",                                  // SSH user
        22,                                      // SSH port
    )
}
```

This is the **primary remote access pattern**. Instead of:
1. SSH into the remote host
2. Run `docker` CLI commands there
3. Parse stdout

We do:
1. bollard connects to the remote Docker daemon over SSH
2. All Docker API calls (create, start, exec, inspect, stop) go through the
   SSH tunnel natively
3. Results are typed Rust structs, not string parsing

### When SSH tunneling doesn't work

For environments where bollard SSH isn't feasible (restricted networks, jump
hosts), the fallback is the same as today: scp the cross-compiled static binary
(`x86_64-unknown-linux-musl`) to the host and run the testbed CLI there:

```bash
# Build static binary
cargo zigbuild --release --target x86_64-unknown-linux-musl -p foundation_testbed

# Deploy
scp target/x86_64-unknown-linux-musl/release/testbed root@hetzner-box:/usr/local/bin/

# Run remotely
ssh root@hetzner-box "testbed start linux-build"
```

This is identical to the existing `mise.toml` Hetzner pipeline.

---

## Hetzner-specific configuration

Following the existing `HETZNER.md` and `mise.toml` patterns, but adapted for
Docker:

```toml
# mise.toml (updated tasks)

[tasks.cloud.create]
description = "Create a Hetzner cloud VM for testbed"
run = "testbed cloud create --provider hetzner --type cx42 --location fsn1"

[tasks.cloud.destroy]
description = "Tear down the Hetzner cloud VM"
run = "testbed cloud destroy"

[tasks.cloud.test]
description = "Full cloud test cycle: create → bootstrap → test → destroy"
depends = ["cloud.create", "cloud.bootstrap"]
run = "testbed start linux-build --remote && testbed build linux-build && testbed cloud destroy"
```

### VM sizing for Docker workloads

| Hetzner type | vCPU | RAM | Suitable for | Cost (€/h) |
|-------------|------|-----|-------------|------------|
| CX32 | 4 | 8 GB | Single Linux container builds | ~0.05 |
| CX42 | 4 | 16 GB | Linux + Windows (dockurr) concurrently | ~0.08 |
| CX52 | 8 | 32 GB | Multi-platform parallel builds | ~0.15 |

The move to Docker reduces the cloud VM requirement: QEMU VMs needed KVM
support (CX22 minimum, which has nested virtualization), plus extra RAM for
each VM. Docker containers share the host kernel and are much more
memory-efficient.

For dockurr/windows, we still need KVM — Hetzner CX22+ supports nested
virtualization, same as today.

---

## Multi-provider abstractions

The `CloudProvider` trait abstracts over cloud vendors:

```rust
pub trait CloudProvider: Send + Sync {
    /// Create a cloud VM, return its IP address.
    fn create_vm(&self, config: &CloudVmConfig) -> Result<CloudVm>;
    /// Destroy a cloud VM.
    fn destroy_vm(&self, vm: &CloudVm) -> Result<()>;
    /// Get cloud-init user data for this provider.
    fn cloud_init(&self) -> String;
}
```

Initial implementation: `HetznerCloudProvider` (using the `hcloud` HTTP API).
Future: `AwsCloudProvider`, `GcpCloudProvider`.

---

## What we don't adopt from vm-uncloud

vm-uncloud uses several patterns that don't apply to the testbed:

| vm-uncloud feature | Why we skip it |
|-------------------|----------------|
| **OpenTofu/Terraform** for infrastructure | Overkill for testbed — we create one VM, not a mesh. The `hcloud` API directly is simpler. |
| **WireGuard mesh** between nodes | Single-node testbed. Docker bridge networks handle inter-container communication. |
| **Caddy wildcard TLS** | No public-facing web services. SSH tunnel handles remote access. |
| **OpenTofu remote state (R2)** | No state to manage beyond the VM's existence. |
| **`uncloud` CLI / `uc deploy`** | We use native Docker Compose or bollard. `uncloud` is a Go binary with its own opinionated model. |
| **macOS keychain (fnox)** for secrets | Environment variables or a `.env` file (gitignored) for API tokens. |
| **Nushell scripts** for orchestration | Rust via bollard + the testbed CLI. This is a Rust project, not a shell-script project. |

The value from vm-uncloud is the **architectural pattern** (minimal cloud-init,
SSH-based bootstrap, remote Docker), not the specific tools.
