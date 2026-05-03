---
feature: "macOS VM Support"
description: "Run macOS guests on Linux via QEMU with OpenCore bootloader, enabling cross-compilation for Apple targets from a Linux host"
status: "pending"
priority: "medium"
depends_on: ["01-qemu-backend", "02-vm-communication"]
estimated_effort: "large"
created: 2026-05-03
last_updated: 2026-05-03
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# macOS VM Support

## Overview

Add support for running macOS as a guest OS on a Linux host via QEMU. This
enables cross-compilation for Apple targets (`aarch64-apple-darwin`,
`x86_64-apple-darwin`) and testing macOS-specific binaries — all from a Linux
machine.

Unlike Windows (UEFI boot) and Linux (SeaBIOS boot), macOS requires a
**bootloader** (OpenCore) to bridge between QEMU's virtual hardware and what
macOS expects. This feature covers both the QEMU argument configuration and the
image acquisition/creation pipeline.

## Why This Matters

| Use case | Why |
|----------|-----|
| Cross-compile for macOS | Build `.dmg`, `.pkg`, Mach-O binaries for Apple targets from Linux CI |
| Test macOS-specific code | Validate platform-conditional (`#[cfg(target_os = "macos")]`) |
| Universal binary testing | Verify both `x86_64-apple-darwin` and `aarch64-apple-darwin` outputs |
| CI/CD without Mac hardware | Run macOS build steps on Linux runners |

## Architecture

### Boot Requirements

macOS on QEMU needs:

1. **OpenCore bootloader** — an EFI application that bridges QEMU's virtual
   hardware to macOS expectations. Without OpenCore, macOS sees unknown hardware
   and refuses to boot.
2. **Custom SMBIOS** — macOS checks for Apple hardware identifiers. We must
   present as a `MacPro7,1` or `iMacPro1,1` via SMBIOS strings.
3. **Penryn CPU** (x86_64) — macOS doesn't recognize generic QEMU CPU models.
   `-cpu Penryn,kvm=on,vendor=GenuineIntel,+invtsc` is the standard.
4. **Apple-specific devices** — `isa-applesmc` for SMC (System Management
   Controller), `VirtIO` for network/disk.
5. **No UEFI pflash needed** — OpenCore runs from a FAT32 EFI partition, not
   from OVMF pflash drives.

### Prebuilt Image Sources

There are **no official macOS Vagrant boxes** on Vagrant Cloud (Apple's EULA
restricts macOS virtualization to Apple hardware). We have three options:

#### Option 1: quickemu (recommended for creation)

[quickemu](https://github.com/quickemu-project/quickemu) automates macOS
installation on QEMU:

- Downloads the macOS installer from Apple's CDN
- Creates a bootable disk image with OpenCore
- Generates the correct QEMU launch script
- Supports macOS 10.15 (Catalina) through 15.x (Sequoia)

**Integration strategy:** Run `quickget macos sonoma` to download and create
the image, then our testbed launches QEMU directly with the disk + OpenCore EFI
files that quickemu creates. We don't use quickemu's launch wrapper — we parse
its config and use our own `build_qemu_args`.

#### Option 2: Build once, host ourselves

Create a macOS qcow2 once on any machine (Mac or Linux with quickemu), then
host the qcow2 + OpenCore EFI files on our own CDN/S3. The `import` command
downloads and caches them like Windows/Linux images.

#### Option 3: User-provided images

Allow users to provide their own qcow2 + OpenCore EFI directory via config.
The testbed detects these and launches accordingly.

### QEMU Arguments for macOS

```bash
qemu-system-x86_64 \
  -enable-kvm \
  -cpu Penryn,kvm=on,vendor=GenuineIntel,+invtsc,vmware-cpuid-freq=on \
  -machine q35 \
  -smp 4,cores=4 \
  -m 8192 \
  -device qemu-xhci \
  -device usb-kbd -device usb-tablet \
  -device isa-applesmc,osk="ourhardworkbythesewordsguardedpleasedontsteal(c)AppleComputerInc" \
  -drive if=pflash,format=raw,readonly=on,file=/path/to/OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=/path/to/OVMF_VARS.fd \
  -device ich9-intel-hda -device hda-output \
  -device ich9-ahci,id=sata \
  -drive id=OpenCore,if=none,format=qcow2,file=/path/to/OpenCore.qcow2 \
  -device ide-hd,bus=sata.1,drive=OpenCore \
  -drive id=macOS,if=none,format=qcow2,file=/path/to/macOS.qcow2 \
  -device ide-hd,bus=sata.2,drive=macOS \
  -netdev user,id=net0,hostfwd=tcp::2223-:22 \
  -device virtio-net-pci,netdev=net0 \
  -display vnc=:3 \
  -monitor unix:/tmp/testbed/macos-build.monitor,server,nowait
```

**Key differences from Windows/Linux:**
| Aspect | Windows/Linux | macOS |
|--------|--------------|-------|
| CPU | `host` | `Penryn` (x86_64 only) |
| Machine | default | `q35` (required) |
| SMC | none | `isa-applesmc` with OSK string |
| Boot device | virtio disk | SATA (AHCI) |
| EFI | OVMF pflash | OpenCore on EFI partition |
| USB | `-usb -device usb-tablet` | `qemu-xhci + usb-tablet + usb-kbd` |

### macOS Profile

```rust
VmProfile {
    name: "macos-build",
    os: GuestOs::MacOS,
    image_name: "macos-sonoma-x86_64.qcow2",
    ssh_port: 2223,        // different from Windows (2222)
    rdp_port: None,
    winrm_port: None,
    vnc_port: 5903,        // different port
    user: "vagrant",
    pass: "vagrant",
    bootstrap: BootstrapMode::SshOnly,
    memory_mib: 8192,
    cpu_cores: 4,
    disk_gb: 80,
    prebaked_url: None,    // no Vagrant Cloud source
}
```

### GuestOs Enum Extension

```rust
pub enum GuestOs {
    Windows,
    Linux,
    MacOS,  // NEW
}
```

### Build QEMU Args Branch

The `build_qemu_args` function gets a new branch:

```rust
if profile.os == GuestOs::MacOS {
    // Machine type
    args.push("-machine".to_string());
    args.push("q35".to_string());

    // CPU (Penryn required for macOS)
    args.push("-cpu".to_string());
    args.push("Penryn,kvm=on,vendor=GenuineIntel,+invtsc,vmware-cpuid-freq=on".to_string());

    // SMC (Apple System Management Controller)
    args.push("-device".to_string());
    args.push("isa-applesmc,osk=\"ourhardworkbythesewordsguardedpleasedontsteal(c)AppleComputerInc\"".to_string());

    // USB controller + keyboard + tablet
    args.push("-device".to_string());
    args.push("qemu-xhci".to_string());
    args.push("-device".to_string());
    args.push("usb-kbd".to_string());
    args.push("-device".to_string());
    args.push("usb-tablet".to_string());

    // SATA controller for disk (macOS doesn't support virtio-blk for boot)
    args.push("-device".to_string());
    args.push("ich9-ahci,id=sata".to_string());

    // OpenCore EFI disk (first SATA device)
    // ... build from quickemu-generated files ...

    // macOS disk (second SATA device)
    // ...

    // Audio
    args.push("-device".to_string());
    args.push("ich9-intel-hda".to_string());
    args.push("-device".to_string());
    args.push("hda-output".to_string());
}
```

## Implementation Phases

### Phase 1: Infrastructure (Tasks 1-3)

1. Add `GuestOs::MacOS` variant to config, update display match arms
2. Create `macos-build` VmProfile with correct ports, RAM, CPU defaults
3. Update `build_qemu_args` with macOS-specific QEMU arguments

### Phase 2: Image Acquisition (Tasks 4-6)

4. Create `src/import/macos.rs` — quickemu integration:
   - Detect if `quickget` / `quickemu` is installed
   - Run `quickget macos <version>` to download installer
   - Parse quickemu config to extract QEMU args, disk paths, EFI paths
5. Add `mise run sys:arch:quickemu` task for installing quickemu on Arch
6. Update `import::ensure_image()` to handle `macos-build` profile

### Phase 3: Bootstrap & SSH (Tasks 7-9)

7. Create `src/bootstrap/macos.rs` — macOS-specific bootstrap:
   - Wait for SSH (no WinRM equivalent)
   - Install Xcode command-line tools (or detect if present)
   - Install mise, Rust toolchain
8. Update `doctor::check_vm()` to check macOS VM health
9. Add macOS-specific SSH bootstrap commands (different from Linux)

### Phase 4: Build Pipeline (Tasks 10-12)

10. Create `src/build/macos.rs` — macOS build pipeline:
    - `cargo build --target aarch64-apple-darwin`
    - `cargo build --target x86_64-apple-darwin`
    - Artifact extraction (`.dmg`, `.app`, `.pkg`)
11. Update CLI `testbed build` command to support `--target darwin`
12. Test full pipeline: start macOS VM → sync code → build → pull artifacts

## Success Criteria

- [ ] `ewe_platform testbed start macos-build --headless` boots macOS with SSH accessible
- [ ] `ewe_platform testbed import macos-build` downloads/creates macOS image via quickemu
- [ ] `ewe_platform testbed build macos-build --target aarch64-apple-darwin` produces Mach-O binary
- [ ] `ewe_platform testbed doctor --profile macos-build` checks macOS VM health
- [ ] macOS VM runs alongside Windows/Linux VMs without port conflicts

## Prebuilt Image Status

| Source | Available? | Format | Size | License Notes |
|--------|-----------|--------|------|---------------|
| Vagrant Cloud | No | — | — | Apple EULA restriction |
| quickemu (`quickget`) | Yes | qcow2 + EFI | ~30 GB | Downloads from Apple CDN |
| quickemu prebuilt | No | — | — | No official prebuilt distribution |
| Self-hosted (our CDN) | TBD | qcow2 + EFI | ~30 GB | Create once, distribute internally |
| User-provided | Yes | qcow2 + EFI dir | varies | User creates via quickemu/UTM |

## Dependencies

### Host Packages

| Package | Purpose | Arch | Debian/Ubuntu |
|---------|---------|------|---------------|
| `quickemu` | macOS image creation | AUR (`yay -S quickemu`) | PPA / manual .deb |
| `OVMF` | EFI firmware (OpenCore runs on top) | `edk2-ovmf` | `ovmf` |
| `qemu-base` | QEMU hypervisor | `qemu-base` | `qemu-system-x86` |
| `python3` | quickemu runtime | `python` | `python3` |
| `jq` | JSON parsing (quickemu config) | `jq` | `jq` |

### Inside Guest

| Tool | Installed via | Purpose |
|------|--------------|---------|
| SSH server | macOS System Preferences / `sudo systemsetup -setremotelogin on` | Remote access |
| Xcode CLT | `xcode-select --install` | C compiler, SDK headers |
| mise | Homebrew / curl | Tool management |
| Rust toolchain | mise | Cross-compilation |

## Verification Commands

```bash
# Install quickemu (Arch)
mise run sys:arch:quickemu

# Create macOS image
cargo run -p ewe_platform -- testbed import macos-build

# Start macOS VM
cargo run -p ewe_platform -- testbed start macos-build --headless

# Build for macOS target
cargo run -p ewe_platform -- testbed build macos-build --target aarch64-apple-darwin

# Health check
cargo run -p ewe_platform -- testbed doctor --profile macos-build
```

## Legal Note

Apple's macOS Software License Agreement restricts installation of macOS to
Apple-branded hardware. Running macOS in a VM on non-Apple hardware (Linux
host) may violate this agreement. This feature is provided for:

1. **Educational purposes** — understanding QEMU/macOS integration
2. **CI on Mac hardware** — the same code paths work on Apple Silicon Macs
   running UTM/QEMU in VM mode
3. **Hackintosh environments** — users who run macOS on non-Apple hardware

Users are responsible for compliance with applicable license terms.
