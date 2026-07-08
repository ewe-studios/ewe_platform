# vm-uncloud Patterns Analysis

## Purpose

Extract reusable architectural patterns from the `vm-uncloud` project
(`/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/vm-uncloud`) for
deploying the Docker-based testbed to cloud providers.

## Project overview

vm-uncloud is a pure IaC project (no Rust, no compiled code) that deploys
containerized services to Hetzner Cloud. It uses:

- **OpenTofu** (Terraform fork) for infrastructure provisioning
- **Uncloud** (Go CLI, `uc`) for container orchestration
- **Caddy** for wildcard TLS ingress
- **WireGuard** for inter-node mesh networking
- **Cloudflare** for DNS and R2 state storage
- **Nushell** scripts for operational logic
- **macOS keychain** (fnox) for secrets management

## Patterns we adopt

### 1. Minimal cloud-init

vm-uncloud's cloud-init installs only `curl` + `ca-certificates`. Docker and
the Uncloud daemon are installed later via SSH by `uc machine init`.

**Why this matters:** cloud-init that installs complex software (Docker,
specific apt repositories) breaks when repository URLs change or packages are
renamed. Deferring installation to an SSH-based bootstrap script makes the
setup resilient to infrastructure changes.

**Our adoption:** The testbed's cloud-init template installs `curl` +
`ca-certificates` + `ufw`. Docker installation happens in a subsequent
`testbed cloud bootstrap` step over SSH.

### 2. Single wildcard DNS + TLS

One `*.domain.com` A record, one wildcard TLS certificate via Cloudflare
DNS-01 challenge. Any subdomain resolves instantly with zero per-host setup.

**Our adoption:** Not directly applicable (the testbed doesn't serve public
web traffic), but the principle of DNS-level multiplexing could apply if we
expose web-based test viewers (dockurr port 8006, VNC).

### 3. Secrets never on disk

API tokens come from the macOS keychain, injected at runtime by `fnox exec`.
Generated secrets (DB passwords, API keys) are also keychain-backed, not in
env files.

**Our adoption:** Hetzner API tokens, registry credentials, and SSH keys
come from environment variables (CI) or a local `.env` file (gitignored).
We don't use `fnox` (macOS-specific), but the principle of "no secrets in
committed files" applies universally.

### 4. Remote-exec provisioner for readiness

OpenTofu's `remote-exec` provisioner blocks on `cloud-init status --wait` before
proceeding. No shell polling loops, no arbitrary sleep delays.

**Our adoption:** The `testbed cloud create` command blocks until cloud-init
reports `done` before returning the VM's IP address.

### 5. Recipe system (compose.yaml per service)

Each deployable service has its own directory with a `compose.yaml` and an
optional `prepare.nu` (generates secrets, sets env vars). The deploy script
is generic — no per-service logic.

**Our adoption:** Each testbed profile gets a directory with its Dockerfile
and compose.yaml. The `ContainerProfile` struct in Rust mirrors this layout
programmatically. The profile system is already generic (the `Provider` trait
handles any profile).

## Patterns we do NOT adopt

| Pattern | Reason |
|---------|--------|
| OpenTofu/Terraform | Overkill for single-VM testbed. Direct `hcloud` API (or hcloud CLI) is simpler. |
| WireGuard mesh | Single-node. Docker bridge networks handle inter-container routing. |
| Caddy wildcard ingress | No public-facing services. SSH tunneling provides remote access. |
| `uncloud` CLI (`uc`) | Go binary with opinionated model. We use native Docker (bollard + CLI). |
| Nushell scripts | This is a Rust project. Operational logic lives in the testbed CLI. |
| macOS keychain (fnox) | macOS-specific. Environment variables are cross-platform and CI-friendly. |
| Cloudflare R2 state | No Terraform state to store. VM existence is tracked by the local CLI. |

## Summary

vm-uncloud provides a proven pattern for "minimal provision, then SSH-bootstrap"
that works across cloud providers. The key architectural insight is that
cloud-init should do as little as possible — just enough to enable SSH access —
and everything else should be scripted and updatable without rebuilding VM
images.
