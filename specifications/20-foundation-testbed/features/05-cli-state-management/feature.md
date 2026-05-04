---
feature: "CLI & State Management"
description: "Persistent VM state, error types via foundation_errstacks, health checks, and all CLI subcommand implementations"
status: "implemented"
priority: "high"
depends_on: ["runner-utilities"]
estimated_effort: "medium"
created: 2026-05-02
last_updated: 2026-05-04
author: "Main Agent"
tasks:
  completed: 14
  uncompleted: 0
  total: 14
  completion_percentage: 100%
---

# CLI & State Management Feature

## Overview

The state management layer and CLI command implementations. Persists VM state (PID, disk path, bootstrap status) to disk, defines error types using `foundation_errstacks`, and implements all `ewe_platform testbed` subcommands.

## Dependencies

- Depends on **04-runner-utilities** (all underlying logic is in place)
- Uses `foundation_errstacks` for error types

## Requirements

### 5.1 VM State Management

Persistent state in `$PWD/.testbed/state/`:

```json
// .testbed/state/windows-build.json
{
    "profile_name": "windows-build",
    "arch": "x86_64",
    "disk_path": "$HOME/.testbed/images/windows-11-x86_64.qcow2",
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
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

pub struct VmProfile {
    pub name: &'static str,
    pub os: GuestOs,           // Windows | Linux | MacOS
    pub arch: Arch,            // Target architecture
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
}
```

Default profiles:

| Name | OS | Arch | SSH | RDP | WinRM | VNC | RAM | Cores | Disk | Bootstrap |
|---|---|---|---|---|---|---|---|---|---|---|
| `windows-build` | Windows | x86_64 | 2222 | 3389 | 5985 | 5900 | 12288 | 4 | 80 GB | Full |
| `windows-build-arm` | Windows | aarch64 | 2225 | 3390 | 55985 | 5904 | 12288 | 4 | 80 GB | Full |
| `windows-test` | Windows | x86_64 | 2322 | 3389 | 5985 | 5901 | 4096 | 2 | 40 GB | SshOnly |
| `linux-build` | Linux | x86_64 | 2422 | — | — | 5902 | 4096 | 4 | 40 GB | Full |
| `linux-build-arm` | Linux | aarch64 | 2425 | — | — | 5905 | 4096 | 4 | 40 GB | Full |
| `linux-test` | Linux | x86_64 | 2522 | — | — | 5903 | 2048 | 2 | 20 GB | SshOnly |
| `linux-dev` | Linux | x86_64 | 2622 | — | — | 5906 | 6144 | 4 | 20 GB | Full |
| `macos-build` | MacOS | x86_64 | 2223 | — | — | 5903 | 8192 | 4 | 80 GB | SshOnly |

Users can override defaults via `testbed.toml` in `$PWD/.testbed/`:

```toml
[vm]
profile = "custom-windows"
memory_mib = 16384
cpu_cores = 8

[[image_stores]]
name = "my_r2"
type = "r2"
bucket = "my-images"

[[image_stores]]
name = "vagrant_cloud"
type = "vagrant"
registry = "libvirt"
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
| `shell <os>` | `testbed::shell(profile)` | Interactive SSH session |
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
| `adopt <os> --disk <path>` | `testbed::adopt(profile, disk_path)` | Register existing qcow2 as managed VM |
| `refresh-network <os>` | `testbed::refresh_network(profile)` | Stop → reallocate ports → relaunch → wait for boot |

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

JSON files stored in `$PWD/.testbed/state/<profile-name>.json`. The state file serves as the "database" for VM management:

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

### Profile & Image Source Resolution

User config merges over defaults:

```
default_profiles (hardcoded in code)
    + testbed.toml [vm] section (project-scoped overrides)
    = resolved_profiles

image_stores (from testbed.toml [[image_stores]] array):
    iterate in order → first store with image wins → download if not cached
```

Image sources support:
- **`direct`** — raw URL to a qcow2/bundle file, cached at `$HOME/.testbed/images/`
- **`vagrant`** — Vagrant Cloud API lookup, version resolution, download with resume

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

### Interactive Shell (`testbed shell`)

The `shell` command opens an interactive SSH session. This is the **only** command that requests a pseudo-terminal (`-tt`):

- **Linux**: `ssh -tt -p <port> <user>@127.0.0.1` — allocates pty for line-buffered terminal I/O
- **Windows**: `ssh -p <port> <user>@127.0.0.1` — **no `-tt`** because `cmd.exe`/PowerShell break with forced pty allocation

For all non-interactive commands (`exec`, `build`, `run`, etc.), `ssh` is invoked **without** `-tt`. This prevents two failures observed in `utm-dev-cli`:

1. **SIGHUP on detached processes**: When an SSH channel with `-tt` closes, the SSH server sends SIGHUP to the pty's process group — killing any `setsid -f` or `nohup &` backgrounded processes.
2. **Windows cmd.exe corruption**: `-tt` forces terminal mode on Windows, which breaks non-interactive command execution and causes output garbling.

The `ssh2` crate's `channel_exec` (used by non-interactive `exec`) does **not** allocate a pty by default, so it's already safe. The `shell` command shells out to the `ssh` CLI where `-tt` matters.

### VM Adopt (`testbed adopt`)

Lets users register an existing qcow2 image as a managed VM:

```
ewe_platform testbed adopt <profile> --disk /path/to/image.qcow2
```

1. Validates that the qcow2 file exists and is non-empty
2. Copies (or symlinks) the disk into `$HOME/.testbed/images/` under a unique name
3. Creates a state file in `$PWD/.testbed/state/` with `pid: None`, `bootstrapped: false`
4. Verifies the VM can boot by starting it and checking SSH reachability
5. On success, the VM appears in `testbed ls` and is manageable via all other subcommands

### Refresh Network (`testbed refresh-network`)

Recovers from stale port forwards and unreachable services:

```
ewe_platform testbed refresh-network <profile>
```

1. Stops the VM gracefully (or force-kills if unresponsive)
2. Waits for the QEMU process to fully exit
3. Reallocates ports (checks for conflicts, may assign new ones)
4. Relaunches the VM with fresh port forwarding configuration
5. Waits for SSH/WinRM to become reachable (same boot-wait logic as `start`)
6. Prints the resolved ports so the user knows what changed

**Why this exists:** On QEMU, port forwards are baked into `-netdev user` at launch. On UTM, port-forward AppleScript changes only apply on cold boot. In both cases, if the user changes their network config, or if a port gets stolen by another process between VM restarts, `refresh-network` is the recovery path. Error messages for "WinRM not reachable" or "boot timeout" should suggest this command.
