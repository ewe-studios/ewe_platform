---
feature: "QEMU Backend"
description: "QEMU process lifecycle management, disk image operations, network configuration, display modes, and VM snapshots"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "large"
created: 2026-05-02
last_updated: 2026-05-02
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# QEMU Backend Feature

## Overview

The lowest-level feature: manages the QEMU process lifecycle, virtual disk images, user-mode networking, display modes, and VM snapshots. This is the hypervisor abstraction layer that all other features build on top of.

## Dependencies

- No internal feature dependencies
- External: `qemu-system-x86_64`, `qemu-img`, KVM kernel module (`kvm_intel` or `kvm_amd`)

## Requirements

### 4.1 QEMU Process Management

- Launch QEMU as a Rust `std::process::Child` with configurable parameters
- Track PID for stale-process detection on restart
- Graceful shutdown via QEMU monitor socket (`system_powerdown`), fallback to `process.kill()`
- Monitor communication via Unix domain socket (`-monitor unix:/tmp/...`)
- Detect if QEMU is still alive via `process.try_wait()`

### 4.2 Disk Image Management

- Download pre-built qcow2 images from configured URLs
- Create new qcow2 disks via `qemu-img create -f qcow2 <path> <size>G`
- Resize existing qcow2 via `qemu-img resize <path> +<N>G`
- Get disk info via `qemu-img info --output=json`
- Verify disk integrity via `qemu-img check`
- Cache downloaded images in `~/.cache/foundation_testbed/images/`

### 4.3 Network Configuration

- User-mode networking via `-netdev user,id=net,hostfwd=...`
- Dynamic port allocation: if default port (2222) is in use, find next available
- Port forwarding map: SSH (default 2222), WinRM (default 5985), RDP (default 3389)
- No root/TAP/bridged networking — everything through QEMU's built-in user netdev

### 4.4 Display Modes

- **Headless**: `-display vnc=:N` → VNC server on localhost:<port>, accessible via any VNC client
- **Headful**: auto-detect best available backend — SPICE (`-display spice-app`, native window) > GTK (`-display gtk`, native window) > VNC (external viewer)
- Display backend detection runs `qemu-system-x86_64 -display help` and parses available backends
- Viewer auto-detection for VNC: tries gvncviewer, vinagre, tigervncviewer, vncviewer
- **USB tablet device** (`-usb -device usb-tablet`) — fixes mouse coordinate mismatch in VNC viewers
- VGA device: `-vga std` (universally available; `-vga virtio` not in `qemu-base`)
- Display mode is set at launch time; cannot be changed while VM is running

### 4.4b UEFI/OVMF Boot (Windows)

- Windows 11 requires UEFI firmware — SeaBIOS fails with "could not read the boot disk"
- OVMF pflash drives added for `GuestOs::Windows` profiles:
  - `OVMF_CODE.fd` as readonly read-only firmware
  - Per-VM `OVMF_VARS.fd` copy in state directory for NVRAM isolation
- OVMF firmware paths vary by distro:
  - Arch Linux: `/usr/share/edk2/x64/OVMF_CODE.4m.fd`
  - Debian/Ubuntu: `/usr/share/OVMF/OVMF_CODE.fd`
- Linux profiles boot with SeaBIOS (default, no OVMF needed)

### 4.5 VM Snapshots

- Save VM state via QEMU monitor `savevm <name>` command
- Load VM state via QEMU monitor `loadvm <name>` command
- Delete snapshots via QEMU monitor `delvm <name>`
- List snapshots via QEMU monitor `info snapshots`
- Snapshots are stored inside the qcow2 file itself (internal snapshots)
- External snapshot support via `qemu-img snapshot` for point-in-time disk copies

## Implementation Phases

### Phase 1: Core Process (Tasks 1-4)
1. Create `src/qemu/mod.rs` — `QemuVm` struct, launch config builder, process spawn
2. Create shutdown logic — monitor socket communication, graceful then hard kill
3. Create `src/qemu/disk.rs` — qcow2 create, info, check wrappers around `qemu-img`
4. Create `src/config.rs` — `VmProfile` struct, `GuestOs` enum, `DisplayMode` enum, default profiles

### Phase 2: Network & Display (Tasks 5-7)
5. Create `src/qemu/net.rs` — port forwarding arg builder, dynamic port allocation
6. Create `src/qemu/display.rs` — VNC/SPICE display arg selection
7. Create `src/qemu/download.rs` — HTTP download with progress bar, resume support, cache

### Phase 3: Snapshots (Tasks 8-9)
8. Create snapshot management — save, load, delete, list via monitor socket
9. Create external snapshot via `qemu-img snapshot -c <name>` for disk-level point-in-time

## Success Criteria

- [ ] `QemuVm::launch(profile, DisplayMode::Headless)` starts a QEMU process with correct args
- [ ] `QemuVm::shutdown()` gracefully powers down the VM within 30 seconds
- [ ] `qcow2::create(path, 80)` produces an 80 GB qcow2 file
- [ ] `qcow2::download(url, dest)` downloads with progress bar, resumes on interruption
- [ ] `snapshot::save(vm, "before-build")` creates an internal snapshot
- [ ] `snapshot::load(vm, "before-build")` restores VM to that state
- [ ] No root privileges required for any operation
- [ ] Multiple VMs can launch simultaneously without port conflicts

## Verification Commands

```bash
# Build the crate
cargo build -p foundation_testbed

# Run unit tests
cargo test -p foundation_testbed qemu::

# Integration test (requires KVM + qcow2 image)
cargo test -p foundation_testbed --test qemu_lifecycle -- --ignored
```

---

## Implementation Plan

### Architecture

The QEMU backend is structured as a process manager:

```
QemuVm
├── process: Child          # The qemu-system-x86_64 process
├── monitor: UnixStream     # QEMU monitor socket for commands
├── profile: &'static VmProfile  # VM configuration
└── disk_path: PathBuf      # Path to the qcow2 file
```

**Launch flow:**
1. Build `Command::new("qemu-system-x86_64")` with all args
2. Create monitor socket temp file
3. Spawn the process
4. Connect to monitor socket
5. Wait for guest boot (SSH/WinRM probe — handled by bootstrap feature, not here)

**Shutdown flow:**
1. Send `system_powerdown\n` via monitor socket
2. Poll `process.try_wait()` with 500ms intervals, 30s deadline
3. If timeout: `process.kill()`

**Key design: no libvirt, no XML, no daemon.** QEMU args are the configuration.

### Disk Image Strategy

We use **pre-built qcow2 images** to avoid the OS installation step. The image download happens in `import/ensure_image()`:

```
~/.cache/foundation_testbed/images/
├── windows-11-x86_64.qcow2     # ~6-10 GB pre-built Windows 11
└── ubuntu-24.04-x86_64.qcow2   # ~1-2 GB pre-built Ubuntu
```

Each profile references its image. If not present, it downloads from a configured URL.

Sources for pre-built images:
- Vagrant boxes (libvirt provider): https://app.vagrantup.com/boxes/search (provides qcow2)
- macOS: native IPSW download + BaseSystem extraction (feature 07)
- VM export: export bootstrapped VMs to R2/S3/GitHub Releases (feature 10)
- Build once with Packer + publish to our own hosting

The bootstrap flow is simpler with pre-built images because the OS already has:
- A user account (vagrant/vagrant)
- SSH server running
- WinRM enabled (for Windows)
- Network configured

So the first boot immediately allows SSH/WinRM connection, and the tool bootstrap (install VS Build Tools, etc.) runs right away. With an ISO, we'd need an unattended XML to automate the Windows setup first — 20-40 minutes before our tool can even connect.

### Port Allocation

Ports are allocated from the profile config. If the default is in use, we scan upward:

```rust
fn allocate_port(start: u16) -> u16 {
    (start..=start + 100)
        .find(|p| is_port_free(*p))
        .unwrap_or_else(|| panic!("no free port in {}-{}", start, start + 100))
}
```

This allows multiple VMs of the same profile type to run simultaneously.
