# Compacted Context — Feature 01: QEMU Backend

## Spec
- `specifications/20-foundation-testbed/requirements.md` — high-level architecture
- `specifications/20-foundation-testbed/features/01-qemu-backend/feature.md` — comprehensive requirements

## Key Design Decisions
- QEMU as direct child process (not libvirt)
- User-mode networking (`-netdev user`)
- VNC headless, SPICE headful
- Sync API (no tokio)
- Pre-built qcow2 images

## Implementation Phases (Feature 01)
- Phase 1: `src/qemu/mod.rs` (QemuVm), `src/qemu/disk.rs` (qcow2 ops), `src/config.rs` (VmProfile, profiles)
- Phase 2: `src/qemu/net.rs` (port forwarding), `src/qemu/display.rs` (VNC/SPICE), `src/qemu/download.rs`
- Phase 3: snapshot management via monitor socket

## Crate Location
- `backends/foundation_testbed/` — new crate
- Dependencies: foundation_errstacks, foundation_core, clap, which, indicatif, serde, serde_json, reqwest (blocking), flate2, base64, dirs

## Workspace Paths
- foundation_errstacks: `./backends/foundation_errstacks`
- foundation_core: `./backends/foundation_core`
- Workspace root: `/home/darkvoid/Boxxed/@dev/ewe_platform`

## Key Structs
```rust
pub struct QemuVm { process: Child, monitor: UnixStream, profile: &'static VmProfile, disk_path: PathBuf }
pub struct VmProfile { name, os, image_name, ssh_port, rdp_port, winrm_port, vnc_port, user, pass, bootstrap, memory_mib, cpu_cores, disk_gb, prebaked_url }
pub enum GuestOs { Windows, Linux }
pub enum DisplayMode { Headless, Headful }
pub enum BootstrapMode { Full, SshOnly }
```

## QEMU Args Template
```
qemu-system-x86_64 -enable-kvm -m <MEM> -smp <CPU>
  -drive file=<DISK>,format=qcow2,if=virtio
  -netdev user,id=net,hostfwd=tcp:127.0.0.1:<SSH>-:22,hostfwd=tcp:127.0.0.1:<WINRM>-:5985,hostfwd=tcp:127.0.0.1:<RDP>-:3389
  -device virtio-net-pci,netdev=net
  -vga virtio
  -monitor unix:<SOCKET>,server,nowait
  -display vnc=:0  (headless) | -display spice-app (headful)
```

## Tasks: 1-9 (Phase 1: 1-4, Phase 2: 5-7, Phase 3: 8-9)
