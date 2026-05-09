# Windows Mount Implementation - Summary of Fixes

## Date: 2026-05-10

## SMB Setup Issues (Resolved)

### Problem
QEMU's built-in SMB server wasn't working due to multiple issues:

1. **Missing Samba config** (`/etc/samba/smb.conf`)
2. **Missing Samba directories** (`/var/log/samba`, `/var/lib/samba`, `/run/samba`)
3. **Missing capabilities** on smbd binary (needs `CAP_NET_BIND_SERVICE` for ports 139/445)
4. **QEMU/Samba compatibility issue** - QEMU doesn't create the `ncalrpc` subdirectory that modern smbd (4.x) requires

### Fixes Applied

1. **Created `scripts/linux/smb-setup.sh`**:
   - Creates required Samba directories
   - Installs minimal smb.conf for QEMU
   - Sets proper ownership for the user running QEMU
   - **Adds `CAP_NET_BIND_SERVICE` capability to smbd** (critical fix)

2. **Created `scripts/linux/smb.conf`**:
   - Minimal Samba configuration compatible with QEMU
   - Uses dynamic path substitution (`%S`) for share path

3. **Added mise task `[tasks.setup-smb]`** in `mise.toml`:
   ```toml
   [tasks.setup-smb]
   description = "Setup SMB for Windows VM mounting (requires sudo)"
   run = "sudo bash scripts/linux/smb-setup.sh"
   ```

### SMB Limitation Discovered

QEMU creates a temporary directory with smb.conf but doesn't create the `ncalrpc` subdirectory required by modern Samba (4.x). This causes smbd to fail to start when spawned by QEMU.

**Workarounds**:
1. Use virtiofs instead (recommended)
2. Start smbd manually with pre-created directory structure
3. Fix QEMU source code to create the required directory

**Test Result**: SMB works when smbd is started manually with proper directory structure, but QEMU's built-in SMB integration doesn't work with modern Samba.

---

## virtiofs Setup Status

### Host Side (Working)

1. **virtiofsd is installed** at `/usr/lib/virtiofsd` (version 1.13.3)
2. **virtiofsd starts correctly** with the expected parameters:
   ```bash
   /usr/lib/virtiofsd \
     --socket-path /path/to/socket \
     --shared-dir /path/to/share \
     --sandbox=none \
     --seccomp=none \
     --inode-file-handles=never
   ```

3. **QEMU virtiofs integration** in `src/qemu/mod.rs`:
   - Spawns virtiofsd before starting VM
   - Creates vhost-user socket
   - Adds `-chardev socket` and `-device vhost-user-fs-pci` to QEMU args

### Guest Side (Configured)

1. **Windows scripts updated**:
   - `register_virtiofs_startup.ps1` - Registers scheduled task for auto-mount
   - `mount_virtiofs.ps1` - Mounts virtiofs with `-t project` tag
   - `check_project_mount.ps1` - Verifies mount is accessible

2. **Required components**:
   - virtio-win drivers (downloaded from Fedora if not present)
   - WinFsp (userspace filesystem framework)
   - virtiofs.exe (from virtio-win package)

### virtiofs Requirements Checklist

- [x] virtiofsd installed on host
- [x] QEMU configured to use virtiofsd
- [x] Windows scripts include `-t project` tag
- [x] WinFsp installation script
- [x] virtio-win driver installation script
- [ ] **Needs testing**: Full Windows VM boot with virtiofs mount

---

## Outstanding Issues

1. **QEMU SMB integration** - Needs QEMU fix or workaround
2. **virtiofs E2E test** - Needs Windows VM test run to verify

## Recommended Next Steps

1. Run Windows VM test with virtiofs to verify the mount works
2. If virtiofs fails, debug the Windows guest side
3. Consider implementing a standalone Samba server as SMB fallback instead of relying on QEMU's built-in SMB
