---
feature: "Runner & Utilities"
description: "Binary launcher inside VMs, screenshot capture, log tailing, error extraction, and file transfer utilities"
status: "implemented"
priority: "high"
depends_on: ["bootstrap-build-pipeline"]
estimated_effort: "medium"
created: 2026-05-02
last_updated: 2026-05-11
author: "Main Agent"
tasks:
  completed: 12
  uncompleted: 0
  total: 12
  completion_percentage: 100%
---

# Runner & Utilities Feature

## Overview

Post-build utilities: launch compiled binaries inside VMs for UI testing, capture screenshots of the VM display, tail build/runtime logs, and transfer files between host and VM. This is the "test and observe" layer that sits on top of the build pipeline.

## Dependencies

- Depends on **03-bootstrap-build-pipeline** (needs build artifacts available)
- Depends on **02-vm-communication** (needs SSH for exec and SCP)
- Depends on **01-qemu-backend** (needs VM running with display configured)

## Requirements

### 4.1 Binary Launcher (`run`)

Launch a compiled binary inside the VM and capture its startup output. All commands run via **nushell** (`nu -c "..."`):

**Linux:**
- Start Xvfb on display `:99` (virtual framebuffer, 1280x800x24)
- Start openbox window manager on `:99` (so windows actually get mapped/composited)
- Launch the binary with `DISPLAY=:99` via `setsid -f` (detached from SSH session)
- Redirect stdout/stderr to `~/.testbed-run/run.log`
- Kill any prior instance of the same binary before launching

**Windows:**
- Launch binary via `Start-Process -RedirectStandardOutput` / `-RedirectStandardError`
- Capture PID for later reference
- Redirect to `%USERPROFILE%\.testbed-run\run.log`

The nushell wrapper provides consistent syntax for both OSes. Internally, Linux still uses `setsid -f` and Windows still uses `Start-Process` (these are OS primitives that can't be abstracted), but the caller code is unified through the `run` API.

**Auto-detect binary:** If no `--bin` flag, auto-detect the most recent bundle:
- Scan `src-tauri/target/{triple}/release/bundle/` for `.msi`/`.exe` (Windows) or `.deb`/`AppImage` (Linux)
- Fall back to the binary name derived from project's `Cargo.toml` `[package] name`

### 4.2 Screenshot Capture

**Linux:**
- `scrot --overwrite /tmp/testbed-screenshot.png` against `DISPLAY=:99`
- SCP the PNG back to host
- Requires Xvfb + openbox running (from `run` command)

**Windows:**
- PowerShell: `[System.Windows.Forms.Screen]` capture or `Add-Type -AssemblyName System.Drawing; [System.Drawing.Bitmap]` screenshot
- Requires RDP or active desktop session
- SCP the PNG back via SSH

### 4.3 Log Tailing (`logs`)

- Dump or tail build logs from inside the VM via SSH
- Modes:
  - **dump**: `cat` the full log file
  - **follow**: `tail -F` (Linux) or `Get-Content -Wait` (Windows PowerShell)
  - **errors**: grep/Select-String for error patterns with context (same patterns as build error extraction)
- Log kinds: `build` (from `~/.testbed-build/build.log`) and `run` (from `~/.testbed-run/run.log`)
- `--tail N` flag to show last N lines

### 4.4 File Transfer (`push` / `pull`)

- `push` — `scp -r` from host to VM
- `pull` — `scp -r` from VM to host
- Delegates to the existing `ssh::upload` / `ssh::download` functions
- Support both files and directories

## Implementation Phases

### Phase 1: Runner (Tasks 1-2)
1. Create `src/runner/mod.rs` — binary launcher for Linux (Xvfb + openbox + app) and Windows (Start-Process)
2. Create auto-detect logic: scan target dir for most recent bundle, derive binary name from Cargo.toml

### Phase 2: Screenshot & Logs (Tasks 3-4)
3. Create `src/runner/screenshot.rs` — Linux (scrot via SSH) and Windows (PowerShell capture)
4. Create `src/runner/logs.rs` — log tailing, dump, error extraction via SSH

### Phase 3: File Transfer (Task 5)
5. Create `src/runner/transfer.rs` — push/pull wrappers around ssh::upload/download

## Success Criteria

- [x] `run(profile)` launches a Tauri app with Xvfb on Linux, window is visible on VNC
- [x] `run(profile)` launches a `.exe` on Windows, output captured to run.log
- [x] `screenshot(profile, "out.png")` produces a PNG of the VM display
- [x] `logs(profile, "build", follow=true)` streams live build output
- [x] `logs(profile, "build", errors=true)` shows only error stanzas with context
- [x] `push(profile, "./local.txt", "/remote.txt")` transfers file to VM
- [x] `pull(profile, "/remote.txt", "./local.txt")` retrieves file from VM

## Verification Commands

```bash
cargo test -p foundation_testbed runner::
cargo test -p foundation_testbed screenshot::
cargo test -p foundation_testbed logs::
```

---

## Implementation Plan

### Xvfb + openbox Pattern (Linux)

Without a window manager, GTK windows open but aren't composited/mapped on bare Xvfb — screenshots return black. With openbox running, windows appear correctly.

The launcher invokes these via `ssh::exec_streaming` with `nu -c "..."`:

```nu
# Nushell script for Linux VM launcher
^pkill Xvfb 2> /dev/null
^pkill openbox 2> /dev/null
^pkill $bin_name 2> /dev/null
sleep 1sec
^setsid -f Xvfb :99 -screen 0 1280x800x24 -nolisten tcp | save --append $log_dir/xvfb.log
sleep 1sec
DISPLAY=:99 ^setsid -f openbox --replace | save --append $log_dir/openbox.log
sleep 1sec
DISPLAY=:99 ^setsid -f $binary_path | save --append $log_dir/run.log
```

Key detail: use `ssh` CLI without `-tt` flag to avoid pty. A pty sends SIGHUP to backgrounded children on channel close, killing our Xvfb+app even with `setsid+nohup`.

### Windows Screenshot via Nushell

```nu
# Nushell invokes PowerShell for .NET screenshot
^Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::new($width, $height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen(0, 0, 0, 0, $bmp.Size)
$bmp.Save($output_path, [System.Drawing.Imaging.ImageFormat]::Png)
```

### Auto-Detect Binary

```rust
fn auto_detect_bin(profile: &VmProfile, session: &ssh2::Session) -> Result<String> {
    // 1. Read package name from Cargo.toml
    // 2. Probe candidate paths on VM:
    //    - CARGO_TARGET_DIR/<triple>/release/<name>.exe
    //    - ~/project/src-tauri/target/<triple>/release/<name>.exe
    //    - ~/project/target/<triple>/release/<name>.exe
    // 3. Return first existing path
}
```
