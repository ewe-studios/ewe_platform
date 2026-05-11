# foundation_testbed on Hetzner

Drive the testbed remotely on a Hetzner Cloud box. QEMU uses `/dev/kvm`
if present and falls back to TCG (10–50× slower per the README) if not —
**so KVM is a performance upgrade, not a hard requirement**. Start cheap,
upgrade if needed.

The mise tasks in `mise.toml` are the single source of truth.
This doc explains the **shape** of the pipeline so Alex (or you next
month) can navigate it.

**Toolchain:** everything mise-installed. `hcloud` for the API,
`zig` + `cargo-zigbuild` for cross-compile from any host to Linux musl.
No brew, no apt, no GCC source builds. Cross-build to a 6 MB
statically-linked binary from a Mac in ~2 min.

**Host OS support:** macOS and Linux work natively. Windows devs should
run from **WSL** — task scripts use `bash`, which WSL provides and mise
runs the same as on Linux. `zig` and `cargo-zigbuild` themselves have
native Windows builds too, so cross-compiling on Windows works
mechanically; WSL is just the path that doesn't require rewriting every
task script.

## Three phases

```
┌──────────────────────────┐
│ Phase 1: host:*          │  Hetzner box lifecycle
│   up / bootstrap /       │  (persistent, idempotent;
│   deploy / ssh /         │   never auto-deletes)
│   status / down          │
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Phase 2: vm:*            │  testbed binary on the
│   doctor / import /      │  remote host, driven via
│   start / stop           │  ssh
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Phase 3: hil:*           │  Orchestrators that
│   probe / probe-kvm /    │  compose host:* + vm:*
│   up / down              │
└──────────────────────────┘
```

Plus a small `hcloud:*` namespace for one-time things (token, ssh-key,
nuke-all).

## Task surface

Run from inside `backends/foundation_testbed/`.

### hcloud:* — one-time utility

| Task | What |
|---|---|
| `hcloud:context` | Save your Hetzner API token in `~/.config/hcloud/cli.toml` |
| `hcloud:check` | Verify auth works (used as a precondition by `host:up` and `hil:probe-kvm`) |
| `hcloud:ssh-key` | Upload your local public key to Hetzner |
| `hcloud:teardown` | Delete every server + key + the local context (full reset) |

### Local build

| Task | What |
|---|---|
| `build` | Native build for your host (`cargo build --release --features cli`) — local dev only |
| `cross:build` | Cross-compile via `cargo-zigbuild` for `x86_64-unknown-linux-musl` — produces the Hetzner-deployable binary |

### host:* — Phase 1, Hetzner box lifecycle

| Task | What |
|---|---|
| `host:up` | Create the persistent box (idempotent; skips if it already exists) |
| `host:bootstrap` | `apt install qemu-system-x86 qemu-utils ovmf …` |
| `host:deploy` | `scp` the cross-built testbed binary to `/usr/local/bin/testbed` |
| `host:ssh` | Interactive SSH into the host |
| `host:status` | Active context + server list + key list |
| `host:down` | Delete the host (the **only** task that deletes; requires confirmation) |

### vm:* — Phase 2, testbed binary on the remote host

Each `vm:*` task is essentially `ssh root@host -- testbed <subcommand>`.

| Task | What |
|---|---|
| `vm:doctor` | Health check on the remote host |
| `vm:import <profile>` | Download a VM image (e.g. `linux-build`, `windows-build`) |
| `vm:start <profile>` | Start a VM (headless) |
| `vm:stop <profile>` | Stop a VM |

### hil:* — Phase 3, orchestrators

| Task | What | Composes (serial) |
|---|---|---|
| `hil:probe-kvm` | Throwaway probe: spin small box, check `/dev/kvm`, destroy | (standalone) |
| `hil:probe` | First-time setup + probe | `hcloud:context` → `hcloud:ssh-key` → `hil:probe-kvm` |
| `hil:up` | Cross-compile binary, bring up host, deploy, doctor | `cross:build` → `host:up` → `host:bootstrap` → `host:deploy` → `vm:doctor` |
| `hil:down` | Tear down the persistent host | `host:down` |

`hil:probe` and `hil:up` execute their steps **serially** via `mise run`
inside a script — guarantees order without forcing each step to declare
inter-task `depends`.

## Workflows

### First-time setup (today)

```bash
cd backends/foundation_testbed
mise run hil:probe
```

Prompts for API token, uploads SSH key, spins a CCX13 for ~3 min, probes
`/dev/kvm`, prints the result, destroys the probe server. Total cost
~€0.02.

### Regular use (after probe passes)

```bash
mise run hil:up                  # ensure host is up + provisioned
mise run vm:import linux-build   # pull a VM image (one time)
mise run vm:start  linux-build   # boot it
# ... work ...
mise run vm:stop   linux-build
# host stays up; no charges for VMs themselves
```

### Stop paying

```bash
mise run host:down               # deletes the host; everything on it is gone
```

### Full reset (e.g. before handing off the project)

```bash
mise run hcloud:teardown
```

Then delete the empty project shell + revoke the API token in the
web console (one click, the API can't do this part).

## Defaults (override via env)

| Var | Default | Notes |
|---|---|---|
| `HETZNER_HOST_NAME` | `testbed-host` | Persistent server name |
| `HETZNER_HOST_TYPE` | `ccx23` | 4 dedicated vCPU, 16 GB — fits one heavy build VM |
| `HETZNER_HOST_LOCATION` | `fsn1` | Falkenstein |
| `HETZNER_HOST_IMAGE` | `ubuntu-24.04` | |
| `HETZNER_PROBE_TYPE` | `ccx13` | Cheapest probe |
| `HETZNER_SSH_KEY` | `~/.ssh/id_ed25519` | Local private key (override for a dedicated Hetzner key) |
| `HETZNER_SSH_KEY_NAME` | `$USER-laptop` | Hetzner-side key registration name |

Full env list documented in the header of `mise.toml`.

## Not yet implemented

- **Migrate host scripts to nushell.** The repo already uses nushell on
  the guest side (`install_nushell()` in `host_bootstrap/install.rs`;
  `aqua:nushell/nushell` in the root `[tools]`). Bringing it host-side
  for our `hil:*` / `host:*` / `vm:*` tasks would (a) bring consistency
  with the testbed's "mise + nushell everywhere" design, and
  (b) give Windows devs native support without WSL. Roughly 17 task
  scripts to translate; defer until either Windows demand appears or
  we're touching the tasks for another reason.
- **State persistence** — when you `host:down`, everything on it is gone.
  Pick one of Volume / Storage Box / R2 when this becomes a problem.
- **Remote `--headful` access** — currently `vm:start` uses
  `--headless`. For VNC over SSH or Tailscale, design that when we
  actually need it.

## Status

- Local macOS build via `mise run build` works (release, 9.2 MB ARM64).
- `cross:build` for Linux musl: **not done yet**.
- Probe + host + vm + hil tasks: **written, not yet exercised**. Pending
  the `hil:probe` run that confirms KVM availability on CCX.
