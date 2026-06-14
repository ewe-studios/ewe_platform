# foundation_testbed

The holistic test harness for the platform — two capabilities, each behind a
Cargo feature:

- **`vms`** — cross-platform VM testbed using QEMU/KVM for building and testing
  binaries. Launches QEMU as a direct child process (no libvirt) with user-mode
  networking, so **no root privileges are required** at runtime.
- **`wasm`** — a CLI-driven `wasm32-unknown-unknown` test harness: runs
  `#[wasm_test]` cases in a real **browser** (the pure-Rust CDP/BiDi driver, no
  node/Playwright) or **Cloudflare Workers** (wrangler).
- **`wasm-embedded-js`** — adds the **embedded Deno runtime** (spec-44): the owned
  `#[wasm_test]` harness runs JS **in-process** via `deno_core` + `deno_web` (V8),
  so `wasm-testbed deno` needs **no `node`/`deno` install** — `cargo build` gives
  JS test execution for free.

## What it does

**VM testbed (`vms`)**

- **Prebuilt image import** — download Vagrant Cloud images (libvirt provider)
  or raw qcow2 images, auto-extracting and caching them
- **QEMU VM lifecycle** — start, stop, restart VMs with configurable profiles
  (CPU, RAM, disk, ports)
- **Headless / headful display** — VNC for all modes; headful auto-launches
  `gvncviewer` or SPICE viewer
- **Bootstrap & provisioning** — wait for SSH/WinRM, run provisioning scripts,
  upload artifacts
- **Health checks** — `doctor` command checks host KVM, QEMU binaries, SSH keys,
  disk space, port availability, and per-VM state

**wasm harness (`wasm`)**

- **Discovery** — enumerate `#[wasm_test]` cases from a built module's exports
- **Runners** — the owned harness runs **in-process on the embedded Deno runtime**
  (`wasm-testbed deno`, with `wasm-embedded-js`) or in a real **browser** (CDP/BiDi
  via `foundation_browser`, `wasm-testbed web`); plus the wasm-bindgen interop modes
  and the Cloudflare Workers (wrangler) path
- **Scaffolding** — `init` writes the per-runner templates into a target project

## Features & binaries

Capabilities are feature-gated; **the default builds both** so the two binaries
are available out of the box. Disable selectively to take just one part.

| Feature | Enables | Pulls in |
|---------|---------|----------|
| `vms` *(default)* | the VM testbed (`src/vms/`) | qemu backend + ssh2, tar, image, `foundation_netio`, … |
| `wasm` *(default)* | the wasm harness (`src/wasm/`) | `foundation_browser`, `foundation_wasm_ui`, `foundation_http`, walrus, … |
| `wasm-embedded-js` | in-process JS runner for `wasm-testbed deno` (spec-44) | `deno_core` + `deno_web` (**links V8** — heavy) + tokio |
| `cli` *(default)* | the `clap` CLI for the VM testbed binary | clap |
| `utm` | macOS UTM/Hypervisor backend (with `vms`) | — |

> `wasm-embedded-js` is **not** in the default set: it links V8 (hundreds of MB,
> notable link time), so a plain `wasm` build (browser testing) stays light. Enable
> it when you want the zero-install, in-process `deno` runner. It deliberately omits
> Web Crypto (`crypto.subtle`) — the harness doesn't use it; see spec-44 §4.4.

**Binaries** (each only builds when its features are on):

| Binary | Requires | Purpose |
|--------|----------|---------|
| `testbed` | `cli` + `vms` | VM lifecycle CLI (`testbed start windows-build`, …) |
| `wasm-testbed` | `wasm` | wasm test runner (`wasm-testbed deno/web <module>`; `deno` needs `wasm-embedded-js`) |

> Why `cli` is in the default set: the `testbed` binary is gated on `cli` (its CLI
> module is `#[cfg(feature = "cli")]` and needs `clap`). Without `cli`, `vms`
> builds as a **library** but the `testbed` binary is skipped. `wasm` enables
> `clap` on its own for `wasm-testbed`.

```bash
# Default — both binaries:
cargo build -p foundation_testbed

# Just the VM CLI:
cargo build -p foundation_testbed --no-default-features --features vms,cli
# Just the wasm harness (browser/wrangler — light, no V8):
cargo build -p foundation_testbed --no-default-features --features wasm
# wasm harness + the in-process embedded Deno runner (links V8):
cargo build -p foundation_testbed --no-default-features --features wasm-embedded-js --profile uat
# vms as a library only (no CLI binary):
cargo build -p foundation_testbed --no-default-features --features vms
```

> **Build profile:** this crate's dev profile uses Cranelift, which currently
> crashes compiling it — build/test with the `uat` profile (LLVM):
> `cargo test -p foundation_testbed --profile uat`. The `web` runner needs a
> browser (`mise run test:browsers`); the owned `deno` runner is **in-process**
> (`wasm-embedded-js`) and needs **nothing installed**.

### Zero-install JS testing (spec-44)

The owned `#[wasm_test]` harness runs entirely in-process — no `node`, no `deno`:

```bash
# Build → discover #[wasm_test] cases → run headless on the embedded Deno runtime:
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-js \
  --profile uat --bin wasm-testbed -- deno path/to/your/wasm-crate
# or via mise:
mise run test:wasm-testbed:deno
```

How it works: `cargo build` links V8 (via `deno_core`), the harness stages a
self-contained runner (`runner.mjs` + `foundation-wasm.js` + `module.wasm` +
`cases.json`) and runs it on an embedded `deno_core` + `deno_web` runtime. Byte
loading + result reporting go through two Rust ops (`op_fwt_read_file`,
`op_fwt_report`) — no stdout parsing, no external process.

## Quick start

```bash
# 1. Install system dependencies (auto-detects your distro)
mise run sys:install-testbed

# 2. Download a prebuilt VM image
cargo run -p ewe_platform -- testbed import windows-build
cargo run -p ewe_platform -- testbed import linux-build

# 3. Start a VM
cargo run -p ewe_platform -- testbed start windows-build --headful
cargo run -p ewe_platform -- testbed start linux-build --headless

# 4. Run health check
cargo run -p ewe_platform -- testbed doctor
```

## System dependencies

### Required

| Component | Purpose | Notes |
|-----------|---------|-------|
| **QEMU** (`qemu-system-x86_64`) | VM hypervisor | KVM acceleration recommended |
| **OVMF/edk2** (UEFI firmware) | Boot Windows 11 VMs | Linux VMs boot with SeaBIOS (built-in) |
| **SSH client** (`ssh2` crate with vendored OpenSSL) | Guest provisioning | Vendored, no system dependency |
| **curl** | Image downloads | Used instead of reqwest to avoid tokio conflicts |
| **tar** | Vagrant box extraction | Standard on all distros |
| **VNC viewer** (`gvncviewer`) | Headful display | Auto-launched for `--headful` mode |

### Optional (better display)

| Component | Purpose | Notes |
|-----------|---------|-------|
| **SPICE** (`qemu-ui-spice-*` / `spice-client-gtk`) | Native SPICE window | Better than VNC: clipboard, resize, audio |
| **virtio-vga** (`qemu-device-display-virtio-vga`) | Para-virtualized GPU | Higher performance than `std` VGA |

### Architecture-specific notes

- **Windows profiles** require UEFI/OVMF firmware. Without it, Windows fails
  with "could not read the boot disk" (SeaBIOS doesn't understand the disk).
- **Linux profiles** boot fine with SeaBIOS (the QEMU default).
- **KVM** is optional but strongly recommended. Without it, QEMU falls back to
  software emulation (TCG) which is ~10-50x slower.

---

## Install via mise (recommended)

### Auto-detect (any distro)

```bash
mise run sys:install-testbed
```

This reads `/etc/os-release` and runs the right commands for your distro.

### Arch Linux

```bash
mise run sys:arch:testbed
```

Installs:
- `qemu-base`, `qemu-system-x86` — QEMU binaries
- `qemu-ui-spice-app`, `qemu-ui-spice-core`, `qemu-chardev-spice` — SPICE display
- `qemu-ui-gtk`, `qemu-ui-opengl` — GTK display
- `edk2-ovmf` — UEFI firmware for Windows
- `gvncviewer` — VNC viewer (fallback)
- `openssh`, `curl` — guest communication

**Package manager:** tries `yay` first (AUR helper), falls back to
`sudo pacman -Syu`. The `--needed` flag skips already-installed packages.

**Why `yay`?** On Arch, you may encounter dependency version mismatches (e.g.
`qemu-base 10.2.2-2` vs `qemu-ui-spice-app 10.2.2-4`). A full system upgrade
(`-Syu`) resolves these, and `yay` avoids needing root for AUR packages.

### Debian 12+

```bash
mise run sys:debian:testbed
```

Installs:
- `qemu-system-x86`, `qemu-system-gui` — QEMU binaries + GTK display
- `qemu-utils` — qemu-img, qemu-nbd tools
- `ovmf` — UEFI firmware (`/usr/share/OVMF/`)
- `gvncviewer` — VNC viewer
- `spice-client-gtk`, `virt-viewer` — SPICE display
- `openssh-client`, `curl` — guest communication

**Package manager:** `sudo apt-get install`

### Ubuntu 22.04+

```bash
mise run sys:ubuntu:testbed
```

Same packages as Debian. The `ovmf` package on Ubuntu provides firmware at
`/usr/share/OVMF/OVMF_CODE.fd` and `/usr/share/OVMF/OVMF_VARS.fd`.

---

## Manual installation (distro-specific commands)

If you prefer not to use mise, run these directly:

### Arch Linux

```bash
# With yay (AUR helper)
yay -Syu --needed qemu-base qemu-system-x86 qemu-ui-spice-app qemu-ui-spice-core \
    qemu-chardev-spice qemu-ui-gtk qemu-ui-opengl edk2-ovmf gvncviewer openssh curl

# Without yay (requires root)
sudo pacman -Syu --needed qemu-base qemu-system-x86 qemu-ui-spice-app \
    qemu-ui-spice-core qemu-chardev-spice qemu-ui-gtk qemu-ui-opengl \
    edk2-ovmf gvncviewer openssh curl
```

### Debian 12+

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    qemu-system-x86 qemu-system-gui qemu-utils ovmf \
    gvncviewer openssh-client curl spice-client-gtk virt-viewer
```

### Ubuntu 22.04+

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    qemu-system-x86 qemu-system-gui qemu-utils ovmf \
    gvncviewer openssh-client curl spice-client-gtk virt-viewer
```

### Fedora 40+

```bash
sudo dnf install -y \
    qemu-system-x86 qemu-ui-spice qemu-ui-gtk \
    edk2-ovmf tigervnc openssh-clients curl
```

### NixOS

Add to your `configuration.nix`:

```nix
environment.systemPackages = with pkgs; [
  qemu_kvm              # QEMU with KVM
  virt-viewer           # SPICE/VNC viewer
  openssh               # SSH client
  curl
];
# OVMF is included with qemu_kvm on NixOS
```

---

## Verify installation

After installing dependencies, run the health check:

```bash
cargo run -p ewe_platform -- testbed doctor
```

This checks:
- `/dev/kvm` exists and is readable (KVM acceleration)
- `qemu-system-x86_64` is on PATH
- SSH key exists (`~/.ssh/id_rsa` or `~/.ssh/id_ed25519`)
- Sufficient disk space in the image cache
- Default ports are not in use

---

## CLI usage

### Import a prebuilt image

```bash
# Download from Vagrant Cloud (gusztavvargadr/windows-11)
cargo run -p ewe_platform -- testbed import windows-build

# Download from Vagrant Cloud (alvistack/ubuntu-24.04)
cargo run -p ewe_platform -- testbed import linux-build
```

Supported profiles for import:

| Profile | Source | Size |
|---------|--------|------|
| `windows-build` | gusztavvargadr/windows-11 (Vagrant Cloud) | ~16 GB qcow2 |
| `windows-minimal` | gusztavvargadr/windows-11 (Vagrant Cloud) | ~16 GB qcow2 |
| `linux-build` | alvistack/ubuntu-24.04 (Vagrant Cloud) | ~1.8 GB qcow2 |
| `linux-minimal` | alvistack/ubuntu-24.04 (Vagrant Cloud) | ~1.8 GB qcow2 |

### Start a VM

```bash
# Headless (VNC server on 127.0.0.1:<port>)
cargo run -p ewe_platform -- testbed start windows-build --headless

# Headful (auto-launches VNC viewer)
cargo run -p ewe_platform -- testbed start windows-build --headful
```

### Stop a VM

```bash
cargo run -p ewe_platform -- testbed stop windows-build
```

### Health check

```bash
# Host-level checks only
cargo run -p ewe_platform -- testbed doctor

# Host + specific VM profile
cargo run -p ewe_platform -- testbed doctor --profile windows-build
```

### List profiles

```bash
cargo run -p ewe_platform -- testbed list
```

---

## Directory layout

| Path | Purpose |
|------|---------|
| `~/.cache/foundation_testbed/images/` | Cached VM disk images (qcow2) |
| `~/.cache/foundation_testbed/state/` | Per-VM state (PID files, NVRAM vars) |
| `/tmp/foundation_testbed/` | QEMU monitor sockets |

### OVMF firmware paths (per distro)

| Distro | OVMF_CODE | OVMF_VARS |
|--------|-----------|-----------|
| Arch Linux | `/usr/share/edk2/x64/OVMF_CODE.4m.fd` | `/usr/share/edk2/x64/OVMF_VARS.4m.fd` |
| Debian/Ubuntu | `/usr/share/OVMF/OVMF_CODE.fd` | `/usr/share/OVMF/OVMF_VARS.fd` |
| Fedora | `/usr/share/edk2/ovmf/OVMF_CODE.fd` | `/usr/share/edk2/ovmf/OVMF_VARS.fd` |

Windows profiles automatically copy `OVMF_VARS` to the state directory for
per-VM NVRAM isolation.

---

## Troubleshooting

### "QemuExited { code: 1 }"

QEMU failed to start. Run the doctor command:
```bash
cargo run -p ewe_platform -- testbed doctor
```
Common causes:
- Missing QEMU binary — install via mise task above
- Missing OVMF firmware — install `edk2-ovmf` (Arch) or `ovmf` (Debian/Ubuntu)
- Port already in use — another VM or service is using the default port
- No disk space — at least 20 GB free for Windows images

### "Display 'spice-app' is not available"

SPICE UI packages are not installed. Run:
```bash
mise run sys:install-testbed
```
Or install the specific package for your distro (see tables above).

### Windows won't boot ("could not read the boot disk")

Windows 11 requires UEFI/OVMF firmware. Ensure:
1. `edk2-ovmf` (Arch) or `ovmf` (Debian/Ubuntu) is installed
2. The OVMF firmware files exist at the paths listed in the table above

### No KVM acceleration (very slow VMs)

Check `/dev/kvm` exists and you have permission:
```bash
ls -la /dev/kvm
# If not readable, add yourself to the kvm group:
sudo usermod -aG kvm $USER
# Then log out and back in
```

### VNC viewer doesn't launch

Install a VNC viewer:
```bash
# Arch
yay -S gvncviewer
# Debian/Ubuntu
sudo apt-get install gvncviewer
# Fedora
sudo dnf install tigervnc
```
Or connect manually: `vncviewer 127.0.0.1:5900` (port shown in `testbed list`).

---

## Windows Mount Setup

Windows VMs support project directory mounting via two mechanisms:

### Method 1: virtiofs (Primary, Recommended)

Uses `virtiofsd` daemon on the host and WinFsp + virtiofs.exe on the guest.

**Host requirements:**
- `virtiofsd` (usually at `/usr/lib/virtiofsd`)

**What happens on VM boot:**
1. virtio-win drivers are auto-installed (if missing)
2. WinFsp is auto-installed (if missing)
3. A scheduled task registers to auto-mount on boot
4. Mount appears at `C:\Users\vagrant\project`

### Method 2: SMB (Fallback)

Uses QEMU's built-in SMB server. The Windows guest mounts `\\10.0.2.4\qemu`.

**One-time host setup:**
```bash
# Run once to configure Samba for QEMU
cd backends/foundation_testbed
mise run setup-smb
```

This creates:
- `/etc/samba/smb.conf` (minimal QEMU-compatible config)
- Required directories (`/var/log/samba`, `/var/lib/samba`, `/run/samba`)
- Sets ownership and permissions
- Adds `CAP_NET_BIND_SERVICE` capability to smbd

**SMB Wrapper for modern Samba (4.x):**

Modern Samba requires an `ncalrpc` subdirectory that QEMU doesn't create. Install this wrapper:

```bash
# Install wrapper (creates required directories on smbd spawn)
sudo bash scripts/linux/install-smbd-wrapper.sh

# To uninstall (restore original)
sudo bash scripts/linux/install-smbd-wrapper.sh --uninstall
```

The wrapper intercepts smbd calls from QEMU, creates the required directories, then execs the real smbd.

### `testbed network` — Start VM with mount testing

Start a VM with specific network/share configuration for testing mounts:

```bash
# Start Windows VM with virtiofs (recommended for Windows)
cargo run -p ewe_platform -- testbed network windows-build --type virtiofs --host-dir .

# Start Windows VM with SMB fallback
cargo run -p ewe_platform -- testbed network windows-build --type smb --host-dir /path/to/project

# Start Linux VM with 9p
cargo run -p ewe_platform -- testbed network linux-build --type 9p --host-dir .

# Start VM in background (daemonize) for testing
cargo run -p ewe_platform -- testbed network windows-build --type virtiofs --host-dir . --daemonize

# Test mount and exit automatically
cargo run -p ewe_platform -- testbed network windows-build --type virtiofs --host-dir . --test-only

# Run with display for interactive debugging
cargo run -p ewe_platform -- testbed network windows-build --type virtiofs --headful
```

**Options:**
| Option | Description |
|--------|-------------|
| `--type` | Mount type: `virtiofs` (Windows), `smb` (Windows), `9p` (Linux/macOS), `none` |
| `--host-dir` | Host directory to share (default: current directory) |
| `--guest-dir` | Override guest mount point |
| `--headful` | Run with graphical display (auto-launches VNC) |
| `--daemonize` | Run VM in background (useful for testing/debugging) |
| `--test-only` | Start VM, verify mount, then stop (for automated testing) |

**Workflow for testing mounts:**

```bash
# 1. Start VM with SMB in background
$ cargo run -p ewe_platform -- testbed network windows-build --type smb --host-dir . --daemonize
VM 'windows-build' started with SMB
  PID: 12345
  SSH: 127.0.0.1:2222
  SMB: \\10.0.2.4\qemu (maps to current directory)

# 2. Check if VM is running
$ cargo run -p ewe_platform -- testbed ls
windows-build  running  ssh:2222  winrm:5985  vnc:5900

# 3. Test SMB mount from inside VM
$ cargo run -p ewe_platform -- testbed exec windows-build --method winrm "net use Z: \\\\10.0.2.4\\qemu"

# 4. Verify mount works
$ cargo run -p ewe_platform -- testbed exec windows-build --method winrm "dir Z:\\"

# 5. Stop VM when done
$ cargo run -p ewe_platform -- testbed stop windows-build
```

### Testing SMB

After setup, test the SMB server:

```bash
# Create a test share
mkdir -p /tmp/test_share
echo "hello" > /tmp/test_share/test.txt

# Run QEMU with SMB (in one terminal)
qemu-system-x86_64 -netdev "user,id=net,smb=/tmp/test_share" -device virtio-net-pci,netdev=net ...

# In the Windows VM, mount:
net use Z: \\10.0.2.4\qemu
```

### Troubleshooting Mounts

| Symptom | Cause | Fix |
|---------|-------|-----|
| `virtiofs.exe not found` | virtio-win drivers not installed | Re-run bootstrap, check virtio-win ISO is attached |
| `failed to load WinFsp DLL` | WinFsp not installed | Re-run bootstrap WinFsp installation |
| `Network path not found` (SMB) | smbd not running | Run `sudo mise run setup-smb` |
| SMB connection refused | Missing ncalrpc dir | Install wrapper: `sudo bash scripts/linux/install-smbd-wrapper.sh` |
| Mount point exists but empty | virtiofs process died | Check Event Log: `Get-EventLog -LogName System -Source "VirtIO*"` |

### Manual Mount Verification

From the Windows VM (PowerShell as Admin):
```powershell
# Check virtio driver installation
Get-PnpDevice -Class System | Where-Object { $_.FriendlyName -like '*VirtIO*' }

# Check VirtIO-FS service
Get-Service VirtIO-FS -ErrorAction SilentlyContinue

# Check WinFsp service
Get-Service WinFsp.Launcher -ErrorAction SilentlyContinue

# Try manual mount (for debugging)
& "C:\Program Files\Virtio-Win\VioFS\virtiofs.exe" -t project -m C:\Users\vagrant\project -d -D 4

# Check Event Log for VirtIO errors
Get-EventLog -LogName System -Source "VirtIO*" -Newest 20

# Verify mount is working
Test-Path "C:\Users\vagrant\project\Cargo.toml"
```
