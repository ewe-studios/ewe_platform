# 06 — Cloud Deployment

**Date:** 2026-07-10
**Status:** Resolved as a design — **not built here; implementation moved to [spec 56](../../../56-vps-deployment-providers/)**

> **Audit 2026-07-17: not built here, and nothing technical is stopping it.**
> The build lives in **[spec 56 — VPS Deployment Providers](../../../56-vps-deployment-providers/)**,
> which turns the "create the VM" row below into three provider crates
> (DigitalOcean, Hetzner, Linode) plus hardening and a shared `VpsDeployment`. There is
> no cloud-init template and no Hetzner/AWS/GCP provisioning code, and no feature
> in the spec's set (01–19) covers this. This file is a plan, not a description of
> the tree. Most of what it depends on now exists and is verified:
>
> | Step | State |
> |---|---|
> | cloud-init provisions curl + ca-certificates | **not built** — a string template; no dependency on anything |
> | bootstrap Docker + binary over SSH (`foundation_sshkit`) | **exists, e2e-verified** — `ssh_backend_tests` (execute/upload/download/auth, 5 tests vs a real container) |
> | subsequent container ops over SSH | **exists, e2e-verified** — `DockerClient::connect_ssh` + `docker system dial-stdio`; `ssh_transport_tests` (info + container round-trip) pass against docker-in-docker with sshd |
> | service exposure + TLS (`foundation_proxy`) | **exists, e2e-verified** — 9/9 reverse-proxy e2e, ACME provisioning |
> | create the VM on Hetzner/AWS/GCP | **not built** — the only piece with an external dependency: a provider API client plus an account to run it against |
>
> So the work left is the cloud-init template, an SSH bootstrap sequence (both
> buildable and testable locally against the docker-in-docker + sshd fixture the
> transport tests already use), and one provider API client for VM creation —
> which cannot be verified here without credentials.
>
> Two corrections before building it:
> - **bollard is gone.** "Remote Docker via bollard SSH" below predates the
>   migration to `foundation_deployment_docker`. The equivalent today is
>   `DockerClient::connect_ssh("ssh://user@host")`.
> - **`connect_ssh` needs the `ssh` feature**, and `ssh` and `buildkit` are
>   mutually exclusive (libssh2/OpenSSL vs BoringSSL — see decision 05, "How the
>   build runs"). A deployment that drives a remote daemon over SSH cannot also
>   drive a standalone buildkitd from the same binary; build images with the
>   `Classic` backend on the remote daemon instead.

## Decision

Adopt the **cloud-init + remote Docker** pattern from `vm-uncloud` for deploying
the testbed to cloud providers (Hetzner, AWS, GCP). The core insight from
vm-uncloud is: (1) cloud-init provisions the bare minimum (curl + ca-certificates),
(2) a bootstrap phase installs Docker and the testbed binary over SSH —
powered by `foundation_sshkit` (decision 13) for connection pooling, key
management, and retry logic — and (3) all subsequent container operations go
through bollard's SSH transport or the testbed CLI over SSH. Service exposure
and TLS termination for dockurr web viewers and test services are handled by
`foundation_proxy` (decision 14).

## Table of Contents

1. [vm-uncloud patterns we adopt](#vm-uncloud-patterns-we-adopt)
2. [Cloud-init template](#cloud-init-template)
3. [Provisioning flow with foundation_sshkit](#provisioning-flow-with-foundation_sshkit)
4. [Remote Docker via bollard SSH](#remote-docker-via-bollard-ssh)
5. [Service exposure via foundation_proxy](#service-exposure-via-foundation_proxy)
6. [Hetzner-specific configuration](#hetzner-specific-configuration)
7. [Multi-provider abstractions](#multi-provider-abstractions)
8. [What we don't adopt from vm-uncloud](#what-we-dont-adopt-from-vm-uncloud)

---

## vm-uncloud patterns we adopt

### 1. Minimal cloud-init

vm-uncloud's `cloud-init/uncloud.yaml` installs only `curl` and
`ca-certificates`. Docker is NOT in cloud-init — it's installed by a subsequent
SSH-based bootstrap. This makes the cloud-init resilient to infrastructure
changes (Docker apt repos changing, package renames).

### 2. SSH-based bootstrap with connection pooling

vm-uncloud uses SSH extensively: `uc machine init` (install Docker + uncloudd),
binary deploy via scp, RDP/viewer tunnels. We replace all of this with
`foundation_sshkit` (decision 13) — connection pooling, key resolution,
`Host` abstraction, and runner strategies (parallel for multi-node, sequential
for single-node bootstrap steps).

### 3. Single wildcard DNS + TLS

vm-uncloud uses a single `*.domain.com` DNS record with Caddy. We replace
Caddy with `foundation_proxy` (decision 14) — same wildcard TLS, plus
health-check routing and zero-downtime deploy capabilities.

### 4. Secrets never on disk

vm-uncloud uses the macOS keychain (via `fnox`) for API tokens. We follow the
same principle: Hetzner tokens, registry credentials, and SSH keys come from
environment variables or a local secrets store, never from committed files.

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

# Basic firewall: allow SSH + Docker bridge traffic
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
      # Allow proxy HTTP/HTTPS (if foundation_proxy is deployed)
      -A INPUT -p tcp --dport 80 -j ACCEPT
      -A INPUT -p tcp --dport 443 -j ACCEPT

runcmd:
  - ufw --force enable
  - cloud-init status --wait  # signal readiness to provisioner
```

---

## Provisioning flow with foundation_sshkit

The old flow used raw `ssh` and `scp` commands. With `foundation_sshkit`:

```
┌─────────────────────────────────────────────────────────────────┐
│  Step 1: Create cloud VM                                        │
│    testbed cloud create --provider hetzner                      │
│      → Creates CX42 (4 vCPU, 16 GB)                             │
│      → Attaches cloud-init                                      │
│      → Blocks until cloud-init completes                        │
│      → Returns Host { hostname, user, port, ... }               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Step 2: Bootstrap the host (via foundation_sshkit)              │
│    let host = Host::parse("root@<ip>")?;                        │
│    let pool = ConnectionPool::new(idle_timeout: 30s);           │
│                                                                 │
│    Runner::Sequential.run(&[host], &backend, |h| {              │
│        Command::new("curl -fsSL https://get.docker.com | sh")   │
│            .pty(true)    // Docker install needs a TTY           │
│    }).await?;                                                   │
│                                                                 │
│    // Deploy the cross-compiled platform binary                  │
│    backend.upload(&host, "bin/platform", "/usr/local/bin/")?;   │
│                                                                 │
│    // Verify                                                  │
│    let result = backend.execute(&host,                           │
│        Command::new("docker ps")).await?;                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Step 3: Run tests                                              │
│    Option A: bollard → SSH → remote Docker (native tunnel)      │
│      let docker = DockerClient::connect_ssh(host, keys)?;      │
│      let group = ContainerGroup::start(defs).await?;           │
│                                                                 │
│    Option B: SSH into host, run platform CLI there               │
│      backend.execute(&host,                                      │
│          Command::new("platform start linux-build")).await?;    │
│                                                                 │
│    Option C: Proxy-based (foundation_proxy fronts services)     │
│      → See "Service exposure" below                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Step 4: Tear down                                              │
│    testbed cloud destroy <host>                                  │
│      → Stops containers, destroys VM, removes firewall rules    │
└─────────────────────────────────────────────────────────────────┘
```

### Key sshkit patterns used

- **`ConnectionPool`** — reuses SSH sessions across bootstrap steps (Docker
  install, binary deploy, verify). No new TCP handshake per command.
- **`Runner::Sequential`** — single-node bootstrap is sequential by nature.
  For multi-node, `Runner::Parallel` fans out.
- **`.pty(true)`** — Docker install and `uc machine init`-style commands need
  a TTY (same vm-uncloud gotcha — bubbletea/Bootstrap UIs open `/dev/tty`).
- **Retry with backoff** — `sshkit` wraps transient failures (connection
  refused during boot, DNS not propagated) in a retry loop with configurable
  backoff. Same as vm-uncloud's `with-pty-retry`.
- **`Host::from_ssh_config()`** — reads `~/.ssh/config` for proxy jump,
  identity file, port. Bastion host config lives once in the user's SSH
  config, not in testbed config.

---

## Remote Docker via bollard SSH

Bollard tunnels the Docker API through an SSH connection — no exposed TCP port:

```rust
use foundation_deployment_platform::docker::DockerClient;

let docker = DockerClient::connect_ssh(
    &host.hostname,
    &host.key_paths,
    &host.user,
    host.port,
).await?;

// All subsequent bollard calls go over the SSH tunnel
let group = ContainerGroup::start(vec![
    ContainerServiceDefinition::new("redis:7").port(6379),
]).await?;
```

### When bollard SSH isn't feasible (jump hosts, restricted networks)

```rust
// Build + deploy static binary via sshkit
backend.upload(&host, "target/release/platform", "/usr/local/bin/").await?;

// Run remotely — platform CLI on the host handles Docker locally
let result = backend.execute(&host,
    Command::new("platform start linux-build")
).await?;
```

The `Host` type from `foundation_sshkit` feeds directly into both paths —
the same SSH config works for `DockerClient::connect_ssh()` and
`Backend::execute()`.

---

## Service exposure via foundation_proxy

`foundation_proxy` (decision 14) replaces the "SSH tunnel for everything"
pattern. Instead of `ssh -L 8006:localhost:8006` to reach dockurr's web
viewer, the proxy provides proper HTTPS:

```
                    ┌──────────────────────────────────┐
  Internet ────────▶│  foundation_proxy (TLS + routing) │
                    │                                  │
                    │  windows.<domain>  → dockurr:8006│
                    │  macos.<domain>     → dockurr:5900│
                    │  app.<domain>       → testbed:3000│
                    │  *.proxy.<domain>   → (any service)│
                    └──────────────────────────────────┘
```

### Dockurr web viewer exposure

```rust
let proxy = ProxyServer::start(ProxyConfig {
    services: vec![
        ServiceConfig {
            name: "windows-viewer".into(),
            host: format!("windows.{}", domain),
            ssl: SslConfig::lets_encrypt("admin@example.com"),
            health_check: HealthCheckConfig::tcp(8006, 30),
            backends: vec![BackendTarget {
                url: format!("http://{}:8006", host_ip),
                weight: 1,
            }],
            ..Default::default()
        },
    ],
    ..Default::default()
}).await?;
```

The proxy handles TLS (Let's Encrypt or Cloudflare DNS-01), health-check-based
routing, and can be deployed alongside the testbed containers on the same
Docker network.

### What this replaces from vm-uncloud

| vm-uncloud approach | foundation equivalent |
|---------------------|----------------------|
| Caddy wildcard TLS | `foundation_proxy` with ACME or Cloudflare DNS-01 |
| `ssh -L 8006:localhost:8006` tunnel | Proxy routes `windows.<domain>:443` → dockurr:8006 |
| `ssh -L 3389:localhost:3389` RDP tunnel | Direct RDP via public IP + firewall (same as vm-uncloud win-batch) |
| noVNC over SSH tunnel | Proxy with basic auth in front (decision 02 dockurr caveats) |
| `uc deploy` service exposure | `ContainerGroup::start()` + proxy registration |

---

## Hetzner-specific configuration

### VM sizing for Docker workloads

| Hetzner type | vCPU | RAM | Suitable for | Cost (€/h) |
|-------------|------|-----|-------------|------------|
| CX32 | 4 | 8 GB | Single Linux container builds | ~0.05 |
| CX42 | 4 | 16 GB | Linux + Windows (dockurr) concurrently | ~0.08 |
| CX52 | 8 | 32 GB | Multi-platform parallel builds + proxy | ~0.15 |

Docker containers share the host kernel and are more memory-efficient than
QEMU VMs. For dockurr/windows, nested KVM is still required — Hetzner CX22+
supports this, same as today.

### mise.toml tasks

```toml
[tasks.cloud.create]
description = "Create a Hetzner cloud VM for testbed"
run = "testbed cloud create --provider hetzner --type cx42 --location fsn1"

[tasks.cloud.bootstrap]
description = "Install Docker + deploy platform binary (via sshkit)"
run = "testbed cloud bootstrap"

[tasks.cloud.deploy-proxy]
description = "Deploy foundation_proxy onto the cloud node for service exposure"
run = "testbed cloud proxy up"

[tasks.cloud.test]
description = "Full cloud test cycle"
depends = ["cloud.create", "cloud.bootstrap"]
run = "testbed start linux-build --remote && testbed build linux-build"

[tasks.cloud.down]
description = "Tear down — stops containers, destroys VM, cleans DNS"
run = "testbed cloud destroy"
```

---

## Multi-provider abstractions

```rust
pub trait CloudProvider: Send + Sync {
    /// Create a cloud VM, return a Host ready for sshkit connection.
    fn create_vm(&self, config: &CloudVmConfig) -> Result<CloudVm>;
    /// Destroy a cloud VM and associated resources.
    fn destroy_vm(&self, vm: &CloudVm) -> Result<()>;
    /// Return cloud-init user data for this provider.
    fn cloud_init(&self) -> String;
}

pub struct CloudVm {
    pub host: Host,            // sshkit Host — parsed, ready to connect
    pub provider_id: String,   // e.g., "hetzner-12345"
    pub region: String,
    pub instance_type: String,
}
```

Initial implementation: `HetznerCloudProvider` (direct `hcloud` HTTP API).
Future: `AwsCloudProvider`, `GcpCloudProvider`.

The `Host` returned by `create_vm` feeds directly into both `foundation_sshkit`
(`Backend::execute()`, `ConnectionPool::get()`) and
`foundation_deployment_platform::docker::DockerClient::connect_ssh()`.

---

## What we don't adopt from vm-uncloud

| vm-uncloud feature | Why we skip it | Foundation alternative |
|-------------------|----------------|----------------------|
| **OpenTofu/Terraform** | Overkill for single-VM testbed | Direct `hcloud` API |
| **WireGuard mesh** | Single-node | Docker bridge networks |
| **Caddy** | Replaced by our own proxy | `foundation_proxy` (decision 14) |
| **`uncloud` CLI (`uc`)** | Go binary, opinionated model | `ContainerGroup` + `DockerClient` (native Rust) |
| **Nushell scripts** | Rust project, not shell | `foundation_deployment_platform` CLI + `foundation_sshkit` |
| **macOS keychain (fnox)** | macOS-specific | Environment variables (CI-friendly, cross-platform) |
| **Cloudflare R2 state** | No Terraform state | VM existence tracked by local CLI |
| **raw `ssh`/`scp` commands** | String-based, fragile | `foundation_sshkit` (decision 13) |
| **SSH tunnels for service access** | Manual, no auth on viewers | `foundation_proxy` TLS + routing (decision 14) |

The value from vm-uncloud is the **architectural pattern** (minimal cloud-init,
SSH-based bootstrap, remote Docker), now implemented with Foundation-native
tools at each layer.
