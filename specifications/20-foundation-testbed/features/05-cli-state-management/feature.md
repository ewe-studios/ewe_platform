---
feature: "CLI & State Management"
description: "Persistent VM state, error types via foundation_errstacks, health checks, and all CLI subcommand implementations"
status: "pending"
priority: "high"
depends_on: ["runner-utilities"]
estimated_effort: "medium"
created: 2026-05-02
last_updated: 2026-05-02
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# CLI & State Management Feature

## Overview

The state management layer and CLI command implementations. Persists VM state (PID, disk path, bootstrap status) to disk, defines error types using `foundation_errstacks`, and implements all `ewe_platform testbed` subcommands.

## Dependencies

- Depends on **04-runner-utilities** (all underlying logic is in place)
- Uses `foundation_errstacks` for error types

## Requirements

### 5.1 VM State Management

Persistent state in `~/.cache/foundation_testbed/state/`:

```json
// ~/.cache/foundation_testbed/state/windows-build.json
{
    "profile_name": "windows-build",
    "disk_path": "/home/user/.cache/foundation_testbed/images/windows-11-x86_64.qcow2",
    "pid": 12345,
    "monitor_socket": "/tmp/foundation-testbed-windows-build.monitor",
    "ssh_port": 2222,
    "winrm_port": 5985,
    "rdp_port": 3389,
    "vnc_port": 5900,
    "bootstrapped": true,
    "created_at": "2026-05-02T10:30:00Z"
}
```

Operations:
- `save(name, state)` — write JSON state file
- `load(name) -> Result<State>` — read and parse state file
- `exists(name) -> bool` — check if state file exists
- `delete(name)` — remove state file
- `list() -> Vec<(String, State)>` — list all known VMs
- **Stale detection**: on load, check if PID is still alive via `kill(pid, 0)`. If process is gone, mark as stopped and clean up.

### 5.2 Error Types (`foundation_errstacks` integration)

```rust
#[derive(Debug)]
pub enum TestbedError {
    QemuNotFound { install_cmd: String },
    KvmUnavailable,
    VmNotRunning { name: String },
    SshFailed { port: u16, source: anyhow::Error },
    WinrmNotReachable { port: u16 },
    BuildFailed { target: String, code: i32 },
    DownloadFailed { status: u16, url: String },
    BootstrapFailed { step: String, message: String },
    ArtifactNotFound { path: String },
    DiskResizeFailed { source: anyhow::Error },
    PortInUse { port: u16 },
    SnapshotFailed { name: String, reason: String },
}
```

All public APIs return `Result<T, TestbedError>` (via `foundation_errstacks::Result<T>` or the crate's own `Result<T>` type alias that wraps into errstacks).

### 5.3 VM Profiles

Centralized profile definitions in `config.rs`:

```rust
pub struct VmProfile {
    pub name: &'static str,
    pub os: GuestOs,           // Windows | Linux
    pub image_name: &'static str,
    pub ssh_port: u16,
    pub rdp_port: Option<u16>,
    pub winrm_port: Option<u16>,
    pub vnc_port: u16,
    pub user: &'static str,
    pub pass: &'static str,
    pub bootstrap: BootstrapMode,  // Full | SshOnly
    pub memory_mib: u32,
    pub cpu_cores: u32,
    pub disk_gb: u32,
    pub prebaked_url: Option<&'static str>,
}
```

Default profiles:

| Name | OS | SSH | RDP | WinRM | VNC | RAM | Cores | Disk | Bootstrap |
|---|---|---|---|---|---|---|---|---|---|
| `windows-build` | Windows | 2222 | 3389 | 5985 | 5900 | 12288 | 4 | 80 GB | Full |
| `windows-test` | Windows | 2322 | 3389 | 5985 | 5901 | 4096 | 2 | 40 GB | SshOnly |
| `linux-build` | Linux | 2422 | — | — | 5902 | 4096 | 4 | 40 GB | Full |
| `linux-test` | Linux | 2522 | — | — | 5903 | 2048 | 2 | 20 GB | SshOnly |

Users can override defaults via a config file (TOML) at `~/.config/foundation_testbed/config.toml`:

```toml
[[profiles]]
name = "custom-windows"
os = "windows"
image_url = "file:///path/to/my-windows.qcow2"
ssh_port = 2722
memory_mib = 16384
cpu_cores = 8
disk_gb = 120

[bootstrap]
# Custom mise.toml to use during bootstrap (overrides the crate's default)
mise_toml_path = "/path/to/my-bootstrap.toml"
```

### 5.4 CLI Subcommand Implementations

Each `ewe_platform testbed <command>` delegates to a `foundation_testbed` public API function:

| Command | API Call | Description |
|---|---|---|
| `start <os> --headless` | `testbed::start(profile, DisplayMode::Headless)` | Boot VM |
| `start <os> --headful` | `testbed::start(profile, DisplayMode::Headful)` | Boot VM with SPICE |
| `stop <os>` | `testbed::stop(profile)` | Graceful shutdown |
| `build <os> --project <path>` | `testbed::build(profile, project_dir)` | Cross-compile |
| `exec <os> "<cmd>"` | `testbed::exec(profile, cmd)` | Run command in VM |
| `shell <os>` | `testbed::shell(profile)` | Interactive SSH |
| `run <os> [--bin <path>]` | `testbed::run(profile, bin)` | Launch binary in VM |
| `screenshot <os> --out <file>` | `testbed::screenshot(profile, out_path)` | Capture display |
| `logs <os> [--follow] [--errors] [--tail N]` | `testbed::logs(profile, opts)` | Tail logs |
| `doctor` | `testbed::doctor()` | Host health checks |
| `doctor <os>` | `testbed::doctor_vm(profile)` | VM health checks |
| `push <os> --from <src> --to <dst>` | `testbed::push(profile, src, dst)` | File to VM |
| `pull <os> --from <src> --to <dst>` | `testbed::pull(profile, src, dst)` | File from VM |
| `package <os> --out <path>` | `testbed::package(profile, out_path)` | Export qcow2 + metadata |
| `resize-disk <os> --plus-gb <N>` | `testbed::resize_disk(profile, plus_gb)` | Grow VM disk |
| `ls` | `testbed::list_vms()` | List all VMs |
| `snapshot save <os> <name>` | `testbed::snapshot_save(profile, name)` | Save VM state |
| `snapshot load <os> <name>` | `testbed::snapshot_load(profile, name)` | Restore VM state |
| `snapshot delete <os> <name>` | `testbed::snapshot_delete(profile, name)` | Delete snapshot |
| `snapshot list <os>` | `testbed::snapshot_list(profile)` | List snapshots |

### 5.5 Doctor (Health Checks)

**Host-level (`ewe_platform testbed doctor`):**
- KVM available (`/dev/kvm` exists, readable)
- `qemu-system-x86_64` on PATH
- `qemu-img` on PATH
- SSH key exists in `~/.ssh/`
- Sufficient disk space in `~/.cache/foundation_testbed/`
- Ports not in use

**VM-level (`ewe_platform testbed doctor windows`):**
- SSH reachable
- WinRM reachable (Windows)
- mise on PATH
- nushell (`nu`) on PATH and set as default shell
- VS Build Tools present (Windows): check for `Hostarm64\x64\link.exe`
- WebView2 installed (Windows)
- rustup default-host correct
- Tauri deps installed (Linux): `dpkg -s libwebkit2gtk-4.1-dev`
- Xvfb present (Linux)

## Implementation Phases

### Phase 1: State & Errors (Tasks 1-3)
1. Create `src/state/mod.rs` — JSON state file management, stale PID detection
2. Create `src/config.rs` — `VmProfile` struct, default profiles, user config override from TOML
3. Create `src/errors.rs` — `TestbedError` enum, integration with `foundation_errstacks`

### Phase 2: CLI Commands (Tasks 4-6)
4. Implement `start`, `stop`, `ls` commands — QEMU lifecycle + state
5. Implement `build`, `exec`, `shell`, `push`, `pull` — build pipeline + SSH
6. Implement `run`, `screenshot`, `logs` — runner utilities

### Phase 3: Doctor & Package (Tasks 7-8)
7. Implement `doctor` (host + VM), `resize-disk`
8. Implement `package` (export qcow2 + metadata), snapshot commands

## Success Criteria

- [ ] All 21 CLI commands are registered and callable via `ewe_platform testbed <cmd>`
- [ ] `ewe_platform testbed doctor` prints actionable health status
- [ ] VM state persists across restarts of the CLI
- [ ] Stale PIDs are detected and cleaned up automatically
- [ ] Custom profiles via `~/.config/foundation_testbed/config.toml` work
- [ ] All errors are `TestbedError` variants with helpful messages

## Verification Commands

```bash
cargo test -p foundation_testbed state::
cargo test -p foundation_testbed config::
cargo test -p foundation_testbed errors::
```

---

## Implementation Plan

### State File Format

JSON files stored in `~/.cache/foundation_testbed/state/<profile-name>.json`. The state file serves as the "database" for VM management:

- **On VM start**: create/update state with PID, ports, disk path
- **On VM stop**: update state to remove PID, mark as stopped
- **On CLI startup**: check all state files, detect stale PIDs, clean up

### Stale Detection

```rust
fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
```

If the QEMU process died (crash, OOM kill, manual `kill`), the state file still exists but the PID is dead. We mark the VM as "stopped" and remove the PID. The user can `start` again and a new QEMU process will launch.

### Profile Override Resolution

User config merges over defaults:

```
default_profiles (hardcoded)
    + user_profiles (from ~/.config/foundation_testbed/config.toml)
    = resolved_profiles
```

If a user defines a profile with the same name as a default, the user's values override the defaults field-by-field (not replace entirely).

### Error Display

Errors are formatted for CLI output:

```
Error: SSH connection failed on port 2222
  → Is the VM running? Try: ewe_platform testbed start windows
  → Check: ewe_platform testbed doctor windows

Error: Build failed for target 'x86_64-pc-windows-msvc' (exit 101)
  → Full log: ewe_platform testbed logs windows
  → Errors only: ewe_platform testbed logs windows --errors
```

Each error variant implements `Display` with actionable suggestions.
