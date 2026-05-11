# Learnings — 20-foundation-testbed

_Last updated: 2026-05-03_

## Design Decisions

- **QEMU as direct child process**, not libvirt — avoids root/sudo, simpler debugging, no daemon dependency
- **User-mode networking** (`-netdev user`) — zero root, declarative port forwarding in QEMU args
- **Display auto-detection** — probe `qemu -display help` for available backends; prefer SPICE (native window, clipboard, resize) > GTK (native window) > VNC (needs external viewer)
- **VNC viewer auto-detection** — try gvncviewer, vinagre, tigervncviewer, vncviewer in order; fall back to printing connection info
- **USB tablet for mouse tracking** — `-usb -device usb-tablet` sends absolute coordinates, fixing the mouse offset/drift bug in VNC viewers
- **tar+scp instead of rsync** — rsync requires daemon on VM; tar+scp works with any SSH server, built into Windows 10+
- **WinRM for Windows bootstrap, SSH for everything else** — WinRM is the only reliable way to run elevated commands on fresh Windows; once OpenSSH is installed, all subsequent communication uses SSH
- **mise handles all tool installation inside VMs** — rust, cargo-binstall, sccache, tauri-cli, nushell, and OS-specific deps via mise. Reduces per-OS scripting surface.
- **nushell (`nu`) as the VM shell** — replaces bash (Linux) and PowerShell/cmd.exe (Windows) with a single cross-platform shell. Bootstrap scripts, build wrappers, and runner commands are written in nushell syntax, not OS-specific shell dialects. mise installs nushell as a tool.
- **Sync API** — no tokio needed; CLI calls are sequential, blocking is fine
- **Pre-built qcow2 images via Vagrant Cloud** — import from Vagrant Cloud API (libvirt provider) for Windows and Linux; auto-extract gzip-compressed tar boxes using `tar` CLI

## Bugs Fixed & Lessons Learned

### QEMU Display
- **`-vga virtio` not available in `qemu-base`** — virtio-vga is a separate device package. Switched to `-vga std` (standard VGA, universally available).
- **`-display spice-app` not available** — requires `qemu-ui-spice-app` package. Both SPICE and GTK display backends may be missing from minimal QEMU installs. Auto-detection now handles this gracefully.
- **SPICE vs VNC tradeoff** — SPICE gives clipboard sharing, audio passthrough, and auto-resizing; VNC is universally available but has lower performance and no features. Use SPICE when available, VNC as fallback.

### macOS Boot
- **Windows 11 requires UEFI/OVMF** — SeaBIOS fails with "could not read the boot disk". OVMF pflash drives (`OVMF_CODE.fd` read-only + per-VM `OVMF_VARS.fd` copy) are required.
- **OVMF firmware paths vary by distro** — Arch: `/usr/share/edk2/x64/OVMF_CODE.4m.fd`, Debian/Ubuntu: `/usr/share/OVMF/OVMF_CODE.fd`. Code must detect the correct path.
- **macOS will require OpenCore** — unlike Windows (UEFI) and Linux (SeaBIOS), macOS needs an OpenCore bootloader to bridge QEMU's virtual hardware. No official macOS Vagrant boxes exist (Apple EULA restriction). We replicate quickget's IPSW download + qcow2 creation logic in Rust (using `std::process::Command` for `qemu-img`, `simple_http` for downloads, `zip` for IPSW extraction) rather than depending on quickemu as an external tool.

### HTTP Downloads
- **`reqwest::blocking` panics inside tokio runtime** — fatal error: "can only call blocking::block_on from outside tokio". Replaced ALL reqwest usage with `curl` subprocess in download.rs and import/mod.rs. This avoids the tokio/reqwest conflict entirely and gives us progress bar integration.

### Vagrant Box Extraction
- **Vagrant boxes are gzip-compressed tar** — not plain tar. Detection via gzip magic bytes (0x1f 0x8b). Extraction via `tar -xzf` CLI, not the `tar` crate (which has iterator/filter incompatibilities with Result entries).
- **Vagrant Cloud API uses `download_url`** — not `url`. The response structure has nested `providers[].architectures[].providers[].download_url` for libvirt boxes.
- **No macOS boxes on Vagrant Cloud** — Apple's EULA restricts macOS virtualization to Apple hardware. macOS images are created via native IPSW download + BaseSystem extraction (replicating quickget logic in Rust).

### Arch Linux Package Conflicts
- **`qemu-base 10.2.2-2` vs `qemu-ui-* 10.2.2-4`** — pacman refuses to install UI packages when base has a different `qemu-common` version pin. Resolution: `pacman -Syu` (full system upgrade) updates both together. `yay -Syu` avoids root for AUR packages.

### Filesystem
- **`fs2::available_space` fails on non-existent directories** — the image cache directory may not exist on first run. Fixed by walking up ancestors to the first existing parent, defaulting to `/` if nothing exists.
- **NVRAM vars must be per-VM** — copying `OVMF_VARS.fd` to `state_dir().join("<profile>.nvram")` isolates each Windows VM's EFI variables. Copy must happen before first boot.

### Build System
- **`tar` crate `entries()` returns `Result` not `Iterator`** — can't use `filter_map` directly. Switched to `tar` CLI subprocess with `tar -tf` to list entries, `tar -xf -O` to extract specific files.
- **`archive.entries()` filter compatibility** — the archive entries iterator has Rust version-specific behavior. CLI approach avoids all compatibility issues.

## What Works Now

- **Windows VMs boot** with UEFI/OVMF + `-vga std` + VNC display
- **Linux VMs boot** with SeaBIOS (default) + VNC display
- **Display auto-detection** — SPICE > GTK > VNC, with viewer auto-launch
- **Mouse tracking fixed** — USB tablet device for absolute coordinates
- **Image import** — Vagrant Cloud API for Windows/Linux, curl subprocess for downloads
- **Health checks** — `testbed doctor` checks KVM, QEMU binaries, SSH keys, disk space, ports
- **Port conflict resolution** — dynamic port allocation when defaults are in use
- **Distro-agnostic install** — `mise run sys:install-testbed` auto-detects Arch/Debian/Ubuntu

## What's Next

- **macOS VM support** — OpenCore bootloader, native IPSW-based image creation, SATA boot disk
- **VM export & distribution** — export VMs as pre-built qcow2 + manifest, upload to R2/S3/GitHub Releases
- **Prebuilt image hosting** — self-hosted CDN for macOS and custom images
- **Snapshot CLI** — save/load/delete/restore VM snapshots via monitor commands
- **Build pipeline** — cross-compile for Windows/macOS from Linux host via VM

## Windows VM Investigation Findings

### SSH Timeout Root Cause
Windows VMs from Vagrant Cloud boot to the login screen, which blocks automated SSH access. The QEMU process runs and port 2222 forwards correctly, but OpenSSH Server requires a user session to be established before it accepts key-based authentication. Diagnosed via `gvncviewer :50` — revealed the login screen was the blocker. Serial console confirmed Windows had fully booted ("Windows Boot Manager" loaded, services started).

### Autologin Solution
Windows Winlogon registry keys (`AutoAdminLogon=1`, `DefaultUsername`, `DefaultPassword`) provide reliable autologin. Set via PowerShell over WinRM during bootstrap (before SSH becomes the primary channel). Placed after `LocalAccountTokenFilterPolicy` step since both are registry modifications.

### Virtio 9p Driver Requirement
Windows guests don't include virtio drivers by default. The `virtio-win-0.1.262.iso` from Fedora Project (~692MB) contains all necessary drivers. Installed via `pnputil -a` during bootstrap. Critical driver: `viofs.inf` (VirtIO Filesystem) — enables 9p/virtio-fs host directory mounts. ISO attached to QEMU as CD-ROM drive, drivers found by scanning CD-ROM label `virtio*`.

### 9p Mount on Windows Limitation
Even with `viofs` driver installed, Windows doesn't have a native `mount` command. The Plan 9 redirector service must be configured separately. Current Windows mount test checks for directory existence at `C:\Users\vagrant\project` (Vagrant-created) rather than a true 9p mount. Full 9p mount support on Windows requires additional work with the Plan 9 redirector service.

### Image Export
Configured Windows qcow2 (17GB) exported to `/home/darkvoid/EweStore/Testbed/` for reuse. EweStore serves as the centralized artifact store for VM images and ISOs.

### WebView2 Installation Failure & Fix (2026-05-10)
**Problem:** After long-running project mount setup (~3.5 min), WinRM becomes unreachable with "WinRM not reachable on port 5985", causing WebView2 installation to fail.

**Root Cause:** WinRM service becomes temporarily unresponsive after extended elevated operations (scheduled task running as SYSTEM for virtiofs mount setup). The WinRM shell connection times out or becomes stale.

**Solution:** Enhanced `install_webview2()` with retry logic and WinRM recovery:
1. **Retry loop** — 3 attempts with delays for check and installation phases
2. **WinRM recovery via SSH** — When WinRM fails, restart the WinRM service via SSH PowerShell
3. **Fresh connections** — Each retry creates a new WinRM connection
4. **SSH fallback** — Last resort: install via SSH PowerShell directly

**Code Pattern:**
```rust
fn install_with_recovery(profile, session, winrm) {
    // Try original WinRM connection with retries
    for attempt in 0..3 {
        match winrm.run_ps(script) {
            Ok(result) => return Ok(result),
            Err(e) => retry_with_delay(),
        }
    }
    
    // Recovery: restart WinRM via SSH
    if winrm_still_failing {
        ssh::exec_ps_windows(session, "Restart-Service WinRM")?;
        sleep(Duration::from_secs(10));
    }
    
    // Retry with fresh connections
    for attempt in 0..3 {
        let fresh_winrm = WinRM::from_profile(profile)?;
        match fresh_winrm.run_ps(script) {
            Ok(result) => return Ok(result),
            Err(e) => retry_or_fallback_to_ssh(),
        }
    }
}
```

**Key Insight:** WinRM is reliable for short operations but can become unresponsive after long-running SYSTEM tasks. Having SSH as a fallback/recovery mechanism makes the bootstrap resilient.
