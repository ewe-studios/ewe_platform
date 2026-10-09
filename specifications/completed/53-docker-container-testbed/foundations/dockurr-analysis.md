# dockurr Image Analysis

## Purpose

Analyze the dockurr Docker images (https://github.com/dockur) to understand
what they offer, how they are configured in production use (via vm-uncloud), and
how they could integrate with `foundation_testbed`.

## Organization

dockurr is a GitHub organization with **27 repositories** that package entire
operating systems and infrastructure services into Docker containers. The OS
images use QEMU internally — the Docker container wraps a full VM. This
provides a Docker interface (environment variables, volumes, ports) for managing
VMs, while the infrastructure images (dnsmasq, samba, tor, etc.) are
conventional containers wrapping standard daemons.

The GitHub org has **52.4k stars** on the flagship `windows` repo, with 4.5k
forks and an MIT license.

Full repository listing:

| # | Repository | Category |
|---|-----------|----------|
| 1 | **windows** | OS (Windows VM) |
| 2 | **windows-arm** | OS (Windows ARM64 VM) |
| 3 | **macos** | OS (macOS VM) |
| 4 | **chromeos** | OS (ChromeOS Flex VM) |
| 5 | **proxmox** | OS (Proxmox VE VM) |
| 6 | **proxmox-backup** | OS (Proxmox Backup Server VM) |
| 7 | **proxmox-mail** | OS (Proxmox Mail Gateway VM) |
| 8 | **proxmox-dm** | OS (Proxmox Datacenter Manager VM) |
| 9 | **virtual-dsm** | OS (Synology DSM VM) |
| 10 | **qemu** | Infrastructure (base QEMU image) |
| 11 | **samba** | Infrastructure (SMB file sharing) |
| 12 | **dnsmasq** | Infrastructure (DNS + DHCP) |
| 13 | **tor** | Infrastructure (Tor proxy) |
| 14 | **chrony** | Infrastructure (NTP time sync) |
| 15 | **stunnel** | Infrastructure (TLS tunnel) |
| 16 | **mdns** | Infrastructure (mDNS) |
| 17 | **munin** | Infrastructure (monitoring) |
| 18 | **strfry** | Infrastructure (Nostr relay) |
| 19 | **statping** | Infrastructure (status page) |
| 20 | **bitcoin** | Application (Bitcoin node) |
| 21 | **piefed** | Application (Piefed) |
| 22 | **lemmy** | Application (Lemmy) |
| 23 | **lemmy-ui** | Application (Lemmy UI, forked) |
| 24 | **casa** | Application (CasaOS) |
| 25 | **umbrel** | Application (umbrelOS) |
| 26 | **zima** | OS (ZimaOS, forked) |
| 27 | **portainer-backup** | Utility (Portainer backup, forked) |

## Available images — Docker Hub stats

All stats as of July 2026. The OS images are the heavy hitters; infrastructure
images have significant pull counts too.

| Image | Docker Hub | Total Pulls | Weekly Pulls | Category |
|-------|-----------|-------------|--------------|----------|
| **windows** | `dockurr/windows` | 1M+ | 35,564 | OS VM |
| **windows-arm** | `dockurr/windows-arm` | 50K+ | 591 | OS VM |
| **macos** | `dockurr/macos` | 100K+ | 5,641 | OS VM |
| **chromeos** | `dockurr/chromeos` | Low | 105 | OS VM |
| **proxmox** | `dockurr/proxmox` | Low | 542 | OS VM |
| **samba** | `dockurr/samba` | 1M+ | 91,605 | Infrastructure |
| **dnsmasq** | `dockurr/dnsmasq` | 500K+ | 56,094 | Infrastructure |
| **tor** | `dockurr/tor` | 50K+ | 2,453 | Infrastructure |
| **chrony** | `dockurr/chrony` | 500K+ | 14,469 | Infrastructure |

**There is no `dockurr/linux` image.** dockurr focuses on OSes that cannot run
natively in containers. Linux does not need QEMU — it runs natively in Docker.

## dockurr/windows — complete configuration

This is the flagship image. The configuration below synthesizes the upstream
README and the real-world usage in vm-uncloud's `recipes/windows/compose.yaml`.

### Environment variables

| Variable | Default | Values / Notes |
|----------|---------|----------------|
| `VERSION` | `"11"` | `"11"`, `"11l"` (LTSC), `"11e"` (Enterprise), `"10"`, `"10l"`, `"10e"`, `"8e"`, `"7u"`, `"vu"`, `"xp"`, `"2k"`, `"2025"`, `"2022"`, `"2019"`, `"2016"`, `"2012"`, `"2008"`, `"2003"`, or a custom ISO URL |
| `RAM_SIZE` | `"4G"` | e.g., `"8G"`, `"12G"` — vm-uncloud uses `"12G"` on cpx42 (16 GB node) |
| `CPU_CORES` | `"2"` | e.g., `"4"`, `"6"` — vm-uncloud uses `"6"` (cpx42 has 8 vCPU) |
| `DISK_SIZE` | `"64G"` | e.g., `"80G"`, `"256G"`. Resizing upward is non-destructive but requires manual partition extension inside Windows afterwards |
| `USERNAME` | `"Docker"` | Default Windows login user |
| `PASSWORD` | `"admin"` | Default Windows login password — change inside Windows on first login |
| `LANGUAGE` | English | 33 languages supported (Arabic to Ukrainian) |
| `REGION` | Based on language | e.g., `"en-US"` |
| `KEYBOARD` | Based on language | e.g., `"en-US"` |
| `KVM` | — | Set `"N"` to force TCG software emulation when `/dev/kvm` is absent. Hetzner Cloud has no `/dev/kvm`, so vm-uncloud's TCG recipes set `KVM: "N"`. On KVM-capable bare metal, omit this and pass `/dev/kvm` as a device |
| `DHCP` | — | `"Y"` to enable macvlan DHCP (Windows gets its own IP from the router) |
| `MANUAL` | — | `"Y"` to skip automatic unattended install |
| `ARGUMENTS` | — | Extra QEMU arguments (e.g., USB passthrough: `"-device usb-host,vendorid=0x1234,productid=0x1234"`) |
| `DISK2_SIZE` | — | Additional disk size |
| `DISK3_SIZE` | — | Additional disk size |

### Devices

| Device | Purpose | Required |
|--------|---------|----------|
| `/dev/kvm` | KVM hardware acceleration | Required for native speed; absent on Hetzner Cloud (use `KVM: "N"` instead) |
| `/dev/net/tun` | Network TUN device | Required for networking |
| `/dev/vhost-net` | vhost-net acceleration | Required for DHCP/macvlan mode |
| `/dev/bus/usb` | USB bus access | Optional — USB device passthrough |
| `/dev/sdb:/disk1` | Direct raw disk passthrough (primary) | Optional — **formatted during install**, destructive |
| `/dev/sdc1:/disk2` | Direct raw disk passthrough (secondary) | Optional — left untouched |

### Capabilities

| Capability | Purpose |
|------------|---------|
| `NET_ADMIN` | Required for network configuration inside the VM |

### Volumes

| Mount | Purpose |
|-------|---------|
| `/storage` | Persistent Windows qcow2 disk image — persists VM state across container redeploys |
| `/storage2` | Second additional disk |
| `/storage3` | Third additional disk |
| `/shared` | Host folder shared to Windows desktop via SMB (appears as `\\host.lan\Data` inside Windows) |
| `/custom.iso` | Custom Windows ISO — binds a local ISO file, bypasses VERSION download |
| `/oem` | Folder copied to `C:\OEM` inside the guest; `install.bat` inside runs once after Windows setup completes |

### Ports

| Port | Purpose | Auth |
|------|---------|------|
| `8006` | Web-based noVNC viewer (primarily for watching installation progress) | **No authentication** — must be accessed over SSH tunnel or behind basic auth, never published directly to the internet |
| `3389` (TCP + UDP) | RDP access | Windows-authenticated — the primary, secure entry point |

### Networking

- Default: Docker bridge networking
- macvlan: Supported for assigning Windows a dedicated IP on the LAN. Requires `DHCP: "Y"` and `/dev/vhost-net`. A macvlan workaround is needed for host-to-container communication (the host cannot reach macvlan containers by default).

### Default credentials

- Username: `Docker`
- Password: `admin`

These should be changed inside Windows on first login. vm-uncloud's
`prepare.nu` deliberately does NOT inject a password env var — it uses
dockur's defaults and the operator changes the password manually. This also
dodges the uncloud env-newline quirk for exact-match values.

### Startup behavior

The image is small (~134 MB) — it does NOT bundle Windows. On first run it:
1. Downloads the Windows ISO from Microsoft servers (6-8 GB for Windows 11)
2. Runs the unattended setup automatically
3. Creates the default user account

First boot takes ~15-30 minutes on TCG (Hetzner Cloud), less on KVM.
Subsequent starts resume from the persistent `/storage` volume instantly.

## KVM vs TCG — the fundamental distinction

This is the axis that determines performance, cost, and which cloud providers
can be used. vm-uncloud demonstrates both paths in production.

### KVM (Kernel-based Virtual Machine)

- **Speed**: Native or near-native. QEMU uses `/dev/kvm` for hardware-accelerated virtualization.
- **Requirements**: The host must expose `/dev/kvm`. This means:
  - Bare metal servers (Vultr Bare Metal, Hetzner Robot/Dedicated)
  - Machines with nested virtualization enabled (cloud VMs with exposed vmx/svm)
- **Hetzner Cloud does NOT expose `/dev/kvm`** on any tier (CX, CPX, CCX, CAX).
- **Cost**: Bare metal is significantly more expensive. Hetzner Robot auction floor: ~€46/mo for 64 GB used dedicated. Vultr BM: hourly billing.
- **Use case**: Interactive Windows desktop, macOS (which refuses to run on TCG), any workload where latency matters.

### TCG (Tiny Code Generator)

- **Speed**: ~5-10x slower than KVM. QEMU emulates every CPU instruction in software.
- **Requirements**: No special hardware. Works anywhere Docker runs.
- **Enabled via**: `KVM: "N"` environment variable (and omitting `/dev/kvm` from devices).
- **Use case**: Batch/unattended workloads (builds, tests, automation). Painful for interactive use but viable.
- **Cost**: Runs on cheap cloud VMs (Hetzner cpx42 at ~€0.04/hr).

### How vm-uncloud handles both

vm-uncloud splits these into **separate node classes and recipes**:

| Path | Node class | Provider | Recipe | Compose |
|------|-----------|----------|--------|---------|
| **TCG** | `win-batch` | Hetzner Cloud cpx42 | `windows` | `KVM: "N"`, no `/dev/kvm` device |
| **KVM** | `win-kvm` | Vultr Bare Metal | `windows-kvm` | No `KVM` env, `devices: [/dev/kvm]` |
| **TCG (dev)** | `dev-win` | Hetzner Cloud cpx42 | `dev-windows` | `KVM: "N"`, builds from Dockerfile with `/oem` baked in |

The two recipes (`windows` vs `windows-kvm`) are nearly identical except for
the KVM toggle and device mapping. This is deliberate — it keeps the config
explicit and auditable rather than conditional logic in a single compose.

Key operational differences:
- **win-batch**: `win:down` takes a **snapshot before destroy** (via `hcloud server create-image`). `win:up` can restore from that snapshot. State-preserving on-demand Windows.
- **win-kvm**: Vultr BM has **no hot-snapshot**. `win-kvm:down` destroys without preserving state. An experimental R2 snapshot-transit pipeline exists (via `r2:*` tasks) but is scaffolded, not verified.
- **macOS**: Only runs on KVM — `KVM: "N"` is not an option. The `macos-kvm` recipe requires bare metal. macOS has no RDP; access is via noVNC viewer (SSH-tunneled) and optionally Remote Login (SSH) enabled inside the guest.

## dev-windows — OEM provisioning pattern

The `dev-windows` recipe shows how to bake first-boot provisioning into a
dockurr/windows image. This is the pattern for producing a "ready-to-use"
Windows environment rather than a bare install.

### Dockerfile (recipes/dev-windows/Dockerfile)

```dockerfile
FROM dockurr/windows:5.15
COPY oem/ /oem/
```

Just two lines. The base image is the same pinned version (`5.15`) used
everywhere. The only addition is copying the `oem/` directory into `/oem`.

### How /oem works

dockurr processes the `/oem` volume after Windows setup completes:
1. Windows unattended install finishes
2. dockurr copies `/oem` → `C:\OEM` inside the guest
3. If `C:\OEM\install.bat` exists, it is executed once as Administrator

### The install.bat (recipes/dev-windows/oem/install.bat)

```batch
REM Installs: git + mise (via winget), OpenSSH Server (via PowerShell),
REM and authorizes the developer's key from \\host.lan\Data\dev\authorized_keys
```

The script does three things:

1. **winget installs**: `git` and `mise` (Windows package manager). If winget
   is not available yet, the script prints a message and continues — the
   operator can run it manually later. This is a best-effort pattern.

2. **OpenSSH Server**: Enables the Windows OpenSSH capability, sets sshd to
   auto-start, opens the firewall rule for port 22. This is experimental —
   vm-uncloud has not verified that dockurr forwards port 22 to the guest.

3. **Key authorization**: Reads `\\host.lan\Data\dev\authorized_keys` (the
   shared volume, `devwin_shared`) and copies it to
   `C:\ProgramData\ssh\administrators_authorized_keys` with correct ACLs.
   This is the primary key-delivery mechanism.

### Why a build step instead of a volume mount

The Dockerfile **builds** the `/oem` into the image rather than mounting it as
a volume. The compose comment explains why:

> dockur runs C:\OEM\install.bat once after Windows setup. We must SHIP that
> file as an image layer, not a bind mount: recipes deploy to a REMOTE node,
> where a local `./oem` path does not exist (a `- ./oem:/oem` volume would
> resolve on the node's filesystem and be empty). `uc deploy` builds this and
> pushes only the small added layer.

This is a key insight for any recipe that needs files inside the guest at boot
time: the files must be in the image, not on the host.

### Host-to-guest file delivery

The primary path for getting files into the Windows guest is the shared volume
(`/data` → `\\host.lan\Data`). For dev-windows:
- The developer puts their repo + SSH key in the `devwin_shared` volume
- `install.bat` reads the key from `\\host.lan\Data\dev\authorized_keys`
- The developer opens a terminal in Windows (via RDP) and runs `mise run build`

For headless/CI use, this shared-volume bridge is also how test artifacts move
in and out.

## Version pinning

vm-uncloud pins `dockurr/windows:5.15` explicitly in every compose file:

```yaml
image: dockurr/windows:5.15
```

And for macOS:
```yaml
image: dockurr/macos:latest
```

The `windows` image is pinned to a specific **major.minor** tag, not `latest`.
This is deliberate — dockurr ships breaking QEMU config changes between
versions, and pinning ensures reproducible deploys. The `macos` image uses
`latest` because macOS on dockurr is less mature and benefits from upstream
fixes, but this is less rigorous than the Windows pinning.

The Dockerfile for dev-windows extends the same pinned base:
```dockerfile
FROM dockurr/windows:5.15
```

This means the OEM layer is always built on a known-good Windows image version.

## Known caveats from vm-uncloud production use

### 1. uncloud appends trailing newline to env values

Every environment variable injected by uncloud gets a `\n` appended. For most
values this is harmless, but for exact-match values like `KVM=N` it can cause
dockurr to misinterpret `"N\n"` as a truthy value and skip TCG mode. The
vm-uncloud windows compose warns:

> uncloud appends a trailing newline to injected env values (see caddy recipe).
> If dockur's KVM=N detection trips on "N\n", switch KVM off via an entrypoint
> trim wrapper like caddy/compose.yaml does. Confirm on the first real deploy.

The caddy recipe solves this by wrapping the command with `tr -d` to strip
CR/LF. The dev-linux entrypoint.sh also strips newlines from env values with
`printf '%s' "${VAR}" | tr -d '\r\n'`.

### 2. noVNC viewer has no authentication

Port 8006 (web viewer) serves an unauthenticated noVNC session. vm-uncloud
**deliberately does not publish this port**:

> Web viewer (noVNC, 8006) — NO auth; intentionally NOT published. Tunnel:
>   ssh -L 8006:localhost:8006 root@<node-ip> → http://localhost:8006

The `win:viewer` task automates this SSH tunnel. For Caddy-published access,
the compose notes that basic auth must be added first:

> To publish via Caddy, put basic auth in front first, then add:
>   - windows.${DOMAIN}:8006/https

### 3. macOS has no RDP

Unlike Windows, dockurr/macos only exposes port 8006 (noVNC) and 5900 (VNC).
There is no RDP endpoint. For headless build use, the operator must enable
Remote Login (SSH) inside the macOS guest after first-boot setup via the
viewer. This is a one-time manual step — there is no /oem equivalent for
macOS automation.

### 4. macOS licensing

Running macOS on non-Apple hardware violates Apple's EULA. The vm-uncloud
macos-kvm compose explicitly flags this:

> LICENSING: running macOS on non-Apple hardware violates Apple's EULA. Fine
> for personal/experimental use; decide the posture before relying on it in
> shared or public CI.

### 5. TCG is slow for interactive use

The ~5-10x slowdown with TCG makes interactive Windows painful. vm-uncloud's
TCG recipe is explicitly for "batch/unattended" workloads. For interactive
Windows, the KVM path (bare metal) is the supported option.

### 6. First-boot ISO download time

The docker image is small (134 MB), but the first boot downloads 6-8 GB from
Microsoft. On a Hetzner Cloud node (good peering), this takes ~5-10 minutes
for the download alone, plus another 10-20 minutes for unattended setup.
`win:rdp:wait` polls :3389 every 30 seconds with a 90-minute timeout.

### 7. AMD CPU core caveat (macOS)

On AMD systems, enabling multiple CPU cores before macOS installation completes
"may actually decrease performance instead or cause other issues like random
crashes" (upstream docs). The recommendation is to boot with `CPU_CORES: "1"`
for installation, then increase after setup.

### 8. Disk resize is one-way

Increasing `DISK_SIZE` is non-destructive (dockurr grows the qcow2), but the
Windows partition must be extended manually inside the guest afterwards.
Shrinking is not supported.

## Infrastructure images

Beyond the OS virtualization images, dockurr maintains several infrastructure
images with significant adoption:

| Image | Purpose | Pulls |
|-------|---------|-------|
| `dockurr/samba` | SMB file server. Used by dockurr/windows internally for `/shared` volume bridging. Also usable standalone. | 1M+ |
| `dockurr/dnsmasq` | DNS + DHCP server. Lightweight, configurable via env vars. | 500K+ |
| `dockurr/chrony` | NTP time server. Ensures accurate time in container environments where clock drift matters. | 500K+ |
| `dockurr/tor` | Tor SOCKS proxy. One-container Tor exit/relay/bridge. | 50K+ |
| `dockurr/stunnel` | TLS tunnel. Wraps any TCP service in TLS. | — |
| `dockurr/mdns` | mDNS (Bonjour/Avahi) for local service discovery. | — |
| `dockurr/munin` | System monitoring. | — |

These are conventional Docker containers (no QEMU), wrapping standard daemons
with environment-variable-driven configuration — the same UX pattern as the OS
images. For the testbed, they are relevant as ready-made sidecars for network
emulation scenarios (DNS, time, file sharing, Tor proxy).

## Relevance to foundation_testbed

### What we would use

**dockurr/windows** is a strong candidate for Windows testing in the testbed.
Its advantages over raw QEMU:

1. **Tested configurations**: dockurr ships QEMU arguments validated by 1M+
   pulls and 52k GitHub stars. We do not need to construct or maintain QEMU
   command lines.
2. **ISO auto-download**: Windows ISOs are fetched from Microsoft on first
   boot. No Vagrant Cloud dependency, no pre-downloaded ISO management.
3. **Persistent storage via Docker volumes**: `/storage` is a named Docker
   volume. The testbed can snapshot, clone, or discard Windows state by
   managing the volume — simpler than tracking qcow2 paths.
4. **Shared folder bridge**: `/shared` → `\\host.lan\Data` gives the testbed
   a file channel to push test binaries into and pull results out of the
   Windows guest without needing WinRM or SSH.
5. **Two-phase access**: RDP (port 3389) is Windows-authenticated and secure.
   noVNC (port 8006) is unauthenticated but provides a fallback viewer when
   RDP/WinRM is not yet configured. Both can be tunneled over SSH.
6. **OEM provisioning**: The `/oem` pattern (baking `install.bat` into a
   derived image) enables the testbed to pre-install test harnesses, SSH
   servers, or build toolchains before the Windows desktop even appears.
7. **KVM/TCG flexibility**: The testbed can target cheap cloud VMs (TCG) for
   CI batch tests, or bare metal (KVM) for interactive debugging or
   performance-sensitive tests.
8. **Version pinning**: `dockurr/windows:5.15` is a known-good version.
   The testbed can pin and upgrade deliberately.

**dockurr/macos** could be used for macOS testing, but the licensing constraint
(Apple EULA violation on non-Apple hardware) restricts it to
personal/experimental use. Not viable for shared CI without legal review.

### What we would not use

| Image | Reason |
|-------|--------|
| `dockurr/chromeos` | Low adoption (~105 pulls/week), ChromeOS is not a testbed target |
| `dockurr/proxmox*` | The testbed does not need to run hypervisors inside containers |
| `dockurr/virtual-dsm` | Synology DSM is irrelevant to the testbed's scope |
| `dockurr/lemmy*`, `piefed`, `casa`, `umbrel`, `zima` | Application images, not infrastructure/testing tools |
| `dockurr/strfry`, `statping`, `bitcoin` | Niche application images |
| `dockurr/qemu` | We use the OS images which already wrap QEMU; no need for the base image directly |
| `dockurr/munin`, `portainer-backup` | Not relevant to testbed operations |

### Integration model

The testbed would integrate dockurr/windows as follows:

1. **Profile definition**: A `ContainerProfile` for Windows testing references
   `dockurr/windows:5.15` as its image. Environment variables (RAM, CPU, disk
   size, KVM toggle) are profile parameters.

2. **OEM layer**: A testbed-specific Dockerfile extends `dockurr/windows:5.15`
   with an `/oem` directory containing `install.bat` that installs the test
   harness (SSH server, build tools, test runner). This image is built once and
   pushed to a registry.

3. **TCG-first deployment**: The default path targets Hetzner Cloud (TCG). This
   is cheap (~€0.04/hr), works immediately (no bare metal provisioning), and is
   sufficient for unattended build/test cycles.

4. **KVM optional**: For interactive debugging or tests that require native
   speed, the testbed supports a KVM node class (bare metal) — following the
   vm-uncloud pattern of separate context with `/dev/kvm` passed as a device.

5. **Volume management**: Windows state lives in a named Docker volume. The
   testbed can snapshot (volume backup), restore, or discard (fresh install)
   via Docker volume operations.

6. **Access pattern**: Primary access is RDP (3389) for authenticated remote
   desktop. Secondary access is the noVNC viewer (8006) over an SSH tunnel for
   debugging the boot process. After OEM provisioning, SSH from the guest
   (experimental) provides a scriptable entry point for test orchestration.

### Windows bootstrap sequence

The full lifecycle for a Windows test run in the testbed:

```
1. PROVISION: testbed creates cloud VM (Hetzner cpx42) with Docker
2. DEPLOY: Docker pulls dockurr/windows:5.15 + OEM-derived image
3. FIRST BOOT: dockurr downloads Windows ISO → unattended install (15-30 min)
4. OEM: C:\OEM\install.bat runs → installs tools, enables SSH
5. READY: RDP on :3389 accepts connections → testbed begins work
6. TEST: testbed pushes binaries via shared folder, executes, collects results
7. TEARDOWN: testbed snapshots (optional), destroys VM
```

The testbed's `ContainerProfile` abstraction already models this lifecycle.
dockurr/windows is just another image with a longer first-boot phase.
