---
feature: "Project Mount & Artifact Layer"
document: "Known Issues - Windows Mount Implementation"
status: "active - fixes required"
created: "2026-05-09"
---

# Windows Mount Implementation Issues

## Critical Issues

### Issue 1: QEMU SMB Server Configuration (RESOLVED - Code Fixed, Runtime Limitation)

**Location**: `src/qemu/mod.rs` (network device configuration), `scripts/windows/setup_smb_mount.ps1`

**Status**: Code fixes applied, but QEMU/Samba compatibility issue prevents runtime operation

**Original Problems**: 
1. SMB IP was `10.0.2.3` - should be `10.0.2.4` (QEMU default)
2. Share name was `smb` - should be `qemu` (QEMU default)
3. QEMU netdev was missing `,smb=/path` parameter

**Fixes Applied**:
1. ✅ **QEMU netdev** (lines 682-687 in `src/qemu/mod.rs`):
   ```rust
   if profile.os == GuestOs::Windows && project_mount.is_some() {
       if let Some(host_path) = project_mount {
           netdev.push_str(&format!(",smb={}", host_path.display()));
       }
   }
   ```

2. ✅ **SMB mount script** (`setup_smb_mount.ps1`):
   - Changed `$smbServer = '10.0.2.3'` → `$smbServer = '10.0.2.4'`
   - Changed `$shareName = 'smb'` → `$shareName = 'qemu'`

**Runtime Issue Discovered**:
QEMU's built-in SMB doesn't work with modern Samba (4.x) because QEMU doesn't create the `ncalrpc` subdirectory required by smbd. When QEMU spawns smbd, it fails immediately.

**Impact**: SMB fallback won't work until either:
- QEMU is patched to create the required directory
- Samba is downgraded to a version that doesn't require ncalrpc
- A standalone Samba server is used instead

**Recommendation**: Use virtiofs as primary method (Issue 2 fixes).
        netdev.push_str(&format!(",smb={}", host_path.display()));
    }
}
```

**Fix Required** (in `setup_smb_mount.ps1`):
```powershell
# OLD (WRONG)
$smbServer = '10.0.2.3'
$shareName = 'smb'

# NEW (CORRECT)
$smbServer = '10.0.2.4'
$shareName = 'qemu'  # or 'smbserver' depending on QEMU version
```

---

### Issue 2: mount_virtiofs.ps1 Missing Required `-t` Tag Parameter (RESOLVED)

**Location**: `scripts/windows/mount_virtiofs.ps1` (line 33)

**Status**: ✅ **FIXED**

**Problem**: The virtiofs.exe requires a mount tag (`-t`) to identify which virtio device to connect to. The script only passed `-m` (mount point) but omitted `-t`.

**Fix Applied**:
```powershell
# OLD (line 33)
Start-Process -FilePath $found -ArgumentList "-m $mountPoint" -WindowStyle Hidden

# NEW (line 33)
Start-Process -FilePath $found -ArgumentList "-t project -m $mountPoint" -WindowStyle Hidden
```

**Verification**: Both `mount_virtiofs.ps1` and `register_virtiofs_startup.ps1` now correctly include `-t project`.

---

### Issue 3: No Service Readiness Check After Driver Installation (PENDING TEST)

**Location**: `src/bootstrap/windows.rs` (lines 517-556)

**Status**: ⏳ **PENDING VERIFICATION**

**Problem**: After installing virtio drivers and WinFsp, the bootstrap immediately attempts to mount without waiting for services to initialize.

**Current Flow**:
1. Install virtio-win drivers
2. Install WinFsp
3. Immediately register and run mount task

**Potential Fix**: Add a wait loop checking for service status before attempting mount:
```powershell
# Wait for VirtIO-FS service readiness
$maxWait = 30
for ($i = 0; $i -lt $maxWait; $i++) {
    $svc = Get-Service -Name 'VirtIO-FS' -ErrorAction SilentlyContinue
    if ($svc -and $svc.Status -eq 'Running') { break }
    Start-Sleep -Seconds 1
}
```

**Impact**: May cause race condition where mount script runs before virtiofs driver is fully operational.

**Action**: Test Windows VM boot to verify if this is an actual issue.

---

### Issue 4: Three Inconsistent Mount Scripts Create Confusion (RESOLVED)

**Location**: `scripts/windows/`

**Status**: ✅ **FIXED**

**Problem**: Three separate mount scripts with inconsistent arguments:

| Script | `-t project` Tag | Purpose | Used By |
|--------|------------------|---------|---------|
| `mount_virtiofs.ps1` | **NO** (was missing) | Standalone mount | Scheduled task (via register script content) |
| `register_virtiofs_startup.ps1` | YES | Registers startup task | `bootstrap_windows.rs` line 545 |
| `setup_virtiofs_mount.ps1` | YES | One-time mount setup | **NOT REFERENCED** in code (dead code) |

**Fix Applied**: Updated `mount_virtiofs.ps1` to include `-t project` parameter (line 33).

**Updated Status**:

| Script | `-t project` Tag | Status |
|--------|------------------|--------|
| `mount_virtiofs.ps1` | **YES** ✅ | Fixed |
| `register_virtiofs_startup.ps1` | **YES** ✅ | Already correct |
| `setup_virtiofs_mount.ps1` | **YES** | Dead code - can be removed or used for manual mounting |

**Note**: `setup_virtiofs_mount.ps1` is not referenced in the Rust code but could be useful for manual debugging.

---

### Issue 5: WinFsp Service Check Insufficient

**Location**: `scripts/windows/install_winfsp.ps1` (lines 6-8)

**Problem**: The check only verifies service existence, not that the DLL is loadable:

```powershell
$service = Get-Service -Name 'WinFsp.Launcher' -ErrorAction SilentlyContinue
if ($service) {
    exit 0  # Returns even if service is stopped or DLL corrupted
}
```

**Impact**: Installation may be skipped even when WinFsp is broken/unusable.

**Fix Required**: Check service status is "Running" and verify DLL exists:
```powershell
if ($service -and $service.Status -eq 'Running' -and 
    (Test-Path 'C:\Program Files (x86)\WinFsp\bin\WinFsp.dll')) {
    exit 0
}
```

---

### Issue 6: setup_virtiofs_mount.ps1 Is Dead Code

**Location**: `scripts/windows/setup_virtiofs_mount.ps1`

**Problem**: This script is embedded but never referenced in the Rust source code.

**Verification**: Search of `src/` shows only these scripts are used:
- `REGISTER_VIRTIOFS_STARTUP_PS1` (used)
- `SETUP_SMB_MOUNT_PS1` (used as fallback)
- `CHECK_PROJECT_MOUNT_PS1` (used for verification)

**Impact**: Confusion during debugging - changes to this file have no effect.

**Fix Required**: Either use this script for initial mount (replacing the inline content in register script) or remove it.

---

### Issue 7: QEMU SMB Configuration Required (RESOLVED)

**Status**: Partially Fixed - SMB server works but QEMU integration has compatibility issues

**Problem**: QEMU's built-in SMB server requires:
1. `/etc/samba/smb.conf` to exist (doesn't by default on most systems)
2. Samba directories to exist with proper permissions (`/var/log/samba`, `/var/lib/samba`, `/run/samba`)
3. **smbd binary to have `CAP_NET_BIND_SERVICE` capability** (to bind to privileged ports 139/445)
4. **QEMU compatibility issue**: QEMU doesn't create the `ncalrpc` subdirectory that modern smbd (4.x) requires

**Solution for Host Setup** (steps 1-3): Run the provided setup script:

```bash
# Using mise (recommended)
cd backends/foundation_testbed
mise run setup-smb

# Or manually with sudo
sudo bash scripts/linux/smb-setup.sh
```

**What the script does**:
1. Creates `/etc/samba/smb.conf` with QEMU-compatible settings
2. Creates required directories (`/var/log/samba`, `/var/lib/samba`, etc.)
3. Sets proper permissions
4. **Adds CAP_NET_BIND_SERVICE capability to smbd** (critical for non-root operation)

**QEMU Integration Issue**:
QEMU creates a temporary directory for smbd but doesn't create the `ncalrpc` subdirectory that modern Samba requires. When smbd starts, it fails because this directory is missing.

**Solution: Use smbd wrapper** (recommended):
```bash
# Install the wrapper that fixes the ncalrpc issue
sudo bash scripts/linux/install-smbd-wrapper.sh
```

The wrapper:
1. Replaces `/usr/bin/smbd` with a bash script
2. Backs up the original to `/usr/bin/smbd.bin`
3. Intercepts smbd calls and creates required directories before exec'ing the real smbd

**To uninstall**:
```bash
sudo bash scripts/linux/install-smbd-wrapper.sh --uninstall
```

**Alternative Workarounds**:
1. **Use virtiofs instead** (recommended) - See virtiofs setup below
2. **Manual SMB server** - Start smbd manually with a custom config
3. **Fix QEMU** - Would require QEMU source code modification

**Manual SMB Server (Alternative)**:
```bash
# Create a simple smb.conf
cat > /tmp/smb.conf << 'EOF'
[global]
workgroup = WORKGROUP
security = user
map to guest = Bad User
interfaces = 127.0.0.1
bind interfaces only = yes
[qemu]
path = /path/to/share
read only = no
guest ok = yes
EOF

# Start smbd
sudo smbd -F -s /tmp/smb.conf

# In Windows VM, connect to host's forwarded port (usually 10.0.2.2)
```

**After setup**: QEMU will automatically spawn smbd when using `-netdev user,smb=/path`

**Verification**:
```bash
# Check config is valid
testparm -s /etc/samba/smb.conf

# QEMU should now start with SMB:
qemu-system-x86_64 -netdev user,id=net,smb=/home/user/project ...
```

---

## Diagnostic Commands

To debug mount issues inside the Windows VM:

```powershell
# Check virtio driver installation
Get-PnpDevice -Class System | Where-Object { $_.FriendlyName -like '*VirtIO*' }

# Check VirtIO-FS service status
Get-Service VirtIO-FS -ErrorAction SilentlyContinue

# Check WinFsp service status
Get-Service WinFsp.Launcher -ErrorAction SilentlyContinue

# Try manual mount with debug output (run as admin)
& "C:\Program Files\Virtio-Win\VioFS\virtiofs.exe" -t project -m C:\Users\vagrant\project -d -D 4

# Check Event Log for VirtIO errors
Get-EventLog -LogName System -Source "VirtIO*" -Newest 20 -ErrorAction SilentlyContinue

# Verify mount point type
(Get-Item C:\Users\vagrant\project).PSDrive | Select-Object Name, Provider, Root

# Check if virtiofs process is running
Get-Process virtiofs -ErrorAction SilentlyContinue
```

---

## Recommended Fix Priority

1. **HIGH**: Fix Issue 2 (add `-t project` to mount_virtiofs.ps1) - This is likely the primary blocker
2. **HIGH**: Fix Issue 1 (add SMB to QEMU netdev) - Required for SMB fallback to work
3. **MEDIUM**: Fix Issue 3 (add service readiness check) - Prevents race conditions
4. **MEDIUM**: Fix Issue 4 (consolidate scripts) - Reduces maintenance burden
5. **LOW**: Fix Issue 5 (improve WinFsp check) - Defensive improvement
6. **LOW**: Fix Issue 6 (remove dead code or use it) - Code cleanup

---

## Success Criteria for Resolution

- [ ] Windows VM boots and virtiofs mount is available at `C:\Users\vagrant\project`
- [ ] Host files are visible inside VM through mount (e.g., `Cargo.toml` exists)
- [ ] SMB fallback works when virtiofs fails
- [ ] Mount persists across VM reboots
- [ ] `test_project_mount_windows` E2E test passes

---

## Architectural Decision: Scheduled Task vs Windows Service

### Question: Should virtiofs run as a Windows Service?

**Answer: No - Scheduled Task is the correct approach.**

### Reasoning

**virtiofs.exe does not support Windows service callbacks** (ServiceMain, HandlerEx). It's designed as a simple daemon process, not a Windows service.

### Comparison

| Aspect | Windows Service | Scheduled Task (Current) |
|--------|-----------------|--------------------------|
| **virtiofs support** | Requires wrapper (srvany/nssm) | Native support |
| **Interactive session** | Runs in Session 0 (isolated) | Can run in user session |
| **WinFsp visibility** | May fail (needs user context) | Works (user has WinFsp access) |
| **Restart on failure** | Automatic | Manual retry needed |
| **Complexity** | High (wrapper required) | Low (native) |

### Why `/IT` Flag is Critical

The current implementation uses:
```powershell
schtasks /Create ... /RU vagrant /RP vagrant /IT
```

The `/IT` (Interactive) flag ensures the mount runs in the user's desktop session where WinFsp can create the visible filesystem. Without this, WinFsp cannot access the user's session and the mount fails.

### Recommendation

**Keep the Scheduled Task approach** but add:
1. Retry logic in the mount script
2. Process monitoring to verify virtiofs stays running
3. Automatic remount on failure detection

See Issue 2 and Issue 3 for specific fixes needed.

---

## References

- QEMU man page: `man qemu-system-x86_64` - "smb=dir[,smbserver=addr]" section
- QEMU built-in SMB server: Uses 10.0.2.4 by default (4th IP in guest network)
- WinFsp documentation: Requires user session for mount visibility
- Virtio-FS Windows documentation: https://github.com/virtio-win/kvm-guest-drivers-windows/wiki/Virtiofs:-Shared-file-system
