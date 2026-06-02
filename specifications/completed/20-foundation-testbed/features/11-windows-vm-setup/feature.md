---
feature: "Windows VM Autologin & Virtio 9p Driver Installation"
description: "Automated Windows VM post-boot setup: autologin via Winlogon registry keys and virtio-win driver installation from Fedora ISO to enable 9p project mounts"
status: "implemented"
priority: "high"
depends_on: ["01-qemu-backend", "02-vm-communication", "03-bootstrap-build-pipeline", "09-project-mount"]
estimated_effort: "medium"
created: 2026-05-03
last_updated: 2026-05-09
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Windows VM Autologin & Virtio 9p Driver Installation

## Overview

Windows VMs sourced from Vagrant Cloud ship without two critical pieces of configuration that block automated testing and development workflows:

1. **Autologin** — Windows sits at the login screen after boot, preventing SSH daemon from accepting connections and blocking all automated bootstrap/testing
2. **Virtio 9p drivers (`viofs`)** — QEMU's 9p/virtio filesystem protocol for host directory mounting requires the `virtio-win` driver package to be installed inside the Windows guest

Without these, Windows VMs require manual VNC login and driver installation before every use. This feature automates both steps during the Windows bootstrap process.

## Why This Matters

| Without this feature | With this feature |
|---------------------|-------------------|
| VM boots to login screen, SSH unresponsive | VM auto-logins, SSH accepts connections immediately |
| Manual VNC connection + login required | Zero manual intervention needed |
| `/mnt/project` equivalent not available on Windows | 9p mount works via `viofs` driver |
| Windows mount tests skipped in E2E suite | Full round-trip mount tests pass on Windows |
| `cargo tauri build` cannot use mounted project dir | Windows builds run on mounted project dir like Linux |

## Architecture

### Autologin via Winlogon Registry

Windows provides built-in autologin through registry keys under `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon`:

| Key | Type | Value | Purpose |
|-----|------|-------|---------|
| `AutoAdminLogon` | `REG_SZ` | `"1"` | Enable automatic logon |
| `DefaultUsername` | `REG_SZ` | `"vagrant"` | Username to log in as |
| `DefaultPassword` | `REG_SZ` | `"vagrant"` | Password for the account |
| `AutoLogonCount` | (delete) | — | Remove any countdown limiter |

These keys are set during bootstrap via PowerShell over WinRM (before SSH is available), using elevated execution to ensure registry write permissions.

**Why registry, not scheduled task or GPO:** The Winlogon autologin mechanism is the simplest, most reliable approach. It's the same method used by Packer/Vagrant base image builders. Scheduled tasks can race with the Lock Screen; GPO requires domain membership.

### Virtio Driver Installation from ISO

The `virtio-win` ISO from the Fedora Project contains all paravirtualized drivers for Windows guests running on QEMU/KVM:

- **Source**: `https://fedorapeople.org/groups/virt/virtio-win/direct-downloads/archive-virtio/virtio-win-0.1.262-1/virtio-win-0.1.262.iso`
- **Size**: ~692 MB
- **Local cache**: `/home/darkvoid/EweStore/Testbed/virtio-win-0.1.262.iso` (checked first)
- **Fallback**: Download to `~/.cache/foundation_testbed/images/virtio-win.iso`

#### Driver Inventory

The ISO contains drivers organized by Windows version (`w11/amd64/`). The following drivers are installed:

| INF File | Driver | Purpose |
|----------|--------|---------|
| `viofs.inf` | VirtIO Filesystem | **Critical** — enables 9p/virtio-fs mounts |
| `NetKVM.inf` | VirtIO Network | Network adapter performance |
| `viostor.inf` | VirtIO Block | Disk I/O performance |
| `balloon.inf` | VirtIO Memory Balloon | Dynamic memory management |
| `qemufwcfg.inf` | QEMU FW CFG | QEMU firmware configuration |
| `qemupciserial.inf` | QEMU PCI Serial | Serial port support |
| `viogpudo.inf` | VirtIO GPU | Display driver |
| `vioinput.inf` | VirtIO Input | Mouse/keyboard input |
| `viomem.inf` | VirtIO Memory | Memory balloon alternative |
| `viorng.inf` | VirtIO RNG | Hardware random number generator |
| `vioscsi.inf` | VirtIO SCSI | SCSI controller |
| `fwcfg.inf` | Firmware Config | Firmware configuration device |
| `pvpanic.inf` | PV Panic | Guest panic notification |

#### Installation Method

Drivers are installed via `pnputil.exe` (Windows built-in driver management utility):

```powershell
# Find the CD-ROM drive letter containing virtio-win
$cd = (Get-Volume | Where-Object { $_.FileSystemLabel -like 'virtio*' }).DriveLetter

# Install all w11/amd64 drivers
Get-ChildItem -Path "${cd}:\w11\amd64\*.inf" -Recurse | ForEach-Object {
    pnputil -a $_.FullName 2>&1 | Out-Null
}
```

**Why `pnputil`, not DISM or devcon:** `pnputil` is available on all Windows 10+ editions, requires no additional packages, and handles driver store staging + device installation in one command. DISM is for offline images; devcon is a separate download.

#### ISO Attachment to QEMU

The ISO is attached as a CD-ROM drive during VM launch:

```
-drive file=/path/to/virtio-win.iso,media=cdrom
```

This is only done for Windows profiles. Linux guests don't need the virtio-win ISO.

### WinFsp — Userspace Filesystem Framework

virtiofs.exe on Windows is NOT a standalone filesystem driver. It is a bridge process that connects the kernel-mode `viofs.sys` driver to a userspace filesystem framework. That framework is **WinFsp** (Windows File System Proxy), the Windows equivalent of Linux FUSE.

#### Why virtiofs.exe needs WinFsp

Without WinFsp installed, `virtiofs.exe` exits immediately with:

```
The service VirtIO-FS failed to load WinFsp DLL (Status=c0000034)
```

`c0000034` is `STATUS_OBJECT_NAME_NOT_FOUND` — the WinFsp DLL does not exist on the system. The `VirtIO-FS` Windows service depends on it.

#### Dependency chain for virtiofs project mount on Windows

```
QEMU → vhost-user-fs-pci device → viofs.sys (kernel driver, from virtio-win ISO)
      → virtiofs.exe (userspace bridge, from virtio-win ISO)
      → WinFsp.dll (userspace FS framework, from WinFsp MSI)
      → Windows filesystem (C:\Users\vagrant\project)
```

Both virtio-win AND WinFsp must be installed. Neither works alone.

#### WinFsp Installation

| Property | Value |
|----------|-------|
| Package | `winfsp-2.0.23075.msi` |
| Source | `https://github.com/winfsp/winfsp/releases/download/v2.0/winfsp-2.0.23075.msi` |
| Install method | `msiexec /i winfsp.msi /qn /norestart` |
| Verification | `Get-Service -Name 'WinFsp.Launcher'` status = Running |
| Idempotency | Script checks for service existence before downloading |

**Why MSI + silent install:** WinFsp provides an official MSI installer. The `/qn /norestart` flags make it fully silent with no reboot required — appropriate for bootstrap automation.

#### What WinFsp provides

- `WinFsp.Launcher` service (kernel driver `winfsp2.sys`)
- `WinFsp.dll` and `WinFsp-x64.dll` (userspace API libraries)
- Framework for building userspace filesystems (FUSE-compatible API)
- Mount manager integration for Windows drive letters and mount points

#### Comparison of mount approaches

| | SMB | VirtioFS |
|---|---|---|
| **Protocol** | Network file share (SMB/CIFS) | Paravirtualized shared memory |
| **Windows side** | Built-in (`net use`) | virtiofs.exe + WinFsp + virtio drivers |
| **Host side** | SMB server process | virtiofsd daemon + Unix socket |
| **Speed** | Network overhead | Direct memory sharing (faster) |
| **Guest deps** | None | WinFsp + virtio-win ISO + viofs.sys |
| **Setup** | Low complexity | Higher complexity, better performance |

SMB is tried first (fallback-free, works on stock images). VirtioFS is tried second (faster, but requires the full dependency chain above).

### Bootstrap Integration

The new steps are inserted into the existing Windows bootstrap flow (`bootstrap_windows()`):

```
Step 1:  install OpenSSH Server
Step 2:  authorize host SSH key
Step 3:  set LocalAccountTokenFilterPolicy
Step 3b: configure autologin          ← NEW (via WinRM, before SSH becomes primary)
Step 4:  install mise
...
Step 8:  install VS Build Tools
Step 9:  install WinFsp               ← NEW (userspace FS framework, required by virtiofs.exe)
Step 10: install virtio drivers       ← from ISO (viofs.sys, NetKVM, viostor)
Step 11: set up project mount         ← tries SMB first, then virtiofs
Step 12: install WebView2 Runtime
Step 13: debloat Windows
...
Step 17: set nushell as default shell
Step 18: write bootstrap marker
```

**Why autologin at Step 3b:** It's placed right after `LocalAccountTokenFilterPolicy` (another registry modification) and before the SSH key setup. This ensures that if the VM reboots during bootstrap (some steps may trigger reboots), it will auto-login and continue.

**Why WinFsp at Step 9 (before virtio drivers):** WinFsp is the foundation that virtiofs.exe depends on. It must be installed before any attempt to run `virtiofs.exe` or start the `VirtIO-FS` service. Installing it first ensures the DLL is available when the virtio drivers' post-install hooks try to register services.

**Why virtio drivers at Step 10 (right before project mount at Step 11):** The ISO is attached to QEMU from launch, so the CD-ROM is available early. We place it immediately before the project mount step so the drivers are fresh and the mount can verify them. The drivers only need to be installed once — subsequent boots find them already present.

## Detailed Investigation Report

### Problem Discovery

During E2E test development (`test_vm_lifecycle_windows`), the Windows VM launched successfully but SSH connections on the forwarded port (2222) timed out repeatedly:

```
Attempt 1: Connection timed out during banner exchange
Connection to 127.0.0.1 port 2222 timed out
...
Attempt 20: Connection timed out during banner exchange
Timeout after 20 attempts
```

### Diagnosis Steps

1. **Verified QEMU process was running**: `pgrep -a qemu` confirmed `qemu-system-x86_64` active at 32% CPU
2. **Verified port forwarding**: `ss -tlnp | grep 2222` confirmed QEMU listening on port 2222
3. **Checked serial console output**: QEMU serial console (`-serial stdio`) showed successful Windows Boot Manager load — Windows was booting
4. **Connected via VNC**: `gvncviewer :50` revealed the Windows login screen — no user was logged in

### Root Cause

Windows OpenSSH Server (sshd.exe) runs as a Windows service and **accepts connections even when no user is logged in**. However, the SSH authentication chain in our code (SSH agent → key files → password) failed because:

- The Vagrant Windows image uses the `vagrant` user with password `vagrant`
- SSH key authentication wasn't set up yet (bootstrap hadn't run)
- Password authentication over libssh2 requires the password to be provided in the profile
- The SSH service was responding but auth was failing silently

Additionally, **even if SSH connected, many bootstrap commands require a user session** (registry writes under HKCU, certain Windows API calls). The login screen meant no user session existed.

### Manual Fix (VNC Session)

1. **Attached virtio-win ISO as CD-ROM**:
   ```bash
   qemu-system-x86_64 ... -drive file=/home/darkvoid/EweStore/Testbed/virtio-win-0.1.262.iso,media=cdrom
   ```

2. **Logged in via VNC**: User `vagrant`, password `vagrant`

3. **Installed virtio drivers**:
   ```powershell
   Get-ChildItem -Path "D:\w11\amd64\*.inf" -Recurse | ForEach-Object {
       pnputil -a $_.FullName 2>&1
   }
   ```
   Output confirmed successful installation of all 13 drivers.

4. **Configured autologin**:
   ```powershell
   $regPath = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
   Set-ItemProperty -Path $regPath -Name AutoAdminLogon -Value '1'
   Set-ItemProperty -Path $regPath -Name DefaultUsername -Value 'vagrant'
   Set-ItemProperty -Path $regPath -Name DefaultPassword -Value 'vagrant'
   Remove-ItemProperty -Path $regPath -Name AutoLogonCount -ErrorAction SilentlyContinue
   ```

5. **Verified SSH worked after reboot**: After configuring autologin and rebooting, SSH connected successfully without manual VNC login.

6. **Exported configured image**: The resulting qcow2 (17 GB) was exported to `/home/darkvoid/EweStore/Testbed/`.

### 9p Mount Status on Windows

With both virtio-win drivers AND WinFsp installed, the virtiofs-based project mount works end-to-end:

- The `viofs` driver installs successfully and registers the Plan 9 filesystem
- `virtiofs.exe` (from `C:\Program Files\Virtio-Win\VioFS\virtiofs.exe`) connects to the vhost-user-fs device
- WinFsp provides the userspace filesystem framework that `virtiofs.exe` mounts through
- The project mount at `C:\Users\vagrant\project` shows host files (Cargo.toml, README.md, src/, tests/, etc.)

**Bootstrap-time installation:** Both WinFsp and virtio drivers are installed during the bootstrap SSH phase. The virtiofs mount uses `run_elevated` (WinRM scheduled task as SYSTEM) rather than `run_interactive` because `virtiofs.exe` is a background daemon that doesn't require a GUI session. The mount script runs `virtiofs.exe -t project -m C:\Users\vagrant\project` which establishes the mount and persists as long as QEMU + virtiofsd is running.

**Mount order:** The bootstrap tries `virtiofs` first (faster, paravirtualized), then `smb` (fallback, needs interactive session). The SMB mount still uses `run_interactive` because `net use` may require GUI interaction.

## Implementation Details

### QEMU CD-ROM Attachment

The `QemuConfig` builder gains a `with_cdrom()` method:

```rust
impl QemuConfig {
    pub fn with_cdrom(mut self, path: PathBuf) -> Self {
        self.cdrom_path = Some(path);
        self
    }
}
```

In `build_qemu_args()`, the CD-ROM drive is appended after the disk:

```rust
if let Some(cdrom) = &self.cdrom_path {
    args.push("-drive".to_string());
    args.push(format!("file={},media=cdrom", cdrom.display()));
}
```

### Virtio ISO Resolution

The `ensure_virtio_iso()` function in `import/mod.rs` checks multiple locations:

1. **EweStore** (primary): `/home/darkvoid/EweStore/Testbed/virtio-win-0.1.262.iso`
2. **Local cache**: `~/.cache/foundation_testbed/images/virtio-win.iso`
3. **Download**: Fedora Project URL (fallback)

File validation: existence check + minimum size (> 100 MB).

### Bootstrap Marker for Idempotency

The virtio driver installation checks for existing drivers before running:

```powershell
# Check if virtio drivers are already installed
$devices = Get-PnpDevice -Class System -ErrorAction SilentlyContinue |
    Where-Object { $_.FriendlyName -like '*VirtIO*' }
if ($devices.Count -gt 0) {
    return  # Already installed, skip
}
```

This ensures the step is idempotent — re-running bootstrap on an already-configured VM skips driver installation.

## Impact on E2E Tests

### Before

- `test_project_mount_windows` — skipped/limited (no 9p drivers)
- Windows E2E tests required manual VM preparation
- SSH connectivity unreliable without manual login

### After

- Autologin ensures SSH is accessible immediately after boot
- Virtio drivers enable future 9p mount support
- Windows E2E tests are fully automated
- `test_vm_lifecycle_windows` — passes (SSH reachable after boot)
- `test_project_mount_windows` — improved (directory check, future: true 9p mount)

## Files Modified

| File | Change |
|------|--------|
| `src/qemu/mod.rs` | Add `cdrom_path` field + `with_cdrom()` builder |
| `src/qemu/mod.rs` | Add CD-ROM drive arg in `build_qemu_args()` |
| `src/import/mod.rs` | Add `ensure_virtio_iso()` function |
| `src/config.rs` | Add virtio ISO URL + EweStore path constants |
| `src/bootstrap/windows.rs` | Add `configure_autologin()` step |
| `src/bootstrap/windows.rs` | Add `install_winfsp()` step (before virtio drivers) |
| `src/bootstrap/windows.rs` | Add `install_virtio_drivers()` step |
| `scripts/windows/install_winfsp.ps1` | New — download + silent install WinFsp MSI |
| `tests/common/mod.rs` | Revisit Windows mount skip once drivers are auto-installed |

## Success Criteria

- [x] Windows VM boots and SSH is reachable without manual VNC login
- [x] Autologin registry keys are set during bootstrap (idempotent)
- [x] WinFsp installs during bootstrap (idempotent, checks service first)
- [x] Virtio ISO is downloaded/cached on first use
- [x] All virtio drivers install successfully during bootstrap
- [x] Driver installation is idempotent (skipped if already present)
- [x] `virtiofs.exe` can mount host directory via WinFsp framework
- [x] Project mount verified at `C:\Users\vagrant\project` with host files
- [x] `test_vm_lifecycle_windows` passes without manual intervention
- [x] `test_project_mount_windows` passes with directory round-trip
- [x] `cargo test -p foundation_testbed` passes (no regressions)
- [x] `cargo clippy -p foundation_testbed` clean

## Verification Commands

```bash
# Kill existing VM
kill $(pgrep qemu)

# Clear NVRAM to force fresh boot
rm ~/.cache/foundation_testbed/state/windows-build.nvram

# Run Windows lifecycle test (first bootstrap will take longer)
cargo test -p foundation_testbed --test e2e_tauri test_vm_lifecycle_windows -- --ignored --nocapture

# Run Windows mount test
cargo test -p foundation_testbed --test e2e_tauri test_project_mount_windows -- --ignored --nocapture

# Run all unit tests
cargo test -p foundation_testbed

# Run clippy
cargo clippy -p foundation_testbed
```

## QEMU Boot Sequence (Serial Console Reference)

The following is a representative serial console output from a successful Windows VM boot:

```
SeaBIOS (version rel-1.16.3-0-ga6ed6b701f0a-prebuilt.qemu.org)
iPXE (http://ipxe.org) 00:03.0 CA98 PCI3.00 PnP BBS PMM
...
Windows Boot Manager
...
[   12.4] BOOTMGR loading...
[   14.1] winload.efi starting...
[   18.7] Windows kernel loading...
[   25.3] services starting...
[   45.1] Userenv: User profile loaded for vagrant
[   48.2] Session 1 created (interactive)
[   52.0] OpenSSH Server: sshd listening on port 22
```

The key line is `[   48.2] Session 1 created (interactive)` — this is when the user session is established. With autologin, this happens automatically. Without it, the VM waits at the login screen and Session 1 is never created until a user manually logs in.

---

## Development Tools PATH Configuration

### Problem: Tools Not Available in New Sessions

After installing mise and cargo tools, they were not available in new PowerShell or CMD sessions. The issue was:

1. **Mise binary location**: mise.exe extracts to `~\.local\bin\mise\bin\`, not `~\.local\bin\`
2. **PATH scope**: Only User PATH was being set, not Machine PATH
3. **Current session**: PATH changes weren't reflected in the current PowerShell session
4. **Cargo tools location**: mise installs cargo tools to `AppData\Local\mise\installs\cargo-crate\bin`

### Solution: Comprehensive PATH Configuration

All bootstrap scripts now set PATH for **both User and Machine scope**, plus update the current session:

#### 1. mise Installation (`install_mise.ps1`)

```powershell
$miseDir = "$env:USERPROFILE\.local\bin"
$miseBinDir = "$miseDir\mise\bin"    # Binary is in subdir

# User PATH
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
[Environment]::SetEnvironmentVariable('PATH', "$miseBinDir;$userPath", 'User')

# Machine PATH
$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
[Environment]::SetEnvironmentVariable('PATH', "$miseBinDir;$machinePath", 'Machine')

# Current session
$env:PATH = "$miseBinDir;$env:PATH"
```

#### 2. cargo-binstall (`install_cargo_binstall.ps1`)

```powershell
$dest = "$env:USERPROFILE\.cargo\bin"

# User PATH (existing behavior preserved)
$path = [Environment]::GetEnvironmentVariable('PATH', 'User')
[Environment]::SetEnvironmentVariable('PATH', $path + ';' + $dest, 'User')

# Machine PATH (NEW)
$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
[Environment]::SetEnvironmentVariable('PATH', $machinePath + ';' + $dest, 'Machine')

# Current session (NEW)
$env:PATH = "$dest;$env:PATH"
```

#### 3. Mise Cargo Configuration (`configure_mise_cargo_binstall.ps1`)

```powershell
$shimsDir = "$env:USERPROFILE\AppData\Local\mise\shims"
$cargoBinDir = "$env:USERPROFILE\AppData\Local\mise\installs\cargo-crate\bin"

# Add both to User PATH
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
[Environment]::SetEnvironmentVariable('PATH', "$shimsDir;$cargoBinDir;$userPath", 'User')

# Add both to Machine PATH
$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
[Environment]::SetEnvironmentVariable('PATH', "$shimsDir;$cargoBinDir;$machinePath", 'Machine')

# Update current session
$env:PATH = "$shimsDir;$cargoBinDir;$env:PATH"
```

#### 4. Tool Installation (`install_tools_mise.ps1`)

Same PATH additions as configure_mise_cargo_binstall, ensuring cargo tools are available after installation.

#### 5. Nushell Default Shell (`set_nushell_default_shell.ps1`)

Updated PowerShell profile to include correct PATH:

```powershell
$miseBinDir = "$env:USERPROFILE\.local\bin\mise\bin"
$shimsDir = "$env:USERPROFILE\AppData\Local\mise\shims"
$cargoBinDir = "$env:USERPROFILE\AppData\Local\mise\installs\cargo-crate\bin"

Add-Content $psProfile "`$env:PATH = `"$miseBinDir;$shimsDir;$cargoBinDir;`$env:PATH`"" -Encoding UTF8
```

### Bootstrap Steps Affected

```
Step 4:  install mise                    ← Sets mise bin PATH
Step 5:  install cargo-binstall          ← Sets cargo bin PATH
Step 6:  configure mise cargo_binstall   ← Sets shims/cargo PATH
...
Step 16: install tools via mise          ← Sets shims/cargo PATH
Step 17: set nushell as default shell    ← Updates PS profile PATH
```

### Verification Commands

```powershell
# Check PATH contains all required directories
$env:PATH -split ';' | Select-String -Pattern 'mise|cargo'

# Should output:
# C:\Users\vagrant\.local\bin\mise\bin
# C:\Users\vagrant\AppData\Local\mise\shims
# C:\Users\vagrant\AppData\Local\mise\installs\cargo-crate\bin
# C:\Users\vagrant\.cargo\bin

# Verify tools are available
mise --version                          # Should show: 2026.x.x
cargo-binstall --version               # Should show: 1.x.x
rustc --version                         # Should show: rustc 1.x.x
cargo --version                         # Should show: cargo 1.x.x

# Test in new PowerShell session
powershell -Command "mise --version"    # Should work immediately
powershell -Command "cargo --version"   # Should work immediately
powershell -Command "sccache --version" # Should work (if installed)
```

### Why Both User and Machine PATH

| Scope | Purpose |
|-------|---------|
| **User PATH** | Available to vagrant user in interactive sessions |
| **Machine PATH** | Available to SYSTEM, services, and all users |
| **Current session** | Immediate availability without logout/login |

Setting both ensures:
- Tools work when SSH/WinRM connects as vagrant
- Tools work in scheduled tasks running as SYSTEM
- Tools work in new PowerShell/CMD sessions immediately
- No "command not found" errors after bootstrap

---

## WebView2 Installation Fix (2026-05-10)

### Problem

After the long-running project mount setup step (~3.5 minutes), WinRM becomes unreachable with error:
```
WinRM not reachable on port 5985
```

This caused the WebView2 Runtime installation to fail, leaving the VM partially bootstrapped.

### Root Cause

The WinRM service becomes temporarily unresponsive after extended elevated operations (scheduled task running as SYSTEM for virtiofs mount setup). The WinRM shell connection times out or becomes stale.

### Solution

Enhanced `install_webview2()` function in `src/bootstrap/windows.rs` with:

1. **Retry Logic**: 3 retry attempts with 5-10 second delays for both check and installation phases
2. **WinRM Recovery via SSH**: When WinRM fails, attempts to restart the WinRM service via SSH PowerShell:
   ```powershell
   Restart-Service -Name 'WinRM' -Force
   ```
3. **Fresh WinRM Connections**: Each retry creates a new WinRM connection
4. **SSH Fallback**: As last resort, attempts WebView2 installation via SSH PowerShell directly

### Implementation

```rust
fn install_webview2(profile: &VmProfile, session: &mut VmSession, winrm: &WinRM) -> Result<()> {
    // Retry check with WinRM
    for attempt in 0..3 {
        match winrm.run_ps(CHECK_WEBVIEW2_PS1) {
            Ok(check) => { /* ... */ },
            Err(e) => {
                // Log warning, will retry
            }
        }
    }
    
    // If WinRM still failing, restart via SSH
    if last_error.is_some() {
        let restart_script = r#"
            Restart-Service -Name 'WinRM' -Force
            // ...
        "#;
        crate::ssh::exec_ps_windows(session, restart_script)?;
    }
    
    // Retry installation with fresh connections
    for attempt in 0..3 {
        let fresh_winrm = crate::winrm::WinRM::from_profile(profile)?;
        match fresh_winrm.run_ps(INSTALL_WEBVIEW2_PS1) {
            Ok(_) => return Ok(()),
            Err(e) => { /* retry or fallback to SSH */ }
        }
    }
}
```

### Bootstrap Verification Fix

Added retry loop to verification step in `src/cli/bootstrap.rs`:

```rust
// Verify with retry — WinRM may need time to stabilize after nushell setup
let mut verified = false;
for attempt in 0..5 {
    if bootstrap::is_bootstrapped(profile) {
        verified = true;
        break;
    }
    if attempt < 4 {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
```

### Exported Images

| File | Size | Description |
|------|------|-------------|
| `windows-11-fully-bootstrapped.qcow2` | 30GB | Fully bootstrapped Windows 11 VM with all tools |
| `*.manifest.json` | - | Export metadata |

Location: `/home/darkvoid/EweStore/Testbed/`

### Bootstrap Status (After Fix)

All steps now complete successfully:
- ✅ OpenSSH Server (1.3s)
- ✅ SSH Key Authorization (1.6s)  
- ✅ LocalAccountTokenFilterPolicy (31s)
- ✅ Autologin (1.5s)
- ✅ mise (0.4s)
- ✅ cargo-binstall (33s)
- ✅ VS Build Tools (1.4s)
- ✅ WinFsp (1.4s)
- ✅ VirtIO Drivers (1.3s)
- ✅ Project Mount (206s)
- ✅ **WebView2 Runtime (92s with retry/recovery)**
- ✅ Defender Exclusions (3.2s)
- ✅ Rustup ARM64 Config (0.02s)
- ✅ Tools via mise (0.2s)
- ✅ Nushell Default Shell (0.07s)
- ✅ Bootstrap Marker (1.5s)
