---
feature: "Provider Architecture — QEMU + UTM backends"
description: "Split foundation_testbed into a provider-based architecture: QEMU on Linux, UTM on macOS. Shared code stays common, provider-specific code is gated behind feature flags and OS detection. Extract CLI into a reusable module, add standalone binary support, and create a nested mise.toml."
status: "pending"
priority: "critical"
depends_on: ["01-qemu-backend", "02-vm-communication", "05-cli-state-management", "06-bin-integration"]
estimated_effort: "x-large"
created: 2026-05-03
last_updated: 2026-05-03
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
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
│   │   ├── state/                # JSON state persistence
│   │   ├── doctor/               # Health checks (provider-agnostic part)
│   │   └── import/               # Image import (Vagrant Cloud, quickemu)
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
│   │       ├── applescript.rs    # UTM AppleScript commands
│   │       ├── bundle.rs         # .utm bundle management
│   │       ├── spice.rs          # SPICE client for headful display
│   │       └── network.rs        # UTM network mode (shared/vlan)
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
| `src/build/mod.rs` | `src/common/build/mod.rs` | No changes |
| `src/state/mod.rs` | `src/common/state/mod.rs` | No changes |

### What Changes in QEMU Provider

1. **`QemuConfig` → `QemuProvider`** — the builder pattern becomes a provider struct
2. **`QemuVm` → `VmHandle`** — unified VM handle across providers
3. **OVMF path detection** — distro-aware lookup (Arch, Debian, Ubuntu, Fedora)
4. **State persistence** — store `provider_id: "qemu"` alongside PID

## UTM Provider (macOS) — New Code

### How UTM Works

UTM (https://github.com/utmapp/UTM) is a macOS GUI frontend for QEMU that uses
Apple's Hypervisor.framework. It manages VMs as `.utm` bundles (directories
containing config.plist, disk images, and metadata).

UTM provides an **AppleScript API** for automation:

```applescript
-- Start a VM
tell application "UTM"
    run VM named "windows-build"
end tell

-- Stop a VM
tell application "UTM"
    stop VM named "windows-build"
end tell

-- Install an ISO (first-time setup)
tell application "UTM"
    install ISO at POSIX file "/path/to/installer.iso" into VM named "my-vm"
end tell

-- List running VMs
tell application "UTM"
    get name of every VM whose running is true
end tell
```

We use the `osascript` CLI tool to run these from Rust:

```rust
fn run_applescript(script: &str) -> Result<String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()?;
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
```

### UTM Provider Implementation

```rust
pub struct UtmProvider;

impl Provider for UtmProvider {
    fn name(&self) -> &'static str { "utm" }

    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle> {
        // 1. Check UTM is installed (app bundle exists)
        // 2. Ensure .utm bundle exists for this profile
        // 3. If first run: create .utm bundle with correct config
        // 4. Run AppleScript: start VM
        // 5. Wait for network ports to be available
        // 6. Return VmHandle with resolved ports
    }

    fn stop(&self, vm_handle: &VmHandle) -> Result<()> {
        // AppleScript: stop VM
    }

    fn is_running(&self, vm_handle: &VmHandle) -> bool {
        // AppleScript: check VM running state
    }

    fn resolved_ports(&self, vm_handle: &VmHandle) -> Result<ResolvedPorts> {
        // UTM uses shared network mode with automatic port forwarding
        // Read from .utm config.plist
    }

    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf> {
        // For macOS hosts, we guide user through UTM's ISO installer
        // or download pre-built .utm bundles
    }

    fn host_health(&self) -> HostHealth {
        // Check: UTM installed, Hypervisor.framework available,
        // sufficient RAM/disk, Rosetta 2 (for x86_64 VMs on Apple Silicon)
    }
}
```

### UTM .utm Bundle Configuration

Each profile maps to a `.utm` bundle in `~/Library/Group Containers/WDNLXAD4W8.com.utmapp.UTM/Virtual Machines/`:

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

Profile for Windows ARM64 on macOS:

```rust
VmProfile {
    name: "windows-build",
    os: GuestOs::Windows,
    image_name: "windows-11-arm64.utm",  // .utm bundle, not qcow2
    ssh_port: 2222,
    rdp_port: Some(3389),
    winrm_port: Some(5985),
    vnc_port: 0,          // UTM uses SPICE, not VNC
    user: "vagrant",
    pass: "vagrant",
    bootstrap: BootstrapMode::Full,
    memory_mib: 8192,
    cpu_cores: 4,
    disk_gb: 80,
    prebaked_url: None,
}
```

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
        .subcommand(clap::Command::new("start")...)
        .subcommand(clap::Command::new("stop")...)
        // ... all subcommands ...
}

/// Dispatch a matched subcommand to its handler.
#[cfg(feature = "cli")]
pub fn dispatch(args: &clap::ArgMatches) -> Result<(), Box<dyn Error + Send + Sync>> {
    match args.subcommand() {
        Some(("start", m)) => handlers::cmd_start(m),
        // ... all dispatch arms ...
        _ => Err("unknown testbed subcommand".into()),
    }
}
```

### Usage from bin/platform (ewe_platform)

```rust
// bin/platform/Cargo.toml
foundation_testbed = { workspace = true, features = ["cli"] }

// bin/platform/src/testbed/mod.rs (10 lines, not 157)
pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(foundation_testbed::cli::build_command())
}

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    foundation_testbed::cli::dispatch(args).map_err(|e| e.into())
}
```

### Standalone Binary (foundation_testbed)

```rust
// backends/foundation_testbed/src/bin/testbed.rs
#[cfg(feature = "cli")]
fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cmd = foundation_testbed::cli::build_command();
    let args = cmd.get_matches();
    foundation_testbed::cli::dispatch(&args)
}
```

This allows:
- `cargo run -p foundation_testbed -- start windows-build` (standalone)
- `ewe_platform testbed start windows-build` (via bin/platform)

Both use the **exact same** command definitions and handlers.

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
**Problem:** `reqwest::blocking` panics inside tokio runtime.
**Solution:** Use `curl` subprocess for all HTTP downloads.
**UTM equivalent:** UTM downloads ISOs via its own UI; we trigger via
AppleScript which handles the download natively.

### Vagrant Box Extraction
**Problem:** Vagrant boxes are gzip-compressed tar, not plain tar.
The `tar` crate has iterator compatibility issues with Rust versioning.
**Solution:** Use `tar` CLI with `tar -tf` for listing, `tar -xf -O` for
extraction. Detect gzip via magic bytes (0x1f 0x8b).
**UTM equivalent:** UTM uses `.utm` bundles (directories) or `.ipsw` for
macOS installers — no tar extraction needed.

### Arch Package Version Conflicts
**Problem:** `qemu-base 10.2.2-2` vs `qemu-ui-* 10.2.2-4` — pacman refuses
to install due to `qemu-common` version pin mismatch.
**Solution:** Use `yay -Syu --needed` for full system upgrade.
**UTM equivalent:** UTM is a single `.app` bundle — no dependency conflicts.

### Port Conflict Resolution
**Problem:** Default ports (2222, 5985, 3389) may be in use.
**Solution:** Dynamic port allocation — scan upward from default.
**UTM equivalent:** UTM's shared network mode handles port conflicts internally.

### fs2 Disk Check on Non-Existent Directories
**Problem:** `fs2::available_space` panics on paths that don't exist.
**Solution:** Walk up ancestors to find first existing parent.
**UTM equivalent:** macOS `statfs` syscall (via same fs2 crate) — same fix
applies, UTM Library path checked first.

### NVRAM Per-VM Isolation
**Problem:** Multiple Windows VMs share NVRAM state, causing boot conflicts.
**Solution:** Copy OVMF_VARS to `state_dir().join("<profile>.nvram")`.
**UTM equivalent:** UTM stores `nvram.bin` per `.utm` bundle — automatic.

## Implementation Phases

### Phase 1: Infrastructure Refactor (Tasks 1-5)

1. Create `src/providers/mod.rs` — Provider trait, VmHandle, ProviderId, factory
2. Move existing `src/qemu/*` → `src/providers/qemu/*`
3. Move shared modules → `src/common/*` (ssh, winrm, bootstrap, build, runner,
   state, doctor, import)
4. Update `Cargo.toml` with feature flags (qemu, utm, cli)
5. Create `backends/foundation_testbed/mise.toml`

### Phase 2: CLI Extraction (Tasks 6-9)

6. Create `src/cli/mod.rs` — build_command() returning clap::Command
7. Move handler logic from `bin/platform/src/testbed/cli.rs` →
   `src/cli/handlers.rs`
8. Simplify `bin/platform/src/testbed/mod.rs` to 10-line delegation
9. Create `src/bin/testbed.rs` — standalone binary

### Phase 3: UTM Provider (Tasks 10-14)

10. Create `src/providers/utm/applescript.rs` — osascript wrapper, VM start/stop/list
11. Create `src/providers/utm/bundle.rs` — .utm config.plist creation/parsing
12. Create `src/providers/utm/network.rs` — shared mode port forwarding config
13. Create `src/providers/utm/mod.rs` — UtmProvider impl
14. Add macOS profiles to config.rs (Windows ARM64 via UTM, native macOS guest)

### Phase 4: Integration (Tasks 15-17)

15. Update root `mise.toml` to reference foundation_testbed/mise.toml
16. Add `#[cfg(target_os)]` provider selection in lib.rs
17. Update README.md with cross-platform documentation

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
