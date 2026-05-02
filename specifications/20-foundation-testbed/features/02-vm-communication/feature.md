---
feature: "VM Communication"
description: "SSH client layer, WinRM SOAP client, and image download/import — platform-agnostic communication with guest VMs"
status: "pending"
priority: "high"
depends_on: ["qemu-backend"]
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

# VM Communication Feature

## Overview

Provides the communication channels between the host and guest VMs. This layer is **hypervisor-agnostic** — it works the same whether the VM is managed by QEMU, UTM, or anything else. The code is ported directly from `utm-dev-cli` with minimal changes.

## Dependencies

- Depends on **01-qemu-backend** (needs VM running with port forwarding configured)
- External: `ssh2` crate (vendored OpenSSL), `reqwest` (blocking client)

## Requirements

### 2.1 SSH Client Layer

- Connect to VM via SSH on the forwarded port (default 2222)
- Authentication priority: SSH agent → key files (`id_ed25519`, `id_rsa`, `id_ecdsa`) → password
- `connect(profile) -> Session` — establishes SSH session with auth fallback chain
- `exec(session, cmd) -> String` — blocking command execution, returns stdout. Commands are executed via `nu -c "..."` after bootstrap (nushell as the execution shell). Before bootstrap, falls back to `bash -c` (Linux) or `cmd /c` (Windows).
- `exec_with_exit(session, cmd) -> (String, i32)` — returns stdout + exit code. Same shell resolution as `exec`.
- `exec_streaming(profile, cmd) -> i32` — spawns `ssh` CLI subprocess for live output (long-running commands like `cargo build` would block `read_to_string` for 10+ minutes). Commands are wrapped in `nu -c "..."`.
- `upload(profile, local, remote) -> ()` — SCP file upload via `scp` CLI (more reliable than libssh2's scp_send against Windows OpenSSH)
- `download(profile, remote, local) -> ()` — SCP file download
- `check(profile) -> Result<()>` — health probe: connect + echo test

### 2.2 WinRM SOAP Client

- Pure Rust WinRM client — no Python/pywinrm dependency, no platform dependency
- HTTP Basic auth over plain HTTP (port 5985, unencrypted — acceptable since it's localhost)
- `WinRM::new(host, port, user, pass)` — creates client with Base64-encoded Basic auth
- `run_ps(script) -> CmdResult` — runs PowerShell via WinRM shell, returns stdout/stderr/exit_code
  - Encodes script as UTF-16LE + Base64 for `powershell -EncodedCommand`
- `run_elevated(ps_code, timeout) -> bool` — runs PowerShell as SYSTEM via scheduled task
  - Writes script to `C:\bootstrap-step.ps1`
  - Creates scheduled task with SYSTEM principal + highest run level
  - Polls for completion via sentinel file `C:\bootstrap-step-done.txt`
  - Falls back to `(Get-ScheduledTask).State` if sentinel detection fails
- `ping() -> bool` — cheap reachability probe (GET /wsman, any HTTP response = WinRM is up)
- SOAP envelope construction with proper WS-Addressing headers
- Shell lifecycle: Create → Execute → Receive (loop until CommandState=Done) → Delete
- Message ID generation (UUID v4-ish format) for request correlation

### 2.3 Image Download / Import

- Download pre-built qcow2 images from configured URLs
- Support Vagrant Cloud API as a source: `GET https://api.cloud.hashicorp.com/vagrant/2022-09-30/registry/{provider}/box/{box_name}/versions`
- Support direct URL download with HTTP Range resume
- Cache images in `~/.cache/foundation_testbed/images/`
- Progress bar with `indicatif` during download
- Validate download size (reject files < 100 MB as likely failed downloads)

## Implementation Phases

### Phase 1: SSH Layer (Tasks 1-2)
1. Create `src/ssh/mod.rs` — port from `utm-dev-cli/src/vm/ssh.rs` (connect, exec, upload, download, check)
2. Create `src/ssh/streaming.rs` — exec_streaming via ssh CLI subprocess for live output

### Phase 2: WinRM Client (Tasks 3-4)
3. Create `src/winrm/mod.rs` — port from `utm-dev-cli/src/vm/winrm.rs` (SOAP client, run_ps, ping)
4. Create `src/winrm/elevated.rs` — port `run_elevated` (scheduled task, sentinel detection)

### Phase 3: Image Import (Tasks 5-6)
5. Create `src/import/mod.rs` — orchestrator, cache directory management
6. Create `src/import/direct.rs` — HTTP download with progress bar, resume support

## Success Criteria

- [ ] `ssh::connect(profile)` establishes connection with auth fallback chain
- [ ] `ssh::exec(session, "echo hello")` returns "hello"
- [ ] `ssh::exec_streaming(profile, "sleep 5 && echo done")` shows live output on terminal
- [ ] `winrm.ping()` returns true when Windows VM WinRM is listening
- [ ] `winrm.run_ps("Get-Host")` returns PowerShell host info with exit code 0
- [ ] `winrm.run_elevated("Restart-Service sshd", 60)` runs as SYSTEM and completes
- [ ] `import::download(url, dest)` downloads with progress, resumes on interruption
- [ ] All code compiles on Linux without macOS dependencies

## Verification Commands

```bash
cargo test -p foundation_testbed ssh::
cargo test -p foundation_testbed winrm::
cargo test -p foundation_testbed import::
```

---

## Implementation Plan

### Architecture

This feature is a thin wrapper around three communication protocols:

```
foundation_testbed::vm_communication
├── ssh          → ssh2 crate (libssh2 bindings) + scp CLI
├── winrm        → reqwest blocking HTTP client + SOAP XML
└── import       → reqwest blocking HTTP client + tar + flate2
```

### Porting Strategy from utm-dev-cli

The `utm-dev-cli` source files port directly:
- `src/vm/ssh.rs` → `src/ssh/mod.rs` (near verbatim, ~166 lines)
- `src/vm/winrm.rs` → `src/winrm/mod.rs` (near verbatim, ~354 lines)

The only changes needed:
1. Replace `super::profiles` imports with `crate::config` references
2. Replace `anyhow::bail` with `foundation_errstacks::TestbedError` variants
3. Add `use foundation_errstacks::{Result, TestbedError}` to each module

No logic changes — SSH is SSH, WinRM is WinRM, regardless of hypervisor.

### WinRM Authentication Note

We use Basic auth over plain HTTP (not HTTPS). This is acceptable because:
- WinRM is bound to `127.0.0.1:5985` (localhost only)
- No network traffic leaves the host machine
- The QEMU user-mode netdev only forwards localhost ports
- Encrypting localhost traffic adds latency with no security benefit

If the VM is ever configured with bridged networking (future feature), this would need to change to Negotiate auth over HTTPS.
