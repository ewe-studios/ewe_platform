---
feature: "Host Bootstrap — mise + nushell + pitchfork"
description: "First-run host setup: install mise, nushell, and pitchfork on the host machine. Creates a unified foundation for shell execution, tool management, and service/process registration across all testbed operations."
status: "implemented"
priority: "critical"
depends_on: []
estimated_effort: "small"
created: 2026-05-03
last_updated: 2026-05-11
author: "Main Agent"
tasks:
  completed: 1
  uncompleted: 0
  total: 1
  completion_percentage: 100%
---

# Host Bootstrap — mise + nushell + pitchfork

## Overview

Before any VM runs, the **host machine** needs three foundational tools:

| Tool | Purpose | Why First |
|------|---------|-----------|
| **mise** | Tool version management | Everything else (Rust, nushell, pitchfork) installs through it |
| **nushell** (`nu`) | Unified cross-platform shell | All VM-side commands, host scripts, and bootstrap logic run through `nu -c` — eliminates bash/PowerShell dialect splits |
| **pitchfork** | Process/service manager | Manages long-running processes (QEMU instances, SSH tunnels, port forwards) with registration, health checks, and lifecycle control |

Together these form the **host foundation**: mise installs and versions tools, nushell provides consistent command execution, pitchfork manages process lifecycle. This is the same stack we install inside VMs — host and VMs share the same toolchain paradigm.

## Why These Three

**mise + nushell** are already established in the project. The addition of **pitchfork** (from the mise developer) solves a specific gap: managing QEMU processes, SSH tunnels, and port forwards as registered services with health checks, rather than raw `std::process::Child` handles.

| Current approach | With pitchfork |
|------------------|----------------|
| Track QEMU PID manually | Register as a named service |
| Manual `kill()` on stop | Graceful shutdown via pitchfork |
| No health monitoring | Automatic liveness checks |
| Stale PID detection on CLI start | pitchfork detects crashed processes |
| No service registration | `pitchfork list` shows all managed VMs |

## Requirements

### 4.1 Host Bootstrap Orchestrator

A synchronous function that runs on first `testbed start` or `testbed doctor`:

```rust
// src/host_bootstrap/mod.rs

pub fn ensure_host_prerequisites() -> Result<HostPrerequisites> {
    let mut state = HostPrerequisites::default();

    // 1. mise — check on PATH, install if missing
    state.mise = ensure_mise()?;

    // 2. nushell — check via `nu --version`, install via mise
    state.nushell = ensure_nushell()?;

    // 3. pitchfork — check via `pitchfork --version`, install via mise
    state.pitchfork = ensure_pitchfork()?;

    Ok(state)
}
```

Idempotent — skips installation if the tool is already present and meets minimum version.

### 4.2 mise Installation

**Linux (Arch, Debian, Ubuntu):**
```bash
curl https://mise.run | sh
```

**macOS:**
```bash
brew install mise
# or
curl https://mise.run | sh
```

**Verification:**
```bash
mise --version  # must be >= 2024.x
```

### 4.3 Nushell Installation

Via mise (consistent across all platforms):
```bash
mise global nu@latest
mise install
```

Bootstrap `mise.toml` (host-level, `~/.config/mise/config.toml`):
```toml
[tools]
nu = "latest"
pitchfork = "latest"

[settings]
cargo_binstall = true  # fast-path for cargo-based tools
```

**Verification:**
```bash
nu --version
nu -c "echo 'hello from nushell'"
```

### 4.4 pitchfork Installation

Via mise:
```bash
mise global pitchfork@latest
mise install
```

**Verification:**
```bash
pitchfork --version
pitchfork list  # should show no running processes
```

### 4.5 Integration with QEMU Provider

The QEMU provider replaces manual process management with pitchfork:

```rust
// Before: manual process tracking
pub struct QemuVm {
    process: std::process::Child,
    pid: u32,
}

// After: pitchfork service registration
pub struct QemuVm {
    service_name: String,  // "testbed-windows-build"
}

// Launch via pitchfork:
pitchfork start --name testbed-windows-build \
    --exec "qemu-system-x86_64 ..." \
    --health-check "kill -0 $PID" \
    --on-failure restart

// Stop via pitchfork:
pitchfork stop --name testbed-windows-build

// Status:
pitchfork list  # shows all running VMs with health status
```

### 4.6 Integration with Build Pipeline

The build pipeline uses pitchfork to register the build process:

```bash
# Build runs as a pitchfork-managed process with log capture
pitchfork run --name testbed-build-windows \
    --log ~/.testbed/builds/windows-build.log \
    -- nu -c "cd /mnt/project && cargo tauri build --target x86_64-pc-windows-msvc"
```

Benefits:
- Build log is captured automatically
- If the build process crashes, pitchfork records the exit code
- `pitchfork logs testbed-build-windows` tails the build output

### 4.7 nushell as Unified Shell

All VM-side command execution goes through nushell:

```rust
// Instead of:
// ssh_exec(profile, "bash -c '...'")  // Linux
// ssh_exec(profile, "powershell -c '...'")  // Windows

// Unified:
ssh_exec(profile, &format!("nu -c '{}'", nushell_script))
```

The nushell script is built using a builder pattern:

```rust
let script = NuScript::new()
    .cd("/mnt/project")
    .run("mise install")
    .run("cargo tauri build --target x86_64-pc-windows-msvc")
    .build();

ssh_exec(profile, &script.to_command());
// → nu -c "cd /mnt/project; mise install; cargo tauri build ..."
```

## Implementation Phases

### Phase 1: Host Bootstrap (Tasks 1-3)

1. Create `src/host_bootstrap/mod.rs` — orchestrator: check/install mise, nushell, pitchfork
2. Create `src/host_bootstrap/install.rs` — platform-specific installers (curl, brew, apt)
3. Add `testbed doctor` host check: verify mise + nu + pitchfork on PATH

### Phase 2: pitchfork Integration (Tasks 4-5)

4. Create `src/common/process.rs` — pitchfork wrapper for process start/stop/list
5. Update QEMU provider to use pitchfork instead of raw `std::process::Child`

### Phase 3: nushell Execution Layer (Tasks 6-7)

6. Create `src/common/nushell.rs` — NuScript builder for cross-platform command execution
7. Update all SSH exec calls to use nushell instead of bash/PowerShell

## Success Criteria

- [x] `ewe_platform testbed doctor` checks for mise, nushell, pitchfork on host
- [x] `ewe_platform testbed start` auto-installs missing prerequisites (with confirmation)
- [x] `mise global` lists nu and pitchfork as installed tools
- [x] `pitchfork list` shows running VMs after `testbed start`
- [x] All VM-side commands execute via `nu -c` (no bash/PowerShell dialect splits)
- [x] Bootstrap is idempotent — running on an already-configured host is a no-op

## Verification Commands

```bash
# Check host prerequisites
ewe_platform testbed doctor

# Verify tools
mise --version
nu --version
pitchfork --version

# List managed processes
pitchfork list

# Test nushell execution
nu -c "echo 'hello from testbed'"
```
