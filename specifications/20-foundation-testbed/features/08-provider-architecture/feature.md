---
feature: "Provider Architecture — QEMU + UTM backends"
description: "Split foundation_testbed into a provider-based architecture: QEMU on Linux, UTM on macOS. Shared code stays common, provider-specific code is gated behind feature flags and OS detection. Extract CLI into a reusable module, add standalone binary support, and create a nested mise.toml."
status: "in-progress"
priority: "critical"
depends_on: ["00-host-bootstrap", "01-qemu-backend", "02-vm-communication", "05-cli-state-management", "06-bin-integration"]
estimated_effort: "x-large"
created: 2026-05-03
last_updated: 2026-05-04
author: "Main Agent"
tasks:
  completed: 5
  uncompleted: 14
  total: 19
  completion_percentage: 26%
---

# Provider Architecture — QEMU + UTM Backends

## Overview

Refactor `foundation_testbed` from a Linux/QEMU-only crate into a **provider-based
architecture** that supports:

- **QEMU provider** — Linux hosts, QEMU/KVM with user-mode networking
- **UTM provider** — macOS hosts, Apple Hypervisor.framework via UTM.app

The goal is a single `testbed` CLI that works identically on both platforms:
`testbed start windows-build --headful` launches a Windows VM whether you're
on Arch Linux (QEMU) or macOS (UTM). The user doesn't care about the
hypervisor — they care that the VM boots and SSH is available.

## Why UTM on macOS

QEMU on macOS has significant limitations:
- **No KVM** — macOS uses Hypervisor.framework; QEMU's `hvf` accelerator exists
  but has poor device emulation support
- **No UEFI for Windows** — QEMU on macOS can't easily boot Windows UEFI images
- **Poor display** — no SPICE, no GTK; VNC only
- **No virtio** — limited paravirtualized device support

UTM solves all of these:
- Uses **Apple's Hypervisor.framework** natively (hardware-accelerated on Apple Silicon)
- Built-in **UEFI support** for Windows ARM64 guests
- **SPICE agent** for clipboard sharing, auto-resize
- **AppleScript API** for programmatic control (start, stop, screenshot, install ISO)
- GUI app with `.utm` bundle format (easy to manage VM configs)

## Architecture

### Provider Trait

The core abstraction. All hypervisor operations go through this trait:

```rust
/// A hypervisor backend provider.
pub trait Provider: Send + Sync {
    /// Human-readable name ("qemu" or "utm").
    fn name(&self) -> &'static str;

    /// Launch a VM with the given profile and display mode.
    /// Returns a handle for process management.
    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle>;

    /// Stop a running VM.
    fn stop(&self, vm_handle: &VmHandle) -> Result<()>;

    /// Check if a VM is running.
    fn is_running(&self, vm_handle: &VmHandle) -> bool;

    /// Get the resolved network ports for a running VM.
    fn resolved_ports(&self, vm_handle: &VmHandle) -> Result<ResolvedPorts>;

    /// Send a monitor/control command to the VM.
    fn monitor_command(&self, vm_handle: &VmHandle, cmd: &str) -> Result<String>;

    /// Import/download an image for this provider.
    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf>;

    /// Check host health (KVM available, binaries on PATH, etc.).
    fn host_health(&self) -> HostHealth;
}
```

### Directory Structure (after refactor)

```
backends/foundation_testbed/
├── Cargo.toml                    # Feature flags: qemu, utm, cli
├── mise.toml                     # Self-contained dev environment
├── README.md
├── src/
│   ├── lib.rs                    # Feature-gated provider exports
│   ├── config.rs                 # SHARED: VmProfile, GuestOs, DisplayMode
│   ├── error.rs                  # SHARED: TestbedError, Result
│   ├── common/                   # SHARED across providers
│   │   ├── mod.rs
│   │   ├── ssh/                  # SSH client (OS-agnostic)
│   │   ├── winrm/                # WinRM SOAP client (Windows-only)
│   │   ├── bootstrap/            # OS bootstrap logic
│   │   ├── build/                # Build pipeline
│   │   ├── runner/               # Binary runner, screenshots, logs
│   │   ├── state/                # JSON state persistence (project-scoped)
│   │   ├── doctor/               # Health checks (provider-agnostic part)
│   │   └── import/               # Image import (Vagrant Cloud, IPSW/macOS)
│   ├── providers/                # Provider implementations
│   │   ├── mod.rs                # Provider trait + factory
│   │   ├── qemu/                 # [feature: qemu] Linux/QEMU backend
│   │   │   ├── mod.rs            # QemuProvider impl
│   │   │   ├── process.rs        # QEMU process lifecycle (existing mod.rs)
│   │   │   ├── disk.rs           # qcow2 management
│   │   │   ├── net.rs            # Port allocation
│   │   │   ├── display.rs        # Display auto-detection
│   │   │   ├── snapshot.rs       # QEMU monitor snapshots
│   │   │   └── download.rs       # HTTP downloads
│   │   └── utm/                  # [feature: utm] macOS/UTM backend
│   │       ├── mod.rs            # UtmProvider impl
│   │       ├── utmctl.rs         # utmctl CLI wrapper (lifecycle)
│   │       ├── applescript.rs    # Network config, resource config, import
│   │       ├── import.rs         # Vagrant UTM registry, bundle prep
│   │       ├── state.rs          # Per-project state (.testbed/state/)
│   │       └── bundle.rs         # .utm config.plist creation/parsing
│   └── cli/                      # [feature: cli] Standalone CLI
│       ├── mod.rs                # build_command() -> clap::Command
│       └── handlers.rs           # Command dispatch (existing cli.rs logic)
```

### Feature Flags (Cargo.toml)

```toml
[features]
default = ["qemu"]

# Linux: QEMU/KVM backend
qemu = ["dep:which", "dep:fs2"]

# macOS: UTM/Hypervisor backend
utm = ["dep:dirs"]

# CLI: standalone clap binary + bin/platform integration
cli = ["dep:clap"]

# All providers (for testing on a machine with both)
all-providers = ["qemu", "utm"]

# Platform-default features (auto-selected by target_os)
linux-default = ["qemu", "cli"]
macos-default = ["utm", "cli"]
```

### Provider Selection (Runtime)

```rust
/// Select the appropriate provider based on the host OS and availability.
pub fn default_provider() -> Result<Box<dyn Provider>> {
    #[cfg(target_os = "linux")]
    {
        return Ok(Box::new(providers::qemu::QemuProvider::new()));
    }
    #[cfg(target_os = "macos")]
    {
        return Ok(Box::new(providers::utm::UtmProvider::new()));
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(TestbedError::UnsupportedHost {
            message: "testbed only supports Linux (QEMU) and macOS (UTM)".into(),
        })
    }
}
```

### VmHandle (Cross-Provider VM Reference)

Different providers track VMs differently (PID vs UUID vs bundle name).
The `VmHandle` abstracts this:

```rust
pub struct VmHandle {
    pub profile: VmProfile,
    pub provider_id: ProviderId,     // "qemu" or "utm"
    pub internal_id: String,         // PID (QEMU) or VM UUID (UTM)
    pub resolved_ports: ResolvedPorts,
    pub display_mode: DisplayMode,
}
```

## Directory & State Layout

This is the **ewe_platform convention** — clear separation between project-scoped
and global data:

```
$PWD/                        # Project root — testbed.toml lives here (committed)
├── testbed.toml             # VM config, image stores, mount points
├── .testbed/                # Local state (gitignored)
│   ├── state/               # JSON state files (vm-{name}.json)
│   └── artifacts/           # Symlink/mirror to VM's build output dir
├── src/                     # Your actual project code
├── Cargo.toml
└── ...

$HOME/.testbed/             # Global (user-scoped, never committed)
├── images/                 # Downloaded/built base images (qcow2, .utm bundles)
│   ├── windows-11-x86_64.qcow2
│   ├── ubuntu-24.04-x86_64.qcow2
│   └── utm/
│       └── windows-11_arm64.utm/   # Extracted UTM bundles
├── snapshots/              # Provider snapshots (QEMU savevm, UTM backups)
└── cache/                  # Temporary downloads, partial files
```

### Design Principles

1. **Persistent VM, ephemeral project data** — The VM keeps its OS, installed
   tools, and system state across restarts. The project root directory (where
   `testbed.toml` lives) is mounted into the VM (`$PWD` → VM `/mnt/project`).
   This means:
   - Build outputs are owned by the host (correct UID/GID, no root-owned files)
   - Artifacts survive VM shutdown
   - No `scp` roundtrip needed after builds
   - `$PWD/.testbed/artifacts/` is a symlink to `/mnt/project/target/` inside the VM

2. **`testbed.toml` for project config** — A TOML file in `$PWD/.testbed/` that
   defines local overrides:
   ```toml
   [vm]
   profile = "linux-build"
   memory_mib = 8192
   cpu_cores = 4

   [mounts]
   project = "."                    # Mount project root (where testbed.toml is)
   guest_path = "/mnt/project"      # VM-side mount point

   [artifacts]
   sync = true                      # Sync build outputs to .testbed/artifacts/

   [[image_stores]]                 # Ordered image backends
   name = "alex_r2"
   type = "r2"
   bucket = "testbed-images"

   [[image_stores]]
   name = "local_cache"
   type = "local"
   directory = "/mnt/fast-storage/images"
   ```

3. **Images are global** — Downloaded base images live in `$HOME/.testbed/images/`
   so they're shared across projects. A project references an image by name,
   not by path.

4. **State is project-scoped** — Each project tracks its own VM state
   (PID, UUID, resolved ports, bootstrap status) in `$PWD/.testbed/state/`.
   This means `testbed stop` in one project doesn't affect another project's VM
   even if they share the same profile name.

5. **All synchronous** — No `tokio`, no `async`/`.await`. All HTTP goes through
   `foundation_core::wire::simple_http` — a fully synchronous, valtron-based
   HTTP client with connection pooling, redirect handling, resume support,
   and body streaming. No subprocess curl, no reqwest panics.

### Image Store Resolution

Images are resolved from an ordered list of **image stores** defined in `testbed.toml`.
The first store that has the requested image wins. Each store is a named backend
with type-specific configuration:

```toml
[[image_stores]]
name = "alex_r2"
type = "r2"
bucket = "testbed-images"

[[image_stores]]
name = "team_s3"
type = "s3"
bucket = "ci-artifacts"
region = "us-east-1"

[[image_stores]]
name = "fast_disk"
type = "local"
directory = "/home/disk/testbed-images"

[[image_stores]]
name = "github"
type = "http"
base_url = "https://github.com/myorg/testbed-images/releases/download"
```

**Store types:**

| Type | Download method | Upload method (`testbed export`) |
|------|----------------|-----------------------------------|
| `local` | `std::fs::copy` | `std::fs::copy` |
| `r2` | `rclone` CLI (assumes user has `rclone` configured) | `rclone copy` |
| `s3` | `aws` CLI (assumes user has `aws` configured) | `aws s3 cp` |
| `http` | `foundation_core::wire::simple_http` with optional auth headers | `simple_http` PUT with credentials |
| `vagrant` | Vagrant Cloud API + `simple_http` download | Read-only (not an export target) |

**HTTP store credentials (optional):**
```toml
[[image_stores]]
name = "private_cdn"
type = "http"
base_url = "https://cdn.example.com/images"
api_key = "Bearer $ENV:CDN_API_KEY"    # $ENV: reads from env
cloudflare_account = "abc123"           # for R2 public URLs
```

**R2/S3 stores assume the user has CLI tools configured:**
- R2: `rclone` with a remote configured (e.g., `rclone:my-r2-remote`)
- S3: `aws` CLI with credentials in `~/.aws/credentials`

The store config references the bucket/account; credentials come from the CLI's
own config. If a user needs custom endpoints (e.g., MinIO), they add an
`endpoint` field.

**Resolution flow:**

```
// Vagrant Cloud is ALWAYS appended as the final fallback
resolved_stores = config.image_stores + default_vagrant_fallback(provider)

for store in resolved_stores:
    match store.type {
        Local { directory } → check if {directory}/{profile.image_name}.qcow2 exists
        R2 { bucket } → rclone ls to check, rclone copy to download
        S3 { bucket, region } → aws s3 ls to check, aws s3 cp to download
        Http { base_url } → HEAD request to check, GET to download
        Vagrant { registry } → Vagrant Cloud API lookup → download URL → download
    }
    if found: return path to downloaded/cached image
```

**Vagrant Cloud is always included as the final fallback**, even if the user
defines no `[[image_stores]]`. When `testbed init` generates `testbed.toml`, it
includes Vagrant Cloud as the default entry. Users can:
- Keep it (default behavior)
- Move it down in the list (custom stores checked first)
- Remove it entirely (opt out of Vagrant Cloud)

**Default Vagrant Cloud fallback:**
- QEMU provider: `type = "vagrant", registry = "libvirt"`
- UTM provider: `type = "vagrant", registry = "utm"`

**Example: `testbed init` generated config**

```toml
[[vms]]
name = "linux-build"
profile = "linux-build"

# Vagrant Cloud is the default — replace or add custom stores above it
[[image_stores]]
name = "vagrant_cloud"
type = "vagrant"
registry = "libvirt"
```

**Example: user adds custom stores, keeps Vagrant as last resort**

```toml
[[image_stores]]
name = "alex_r2"
type = "r2"
bucket = "my-testbed-images"
account_id = "abc123def456"

[[image_stores]]
name = "local_cache"
type = "local"
directory = "/mnt/fast-storage/testbed-images"

# Vagrant Cloud still here as final fallback
[[image_stores]]
name = "vagrant_cloud"
type = "vagrant"
registry = "libvirt"
```

When the user runs `testbed start linux-build`, it checks:
1. `$HOME/.testbed/images/` (already cached) → skip download
2. `alex_r2` → `rclone copy my-r2-remote:my-testbed-images/linux-build-x86_64.qcow2 ...`
3. `local_cache` → copy from `/mnt/fast-storage/testbed-images/linux-build-x86_64.qcow2`
4. `vagrant_cloud` → Vagrant Cloud API lookup → download

First match wins. Downloaded images are cached in `$HOME/.testbed/images/` so
subsequent starts don't re-download.

## VmProfile Struct (Shared)

The `VmProfile` struct is shared across providers:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,   // x86_64 / amd64
    Aarch64,  // ARM64
}

pub struct VmProfile {
    pub name:        &'static str,
    pub os:          GuestOs,
    pub arch:        Arch,               // Target architecture of the VM
    pub image_name:  &'static str,       // qcow2 filename (QEMU) or Vagrant box tag (UTM)
    pub ssh_port:    u16,
    pub rdp_port:    Option<u16>,
    pub winrm_port:  Option<u16>,
    pub vnc_port:    u16,
    pub user:        &'static str,
    pub pass:        &'static str,
    pub bootstrap:   BootstrapMode,      // Full or SshOnly
    pub memory_mib:  u32,
    pub cpu_cores:   u32,
}

impl VmProfile {
    /// Returns the Rust target triple for this profile.
    pub fn build_target(&self) -> &'static str {
        match (self.os, self.arch) {
            (GuestOs::Windows, Arch::X86_64)   => "x86_64-pc-windows-msvc",
            (GuestOs::Windows, Arch::Aarch64)  => "aarch64-pc-windows-msvc",
            (GuestOs::Linux,   Arch::X86_64)   => "x86_64-unknown-linux-gnu",
            (GuestOs::Linux,   Arch::Aarch64)  => "aarch64-unknown-linux-gnu",
            (GuestOs::MacOS,   Arch::X86_64)   => "x86_64-apple-darwin",
            (GuestOs::MacOS,   Arch::Aarch64)  => "aarch64-apple-darwin",
        }
    }
}
```

Profiles:

| Profile | OS | Arch | image_name | RAM | CPU | Bootstrap | Build Target |
|---------|-----|------|------------|-----|-----|-----------|-------------|
| windows-build | Windows | x86_64 | windows-11-x86_64 | 12 GB | 4 | Full | x86_64-pc-windows-msvc |
| windows-build-arm | Windows | aarch64 | windows-11-aarch64 | 12 GB | 4 | Full | aarch64-pc-windows-msvc |
| windows-test | Windows | x86_64 | windows-11-x86_64 | 4 GB | 2 | SshOnly | x86_64-pc-windows-msvc |
| linux-build | Linux | x86_64 | ubuntu-24.04-x86_64 | 4 GB | 4 | Full | x86_64-unknown-linux-gnu |
| linux-build-arm | Linux | aarch64 | ubuntu-24.04-aarch64 | 4 GB | 4 | Full | aarch64-unknown-linux-gnu |
| linux-test | Linux | x86_64 | ubuntu-24.04-x86_64 | 2 GB | 2 | SshOnly | x86_64-unknown-linux-gnu |
| linux-dev | Linux | x86_64 | debian-12-x86_64 | 6 GB | 4 | Full | x86_64-unknown-linux-gnu |
| macos-build | MacOS | x86_64 | macos-sonoma-x86_64 | 8 GB | 4 | SshOnly | x86_64-apple-darwin |

## QEMU Provider (Linux) — Existing Code, Moved

The current `foundation_testbed` code **becomes the QEMU provider**. Most files
stay the same, just reorganized under `providers/qemu/`:

| Current Path | New Path | Changes |
|-------------|----------|---------|
| `src/qemu/mod.rs` | `src/providers/qemu/process.rs` | Extract into `QemuProvider::launch()` |
| `src/qemu/disk.rs` | `src/providers/qemu/disk.rs` | No changes |
| `src/qemu/net.rs` | `src/providers/qemu/net.rs` | No changes |
| `src/qemu/display.rs` | `src/providers/qemu/display.rs` | No changes |
| `src/qemu/download.rs` | `src/providers/qemu/download.rs` | No changes |
| `src/qemu/snapshot.rs` | `src/providers/qemu/snapshot.rs` | No changes |
| `src/config.rs` | `src/config.rs` | No changes (shared) |
| `src/doctor/mod.rs` | `src/common/doctor/mod.rs` | Add provider-agnostic checks |
| `src/import/mod.rs` | `src/common/import/mod.rs` | Add UTM image source handling |
| `src/ssh/mod.rs` | `src/common/ssh/mod.rs` | No changes |
| `src/winrm/mod.rs` | `src/common/winrm/mod.rs` | No changes |
| `src/bootstrap/mod.rs` | `src/common/bootstrap/mod.rs` | Add macOS bootstrap |
| `src/build/mod.rs` | `src/common/build/mod.rs` | Add mount-aware build paths |
| `src/state/mod.rs` | `src/common/state/mod.rs` | Change paths: `$PWD/.testbed/state/` |

### QEMU Provider: 9p Mount Setup

For the host directory mount, QEMU uses **virtio-9p** (Plan 9 filesystem protocol):

```bash
# QEMU args for host directory mount:
-virtfs local,path=/path/to/host/dir,mount_tag=project,security_model=mapped,id=fs0
```

Inside the VM:
```bash
mount -t 9p -o trans=virtio,version=9p2000.L project /mnt/project
```

**Note:** 9p has performance limitations for large builds. An alternative is
`virtio-fs` (better performance, requires more setup). Start with 9p; if build
times are unacceptable, evaluate virtio-fs.

## UTM Provider (macOS) — New Code

### How UTM Works

UTM (https://github.com/utmapp/UTM) is a macOS GUI frontend for QEMU that uses
Apple's Hypervisor.framework. It manages VMs as `.utm` bundles (directories
containing config.plist, disk images, and metadata).

UTM provides two interfaces for automation:

**1. `utmctl` CLI** — at `/Applications/UTM.app/Contents/MacOS/utmctl`

```bash
# List all VMs (tabular: UUID  STATUS  NAME)
utmctl list

# Start/stop a VM by name
utmctl start "windows-build"
utmctl stop "windows-build"
```

VM lifecycle (list, start, stop) uses `utmctl`. Minimum tested version is
`4.6.5` — detect via `/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" /Applications/UTM.app/Contents/Info.plist`
and warn (non-fatal) if older.

**2. AppleScript via `osascript`** — for network config, resource config, and bundle import

```applescript
-- Find emulated NIC index (needed before setting port forwards)
tell application "UTM"
  set vm to virtual machine id "{uuid}"
  set cfg to configuration of vm
  set nis to network interfaces of cfg
  repeat with ni in nis
    if mode of ni is emulated then
      return index of ni
    end if
  end repeat
  return -1
end tell

-- Set port forwards on a specific NIC
tell application "UTM"
  set vm to virtual machine id "{uuid}"
  set config to configuration of vm
  set networkInterfaces to network interfaces of config
  repeat with anInterface in networkInterfaces
    if index of anInterface is {nic_index} then
      set portForwards to {}
      set newPortForward to {protocol:"TcPp", guest address:"", guest port:"22", host address:"127.0.0.1", host port:"2222"}
      copy newPortForward to the end of portForwards
      set port forwards of anInterface to portForwards
    end if
  end repeat
  update configuration of vm with config
end tell

-- Import a .utm bundle
tell application "UTM" to import new virtual machine from POSIX file "/path/to/bundle.utm"

-- Set VM resources
tell application "UTM"
  set vm to virtual machine id "{uuid}"
  set cfg to configuration of vm
  set memory of cfg to 8192
  set cpu cores of cfg to 4
  update configuration of vm with cfg
end tell
```

**Division of labor:**
| Operation | Interface |
|-----------|-----------|
| List VMs | `utmctl list` |
| Start VM | `utmctl start <name>` |
| Stop VM | `utmctl stop <name>` |
| Version check | `PlistBuddy` on Info.plist |
| Port forwards | AppleScript (find NIC, set rules) |
| Resource config | AppleScript (memory, cpu cores) |
| Bundle import | AppleScript (`import new virtual machine from POSIX file`) |
| ISO install | AppleScript (`install ISO at POSIX file`) |

### UTM Provider Implementation

```rust
pub const UTMCTL: &str = "/Applications/UTM.app/Contents/MacOS/utmctl";
pub const MIN_UTM_VERSION: &str = "4.6.5";

pub struct UtmProvider;

impl Provider for UtmProvider {
    fn name(&self) -> &'static str { "utm" }

    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle> {
        // 1. ensure_utm() — check utmctl works, install via brew if missing,
        //    wait for UTM.app to be ready (30s deadline)
        // 2. ensure_imported(profile) — download + import .utm bundle if not present
        // 3. utmctl start <name> — with 3 retries on failure
        // 4. wait_for_boot(profile, timeout) — SSH for Linux, WinRM for Windows
        // 5. configure_network(uuid, profile) — AppleScript: find NIC, set port forwards
        // 6. configure_resources(uuid, profile.memory, profile.cpu) — AppleScript
        // 7. Return VmHandle with UUID + resolved ports
    }

    fn stop(&self, vm_handle: &VmHandle) -> Result<()> {
        // utmctl stop <name> (only if running)
    }

    fn is_running(&self, vm_handle: &VmHandle) -> bool {
        // utmctl list → find by name → status == "started"
    }

    fn resolved_ports(&self, vm_handle: &VmHandle) -> Result<ResolvedPorts> {
        // Read from VmProfile — ports are static in shared network mode
    }

    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf> {
        // Vagrant Cloud UTM registry API → download .tar.gz → extract .utm →
        // rewrite config.plist Name → import via AppleScript → verify bundle on disk
    }

    fn host_health(&self) -> HostHealth {
        // Check: UTM installed (>= 4.6.5), Hypervisor.framework available,
        // sufficient RAM/disk, Rosetta 2 (for x86_64 VMs on Apple Silicon)
    }
}
```

### UTM: Host Directory Mount

UTM supports **directory sharing** through its shared directories feature.
The host directory is accessible inside the VM via the `virtiofs` mount or
through UTM's shared directory mechanism. For UTM, the mount point is
configured via AppleScript or the `.utm` bundle's `config.plist`:

```xml
<key>SharedDirectories</key>
<array>
    <dict>
        <key>HostPath</key>
        <string>/Users/alex/projects/my-app</string>
        <key>GuestPath</key>
        <string>/mnt/project</string>
        <key>ReadOnly</key>
        <false/>
    </dict>
</array>
```

### UTM Image Import Flow

UTM uses the same `[[image_stores]]` resolution as QEMU. The download
source is determined by iterating stores in order:

1. **Check if already imported** — `utmctl list` → find VM by `profile.name`
2. **Resolve image from stores** — iterate `[[image_stores]]` in order:
   - **Vagrant store** (`type = "vagrant"`): query
     `api.cloud.hashicorp.com/vagrant/2022-09-30/registry/utm/box/{box_name}/versions`
     → parse version → get direct download URL → stream with progress + resume
   - **HTTP store** (`type = "http"`): HEAD request to check, GET to download
     (e.g., Cloudflare R2 public URL, GitHub Releases)
   - **R2/S3 stores**: `rclone`/`aws` CLI to check and download
   - **Local store**: check if file exists on disk
3. **Extract** — `.box` files are `.tar.gz` containing a `.utm` bundle directory.
   Use `flate2` + `tar` crates with progress tracking.
4. **Prepare bundle** — copy to `$HOME/.testbed/images/utm/imports/{profile.name}.utm`,
   then **rewrite `config.plist`** `<key>Name</key>` value to `profile.name`. This is
   critical: if two profiles share the same box (e.g., `linux-test` + `linux-build`
   both on `ubuntu-24.04`), UTM will collide them in
   `~/Library/Containers/com.utmapp.UTM/Data/Documents/` without the rename.
5. **Import** — AppleScript: `tell application "UTM" to import new virtual machine
   from POSIX file "{path}"`. Wait up to 30s for the new VM UUID to appear in
   `utmctl list`.
6. **Verify** — check that `~/Library/Containers/com.utmapp.UTM/Data/Documents/{name}.utm`
   exists on disk. If not, delete the orphan via `utmctl delete {uuid}` and error.

### UTM State Management

Per-project state stored in `$PWD/.testbed/state/vm-{name}.json`:

```json
{
  "uuid": "A1B2C3D4-...",
  "display_name": "windows-build"
}
```

This allows cross-referencing between the profile name (what the user types)
and the UTM UUID (what `utmctl` and AppleScript need). State is loaded/saved
via `serde_json`.

### UTM Bootstrap (Windows)

The Windows bootstrap is extensive and idempotent — each step checks before
installing. Key patterns to port:

1. **OpenSSH Server** — `Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0`, edit `sshd_config` (comment out `Match Group administrators`, `AuthorizedKeysFile __PROGRAMDATA__`), install authorized key in **both** `~/.ssh/authorized_keys` and `C:\ProgramData\ssh\administrators_authorized_keys` with proper ACLs.
2. **LocalAccountTokenFilterPolicy** — registry set to `1` for WinRM to work with local admin accounts.
3. **VS Build Tools** — check `Hostarm64\x64\link.exe` exists (the ARM64-host x64-target cross-linker). If not, download `vs_buildtools.exe`, install `VCTools` workload + `--includeRecommended`. On ARM64 hosts, Microsoft does NOT ship `Hostarm64\arm64` native toolchain — this is why we check for the cross-linker.
4. **WebView2 Runtime** — Microsoft Evergreen Bootstrapper (not winget, which fails on fresh Vagrant boxes).
5. **mise** — try `winget install jdx.mise` first, fall back to `Invoke-WebRequest https://mise.run`.
6. **cargo-binstall** — direct `.exe` download from GitHub Releases (not `cargo install`, which would compile from source ~5 min, defeating the purpose).
7. **rustup default-host** — on ARM64 hosts, force `x86_64-pc-windows-msvc` BEFORE mise installs Rust, because VS Build Tools only has `Hostarm64\x64` cross-linker.

### UTM .utm Bundle Configuration

Each profile maps to a `.utm` bundle in `~/Library/Containers/com.utmapp.UTM/Data/Documents/`:

```
windows-build.utm/
├── config.plist          # UTM VM configuration
├── disk-0.qcow2          # Virtual disk
├── nvram.bin             # UEFI NVRAM (Windows ARM64)
└── ...
```

The `config.plist` is an XML property list:

```xml
<dict>
    <key>Architecture</key>
    <string>x86_64</string>   <!-- or arm64 for Apple Silicon VMs -->
    <key>CPUCount</key>
    <integer>4</integer>
    <key>MemorySize</key>
    <integer>8589934592</integer>  <!-- 8 GB in bytes -->
    <key>NetworkConfig</key>
    <dict>
        <key>NetworkMode</key>
        <string>Shared</string>
        <key>PortForwardRules</key>
        <array>
            <dict>
                <key>GuestPort</key> <integer>22</integer>
                <key>HostPort</key>  <integer>2222</integer>
                <key>Protocol</key>  <string>TCP</string>
            </dict>
        </array>
    </dict>
</dict>
```

### UTM Network Modes

| Mode | Description | Port Forwarding |
|------|-------------|-----------------|
| **Shared** | NAT with port forwarding | Configurable in `.utm` config.plist |
| **Bridged** | VM gets own IP on LAN | No forwarding needed |
| **Host Only** | VM isolated to host | Loopback only |

We use **Shared** mode (equivalent to QEMU's user-mode networking) with
port forwarding rules matching our profile definitions.

### UTM Display Modes

| Mode | Implementation |
|------|---------------|
| **Headless** | UTM runs without displaying the SPICE window |
| **Headful** | Open SPICE client via `open utm://...` URL scheme or AppleScript |

UTM uses SPICE internally for display, so no separate viewer detection is
needed — UTM's built-in SPICE client handles clipboard, resize, and audio.

### Windows on macOS via UTM

UTM supports Windows 11 ARM64 on Apple Silicon Macs:
- Uses Windows ARM64 VHDX images from Microsoft (free dev images)
- UEFI boot built-in (no OVMF configuration needed)
- SPICE guest tools for display acceleration
- SSH available after installing OpenSSH Server via Win32-OpenSSH

## CLI Module Extraction

### Current State

CLI registration logic lives in `bin/platform/src/testbed/mod.rs` — 157 lines
of clap subcommand definitions and dispatch. This is duplicated logic if we
want a standalone `foundation_testbed` binary.

### New Structure: `src/cli/mod.rs`

```rust
#[cfg(feature = "cli")]
pub mod cli;

/// Build the complete testbed CLI command tree.
/// Returns a `clap::Command` ready to be added as a subcommand to any parent.
#[cfg(feature = "cli")]
pub fn build_command() -> clap::Command {
    clap::Command::new("testbed")
        .about("VM orchestration — build, test, and run cross-platform binaries")
        .subcommand_required(true)
        .subcommand(clap::Command::new("init")
            .about("Initialize project — creates testbed.toml"))
        .subcommand(clap::Command::new("start")
            .about("Start a VM and hold the process — Ctrl-C stops it gracefully")
            .arg(Arg::new("name").required(true)))
        .subcommand(clap::Command::new("stop")
            .about("Stop a running VM")
            .arg(Arg::new("name").required(true)))
        // ... all subcommands ...
}
```

### `testbed init`

Creates `$PWD/testbed.toml` with a minimal default:

```toml
[[vms]]
name = "linux-build"
profile = "linux-build"

# Vagrant Cloud — default image source. Move down or remove to use other stores first.
[[image_stores]]
name = "vagrant_cloud"
type = "vagrant"
registry = "libvirt"
```

**Behavior:**
- If `testbed.toml` already exists → error: "already initialized"
- Creates `.testbed/` directory if it doesn't exist
- Creates `.testbed/state/` directory
- Prints: "Initialized testbed in testbed.toml — edit to add more VMs or image stores"

### `testbed start <name>` — Foreground Hold

This is the key UX difference from feature 05's original `start`:

1. **Reads `testbed.toml`** — finds the VM definition by `name`
2. **Ensures the image exists** — downloads if needed
3. **Ensures prerequisites** — host bootstrap (mise, nushell, pitchfork)
4. **Launches the VM** — via the appropriate provider (QEMU or UTM)
5. **Bootstraps if needed** — installs tools, configures SSH/WinRM
6. **Prints connection info** — ports, SSH command, mount status
7. **HOLDS THE FOREGROUND PROCESS** — blocks on `pitchfork wait` or signal listener
   - User can `Ctrl-C` at any time
   - On signal: graceful VM shutdown, resource cleanup, state saved
8. **Cleans up on exit** — stops VM, saves state, unmounts

```rust
fn cmd_start(matches: &ArgMatches) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    let config = TestbedConfig::load()?;  // reads testbed.toml
    let vm = config.find_vm(name)?;

    ensure_host_prerequisites()?;

    let provider = default_provider()?;
    let handle = provider.launch(&vm.profile, vm.display_mode)?;

    if vm.bootstrap && !is_bootstrapped(&handle)? {
        bootstrap_vm(&handle)?;
    }

    println!("VM '{}' ready", name);
    print_connection_info(&handle)?;

    // HOLD THE PROCESS — block until Ctrl-C / SIGTERM / SIGINT
    println!("Press Ctrl-C to stop the VM");
    wait_for_signal()?;

    // GRACEFUL SHUTDOWN
    println!("Stopping VM '{}'...", name);
    provider.stop(&handle)?;
    save_state(name, &handle)?;
    Ok(())
}
```

**Why foreground-hold instead of daemon:**
- User has a single, obvious way to stop the VM (Ctrl-C)
- No background process leaks (if the terminal dies, the VM dies)
- Simple mental model: `start` holds, `stop` kills from another terminal
- Works the same in CI (script starts, does work, Ctrl-C / signal stops)

### `testbed start --background <name>` (optional)

For users who want the VM to run detached:

```bash
testbed start --background linux-build
# → VM starts, process detaches, returns immediately
# → Use `testbed stop linux-build` to kill it later
# → Use `testbed ls` to see running VMs
```

Implementation: spawns the foreground hold in a detached subprocess (via `nohup` or `std::process::Command` with detached stdio).

### Full CLI Command Tree

```
testbed
├── init                            # Create testbed.toml
├── start <name> [--background]     # Launch VM, hold foreground (default)
├── stop <name>                     # Gracefully stop a VM
├── build <name> [--target <triple>] # Run build inside VM (requires mount)
├── exec <name> "<cmd>"             # Run command inside VM via nushell
├── shell <name>                    # Interactive SSH session
├── run <name> [--bin <path>]       # Launch binary inside VM
├── screenshot <name> --out <file>  # Capture display
├── logs <name> [--follow]          # Tail build/run logs
├── doctor [--vm <name>]            # Host + optional VM health check
├── push <name> --from <src> --to <dst>  # File to VM
├── pull <name> --from <src> --to <dst>  # File from VM
├── ls                              # List VMs (defined + running)
├── mount [status | verify]         # Show mount status for all VMs
├── image [list | clean]            # Show/clean cached images
└── snapshot
    ├── save <name> <label>
    ├── load <name> <label>
    ├── delete <name> <label>
    └── list <name>
```

## mise.toml for foundation_testbed

### Nested mise.toml

Create `backends/foundation_testbed/mise.toml`:

```toml
[env]
PROJECT_ROOT = "{{config_root}}"

[tools]
# QEMU provider (Linux)
"mise:qemu" = { version = "latest", os = "linux" }
"apt:qemu-system-x86" = { version = "latest", os = "linux" }

# UTM provider (macOS)
# UTM is a Mac App Store / Homebrew install, not mise-managed

[tasks.check]
description = "Check testbed prerequisites"
run = """
#!/usr/bin/env bash
# Detect OS and check provider
if [[ "$(uname)" == "Linux" ]]; then
    which qemu-system-x86_64 || { echo "QEMU not installed"; exit 1; }
    test -e /dev/kvm || echo "Warning: /dev/kvm not available (no KVM acceleration)"
elif [[ "$(uname)" == "Darwin" ]]; then
    test -d /Applications/UTM.app || { echo "UTM not installed"; exit 1; }
fi
echo "✓ Testbed prerequisites satisfied"
"""

[tasks.doctor]
description = "Run foundation_testbed health check"
run = "cargo run --release --bin testbed -- doctor"

[tasks.start]
description = "Start a VM"
run = """
#!/usr/bin/env bash
PROFILE="${1:-linux-build}"
cargo run --release --bin testbed -- start "$PROFILE"
"""

[tasks.stop]
description = "Stop a VM"
run = """
#!/usr/bin/env bash
PROFILE="${1:-linux-build}"
cargo run --release --bin testbed -- stop "$PROFILE"
"""
```

### Root mise.toml Reference

Mise supports **file includes** via the `includes` key. The root `mise.toml`
adds:

```toml
# mise.toml (root)
[env]
_.file = ["./backends/foundation_testbed/mise.toml"]
```

Or alternatively, tasks can reference each other across files when mise
scans the workspace. The foundation_testbed mise.toml is fully self-contained
and works standalone when `cd backends/foundation_testbed && mise run doctor`.

## Lessons Learned from Existing Implementation

### Display Backend Detection
**Problem:** `-display spice-app` and `-vga virtio` are not in `qemu-base` on
Arch. Hardcoding display backends causes startup failures.
**Solution:** Auto-detect available backends via `qemu -display help`, prefer
SPICE > GTK > VNC, auto-detect external viewers for VNC.
**UTM equivalent:** UTM always uses SPICE internally — no detection needed.

### Mouse Tracking in VNC
**Problem:** VNC viewers have severe mouse coordinate offset/drift.
**Solution:** Add `-usb -device usb-tablet` (absolute coordinates).
**UTM equivalent:** UTM's SPICE client has correct mouse tracking by default.

### UEFI Boot for Windows
**Problem:** Windows 11 requires UEFI/OVMF. SeaBIOS fails silently.
**Solution:** Add OVMF pflash drives for Windows profiles, copy OVMF_VARS
per-VM to state directory.
**UTM equivalent:** UTM has UEFI built-in — no configuration needed.

### HTTP Downloads
**Problem:** `reqwest::blocking` panics inside tokio runtime (bin/platform uses `tokio::main`).
**Problem:** Spawning `curl` subprocess is fragile — no progress callbacks, no connection pooling, no resume on interrupt.
**Solution:** Use `foundation_core::wire::simple_http` for all HTTP. `SimpleHttpClient` is fully synchronous, supports:
- `client.get(url)` → `SimpleHttpClient` with builder pattern
- `body_reader::collect_string(stream)` for text responses
- Configurable `max_body_size`, `max_retries`, `read_timeout`
- Connection pooling and redirect following
- `HttpRequestPending` states for progress tracking (Connecting, AwaitingResponse)
- Range header support for resume-after-interrupt downloads

Usage pattern for downloading images:
```rust
use foundation_core::wire::simple_http::client::{SimpleHttpClient, body_reader};

let mut client = SimpleHttpClient::from_system()
    .max_body_size(None)
    .batch_size(64 * 1024)
    .read_timeout(Duration::from_secs(30))
    .max_retries(5);

let request = client.get(url)?
    .header("Range", format!("bytes={}-", resume_from))
    .build()?;
// ... execute via valtron, stream body to file
```

### Vagrant Box Extraction
**Problem:** Vagrant boxes are gzip-compressed tar, not plain tar.
The `tar` crate has iterator compatibility issues with Rust versioning.
**Solution:** Use `simple_http` to stream the download body directly into
`flate2::read::GzDecoder` → `tar::Archive`. No intermediate file needed —
stream decompress and extract in one pass. For listing contents, download to
a temp file first and use CLI `tar -tf`.

### Arch Package Version Conflicts
**Problem:** `qemu-base 10.2.2-2` vs `qemu-ui-* 10.2.2-4` — pacman refuses
to install due to `qemu-common` version pin mismatch.
**Solution:** Use `yay -Syu --needed` for full system upgrade.
**UTM equivalent:** UTM is a single `.app` bundle via `brew install --cask utm` — no dependency conflicts.

### UTM Bundle Name Collision
**Problem:** Multiple profiles using the same Vagrant box (e.g., `linux-test`
and `linux-build` both on `ubuntu-24.04`) import as the same VM name in UTM,
causing the second import to become a half-broken orphan.
**Solution:** Before import, copy the extracted `.utm` bundle to a temp location
and rewrite `<key>Name</key>` in `config.plist` to `profile.name`. After import,
verify the bundle exists on disk; if not, delete the orphan UUID and error.

### UTM Version Compatibility
**Problem:** UTM releases can break AppleScript APIs or change behavior.
**Solution:** Check `CFBundleShortVersionString` via `PlistBuddy` against a
`MIN_UTM_VERSION` constant (currently `4.6.5`). Warn (non-fatal) if older.
Bump the constant when a UTM release ships a fix or feature utm-dev relies on.

### Windows ARM64 Toolchain Gap
**Problem:** VS Build Tools on ARM64 Windows ships only `Hostarm64\x64` and
`Hostarm64\x86` cross-tools — no `Hostarm64\arm64` native toolchain exists.
**Solution:** Force rustup's `default-host` to `x86_64-pc-windows-msvc` before
any project's `mise install` runs. Check for `Hostarm64\x64\link.exe` as the
idempotent marker (not the `--add VC.Tools.ARM64` component flag, which
installs but doesn't produce the binary on ARM64).

### WinRM + Local Admin Accounts
**Problem:** WinRM authentication fails for local admin accounts on Windows
due to UAC token filtering.
**Solution:** Set `LocalAccountTokenFilterPolicy = 1` in registry during
bootstrap. Also install SSH authorized keys in both user and admin paths.

### winget on Fresh Vagrant Boxes
**Problem:** `winget install` consistently fails on fresh Vagrant Windows boxes
because winget's Store source isn't primed.
**Solution:** Use direct downloaders (WebView2 Evergreen Bootstrapper,
cargo-binstall .exe from GitHub Releases) instead of winget for critical tools.
Try winget as a fallback for mise only.

### Port Conflict Resolution
**Problem:** Default ports (2222, 5985, 3389) may be in use.
**Solution (QEMU):** Dynamic port allocation — scan upward from default.
**UTM:** Port forwards are statically configured in the `.utm` bundle's
`config.plist` via AppleScript. Profile ports are fixed — no dynamic
allocation needed because UTM's shared network mode isolates per-VM.

### fs2 Disk Check on Non-Existent Directories
**Problem:** `fs2::available_space` panics on paths that don't exist.
**Solution:** Walk up ancestors to find first existing parent.
**UTM:** Same fix applies — check `$HOME/.testbed/images/` exists first.

### NVRAM Per-VM Isolation
**Problem:** Multiple Windows VMs share NVRAM state, causing boot conflicts.
**Solution (QEMU):** Copy OVMF_VARS to `$HOME/.testbed/snapshots/<profile>.nvram`.
**UTM:** UTM stores `nvram.bin` per `.utm` bundle — automatic isolation.

### Boot Wait Timeout
**Problem:** Windows VMs take significantly longer to boot than Linux VMs
(2-5 min vs 30-60s).
**Solution:** Separate timeout constants — `wait_for_winrm` uses 300s for
Windows, `wait_for_ssh` uses 120s for Linux. Poll every 5s with progress
logging every 30s.

### Emulated NIC Discovery (UTM)
**Problem:** UTM VMs can have multiple network interfaces; port forwards
must be set on the emulated NIC, not the virtio or bridged ones.
**Solution:** AppleScript iterates `network interfaces of configuration` and
finds the one where `mode is emulated`, returns its `index`. Then a second
AppleScript sets port forwards on that specific NIC index.

### Host Directory Mount
**Problem:** Without a mounted directory, build artifacts live inside the VM
disk image. They're inaccessible after the VM stops, and files pulled via `scp`
have incorrect ownership (VM user vs host user).
**Solution:** Mount the host's project directory into the VM via 9p (QEMU) or
shared directories (UTM). Builds write directly to the host filesystem.
`$PWD/.testbed/artifacts/` mirrors the VM's build output directory.

## Implementation Phases

### Phase 1: Infrastructure Refactor (Tasks 1-5)

1. Create `src/providers/mod.rs` — Provider trait, VmHandle, ProviderId, factory
2. Move existing `src/qemu/*` → `src/providers/qemu/*`
3. Move shared modules → `src/common/*` (ssh, winrm, bootstrap, build, runner,
   state, doctor, import) — update state paths to `$PWD/.testbed/state/`
4. Update `Cargo.toml` with feature flags (qemu, utm, cli)
5. Create `backends/foundation_testbed/mise.toml`

### Phase 2: CLI Extraction (Tasks 6-9)

6. Create `src/cli/mod.rs` — build_command() returning clap::Command
7. Move handler logic from `bin/platform/src/testbed/cli.rs` →
   `src/cli/handlers.rs`
8. Simplify `bin/platform/src/testbed/mod.rs` to 10-line delegation
9. Create `src/bin/testbed.rs` — standalone binary

### Phase 3: UTM Provider (Tasks 10-16)

10. Create `src/providers/utm/utmctl.rs` — utmctl wrapper: list_vms(), start_vm(), stop_vm(), version check via PlistBuddy
11. Create `src/providers/utm/applescript.rs` — osascript wrapper: configure_network() (NIC discovery + port forwards), configure_resources(), import_bundle()
12. Create `src/providers/utm/import.rs` — Vagrant Cloud UTM registry API client, download with progress + resume, tar.gz extraction, bundle preparation with plist Name rewrite, import + verify
13. Create `src/providers/utm/state.rs` — per-project state in `$PWD/.testbed/state/vm-{name}.json` (uuid + display_name)
14. Create `src/providers/utm/bundle.rs` — .utm config.plist creation/parsing for new VMs
15. Create `src/providers/utm/mod.rs` — UtmProvider impl, ensure_utm(), wait_for_boot() dispatch (SSH/WinRM)
16. Add macOS profiles to config.rs (Windows ARM64 via UTM, native macOS guest)

### Phase 4: Integration (Tasks 17-19)

17. Update root `mise.toml` to reference foundation_testbed/mise.toml
18. Add `#[cfg(target_os)]` provider selection in lib.rs
19. Update README.md with cross-platform documentation

## Success Criteria

- [ ] `cargo build -p foundation_testbed --features qemu` builds on Linux
- [ ] `cargo build -p foundation_testbed --features utm` builds on macOS
- [ ] `cargo build -p foundation_testbed --features cli` provides standalone binary
- [ ] `ewe_platform testbed start windows-build` works on Linux (QEMU) and macOS (UTM)
- [ ] `ewe_platform testbed doctor` reports provider-specific health
- [ ] `foundation_testbed/mise.toml` works standalone (cd into dir, `mise run doctor`)
- [ ] Root `mise.toml` references foundation_testbed tasks
- [ ] Zero code duplication between QEMU and UTM providers for shared logic
- [ ] Clap dependency only compiled when `cli` feature is enabled
- [ ] All existing tests pass with `--features qemu`
- [ ] Host directory mount works: builds inside VM produce files in `$PWD/.testbed/artifacts/`
- [ ] VM state stored in `$PWD/.testbed/state/`, images in `$HOME/.testbed/images/`
- [ ] No async dependencies — all I/O is synchronous

## Verification Commands

```bash
# Linux: QEMU provider
cargo build -p foundation_testbed --features qemu,cli
cargo run -p foundation_testbed --features qemu,cli -- start linux-build

# macOS: UTM provider
cargo build -p foundation_testbed --features utm,cli
cargo run -p foundation_testbed --features utm,cli -- start windows-build

# Standalone binary
cargo build -p foundation_testbed --features qemu,cli --bin testbed
./target/debug/testbed start linux-build

# Via ewe_platform
cargo build -p ewe_platform
cargo run -p ewe_platform -- testbed start windows-build

# Nested mise
cd backends/foundation_testbed
mise run doctor
```

## Legal & Licensing Notes

- **macOS on non-Apple hardware**: Apple's EULA restricts macOS virtualization
  to Apple-branded computers. The macOS provider (UTM) is intended for use on
  Mac hardware only.
- **UTM**: Licensed under GPL 3.0. We do not distribute UTM or link against it;
  we communicate via AppleScript/osascript (external process), which does not
  create a derivative work under GPL.
- **Windows on ARM**: Microsoft provides free developer VHDX images for
  testing; commercial use requires appropriate licensing.
