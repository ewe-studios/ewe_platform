---
feature: "Bootstrap & Build Pipeline"
description: "Idempotent OS bootstrapping, code sync, tool installation, cargo tauri build, and artifact retrieval inside VMs"
status: "implemented"
priority: "high"
depends_on: ["vm-communication"]
estimated_effort: "large"
created: 2026-05-02
last_updated: 2026-05-11
author: "Main Agent"
tasks:
  completed: 1
  uncompleted: 0
  total: 1
  completion_percentage: 100%
---

# Bootstrap & Build Pipeline Feature

## Overview

The core value-delivery feature. Bootstraps guest OSes with development tools, syncs project code into VMs, runs `cargo tauri build`, and pulls back the compiled artifacts. This is the pipeline that makes Windows cross-compilation from Linux practical.

## Dependencies

- Depends on **02-vm-communication** (needs SSH and WinRM working)
- Depends on **01-qemu-backend** (needs VM running)
- External: `mise` must be installed on the host (manages qemu); inside VMs, the bootstrap installs its own mise

## Requirements

### 3.1 Windows Bootstrap (via WinRM → SSH)

Bootstrap is **step-by-step idempotent** — each step checks whether it's already been completed before running. If bootstrap fails mid-way, re-running picks up where it left off (no single marker file that skips everything).

Steps (in order):

1. **OpenSSH Server** — install via Windows Capability (`Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0`), configure `sshd_config`:
   - Uncomment `PasswordAuthentication yes`
   - Comment out `Match Group administrators` block (so `~/.ssh/authorized_keys` works for all users)
   - Comment out `AuthorizedKeysFile __PROGRAMDATA__`
   - Start sshd service, set to automatic
2. **SSH key authorization** — install host's public key in **both** `~/.ssh/authorized_keys` and `C:\ProgramData\ssh\administrators_authorized_keys` (Windows OpenSSH redirects admin users to the latter regardless of sshd_config). Set proper ACLs via `icacls`.
3. **LocalAccountTokenFilterPolicy** — registry `HKLM\...\System\LocalAccountTokenFilterPolicy = 1`
4. **Windows Defender exclusions** — add project and tool directories to Defender exclusion list to prevent real-time scanning from slowing builds or quarantining self-compiled binaries:
   ```powershell
   Add-MpPreference -ExclusionPath "C:\Users\vagrant\project"
   Add-MpPreference -ExclusionPath "C:\Users\vagrant\.cargo"
   Add-MpPreference -ExclusionPath "C:\Users\vagrant\.rustup"
   ```
5. **mise** — install mise via winget first, fall back to PowerShell installer (`Invoke-Expression (Invoke-WebRequest -Uri 'https://mise.run' -UseBasicParsing).Content`)
6. **mise install** — run `mise install` with a bootstrap `mise.toml` that declares:
   - `rust = "stable"`
   - `"cargo:cargo-binstall" = "latest"`
   - `"cargo:sccache" = "latest"`
   - `"cargo:tauri-cli" = "2"`
   - `nu = "latest"` (nushell)
7. **cargo-binstall** — direct `.exe` download from GitHub Releases (avoids compiling from source, which defeats the purpose):
   ```powershell
   Invoke-WebRequest -Uri 'https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-pc-windows-msvc.zip' -OutFile $zip
   ```
8. **mise config: cargo_binstall = true** — persist in user mise config so cargo: tools fetch prebuilt binaries
9. **rustup default-host for ARM64** — on ARM64 Windows hosts, VS Build Tools only ships `Hostarm64\x64` and `Hostarm64\x86` (no native ARM64 linker). Force rustup to `x86_64-pc-windows-msvc` so `mise install rust` installs the x86_64 toolchain that links cleanly under emulation:
   ```powershell
   rustup set default-host x86_64-pc-windows-msvc
   rustup default --force-non-host stable-x86_64-pc-windows-msvc
   ```
10. **Set nushell as default shell** — configure Windows registry so `nu` is the default interactive shell for SSH sessions
11. **VS Build Tools** — install `vs_buildtools.exe` with `Microsoft.VisualStudio.Workload.VCTools` + `--includeRecommended`. **Idempotent check**: probe for actual binary `Hostarm64\x64\link.exe` (or `Hostx64\x64\link.exe` on x86_64) rather than checking component registration — VS installer returns exit 0 without installing on ARM64, making component checks unreliable.
12. **WebView2 Runtime** — install via Microsoft's Evergreen Bootstrapper (`https://go.microsoft.com/fwlink/p/?LinkId=2124703`). Do **not** use `winget install Microsoft.EdgeWebView2Runtime` — it's unreliable on fresh Vagrant boxes where winget's Store source isn't primed.
13. **Bootstrap marker** — write `C:\Users\vagrant\.testbed-bootstrapped`

All elevated steps use `winrm.run_elevated()` (scheduled task as SYSTEM). Non-elevated steps use `ssh.exec()` with `nu -c "command"`. Each step is wrapped in an idempotent check — if the binary/file/service already exists, skip.

### 3.2 Linux Bootstrap (via SSH)

All tool installation is handled by **mise** with a `mise.toml` profile. Bootstrap steps:

1. **base packages** — `apt-get install build-essential curl git pkg-config` (minimal, only things mise can't install)
2. **Tauri Linux dependencies** — `apt-get install` for system libraries mise can't provide: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`, `libxdo-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `xvfb`, `scrot`, `openbox`
3. **mise** — `curl https://mise.run | sh`
4. **mise install** — run `mise install` with a bootstrap `mise.toml` that declares:
   - `rust = "stable"`
   - `"cargo:cargo-binstall" = "latest"`
   - `"cargo:sccache" = "latest"`
   - `"cargo:tauri-cli" = "2"`
   - `nu = "latest"` (nushell)
5. **Set nushell as default shell** — `chsh -s $(which nu)` or configure SSH `ForceCommand`
6. **Host SSH key authorization** — install host's public key into `~/.ssh/authorized_keys`
7. **Bootstrap marker** — write `/home/vagrant/.testbed-bootstrapped`

**Key shift from the original design:** System package installation (apt/yum/winget) is kept to the absolute minimum — only things that mise cannot install (system libraries, kernel modules, display servers). Everything else (rust, cargo tools, nushell, sccache, tauri-cli) goes through mise. This means:
- Bootstrap scripts are smaller and more consistent
- Tool versions are declared in TOML, not hardcoded in shell scripts
- The same `mise.toml` format works on both Windows and Linux
- Adding a new tool is adding one line to `mise.toml`, not writing new shell logic

### 3.3 Build Pipeline

End-to-end build orchestration inside the VM. All commands run via **nushell** (`nu -c "..."`):

1. **Pre-flight check** — verify project's `mise.toml` declares `rust` and `cargo:tauri-cli`
2. **Code sync** — create tar archive (exclude `target/`, `node_modules/`, `.git/`, `.build/`), SCP to VM, untar
3. **Tool installation** — run `mise install` inside VM (installs rust, tauri-cli, sccache, nu per project's `mise.toml`)
4. **Build** — run `nu -c "cargo tauri build --target '<triple>'"` inside VM
   - Windows: `x86_64-pc-windows-msvc`
   - Linux: `aarch64-unknown-linux-gnu`
   - Build output tee'd to `.testbed/logs/<profile>-build.log` (mounted directory, accessible on host immediately)
5. **Artifact retrieval** — scan host-side `target/<triple>/release/` (no SCP needed with project mount; with sync-only fallback, tar the bundle dir, SCP back to host, extract into `.build/{platform}/{arch}/`)
6. **Error extraction** — on failure, read host-side log file and grep for error stanzas with context

The nushell wrapper provides consistent path handling (`^env.PROJECT_DIR` works on both Windows and Linux), consistent environment variable setting, and consistent error capture across OSes.

### 3.4 Linux Multi-Arch (x86_64 cross-compile)

When building `x86_64-unknown-linux-gnu` from an ARM64 Linux VM:
- Enable Debian multiarch: `dpkg --add-architecture amd64`
- Install `:amd64` system libraries (libwebkit2gtk:amd64, gcc-x86-64-linux-gnu, etc.)
- Set `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc`
- Set `PKG_CONFIG_PATH` to point at amd64 pkg-config paths
- Set `PKG_CONFIG_ALLOW_CROSS=1`

## Implementation Phases

### Phase 1: Bootstrap (Tasks 1-4)
1. Create `src/bootstrap/mod.rs` — orchestrator, checks bootstrap marker, delegates to OS-specific module
2. Create `src/bootstrap/windows.rs` — Windows bootstrap via WinRM (OpenSSH, mise, nushell, marker)
3. Create `src/bootstrap/linux.rs` — Linux bootstrap via SSH (apt base, mise, nushell, marker)
4. Create `src/bootstrap/boot_wait.rs` — `wait_for_ssh` (poll SSH echo) and `wait_for_winrm` (poll GET /wsman) with timeout

### Phase 2: Build Pipeline (Tasks 4-6)
4. Create `src/build/mod.rs` — port from `utm-dev-cli/src/vm/build.rs`: pre-flight, code sync, tool install, build execution, artifact retrieval
5. Create `src/build/multiarch.rs` — Linux x86_64 cross-compile prep (multiarch, cross-linker, pkg-config paths)
6. Create `src/build/logs.rs` — build log error extraction, `dump_build_log_errors` port

### Phase 3: Boot Wait (Task 7)
7. Create `src/bootstrap/boot_wait.rs` — `wait_for_ssh` (poll SSH echo) and `wait_for_winrm` (poll GET /wsman) with timeout

## Success Criteria

- [x] First `ewe_platform testbed start windows` bootstraps a fresh VM in ~15-25 minutes
- [x] Subsequent `start windows` skips bootstrap (marker file present) in <1 minute
- [x] Failed mid-way bootstrap resumes from last incomplete step (step-by-step idempotency)
- [x] `ewe_platform testbed build windows --project ./tauri-app` produces `.msi` in `.build/windows/x86_64/`
- [x] `ewe_platform testbed build linux --project ./tauri-app` produces `.deb` in `.build/linux/arm64/`
- [x] Build failure extracts error stanzas from log automatically
- [x] Bootstrap is idempotent — re-running on already-bootstrapped VM is a no-op
- [x] Linux multiarch enables x86_64 cross-compilation from ARM64 Linux VM
- [x] Nushell is set as the default shell on both Windows and Linux VMs after bootstrap
- [x] All VM-side commands execute via `nu -c "..."` with consistent behavior across OSes
- [x] Windows Defender exclusions configured (project, .cargo, .rustup directories)
- [x] ARM64 Windows hosts force x86_64 rustup default-host (VS Build Tools workaround)
- [x] Host SSH public key installed in VM (both user + admin paths on Windows)
- [x] Per-phase elapsed timing printed on build completion
- [x] `testbed init` creates `$PWD/.testbed/scripts/<vm>/startup/` and `shutdown/` directories with built-in defaults

## Verification Commands

```bash
cargo test -p foundation_testbed bootstrap::
cargo test -p foundation_testbed build::
```

---

## Implementation Plan

### Bootstrap Architecture with mise + nushell

The bootstrap uses a **two-tier mise strategy**:

**Tier 1 — Bootstrap mise.toml** (hardcoded in the crate):
```toml
# Used during initial VM bootstrap to set up the dev environment
[tools]
rust = "stable"
nu = "latest"
"cargo:cargo-binstall" = "latest"
"cargo:sccache" = "latest"
"cargo:tauri-cli" = "2"

[settings]
cargo_binstall = true
```

**Tier 2 — Project mise.toml** (from the user's project):
```toml
# The user's project declares what it needs
[tools]
rust = "stable"
"cargo:tauri-cli" = "2"
node = "22"
```

The bootstrap installs Tier 1 first. Then at build time, `mise install` picks up the project's Tier 2 `mise.toml`. Tools already installed by Tier 1 are skipped (idempotent).

**Nushell as the execution layer:**

Instead of writing separate bash scripts for Linux and PowerShell scripts for Windows:

```nu
# Cross-platform nushell script (runs on both Windows and Linux VMs)
let project_dir = $env.TESTBED_PROJECT_DIR
let target = $env.TESTBED_TARGET

cd $project_dir
mise install
cargo tauri build --target $target | tee $env.TESTBED_LOG
```

This eliminates the bash/cmd.exe dialect split. `ssh.exec(profile, "nu -c '...'")` works identically on both OSes.

**What mise cannot install (still needs apt/winget):**
- System libraries: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, etc. (Linux apt)
- VS Build Tools (Windows — Microsoft installer, not a mise tool)
- WebView2 Runtime (Windows — Microsoft installer)
- Xvfb, openbox, scrot (Linux — display/X11 tools)
- OpenSSH Server (Windows — Windows capability)

Everything else (rust, cargo tools, nushell, node, python, etc.) goes through mise.

### Build Log Architecture

Both Linux and Windows builds tee their output to a log file in the mounted directory:
- Host path: `$PWD/.testbed/logs/<profile>-build.log`
- Inside VM: `/mnt/project/.testbed/logs/<profile>-build.log`

The log is accessible on the host immediately — users can `tail -f`, `less`, `grep` it without any CLI subcommand. The `testbed logs` command provides convenience wrappers (`--follow`, `--errors`, `--tail N`), but the full raw file is always available on disk.

The `dump_build_log_errors` function greps for error patterns on failure:
```
error[ E | error[: | FAILED | panic | fatal error | mise ERROR |
unresolved external symbol | LNK[0-9]+ | cannot find -l | linker .* not found
```

### Speed Optimizations (ported from utm-dev-cli)

- **`MISE_CARGO_BINSTALL=true`** — uses cargo-binstall to fetch prebuilt binaries from GitHub Releases (~30 sec download vs ~25 min compile for tauri-cli)
- **`CARGO_INCREMENTAL=0`** — improves sccache hit rate
- **`RUSTC_WRAPPER=sccache`** — cross-project artifact caching
- **Preserved VM target dir** — `D:\target` (Windows) or project-local on Linux is preserved across builds

### Code Sync Approach

```
Host: tar czf /tmp/sync.tar.gz --exclude=target --exclude=node_modules --exclude=.git --exclude=.build .
Host: scp -P 2222 /tmp/sync.tar.gz vagrant@localhost:sync.tar.gz
VM:   mkdir -p ~/project && cd ~/project && tar xzf ~/sync.tar.gz
```

tar+scp is used instead of rsync because:
- rsync requires a daemon or SSH rsync binary on the VM
- tar+scp works with any SSH server (Windows 10+ has tar built-in)
- Archive is small (~5-50 MB for typical projects, excludes build artifacts)

### Step-by-Step Idempotent Bootstrap

Bootstrap checks each step individually rather than relying on a single marker file:

```rust
// Each step checks before running
if !is_step_done("openssh") {
    install_openssh(session)?;
    mark_step_done("openssh")?;
}
if !is_step_done("ssh_keys") {
    install_host_pubkey(session)?;
    mark_step_done("ssh_keys")?;
}
// ... etc
```

`is_step_done()` checks for a concrete artifact (service running, file present, binary exists) rather than a marker file. The marker file (`~/.testbed-bootstrapped`) is the final step, written only after all others succeed. If bootstrap fails at step 7 and you re-run, steps 1-6 are skipped (they're already done), and it continues from step 7.

### shell_quote for Safe Shell Escaping

Single-quote wrapping with embedded quote escaping for safe shell argument passing:

```rust
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
```

Usage: `format!("echo {}", shell_quote(user_input))` → `echo 'hello'\''world'`

This prevents injection when embedding arbitrary strings (like SSH public keys) into shell commands. For nushell-wrapped commands, `format!("nu -c {cmd:?}")` uses Rust's Debug formatting which handles escaping, but `shell_quote` is needed for raw bash/cmd.exe invocations during bootstrap (before nushell is installed).

### Per-Phase Elapsed Timing

Build pipeline reports per-phase elapsed time:

```
⌚ sync: 3s | mise install: 8m | cargo tauri build: 22m | total: 30m
```

Each phase wraps `Instant::now()` + `Duration` for human-readable output. Helps users identify bottlenecks.
