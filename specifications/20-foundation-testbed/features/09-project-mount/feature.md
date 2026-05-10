---
feature: "Project Mount & Artifact Layer"
description: "Host directory mounting into VMs via 9p/virtiofs (QEMU) or shared directories (UTM), artifact mirroring, testbed.toml project config, and sync-free build output access"
status: "implemented"
priority: "high"
depends_on: ["08-provider-architecture"]
estimated_effort: "medium"
created: 2026-05-03
last_updated: 2026-05-04
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Project Mount & Artifact Layer

## Overview

Instead of syncing code into the VM and pulling artifacts back out via `scp`,
this feature **mounts the host project directory directly into the VM**. Builds
run inside the VM write their output to the mounted directory, which appears
immediately on the host filesystem with correct ownership.

This eliminates the build → scp → extract dance and means build artifacts are
accessible even after the VM has been shut down.

## Why This Matters

| Without mount | With mount |
|--------------|------------|
| `testbed build` copies code into VM via `tar + scp` | Host directory is mounted at `/mnt/project` |
| Build outputs live inside VM disk image | Build outputs appear in `$PWD/.testbed/artifacts/` |
| Pulling artifacts requires `scp` roundtrip | No pull needed — files are already on host |
| Artifacts lost if VM is deleted | Artifacts persist (they're host files) |
| File ownership is VM user (UID mismatch) | File ownership is host user |
| Every build copies code (~100MB for medium projects) | Code accessed directly via mount |

## Architecture

### Directory Layout

```
$PWD/.testbed/
├── testbed.toml            # Project-level VM configuration
├── state/                  # VM state JSON files
│   └── vm-{name}.json
├── logs/                   # Full build logs per VM
│   ├── {profile}-build.log
│   └── {profile}-run.log
├── artifacts/              # Mirror of VM build output directory
│   └── (symlink or actual files from mount)
└── mounts/
    └── (project root)      # Host directory to mount into VM

$HOME/.testbed/
├── images/                 # Base images (shared across projects)
│   ├── windows-11-x86_64.qcow2
│   ├── ubuntu-24.04-x86_64.qcow2
│   └── ...
├── snapshots/              # Provider snapshots
└── cache/                  # Temporary downloads
```

### testbed.toml

The project-level configuration file, created by `testbed init`:

```toml
# testbed.toml

# Global settings
[defaults]
bootstrap = true                    # Auto-bootstrap new VMs
display = "headless"                # headless | headful

# VM definitions — one table per VM
[[vms]]
name = "linux-build"                # Local name for this VM
profile = "linux-build"             # Which VmProfile to use
arch = "x86_64"                     # x86_64 | aarch64
memory_mib = 8192                   # Override profile default
cpu_cores = 4

[[vms]]
name = "windows-build"
profile = "windows-build"
arch = "x86_64"
memory_mib = 12288
cpu_cores = 4

[[vms]]
name = "macos-cross"
profile = "macos-build"
arch = "x86_64"
memory_mib = 8192
cpu_cores = 4

# Image stores — ordered list, first match wins
[[image_stores]]
name = "my_r2"
type = "r2"
bucket = "my-images"

[[image_stores]]
name = "vagrant_cloud"
type = "vagrant"
registry = "libvirt"

# Mount configuration
[mounts]
project = "."                       # Host directory to mount (relative to .testbed/)
guest_path = "/mnt/project"         # Where to mount inside all VMs
readonly = false

# Artifact mirroring
[artifacts]
sync = true                         # Mirror build outputs to .testbed/artifacts/
watch_dirs = ["target", "dist"]     # Which guest dirs to mirror (default: mounted root)
```

**Key design points:**

- **`[[vms]]` is an array of tables** — each entry defines a VM in this project. Users can have any number of VMs (1 to N).
- **`name` is the user-facing identifier** — used in `testbed start <name>`, `testbed stop <name>`, etc.
- **`profile` references a built-in VmProfile** — the profile provides defaults for ports, credentials, bootstrap mode, etc.
- **Each VM can have different architectures** — one project could run a `x86_64` Linux VM alongside an `aarch64` Linux VM for cross-arch testing.
- **Mounts are shared** — all VMs in a project mount the same host directory. This means code synced once is available to all VMs.

### Custom Startup / Shutdown Scripts

Users can define per-VM scripts that run at VM start and stop time. Scripts are stored in `$PWD/.testbed/scripts/` and are created automatically by `testbed init`.

**Directory structure:**
```
$PWD/.testbed/scripts/
├── <vm-name>/
│   ├── startup/
│   │   ├── 00-init.sh          # runs first
│   │   ├── 01-network.sh
│   │   └── 99-done.sh          # runs last
│   └── shutdown/
│       ├── 00-cleanup.sh
│       └── 99-final.sh
├── windows-build/
│   ├── startup/
│   │   ├── 00-defender.ps1     # Windows Defender exclusions
│   │   └── 01-services.ps1
│   └── shutdown/
│       └── 00-logs.ps1
└── linux-build/
    ├── startup/
    │   └── 00-mount.sh
    └── shutdown/
        └── 00-sync.sh
```

**Execution rules:**
- Scripts are sorted by filename (natural sort) and executed in order
- Startup scripts run **after** the VM has booted and SSH/WinRM is reachable
- Shutdown scripts run **before** the VM is powered off
- Scripts are executed via SSH (Linux: `bash`, Windows: `powershell -NoProfile -ExecutionPolicy Bypass`)
- If a script fails (non-zero exit), execution stops and the error is surfaced to the user
- Scripts can be shell scripts (`.sh`), nushell (`.nu`), or PowerShell (`.ps1`)
  - `.sh` and `.nu` → executed via SSH on the guest
  - `.ps1` → executed via SSH on Windows guests (PowerShell is available)
- Empty directories are silently skipped

**Built-in scripts (shipped with the binary, dumped by `testbed init`):**

When `testbed init` runs, it creates the scripts directory with built-in defaults:

```
$PWD/.testbed/scripts/windows-build/startup/00-defender.ps1
$PWD/.testbed/scripts/linux-build/startup/00-mount.sh
```

These contain sensible defaults (Defender exclusions for Windows, mount verification for Linux) that users can edit or delete.

**00-defender.ps1 (Windows — Defender exclusions):**
```powershell
# Windows Defender exclusions for development directories
# Edit or delete if you don't need these
Add-MpPreference -ExclusionPath "C:\Users\vagrant\project" -ErrorAction SilentlyContinue
Add-MpPreference -ExclusionPath "C:\Users\vagrant\.cargo" -ErrorAction SilentlyContinue
Add-MpPreference -ExclusionPath "C:\Users\vagrant\.rustup" -ErrorAction SilentlyContinue
```

**00-mount.sh (Linux — mount verification):**
```bash
# Verify project mount is accessible
if mount | grep -q "/mnt/project"; then
  echo "✓ /mnt/project mounted"
else
  echo "⚠ /mnt/project not mounted, attempting mount..."
  sudo mount -t 9p -o trans=virtio,version=9p2000.L project /mnt/project 2>/dev/null || true
fi
```

Users can add, rename, or delete scripts freely. The numbering scheme (`00-`, `01-`, `99-`) gives them explicit control over execution order.

### Minimal testbed.toml (auto-generated by `testbed init`)

```toml
[[vms]]
name = "linux-build"
profile = "linux-build"
```

That's the default — one VM, standard profile. Users edit to add more or customize.

### Auto-generated `.gitignore`

`testbed init` creates `$PWD/.testbed/.gitignore` alongside `testbed.toml`:

```gitignore
# $PWD/.testbed/.gitignore — auto-generated by testbed init
# Commit testbed.toml and scripts/; ignore ephemeral data.

# VM state — host-specific PIDs, ports, disk paths
/state/

# Build/run logs — can be large, regenerated on next build
/logs/

# Build artifacts — live in host's target/ directory, symlinked here
/artifacts/

# Mount points — symlink to project root
/mounts/

# Keep: testbed.toml (project config) ✓
# Keep: scripts/ (user custom startup/shutdown scripts) ✓
```

This means `$PWD/.testbed/testbed.toml` and `$PWD/.testbed/scripts/**` can be committed to the project repo without dragging in large or host-specific files. Users who want to commit `testbed.toml` at the project root level can symlink or copy it — the source of truth lives in `$PWD/.testbed/`.

### Provider-Specific Mount Implementation

#### QEMU: virtio-9p

QEMU uses the Plan 9 filesystem protocol over virtio:

```rust
// src/providers/qemu/mount.rs
pub fn mount_args(host_path: &Path, guest_tag: &str) -> Vec<String> {
    vec![
        "-virtfs".to_string(),
        format!(
            "local,path={},mount_tag={},security_model=mapped,id=fs0",
            host_path.display(), guest_tag
        ),
    ]
}
```

Inside the VM, the mount is established via bootstrap script:
```bash
mkdir -p /mnt/project
mount -t 9p -o trans=virtio,version=9p2000.L project /mnt/project
# Add to fstab for persistence across VM reboots:
echo "project /mnt/project 9p trans=virtio,version=9p2000.L 0 0" >> /etc/fstab
```

**Windows guests:** Windows does not include virtio drivers by default. The `virtio-win` ISO (Fedora Project) must be attached as a CD-ROM and drivers installed via `pnputil`. The critical driver is `viofs.inf` (VirtIO Filesystem). See [Feature 11: Windows VM Setup](../11-windows-vm-setup/feature.md) for the automated installation process.

**Windows mount methods:** Windows VMs support two mount mechanisms:

1. **virtiofs (Primary)** - Uses `virtiofsd` daemon on host + WinFsp on Windows guest
   - Host: `virtiofsd --socket-path /path/to.sock --shared-dir /project`
   - Guest: `virtiofs.exe -t project -m C:\Users\vagrant\project`
   - QEMU args: `-chardev socket,... -device vhost-user-fs-pci,chardev=...`
   - Requires shared memory: `-object memory-backend-memfd,id=mem,size=XM,share=on -machine memory-backend=mem`

2. **SMB (Fallback)** - Uses QEMU's built-in SMB server
   - QEMU arg: `-netdev user,id=net,smb=/project`
   - Guest: `net use Z: \\10.0.2.4\qemu`
   - Requires host setup: `sudo mise run setup-smb`
   - May need wrapper fix on modern Samba: `sudo bash scripts/linux/install-smbd-wrapper.sh`

**Windows mount bootstrap:** The Windows bootstrap automatically:
1. Installs virtio-win drivers (if missing)
2. Installs WinFsp (required by virtiofs.exe)
3. Registers a scheduled task to auto-mount virtiofs on boot
4. Falls back to SMB if virtiofs fails

**Windows mount verification:** The bootstrap checks mount by attempting to read `C:\Users\vagrant\project\Cargo.toml` to verify the mount is working, not just present.

**Performance note:** 9p has overhead for many small files (e.g., `target/` with
thousands of `.d` files). For heavy builds, evaluate `virtio-fs` (requires
`virtiofsd` daemon on host). Start with 9p — it works, and we can measure.

#### UTM: Shared Directories

UTM supports directory sharing through its shared directories feature:

```xml
<!-- In .utm bundle's config.plist -->
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

Configuration via AppleScript:
```applescript
tell application "UTM"
    set vm to virtual machine id "{uuid}"
    set cfg to configuration of vm
    -- Add shared directory to config
    -- (exact AppleScript syntax depends on UTM version)
    update configuration of vm with cfg
end tell
```

Inside the VM, the shared directory appears at the configured `GuestPath`.
UTM uses `virtiofs` under the hood — better performance than 9p.

### Build Pipeline Integration

The build pipeline (feature 03) changes to use the mount point:

**Before (no mount):**
```
1. tar host project → scp to VM → extract in VM
2. cargo build --target x86_64-pc-windows-msvc
3. scp artifacts back to host
4. clean up VM temp files
```

**After (with mount):**
```
1. cargo build --target x86_64-pc-windows-msvc
   (runs inside VM, writes to /mnt/project/target/)
2. Artifacts already on host at $PWD/.testbed/artifacts/
```

The `build()` function:

```rust
pub fn build(profile: &VmProfile, project_dir: &Path) -> Result<Vec<PathBuf>> {
    // Ensure mount is active
    ensure_mounted(profile, project_dir)?;

    // Run build command via SSH (mount point is /mnt/project inside VM)
    let build_cmd = format!(
        "cd /mnt/project && cargo build --target {}",
        profile.build_target()
    );
    ssh_exec(profile, &build_cmd)?;

    // Scan for artifacts on host side (no scp needed)
    let artifacts_dir = project_dir.join("target").join(profile.build_target()).join("release");
    let artifacts = scan_artifacts(&artifacts_dir, profile)?;

    // Mirror to .testbed/artifacts/ if configured
    if config.artifacts_sync {
        mirror_artifacts(&artifacts, &config.artifacts_dir)?;
    }

    Ok(artifacts)
}
```

### Artifact Mirroring

The `artifacts/` directory under `$PWD/.testbed/` mirrors the build outputs:

```
$PWD/.testbed/artifacts/
├── windows/
│   ├── my-app.exe
│   └── my-app.msi
├── linux/
│   └── my-app
└── darwin/
    └── my-app
```

This is either:
1. **A symlink** to the actual build output directory (instant, no copy), or
2. **A copy** (if the mount uses a protocol that doesn't support symlinks well)

Option 1 is preferred — the `artifacts/` directory is a direct symlink to
`$PWD/target/<target-triple>/release/` or the equivalent build output path.

### SMB Setup for Windows Fallback

When virtiofs fails or is unavailable, Windows guests can fall back to SMB mounting via QEMU's built-in SMB server.

**Prerequisites:**
1. Samba installed on host (provides `smbd` binary)
2. `CAP_NET_BIND_SERVICE` capability on smbd (for ports 139/445)
3. `/etc/samba/smb.conf` exists (can be minimal)

**One-time setup:**
```bash
# Using mise (recommended)
cd backends/foundation_testbed
mise run setup-smb

# Or manually
sudo bash scripts/linux/smb-setup.sh
```

**The setup script does:**
- Creates `/etc/samba/smb.conf` with QEMU-compatible settings
- Creates required directories (`/var/log/samba`, `/var/lib/samba`, `/run/samba`)
- Sets ownership for current user
- Adds `CAP_NET_BIND_SERVICE` capability to smbd (required for privileged ports)

**SMB Wrapper (for modern Samba 4.x):**

Modern Samba requires an `ncalrpc` subdirectory that QEMU doesn't create. Install the wrapper to fix this:

```bash
# Install wrapper
sudo bash scripts/linux/install-smbd-wrapper.sh

# To uninstall (restore original)
sudo bash scripts/linux/install-smbd-wrapper.sh --uninstall
```

The wrapper:
- Intercepts smbd calls from QEMU
- Creates `ncalrpc` and `cores` directories before exec'ing real smbd
- Backs up original to `/usr/bin/smbd.bin`

**Why this is needed:** QEMU spawns smbd when a Windows guest connects to the share, but modern Samba (4.x) exits immediately if the `ncalrpc` directory is missing. The wrapper ensures this directory exists before smbd starts.

### Build Logs

Build logs are written to the mounted directory so they're immediately accessible
on the host, not trapped inside the VM disk image:

**Inside VM path**: `/mnt/project/.testbed/logs/<profile>-build.log`  
**Host path**: `$PWD/.testbed/logs/<profile>-build.log`

The build command redirects output:
```bash
# Inside VM (via SSH)
cd /mnt/project && cargo tauri build --target x86_64-pc-windows-msvc 2>&1 | tee .testbed/logs/windows-build.log
```

**Why the full log, not just error extraction:**
- Users can `tail`, `less`, `head`, `grep`, or pipe the full log however they want
- The log persists after VM shutdown (it's a host file)
- No need to SSH back in to pull error snippets — it's already on the host
- CI systems can capture it as an artifact directly

The `logs` CLI command (`ewe_platform testbed logs <profile>`) reads from the
host-side file:

| Option | Behavior |
|--------|----------|
| (no flags) | Print last 50 lines |
| `--follow` | `tail -f` equivalent — stream new lines as they appear |
| `--errors` | Grep for error patterns (same patterns as current `build/logs.rs`) |
| `--tail N` | Print last N lines |
| `--kind build` | Read from `*-build.log` (default) |
| `--kind run` | Read from `*-run.log` (launched binary output) |

When a build fails, the CLI prints the error-extracted output inline **and** the
full log is already available at `$PWD/.testbed/logs/<profile>-build.log`:

```
Error: Build failed for target 'x86_64-pc-windows-msvc' (exit 101)

  error[E0308]: mismatched types
     --> src/main.rs:10:5
      | expected `String`, found integer

  Full log: .testbed/logs/windows-build.log
  Errors only: ewe_platform testbed logs windows --errors
```

### Mount Lifecycle

```
testbed start
  → launch VM with mount args
  → wait_for_boot
  → ensure mount is active inside VM (run `mount | grep project`)
  → if not mounted, run mount command via SSH
  → verify: run `ls /mnt/project` via SSH
  → run startup scripts in $PWD/.testbed/scripts/<vm>/startup/ (numbered order)
  → report any script failures to user

testbed stop
  → run shutdown scripts in $PWD/.testbed/scripts/<vm>/shutdown/ (numbered order)
  → report any script failures to user
  → graceful VM shutdown
  → mount is automatically unmounted (VM process ends)

testbed build
  → verify mount is active
  → run build command inside VM (working dir: /mnt/project)
  → scan host-side build output directory for artifacts
```

### Fallback: No Mount Available

If the mount mechanism fails (9p not supported in guest, UTM shared dirs
broken), fall back to the sync-based approach:

```rust
pub fn build_with_fallback(profile: &VmProfile, project_dir: &Path) -> Result<Vec<PathBuf>> {
    match build(profile, project_dir) {
        Ok(artifacts) => Ok(artifacts),
        Err(MountError::NotAvailable) => {
            eprintln!("  Mount not available, falling back to sync-based build...");
            build_with_sync(profile, project_dir)
        }
        Err(e) => Err(e),
    }
}
```

The sync-based build (`build_with_sync`) is the existing implementation from
feature 03 — it copies code in and pulls artifacts out. It's slower but works
as a safety net.

## Network Mount Testing Command

The `testbed network` command provides a dedicated way to test different mount configurations without affecting the main VM lifecycle:

```bash
# Test virtiofs mount (daemonize mode - auto stops after test)
cargo run -p ewe_platform -- testbed network windows-build --type virtiofs --host-dir . --daemonize

# Test SMB mount (daemonize mode)
cargo run -p ewe_platform -- testbed network windows-build --type smb --host-dir . --daemonize

# Test with display for interactive debugging
cargo run -p ewe_platform -- testbed network windows-build --type virtiofs --headful

# Keep VM running for manual testing (no daemonize)
cargo run -p ewe_platform -- testbed network windows-build --type virtiofs --host-dir .
```

**Command Options:**
| Option | Description |
|--------|-------------|
| `--type` | Mount type: `virtiofs`, `smb`, `9p`, `none` |
| `--host-dir` | Host directory to share (default: current directory) |
| `--guest-dir` | Override guest mount point |
| `--headful` | Run with graphical display (auto-launches VNC) |
| `--daemonize` | Run diagnosis in background thread, stop VM when done |

**How it works:**
1. Starts VM with specified mount type configured in QEMU
2. Spawns background thread to wait for SSH readiness (with 120s timeout)
3. Main thread receives SSH ready signal via MPSC channel
4. Runs mount verification with type-specific checks:
   - **virtiofs**: Checks virtiofs.exe exists, WinFsp service running, mount point accessible
   - **SMB**: Shows manual mount instructions (`net use Z: \\10.0.2.4\qemu`)
   - **9p**: Verifies Linux mount at guest path
5. If `--daemonize`: stops VM and prints diagnosis
6. If no `--daemonize`: keeps VM running for interactive debugging

**SMB Notes:**
- SMB requires the smbd wrapper to be installed: `sudo bash scripts/linux/install-smbd-wrapper.sh`
- The wrapper fixes QEMU/Samba compatibility by creating the `ncalrpc` directory that modern Samba requires
- SMB mount is not automatic - user must run `net use` inside the VM

**virtiofs Notes:**
- Requires virtio-win drivers and WinFsp to be installed (run `testbed bootstrap` first)
- The mount should be automatic via scheduled task registered during bootstrap
- If mount fails, check: virtiofs.exe location, WinFsp service status, scheduled task registration

---

## Windows Mount Implementation Fixes & Discoveries

### Summary of Issues Found and Fixed

Through extensive debugging and testing, we discovered and fixed multiple issues with Windows virtiofs mounting. This section documents every issue found, the root cause, and the solution implemented.

### Issue 1: SMB Compatibility with Modern Samba (RESOLVED)

**Problem:** QEMU's built-in SMB server uses `/usr/bin/smbd` but doesn't create the `ncalrpc` subdirectory that modern Samba (4.x+) requires. When QEMU spawns smbd, it fails immediately.

**Error Signature:**
```
QEMU creates: /tmp/qemu-smb.XXXXX/smb.conf
smbd expects: /tmp/qemu-smb.XXXXX/ncalrpc/
Result: smbd exits with error, no SMB server running
```

**Solution:** SMBD Wrapper Script

Created `scripts/linux/install-smbd-wrapper.sh` which:
1. Backs up original `/usr/bin/smbd` to `/usr/bin/smbd.bin`
2. Installs a bash wrapper at `/usr/bin/smbd`
3. The wrapper intercepts smbd calls, creates required directories (`ncalrpc`, `cores`), then execs the real smbd

**Install:**
```bash
sudo bash backends/foundation_testbed/scripts/linux/install-smbd-wrapper.sh
```

**Uninstall:**
```bash
sudo bash backends/foundation_testbed/scripts/linux/install-smbd-wrapper.sh --uninstall
```

**Verification:**
```bash
# Check wrapper is installed
cat /usr/bin/smbd | head -5
# Should show: #!/bin/bash (wrapper script)

# Check SMB is working in VM
net use Z: \\10.0.2.4\qemu
```

### Issue 2: Scheduled Task User Account (CRITICAL FIX)

**Problem:** The scheduled task for virtiofs auto-mount was configured with:
```powershell
/RU vagrant /RP vagrant /IT
```

This caused multiple failures:
1. **ONSTART trigger incompatible with user accounts** - Requires SYSTEM or interactive logon
2. **Password mismatch** - Task scheduler couldn't authenticate vagrant user
3. **Result code 267011** - "The directory name is invalid" (credential/auth failure)
4. **Task never ran** - LastRunTime showed 11/30/1999 (never executed)

**Solution:** Use SYSTEM Account

Changed task creation to:
```powershell
/RU SYSTEM
```

**Why SYSTEM works:**
- No password required
- Runs before user logon (works with ONSTART)
- Has access to all filesystem resources
- virtiofs process runs as service, visible to all users
- Mount point accessible in user sessions

**Code Change:**
```powershell
# BEFORE (broken):
schtasks /Create /TN $taskName /TR ... /SC ONSTART /RU vagrant /RP vagrant /IT

# AFTER (working):
schtasks /Create /TN $taskName /TR ... /SC ONSTART /RU SYSTEM
```

**Verification in VM:**
```powershell
Get-ScheduledTask -TaskName "FoundationTestbed_VirtiofsMount"
# State: Ready
# LastRunTime: <recent timestamp>
# LastTaskResult: 0 (success)

Get-Process -Name "virtiofs"
# Should show running process
```

---

### Issue 2b: Session 0 Mount Visibility (CRITICAL FOLLOW-UP FIX)

**Problem:** After fixing Issue 2 with SYSTEM account, the mount was "invisible" to user sessions:
- virtiofs process: Running (PID=7476)
- Session ID: **0** (service session)
- Mount visible in PowerShell: Yes (as SYSTEM)
- Mount visible in File Explorer: **NO**
- User error: "A device attached to the system is not functioning"

**Root Cause:**
WinFsp creates filesystem mounts in the **session context** of the process:
- **Session 0**: Service session (SYSTEM, services)
- **Session 1+**: User interactive sessions

When virtiofs runs as SYSTEM in Session 0, the mount is only visible to:
- SYSTEM processes
- Services
- Not visible to user File Explorer or applications

**Error Signature:**
```powershell
Get-ChildItem "C:\Users\vagrant\project"
# Error: "A device attached to the system is not functioning"

# Process details:
Get-Process virtiofs | Select SessionId  # Shows 0
```

**Solution:** Use ONLOGON with User Account

Changed task to run when user logs in:
```powershell
# BEFORE (broken - Session 0):
schtasks /Create /TN $taskName /TR ... /SC ONSTART /RU SYSTEM

# AFTER (working - Session 1):
schtasks /Create /TN $taskName /TR ... /SC ONLOGON /RU vagrant
```

**Why ONLOGON works:**
- Task triggers when user logs in (not at boot)
- Process runs in user's Session (1, 2, etc.)
- WinFsp mount visible to that user
- Mount accessible in File Explorer and apps

**Trade-offs:**
- Mount not available before user logon (no issue for interactive use)
- Requires user to log in (automatic in our setup)
- Mount persists across user sessions (until logoff)

**Verification:**
```powershell
# Check process session
Get-Process virtiofs | Select-Object Id, SessionId
# Id  SessionId
# --  ---------
# 2964        1    # Should be 1, not 0

# Check mount access
Get-ChildItem "C:\Users\vagrant\project"
# Should list files (76+ items)

# Check in File Explorer
explorer.exe "C:\Users\vagrant\project"
# Should show files and allow navigation
```

**Final Working Configuration:**
```powershell
$taskName = "FoundationTestbed_VirtiofsMount"
$mountScript = "C:\Users\vagrant\mount_virtiofs.ps1"

# Delete existing
schtasks /Delete /TN $taskName /F 2>&1 | Out-Null

# Create with ONLOGON (runs in user session)
schtasks /Create /TN $taskName `
    /TR "powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$mountScript`"" `
    /SC ONLOGON /RU vagrant /F

# Run immediately
schtasks /Run /TN $taskName
```

---

### Q&A: Why does ONLOGON + vagrant user work when ONSTART + vagrant user failed?

**Question:** I thought running as the vagrant user was the problem. Why does it work with ONLOGON?

**Answer:** The issue wasn't the **user** - it was the **trigger timing**.

| Configuration | Result | Why |
|---------------|--------|-----|
| `ONSTART /RU vagrant /RP vagrant` | **FAIL** | ONSTART runs at boot before user logs in. User session doesn't exist yet, requires password with `/RP`, fails with error 267011. |
| `ONSTART /RU SYSTEM` | **PARTIAL** | Runs at boot in Session 0, but mount invisible to user sessions. User sees "device not functioning". |
| `ONLOGON /RU vagrant` | **SUCCESS** | Runs AFTER user logs in. User session (Session 1) exists, no password needed, mount visible to user. |

**Key Insight:**
- `ONSTART` triggers **before** login - user account not available
- `ONLOGON` triggers **after** login - user account active with Session 1
- `/IT` (interactive) doesn't work with `ONSTART` - no desktop session yet

**Why no `/RP` password needed with ONLOGON?**
Windows Task Scheduler captures the user's credentials at logon time when they type their password. No explicit `/RP` required - it uses the cached logon token.

**Session Context:**
```powershell
# ONSTART + SYSTEM: Session 0 (service) - mount invisible
Get-Process virtiofs | Select SessionId  # Returns 0

# ONLOGON + vagrant: Session 1 (user) - mount visible
Get-Process virtiofs | Select SessionId  # Returns 1
```

---

### Issue 3: virtiofs.exe Discovery (ROBUSTNESS IMPROVEMENT)

**Problem:** Scripts used hardcoded paths to find `virtiofs.exe`. If drivers installed to non-standard location or registry had different InstallLocation, scripts would fail.

**Original Code:**
```powershell
$found = $null
foreach ($p in @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)) {
    if (Test-Path $p) { $found = $p; break }
}
if (-not $found) { throw "not found" }
```

**Solution:** Three-Tier Discovery

Implemented fast-path + fallback pattern across all scripts:

**Tier 1 - Fast Path (Common Locations):**
```powershell
$commonPaths = @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)
foreach ($p in $commonPaths) {
    if (Test-Path $p) { $found = $p; break }
}
```

**Tier 2 - Registry (Virtio-win-installer):**
```powershell
$reg = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Virtio-win-driver-installer'
if ($reg -and $reg.InstallLocation) {
    $regPath = Join-Path $reg.InstallLocation "VioFS\virtiofs.exe"
    if (Test-Path $regPath) { $found = $regPath }
}
```

**Tier 3 - Search (Fallback):**
```powershell
$virtioEntry = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' |
    Where-Object { $_.DisplayName -like '*virtio*' } |
    Select-Object -First 1
if ($virtioEntry -and $virtioEntry.InstallLocation) {
    $testPath = Join-Path $virtioEntry.InstallLocation "VioFS\virtiofs.exe"
    if (Test-Path $testPath) { $found = $testPath }
}
```

**Affected Scripts:**
- `scripts/windows/check_virtio.ps1`
- `scripts/windows/mount_virtiofs.ps1`
- `scripts/windows/register_virtiofs_startup.ps1` (main + embedded)
- `src/cli/qemu.rs` (verify_mount function)

### Issue 4: Mount Process Lifecycle

**Problem Discovered:** When running virtiofs.exe via WinRM/SSH, the process terminates when the remote session ends. This is because:
1. virtiofs.exe runs in the session's process tree
2. Session teardown kills child processes
3. Mount becomes inaccessible

**Evidence:**
```powershell
# During SSH session:
Get-Process virtiofs  # Shows PID=3096
# After SSH disconnect:
Get-Process virtiofs  # No process found
```

**Solution:** SYSTEM Task + Auto-Restart

The scheduled task approach solves this because:
1. SYSTEM task runs outside user session (service context)
2. ONSTART trigger runs at boot, before any user logon
3. Process survives user logon/logoff
4. Mount persists across SSH connections

**Task Configuration (Working):**
```powershell
schtasks /Create /TN FoundationTestbed_VirtiofsMount `
    /TR "powershell -ExecutionPolicy Bypass -File C:\Users\vagrant\mount_virtiofs.ps1" `
    /SC ONSTART /RU SYSTEM
```

### Issue 5: Mount Point State Management

**Problem:** Stale mount directories can prevent new mounts.

**Discovery:**
- If `C:\Users\vagrant\project` exists from previous attempt
- virtiofs.exe may fail with "directory already exists" or "device busy"
- Error 183: "Cannot create a file when that file already exists"

**Solution:** Pre-Cleanup in Mount Script

```powershell
if (Test-Path $mountPoint) {
    Remove-Item -Path $mountPoint -Recurse -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}
```

This ensures clean mount point before starting virtiofs.

### Issue 6: CLI Dispatch Bug (ew_platform vs testbed binary)

**Problem:** `ewe_platform testbed ls` showed no output, but `cargo run -p foundation_testbed --features cli -- ls` worked.

**Root Cause:** Incorrect subcommand matching in `bin/platform/src/testbed/mod.rs`:
```rust
// WRONG - double-nesting:
match args.subcommand() {
    Some(("testbed", sub)) => foundation_testbed::cli::run(sub)?,  // Already stripped
    _ => {}
}
```

**Fix:** Pass args directly:
```rust
// CORRECT:
foundation_testbed::cli::run(args)?;
```

**Impact:** All `ewe_platform testbed` commands were silently failing.

### Verification Commands

**Verify Mount in VM:**
```powershell
# Check virtiofs running
Get-Process -Name "virtiofs"

# Check mount point
Get-ChildItem "C:\Users\vagrant\project"

# Check scheduled task
Get-ScheduledTask -TaskName "FoundationTestbed_VirtiofsMount" | 
    Select-Object TaskName, State, @{N="LastRunTime";E={(schtasks /query /tn $_.TaskName /fo csv /v | ConvertFrom-Csv).LastRunTime}}
```

**Verify from Host:**
```bash
# Test mount via CLI
./target/debug/ewe_platform testbed exec windows-build --method winrm "dir C:\Users\vagrant\project"

# Network test command
./target/debug/ewe_platform testbed network windows-build --type virtiofs --host-dir . --daemonize
```

**Expected Results:**
- virtiofs process: Running (PID visible)
- Mount point: 70+ files/directories visible
- Scheduled task: State=Ready, LastRunTime=recent
- File access: Can read/write files through mount

---

## Implementation Phases

### Phase 1: Config Parsing (Tasks 1-2)

1. Create `src/common/config.rs` — `TestbedConfig` struct that parses `testbed.toml`:
   - `[[vms]]` array → `Vec<VmDefinition>` (name, profile, arch, overrides)
   - `[[image_stores]]` array → `Vec<ImageStore>` (ordered image backends)
   - `[mounts]` → `MountConfig` (host path, guest path, readonly)
   - `[artifacts]` → `ArtifactConfig` (sync flag, watch dirs)
   - `VmDefinition::resolve_profile()` → merges profile defaults + overrides
2. Create `src/cli/handlers.rs:cmd_init()` — generates minimal `testbed.toml`, built-in scripts, and `.gitignore`

1. Create `src/providers/qemu/mount.rs` — build 9p mount args, add to QEMU launch
2. Update bootstrap to auto-mount on guest boot (fstab entry, mount verification)
3. Update `build()` to use mount path instead of sync

### Phase 2: UTM Shared Directories (Tasks 4-5)

4. Create `src/providers/utm/shared_dir.rs` — AppleScript config for shared directories
5. Update UTM bootstrap to verify shared directory mount inside guest

### Phase 3: Project Config & Artifacts (Tasks 6-8)

6. Create `testbed.toml` parsing — `VmConfig` struct with mount/artifacts settings
7. Create `src/common/artifacts.rs` — artifact scanning, mirroring, symlink management
8. Update CLI: `testbed mount status`, `testbed mount verify` commands

## Success Criteria

- [ ] `ewe_platform testbed start linux-build` mounts `$PWD` into VM at `/mnt/project`
- [ ] Running `cargo build` inside VM produces artifacts visible on host immediately
- [ ] `$PWD/.testbed/artifacts/` mirrors build outputs
- [ ] Build logs written to `$PWD/.testbed/logs/<profile>-build.log` (host-side, accessible after VM shutdown)
- [ ] `testbed logs windows --errors` prints error patterns from the host-side log file
- [ ] `testbed logs windows --follow` streams new lines as the build runs
- [ ] `testbed mount verify` confirms mount is active and readable
- [ ] Fallback to sync-based build works when mount is unavailable
- [ ] UTM shared directories work for macOS UTM provider
- [ ] Artifacts persist after VM shutdown (they're host files)
- [ ] `testbed init` creates `$PWD/.testbed/scripts/<vm>/startup/` and `shutdown/` with built-in defaults
- [ ] Startup scripts execute in numbered order after VM boot + SSH is reachable
- [ ] Shutdown scripts execute in numbered order before VM powers off
- [ ] Custom user scripts (`.sh`, `.nu`, `.ps1`) run correctly on their respective guest OSes
- [ ] Script failure (non-zero exit) stops execution and surfaces the error to the user
- [ ] `testbed init` creates `$PWD/.testbed/.gitignore` that ignores `state/`, `logs/`, `artifacts/`, `mounts/` but allows committing `testbed.toml` and `scripts/`

## Verification Commands

```bash
# Start VM with mount
ewe_platform testbed start linux-build

# Verify mount inside VM
ewe_platform testbed exec linux-build "ls /mnt/project"

# Build (artifacts appear on host)
ewe_platform testbed build linux-build

# Check artifacts on host
ls .testbed/artifacts/

# Mount status
ewe_platform testbed mount status
```
