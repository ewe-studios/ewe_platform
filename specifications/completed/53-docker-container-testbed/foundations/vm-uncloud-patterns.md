# vm-uncloud Patterns Analysis

## Purpose

Extract reusable architectural patterns from the `vm-uncloud` project
(`/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/vm-uncloud`) for
deploying the Docker-based testbed to cloud providers. vm-uncloud is a
production-hardened IaC project — it has been running live at
`amplifycms.com` for months, managing a multi-node Uncloud cluster on
Hetzner Cloud with Windows VMs, dev containers, and a service mesh.

## Project overview

vm-uncloud is a pure IaC project (no Rust, no compiled code) that deploys
containerized services to cloud providers. Its stack:

- **OpenTofu** (Terraform fork) 1.12.3 — infrastructure provisioning
- **Uncloud** (Go CLI, `uc`) v0.20.0 — container orchestration over WireGuard mesh
- **Caddy** — wildcard TLS ingress via Cloudflare DNS-01
- **WireGuard** — inter-node mesh networking (managed by uncloud)
- **Cloudflare** — DNS hosting and R2 state storage
- **Nushell** 0.113.1 — all scripting (OS-neutral, no bash-isms)
- **fnox** 1.27.1 — macOS keychain secrets client
- **mise** — tool version manager and task runner

Every tool is pinned to an exact version in `mise.toml`. `mise install`
provides the entire toolchain — the only prerequisite is mise itself.

## 1. Node class system

vm-uncloud does not run everything on one VM. It defines **node classes** —
each a separate OpenTofu workspace + uncloud context, with independent
lifecycle (`up`/`down`). This is the core architectural pattern.

| Class | SKU | Virt | Lifecycle | Purpose | Status |
|-------|-----|------|-----------|---------|--------|
| **cluster** | Hetzner `cpx22` (4 vCPU / 8 GB) | Container | Always-on | Web containers (Moltis, WordPress, Imaginary). One wildcard DNS-01 Caddy ingress. | **Live** at `amplifycms.com` |
| **dev** | `cpx42` (8 vCPU / 16 GB) or `cax*` (ARM) | Container | Ephemeral | Remote Linux dev container that matches deploy target arch (real `linux/amd64`). Toolchains from the project's mise.toml. | **Working** |
| **win-batch** | `cpx42` | dockur **TCG** | Ephemeral (teardownable) | Windows batch/unattended workloads. Snapshotted before destroy, restorable. | **Working** |
| **win-kvm** | Vultr Bare Metal | dockur **KVM** | Ephemeral | Interactive Windows at native speed. No hot-snapshot — state is lost on destroy (R2 transit pipeline scaffolded). | **Scaffolded** — needs Vultr key + live run |
| **dev-win** | `cpx42` | dockur TCG | Ephemeral | Remote dev on real Windows with OEM provisioning (mise + git + OpenSSH). RDP primary, SSH experimental. | **Experimental** |

Each class has its own:
- **Tofu workspace**: Isolates state. `tofu/<class>.tfvars` defines node count, server type, location, firewall rules.
- **Uncloud context**: Named context (`--context win-batch`, `--context dev`) so `uc` commands target the right machines.
- **Firewall rules**: The cluster opens `22/80/443/51820`. Windows nodes add `:3389` (restricted to `ssh_allowed_ips`). Dev nodes add the SSH port.
- **Lifecycle tasks**: `mise run win:up`, `win:deploy`, `win:down` vs `dev:up`, `dev:deploy`, `dev:down`.
- **Cost model**: Each class has a price entry in `state/prices-*.jsonl`.

The key insight: **one repo, one tool, one provisioning idiom — but not one
node**. Workloads differ in size, burst patterns, and whether they need real
virtualization. Separate contexts prevent the Windows VM from starving the
web containers and vice versa.

### Teardown discipline per class

| Class | Down behavior | State preservation |
|-------|--------------|-------------------|
| **cluster** | `cluster:down` — tofu destroy (whole node). Prompts for confirmation. | No snapshot — services are stateless or have their own persistence |
| **win-batch** | `win:down` — **snapshot THEN destroy**. `win:snapshot` runs `hcloud server create-image` first. `win:up` restores from the latest snapshot. | Yes — hot-snapshot via hcloud API |
| **win-kvm** | `win-kvm:down` — destroy. No automatic snapshot. An experimental R2 transit pipeline exists (`win-kvm:snapshot` → upload qcow2 to Cloudflare R2) but is unverified. | Scaffolded only |
| **dev** | `dev:down --yes` — auto-confirmed destroy. Dev state (synced repo) lives on the developer's Mac. | Not needed |
| **dev-win** | `dev:win:down --yes` — auto-confirmed destroy. Guest disk is NOT snapshotted. | No — use win-batch if state preservation matters |

The `win:down` task uses a `depends = ["win:snapshot"]` in mise so the
snapshot always runs first. This is cost discipline in code: you cannot
accidentally destroy a Windows node without saving state.

## 2. KVM vs TCG — why there are separate recipes

This distinction drives the entire Windows architecture. See the dockurr
analysis for technical details on KVM/TCG. Here we focus on the
**infrastructure consequences**.

### Hetzner Cloud has no /dev/kvm

This is a hard constraint. No Hetzner Cloud tier (CX, CPX, CCX, CAX) exposes
nested virtualization. KVM-fast Windows requires bare metal.

### Bare metal options

| Provider | Product | Billing | KVM | Snapshot |
|----------|---------|---------|-----|----------|
| Hetzner Robot | Dedicated `ax*` | Monthly (~€46/mo auction floor for 64 GB) | Yes | Server-side snapshot |
| Vultr | Bare Metal | Hourly | Yes | No hot-snapshot (R2 transit only) |
| Hetzner Cloud | `cpx42` | Hourly (~€0.04/hr) | No (TCG only) | `hcloud server create-image` |

### Why vm-uncloud chose Vultr BM for KVM

- Hourly billing matches the ephemeral use pattern (unlike Hetzner Robot's monthly minimum).
- API-driven provisioning via `vultr-cli` (same pattern as `hcloud` for Hetzner).
- The trade-off: no hot-snapshot. State preservation requires the experimental R2 pipeline.

### The architectural split

Rather than conditional logic in one compose file, vm-uncloud has **two
separate recipes** that are nearly identical:

`recipes/windows/compose.yaml` (TCG):
```yaml
environment:
  KVM: "N"
# No devices: [/dev/kvm]
```

`recipes/windows-kvm/compose.yaml` (KVM):
```yaml
# No KVM: "N"
devices:
  - /dev/kvm
```

This is deliberate: explicit, auditable, no runtime branching. The deploy
task targets the right recipe for the node class:
```bash
mise run recipe:deploy windows -- --context win-batch     # TCG
mise run recipe:deploy windows-kvm -- --context win-kvm   # KVM
```

### Pricing consequence

TCG on cpx42: ~€0.04/hr (€29/mo continuous). KVM on Vultr BM: ~€0.20/hr
(€144/mo continuous). The cost difference is ~5x, roughly matching the
performance difference. For unattended batch tests, TCG is the clear default;
KVM is for interactive debugging.

## 3. Minimal cloud-init

vm-uncloud's cloud-init (`cloud-init/uncloud.yaml`) is 12 lines:

```yaml
#cloud-config
package_update: true
package_upgrade: false
packages:
  - curl
  - ca-certificates
```

That is it. No Docker, no uncloud daemon, no SSH configuration, no firewall
rules, no user accounts. The rationale:

> We deliberately do NOT install uncloud here — that's `uc machine init`'s
> job (single installer, no double-install, no half-started daemon).
> cloud-init just makes sure curl/ca-certificates are present (the uncloud
> installer fetched over SSH needs them) and provides a clean
> `cloud-init status --wait` signal for tofu's readiness provisioner.

**Why this matters for the testbed:**

1. **Resilience to infrastructure rot**: cloud-init that installs Docker from
   `apt.docker.com` breaks when GPG keys rotate or repository URLs change.
   Deferring installation to an SSH bootstrap step makes the setup survive
   infrastructure changes without rebuilding images.

2. **Single source of truth**: The uncloud daemon is installed by `uc machine
   init` — one code path, not duplicated between cloud-init and SSH bootstrap.

3. **Clean readiness signal**: OpenTofu's `remote-exec` provisioner blocks on
   `cloud-init status --wait`. When that returns, the machine is SSH-ready.
   No polling loops, no arbitrary `sleep` delays.

4. **Firewall is tofu's job**: Security groups (Hetzner firewall) are
   provisioned by OpenTofu alongside the server. cloud-init does not touch
   iptables/ufw. One tool owns each concern.

### The full bootstrap chain

```
tofu apply
  → Hetzner API: create server with cloud-init (curl + ca-certificates)
  → remote-exec: wait for SSH, then `cloud-init status --wait`
  → tofu outputs: IPs, domain, SSH key path
uc machine init root@<ip>
  → SSH in, install uncloudd + Corrosion (Docker image)
  → Join WireGuard mesh
  → Machine is ready for `uc deploy`
```

**Our adoption:** The testbed's cloud-init installs `curl` + `ca-certificates`
+ `ufw` (for basic firewall before the orchestration layer configures it).
Docker installation happens in a subsequent `testbed cloud bootstrap` step
over SSH. The `cloud-init status --wait` signal gates the bootstrap.

## 4. Recipe system

Every deployable service is a **recipe** — a directory under `recipes/`
containing:

| File | Purpose | Required |
|------|---------|----------|
| `compose.yaml` | Docker Compose file with uncloud extensions (`x-ports`) | Yes |
| `prepare.nu` | Nushell script that generates config, outputs JSON on stdout | No |
| `Dockerfile` | If the service needs a custom image build | No |
| `README.md` | Self-documenting the recipe's purpose, config, access | Convention |

### How prepare.nu works

`prepare.nu` is the bridge between secrets/context and the compose file:

1. The deploy script (`scripts/recipe-deploy.nu`) detects `prepare.nu` in the
   recipe directory
2. It runs `nu prepare.nu` with `DOMAIN` and other context in the environment
3. `prepare.nu` outputs a **JSON object on stdout** — stderr is for human notes
4. The deploy script **merges** that JSON into the compose environment

This means:
- **Secrets are never in compose files**: API keys, passwords, tokens are
  generated or fetched by `prepare.nu` and injected at deploy time.
- **Compose files are portable**: The same `compose.yaml` works locally
  (`recipe:local`) and remotely (`recipe:deploy`) — `prepare.nu` adapts.
- **No per-service deploy logic**: The deploy script is 100% generic. Adding a
  recipe touches no shared code.

### Example: dev-linux prepare.nu

The dev-linux `prepare.nu` resolves:
1. The developer's SSH public key (from `~/.ssh/id_ed25519.pub`, `id_rsa.pub`,
   or `DEV_SSH_PUBKEY` override)
2. The OrangeVault dev/CI account credentials (from `fnox` keychain)
3. The login user (`DEV_USER`, default `vscode`) and SSH port (`DEV_SSH_PORT`,
   default `2222`)

All of this is output as JSON, merged into the compose environment at deploy
time. The compose file references these as `${SSH_PUBKEY}`, `${DEV_USER}`, etc.

### Example: windows prepare.nu

The windows `prepare.nu` is deliberately minimal:

```nu
def main [] {
  # No generated secret: dockur's default account (Docker/admin) is used.
  # Change the password inside Windows on first login.
  { HOST: $"windows.($dom)" } | to json
}
```

It **deliberately does not inject a password** — using dockur's defaults
avoids the uncloud env-newline quirk for exact-match values and keeps the
attack surface minimal. The operator changes the password manually after
first login.

### Local vs remote

The same recipe runs locally with `recipe:local`:
```bash
mise run recipe:local rauthy          # up
mise run recipe:local rauthy --down   # teardown
```

This uses plain `docker compose` against the local Docker daemon. Local
deltas go in an optional `compose.local.yaml`. This is DRY — the recipe's
core logic lives in one place.

**Our adoption:** Each testbed profile already has a directory with its
Dockerfile and compose.yaml. The `ContainerProfile` struct mirrors this
layout programmatically. The profile system is generic (the `Provider` trait
handles any profile). We could adopt the `prepare.nu` pattern for
profile-specific config generation — replacing hardcoded defaults with a
script that adapts to the deployment context (KVM vs TCG, RAM sizing, etc.).

## 5. Version pinning

Every tool in `mise.toml` is pinned to an exact version. This is not
accidental — it is a core architectural decision for reproducible deploys.

| Tool | Version | Purpose |
|------|---------|---------|
| opentofu | 1.12.3 | Infrastructure provisioning |
| hcloud | 1.65.0 | Hetzner Cloud CLI |
| uncloud (`uc`) | 0.20.0 | Container orchestration CLI |
| fnox | 1.27.1 | Secrets client (keychain → env) |
| nushell | 0.113.1 | All scripting |
| orangevault-cli | 0.1.1 | Dev/CI vault account provisioning |
| http-nu | 0.17.0 | Web GUI server |
| xs | 0.17.0 | Event bus |
| yoke | 0.4.1 | LLM agent as Unix pipe |
| aws-cli | 2.35.11 | R2 snapshot transit (win-kvm) |
| vultr-cli | latest | Vultr BM provisioning (less critical — used only for win-kvm) |

Note `vultr-cli` is the sole `latest` — everything else is pinned. Even the
shared task lib from GitHub is pinned by tag (`?ref=v0.69.0`).

### Why pinning matters

1. **Reproducible deploys**: `mise install` on any machine produces the exact
   same toolchain. No "it worked last week" regressions.
2. **Deliberate upgrades**: Bumping a version is a conscious decision with a
   git diff. The comments in `mise.toml` explain WHY each version matters:
   > v0.20 moved Corrosion to a managed Docker container (auto-migrates).
   > Bump deliberately: the gRPC proxy format changed, so CLI + all cluster
   > machines must be the same major — upgrade while no cluster is running.

3. **CI parity**: `mise run ci` runs the same tools locally and in CI
   (`.github/workflows/check.yml`). No version drift between dev and CI.

### The uncloud v0.20 binary rename

A significant gotcha: starting at v0.20, the CLI binary was renamed from
`uncloud` to `uc`. The daemon binary (`uncloudd`) is unchanged. Scripts call
`^uc`. The gRPC proxy format also changed in 0.20, so the CLI and all
cluster machines must share the same major version — upgrade with no cluster
running, or re-init.

The v0.20 daemon also pulls the Corrosion Docker image on first boot, which
can exceed systemd's service start timeout, causing `uc machine init/add` to
fail intermittently. vm-uncloud handles this with retry logic (see Pattern 8).

**Our adoption:** The testbed should pin Docker image versions (`dockurr/windows:5.15`)
and tool versions explicitly. The `ContainerProfile` struct should carry a
version field that flows to the compose file.

## 6. Secrets never on disk

vm-uncloud has **zero committed secrets**. No `.env` files, no
`terraform.tfvars` with tokens, no hardcoded credentials. The chain:

```
macOS Keychain (System.keychain)
    ↓ fnox get <KEY>
Environment variables (injected at runtime)
    ↓ fnox exec -- <command>
tofu / hcloud / uc / nu scripts
```

### How it works

Every task in `mise.toml` is wrapped in `fnox exec`:

```toml
[tasks."cluster:up"]
run = "fnox exec --if-missing ignore -- nu scripts/cluster-up.nu"
```

`fnox exec` reads secrets from the macOS keychain and injects them as
environment variables into the child process. The `--if-missing ignore` flag
means: if a secret is not in the keychain, proceed anyway (the command may
not need it). This is safe because `tofu` and `hcloud` fail cleanly when
`HCLOUD_TOKEN` is missing.

### What secrets are keychain-backed

| Key | Purpose | Set via |
|-----|---------|---------|
| `HCLOUD_TOKEN` | Hetzner Cloud API | `mise run secrets:set` |
| `CLOUDFLARE_API_TOKEN` | Cloudflare DNS API | `mise run secrets:set` |
| `VULTR_API_KEY` | Vultr API (win-kvm) | `mise run vultr:onboard` |
| `HETZNER_ROBOT_USER` / `_PASSWORD` | Hetzner Robot API (dedicated pricing) | Manual `fnox set` |
| `R2_ACCESS_KEY_ID` / `_SECRET` | Cloudflare R2 (win-kvm snapshots) | `mise run r2:bootstrap` |
| `ANTHROPIC_API_KEY` | LLM access (ai:ask) | Manual `fnox set` |
| `ORANGEVAULT_DEV_*` | Dev/CI OrangeVault account (5 env vars) | `mise run dev:secrets:set` |

### The fnox set discipline

> `fnox set` ALWAYS with `-p keychain` (else plaintext into `fnox.toml`).

This is a stated gotcha in CLAUDE.md. Without `-p keychain`, fnox stores the
secret in a plaintext `fnox.toml` file — which could be accidentally committed.

### Derived secrets (prepare.nu pattern)

For secrets that are *generated* rather than static tokens (database
passwords, API keys for services), the `prepare.nu` pattern is used:

1. `prepare.nu` generates a random password (e.g., `openssl rand -base64 24`)
2. It stores the password via `fnox set -p keychain`
3. It outputs the password as JSON for the compose environment
4. On subsequent deploys, `prepare.nu` reads the password from the keychain via `fnox get`

This means generated secrets are persistent (survive redeploys) but never
touch disk. The turso recipe demonstrates this: the libSQL auth token JWT
keypair is generated on first deploy and stored in the keychain.

### What this means for the testbed

The testbed does not use macOS keychain (not cross-platform). But the
**principle** applies:

1. **No secrets in committed files**: API tokens, SSH keys, registry
   credentials come from environment variables (CI) or a gitignored config
   file (local dev).
2. **Generated secrets are persisted**: The testbed may need to generate and
   store Windows admin passwords, SSH host keys, or test credentials. These
   should be stored in a local state file (gitignored) or retrieved from a
   secrets manager on each run — never hardcoded.
3. **The `--if-missing ignore` pattern**: Commands should be resilient to
   missing optional secrets. Fail cleanly, not cryptically.

## 7. Docker-in-Docker / sibling pattern

The dev-linux recipe provides a remote dev container that can build Docker
images. Instead of Docker-in-Docker (running a Docker daemon inside the
container), it uses the **Docker sibling** pattern.

### How it works

```yaml
volumes:
  - /var/run/docker.sock:/var/run/docker.sock
```

The host's Docker socket is mounted into the container. The Docker CLI is
installed in the container image (`apt-get install docker.io`). The dev user
is added to the `docker` group. The result: commands like `docker build` and
`docker compose` inside the container execute against the **host's** Docker
daemon.

### The GID problem

The Docker socket's group ID differs per node. The `docker` group inside the
container may have a different GID than the socket's GID on the host. The
entrypoint reconciles this at runtime:

```bash
if [ -S /var/run/docker.sock ]; then
  SOCK_GID="$(stat -c %g /var/run/docker.sock)"
  if [ "$SOCK_GID" != "0" ]; then
    if getent group "$SOCK_GID" >/dev/null 2>&1; then
      usermod -aG "$(getent group "$SOCK_GID" | cut -d: -f1)" "$DEV_USER"
    else
      groupmod -g "$SOCK_GID" docker
    fi
  fi
fi
```

This is a real gotcha solved by the entrypoint. Without it, the dev user
cannot access the Docker socket and `mise run docker` tasks fail with
permission errors.

### Why not Docker-in-Docker

Docker-in-Docker (running a full Docker daemon inside a container) has known
issues with storage drivers, cgroup nesting, and image layer duplication.
The sibling pattern avoids all of these by reusing the host daemon.

### Relevance to the testbed

The testbed does not need to build images inside Windows containers, but the
pattern is relevant for:
- **Linux test containers** that need to build Docker images as part of
  integration tests
- **CI environments** where the testbed itself runs in a container and needs
  to spawn sibling containers

## 8. PTY retry for headless operation

`uncloud machine init` opens `/dev/tty` directly for its Bubble Tea TUI
spinner. In a headless environment (CI, agent, non-TTY shell), this fails
with a cryptic error. vm-uncloud's solution is a PTY wrapper with retry.

### The PTY wrapper

```nu
def with-pty [cmd: list<string>] {
  if $nu.os-info.name == "macos" {
    ^script -q /dev/null ...$cmd
  } else {
    ^script -qec ($cmd | str join " ") /dev/null
  }
}
```

`script` creates a pseudo-terminal, satisfying the TUI's `/dev/tty` requirement.
The command is OS-dispatched (macOS `script` vs Linux `script` have different
flag syntax).

### The retry wrapper

v0.20 of uncloud introduced a race condition: the daemon pulls the Corrosion
Docker image on first boot, and that pull can exceed systemd's service start
timeout. `uc machine init` fails, but the daemon auto-restarts and eventually
comes up (the image is cached on subsequent attempts). The solution:

```nu
def with-pty-retry [cmd: list<string>, --tries: int = 3] {
  for attempt in 1..$tries {
    let ok = (try { with-pty $cmd; true } catch { false })
    if $ok { return }
    if $attempt < $tries {
      print -e "  ⚠ machine op failed — likely the v0.20 corrosion-pull race; retrying in 15s…"
      sleep 15sec
    }
  }
  error make { msg: $"machine op failed after ($tries) attempts" }
}
```

Both `cluster-up.nu` and `arm.nu` use `with-pty-retry` for `machine init` and
`machine add`. Verified live on 2026-06-27: init failed once then succeeded on
retry.

### What was tried and rejected

> `| cat` does NOT help, a PTY does.

Upstream uncloud closed issue #386 as not-reproducible, so vm-uncloud keeps
the PTY wrapper rather than carrying a fork. If uncloud ships a `--plain` or
`--no-tui` flag, the wrapper can be dropped.

### Relevance to the testbed

The testbed may need to run CLI tools that require a TTY (Docker Compose with
interactive prompts, certain installers). The `script` PTY wrapper is a
portable solution when `--yes` or `--non-interactive` flags are not available.

## 9. uncloud env newline caveat

Every environment variable injected by uncloud gets a trailing `\n` appended.
This is a persistent behavior (not yet fixed upstream) that breaks
exact-match values.

### Where it bites

The Cloudflare API token for Caddy's DNS-01 challenge failed because the
token was compared exactly and `"token\n" != "token"`. The fix in
`caddy/compose.yaml`:

```yaml
command: sh -c "printf '%s' \"$CLOUDFLARE_API_TOKEN\" | tr -d '\n' | ..."
```

### Where vm-uncloud guards against it

1. **caddy recipe**: Wraps the command with `tr -d` to strip CR/LF.
2. **dev-linux entrypoint.sh**: Every env var is trimmed:
   ```bash
   DEV_USER="$(printf '%s' "${DEV_USER:-vscode}" | tr -d '\r\n')"
   PUBKEY="$(printf '%s' "${SSH_PUBKEY:-}" | tr -d '\r')"
   ```
3. **windows recipe**: The documented concern in the compose:
   > If dockur's KVM=N detection trips on "N\n", switch KVM off via an
   > entrypoint trim wrapper like caddy/compose.yaml does. Confirm on the
   > first real deploy.

### Where it is intentionally dodged

The windows `prepare.nu` does NOT inject a `PASSWORD` variable, partly to
avoid the newline issue on an exact-match sensitive value.

### Relevance to the testbed

If the testbed injects environment variables into containers (via Docker
Compose, Kubernetes, or direct API), this is not an issue — Docker handles
env var values correctly. But if the testbed ever uses uncloud as its
orchestration layer, every injected env var should be trimmed on consumption.

## 10. Single wildcard DNS + TLS

One `*.domain.com` A record pointing at the cluster node. One wildcard TLS
certificate via Cloudflare DNS-01 challenge. Any subdomain resolves instantly
with zero per-host setup.

### How it works

```
*.amplifycms.com  A  →  <cluster-node-ip>
```

Caddy, deployed as a recipe on the cluster, obtains a wildcard certificate:

```yaml
# caddy/compose.yaml
environment:
  DOMAIN: ${DOMAIN}
  CLOUDFLARE_API_TOKEN: ${CLOUDFLARE_API_TOKEN}
```

Caddy's DNS-01 plugin solves the ACME challenge via Cloudflare API. The result
is a single certificate valid for `*.amplifycms.com` and `amplifycms.com`.

Any service that publishes via `x-ports` on a subdomain gets TLS instantly:

```yaml
services:
  api:
    x-ports: ["api.${DOMAIN}:8080/https"]
```

No per-service certificate provisioning. No HTTP-01 challenges. No Let's
Encrypt rate limit churn. This is the key enabler of the recipe system: adding
a service is just a compose file — DNS and TLS are already solved.

### The Caddy deployment

`cluster-up.nu` deploys Caddy immediately after machine init:

```nu
if ($wildcard | is-not-empty) {
  with-env { DOMAIN: $domain } { ^uc deploy -f caddy/compose.yaml -y }
}
```

Windows nodes skip this — they serve RDP (non-HTTP), so no wildcard cert.

### Relevance to the testbed

Not directly applicable (the testbed does not serve public web traffic). But
the principle of DNS-level multiplexing applies if the testbed exposes
web-based viewers (dockurr port 8006, VNC for macOS, build status dashboards).
A single wildcard DNS entry + Caddy with basic auth would provide secure
access to all testbed web UIs without per-instance TLS configuration.

## 11. Remote dev loop

vm-uncloud supports a remote development workflow where the developer keeps
their repo on their Mac and compiles/tests on a real `linux/amd64` box that
matches the deploy target.

### The setup

1. Provision a dev node: `mise run dev:up`
2. Deploy the dev-linux container: `mise run dev:deploy`
3. Wait for SSH: `mise run dev:ssh:wait`
4. Open VS Code Remote-SSH: `mise run dev:code`

### The shared task lib

Consumer projects include a shared mise task library via a single line:

```toml
# project's mise.toml
[task_config]
includes = ["git::https://github.com/joeblew999/vm-uncloud.git//tasks/dev.toml?ref=<tag>"]
```

This provides namespaced tasks (`uncloud:dev:*`) that drive the remote loop:

```bash
mise run uncloud:dev:build     # rsync repo to node + run `mise run build`
mise run uncloud:dev:release   # build + docker + GitHub release, on the node
```

The shared lib is version-pinned by git tag. The tasks are namespaced so they
cannot clash with the project's own tasks.

### The dev container

The dev-linux image (Dockerfile) bakes the universal prerequisites:
- `build-essential`, `pkg-config`, `libssl-dev` (C toolchain for Rust)
- `git`, `rsync` (workspace sync)
- `docker.io` (builds against mounted `/var/run/docker.sock`)
- `mise` (system-wide, so each project drives its own toolchain)
- `fnox` + Bitwarden CLI (secrets clients, for OrangeVault-backed project secrets)
- `openssh-server` (the entry point)

Specific language toolchains (Rust, Node, Go, Python) are NOT baked. They come
from the project's `mise.toml` — `mise install` fetches them on first
`mise run` after the repo is synced.

### OrangeVault for dev secrets

The dev container can pull project secrets from a dedicated OrangeVault
(Bitwarden-compatible) account. The `ov-bootstrap` script runs at container
start, unlocking the vault session so `fnox exec` resolves project secrets
in-container. The vault account is a dedicated dev/CI account — never a
personal vault.

### Relevance to the testbed

The testbed is a Rust CLI, not a dev container. But the pattern of keeping
the project repo local and running builds remotely is similar to the testbed's
model: the operator's machine runs `testbed cloud create`, the test runs
execute on the remote VM, and results are collected back. The shared task lib
pattern (version-pinned includes) is a clean way to distribute operational
tooling across repos.

## 12. Cost discipline and state tracking

vm-uncloud tracks everything it spends money on. This is not an afterthought
— it is designed in from the start.

### The deploy ledger

`state/log.jsonl` is a JSONL file (committed to git) that records every
up/deploy/down event:

```
{"action":"up","cluster":"hetzner","ips":"...","server_type":"cpx22","location":"fsn1","fqdns":"*.amplifycms.com","timestamp":"..."}
{"action":"deploy","cluster":"hetzner","images":["..."],"timestamp":"..."}
{"action":"down","cluster":"win-batch","timestamp":"..."}
```

Every lifecycle script (`cluster-up.nu`, `cluster-down.nu`, `recipe-deploy.nu`)
appends to this ledger (best-effort, `| ignore` — never breaks the primary
operation).

### The price model

`state/prices-*.jsonl` is a set of price catalogs, one per provider class,
refreshed from live APIs:

| File | Source | Refresh task |
|------|--------|-------------|
| `prices-hetzner-cloud.jsonl` | hcloud API (pricing endpoint) | `prices:refresh:hetzner-cloud` |
| `prices-hetzner-dedicated.jsonl` | Hetzner Robot API (new orders) | `prices:refresh:hetzner-dedicated` |
| `prices-hetzner-auction.jsonl` | Hetzner Robot API (auction floor) | `prices:refresh:hetzner-auction` |
| `prices-static.jsonl` | Manually maintained: storage, egress, software, Vultr, Equinix | Static |

The price model is used to **pick node classes per workload**: a cpx22 for the
cluster (~€0.02/hr), a cpx42 for Windows TCG (~€0.04/hr), a Vultr BM for KVM
(~€0.20/hr), or a Hetzner auction dedicated for always-on KVM (~€46/mo).

### Snapshot-before-destroy for expensive state

Windows nodes are snapshotted before teardown. The `win:down` task has a hard
dependency on `win:snapshot`:

```toml
[tasks."win:down"]
depends = ["win:snapshot"]
run = "fnox exec --if-missing ignore -- nu scripts/cluster-down.nu --context win-batch"
```

This is cost discipline enforced in code. You cannot accidentally destroy a
Windows node without saving state.

### The GUI status board

A read-only web dashboard (`gui/server/serve.nu`, served by http-nu + Datastar)
shows nodes, services, snapshots, prices, and the ledger. Supervised by
pitchfork as a daemon. This gives visibility into "what is running and what is
it costing" without needing to SSH into machines.

### Relevance to the testbed

The testbed should adopt:
1. **A deploy ledger**: Record every create/destroy event with VM type,
   duration, and cost. This enables "how much did testing cost this month"
   queries.
2. **Snapshot-before-destroy**: For expensive or slow-to-provision state
   (Windows VMs with installed toolchains, macOS VMs), snapshot before
   teardown. Restore on next create for instant readiness.
3. **Price-awareness**: The testbed CLI should display estimated cost before
   creating a VM, and actual cost after teardown.

## 13. What we adopt vs. skip

### Adopt

| Pattern | How | Why |
|---------|-----|-----|
| **Minimal cloud-init** | cloud-init installs curl + ca-certificates only. Docker + testbed agent installed via SSH bootstrap. | Resilient to infrastructure rot. Single source of truth for installation. |
| **Node class separation** | Separate contexts for always-on vs ephemeral, TCG vs KVM, Linux vs Windows. | Workloads have different sizing, cost, and lifecycle requirements. |
| **Version pinning** | Pin Docker images (`dockurr/windows:5.15`) and tool versions. | Reproducible deploys. Deliberate upgrades. |
| **Secrets never on disk** | API tokens from env vars. Generated credentials in gitignored state file. | No credential leaks from committed code. |
| **Recipe system structure** | Each testbed profile = directory with compose.yaml + optional prepare script. | Same structure vm-uncloud uses. Generic deploy logic, no per-profile special cases. |
| **PTY retry for TUI tools** | Wrap tools that need a TTY in `script -qec` with retry. | Headless CI compatibility. |
| **Deploy ledger** | JSONL log of create/destroy events. | Cost tracking. Audit trail. |
| **Snapshot-before-destroy** | For Windows profiles: snapshot the qcow2 volume before teardown. | State preservation for expensive provisioning. |
| **RDP readiness polling** | Poll `:3389` rather than arbitrary sleep. Desktop notification when ready. | The `rdp-wait.nu` pattern (30s poll, 90min timeout, OS-native notification) is directly reusable. |
| **Shared folder bridge** | Use dockurr's `/data` → `\\host.lan\Data` for file transfer into Windows guests. | Avoids WinRM complexity. Works before SSH is configured. |

### Skip

| Pattern | Reason |
|---------|--------|
| **OpenTofu / Terraform** | Overkill for single-VM testbed. Direct `hcloud` API (or hcloud CLI) is simpler. The testbed manages VMs directly via provider SDKs. |
| **WireGuard mesh** | Single-node. Docker bridge networks handle inter-container routing. |
| **Caddy wildcard ingress** | No public-facing services. SSH tunneling provides remote access. |
| **uncloud CLI (`uc`)** | Go binary with opinionated model. We use native Docker (bollard SDK + CLI). |
| **Nushell scripts** | This is a Rust project. Operational logic lives in the testbed CLI. |
| **macOS keychain (fnox)** | macOS-specific. Environment variables are cross-platform and CI-friendly. |
| **Cloudflare R2 state** | No Terraform state to store. VM existence is tracked by the local CLI. |
| **OrangeVault service registry** | Overkill for testbed. Test results and VM metadata stay local. |
| **ARM capacity watcher** | The testbed does not need to hoard scarce ARM instances. |
| **GUI status board** | The testbed CLI provides status via terminal output. A web dashboard is out of scope. |

## Summary

vm-uncloud provides a production-validated model for "minimal provision, then
SSH-bootstrap" that works across cloud providers and virtualization modes. The
key architectural insights for the testbed:

1. **Cloud-init should do as little as possible** — just enough to enable SSH
   access. Everything else is scripted and updatable without rebuilding images.

2. **Separate node classes for separate concerns** — do not co-locate a Windows
   VM with web containers. Each workload gets its own instance with appropriate
   sizing, firewall rules, and lifecycle.

3. **TCG is the default for cost-sensitive batch work** — KVM is for when
   performance matters. The two modes are explicitly separate recipes, not
   conditional logic in one file.

4. **Version pin everything** — tool versions, Docker image tags, shared task
   libs. Reproducibility is worth the maintenance overhead of deliberate
   upgrades.

5. **Secrets flow through the environment, never through files** — no .env
   files, no hardcoded tokens. The testbed should follow the same principle
   with environment variables or a local gitignored config.

6. **State that costs money to recreate should be snapshotted** — Windows VMs
   take 15-30 minutes to provision. A snapshot-before-destroy pattern makes
   them instantly available on the next `up`.

7. **Operational scripts should work headless** — TUI tools need PTY wrappers.
   Commands that can fail transiently need retry loops. The `with-pty-retry`
   pattern is a reusable solution.
