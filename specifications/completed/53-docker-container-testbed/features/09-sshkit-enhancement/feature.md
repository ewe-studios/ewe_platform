---
feature: "Migrate testbed ssh/ module into foundation_sshkit"
description: "Move PowerShell, interactive shell, streaming exec, Vagrant key, and OS-aware shell wrapping from the platform's ssh/ module into foundation_sshkit, then delete the platform module"
status: "in-progress"
priority: "medium"
phase: 3
depends_on: ["13-foundation-sshkit"]
estimated_effort: "medium"
created: 2026-07-15
---
# Feature 09: Migrate testbed ssh/ into foundation_sshkit

## Why

`foundation_testbed/src/vms/ssh/` duplicates sshkit's core operations (connect,
exec, upload, download) with raw `ssh2::Session` calls, then adds platform-specific
extras on top. After the vms/ migration to `foundation_deployment_platform`, these
extras should live in sshkit — the canonical SSH crate.

## What moves into sshkit

| Item | Current location | New location in sshkit |
|------|-----------------|----------------------|
| `exec_ps_windows` — PowerShell with CLIXML stripping | `ssh/mod.rs` | New `src/powershell.rs` — `ps_exec(session, script) → Result<(String, i32)>` |
| `shell` — interactive SSH via CLI | `ssh/mod.rs` | `src/shell.rs` — `interactive(host, port, user)` |
| `exec_streaming` — live output via SSH CLI | `ssh/streaming.rs` | `src/shell.rs` — `exec_streaming(host, port, user, cmd)` |
| `wrap_command` / `wrap_command_bash` / `wrap_command_cmd` — OS-aware wrapping | `ssh/mod.rs` | `command.rs` — methods on `Command`: `.bash()`, `.cmd()`, `.powershell()` |
| Vagrant insecure key in auth chain | `ssh/mod.rs` | `pool.rs` — `authenticate()` already tries keys; add vagrant key path |
| `find_ssh_key` — key file search | `ssh/mod.rs` | `pool.rs` — already duplicated as `default_identity_files()` |
| `VmSession` struct (port, os, user, session) | `ssh/mod.rs` | Not needed — sshkit's `Host` + `Session` replaces it |

## What stays in the platform

Nothing. After migration, `ssh/mod.rs` and `ssh/streaming.rs` are deleted. All 21
consumer files use `foundation_sshkit` directly.

## Consumer migration pattern

```rust
// BEFORE: platform's ssh/ module
use crate::vms::ssh::{connect, exec, upload, VmSession};
let mut session = connect(&profile)?;
let output = exec(&mut session, "echo hello")?;

// AFTER: sshkit
use foundation_sshkit::{connect_session, Host};
let host = Host::parse(&format!("{}@127.0.0.1:{}", profile.user, profile.ssh_port))
    .with_password(profile.pass);
let session = connect_session(&host)?;
let (stdout, _exit) = foundation_sshkit::powershell::ps_exec(&session, script)?;
```

## Scope

### sshkit additions

1. **`src/powershell.rs`** — `ps_exec(session: &Session, script: &str) -> Result<(String, i32)>`
   - Encode script as UTF-16LE + Base64
   - Execute via `powershell -NoProfile -EncodedCommand`
   - Strip CLIXML envelope from stdout

2. **`src/shell.rs`** — `interactive(host: &Host) -> Result<()>`
   - Spawn `ssh -tt` (Linux) or `ssh` (Windows) CLI
   - Inherit stdin/stdout/stderr
   - `exec_streaming(host: &Host, cmd: &str) -> Result<i32>`
   - Spawn `ssh` CLI, pipe stdout/stderr, stream lines to terminal

3. **`command.rs` additions** — `.bash()`, `.cmd()`, `.powershell()` builder methods

4. **`pool.rs` addition** — Vagrant insecure key in `default_identity_files()` search path

### Platform changes

5. Delete `src/ssh/mod.rs` and `src/ssh/streaming.rs`
6. Update 21 consumer files to use sshkit

### Decisions to update

- Decision 13 (foundation_sshkit) — note the enhancement
- Decision 28 (testbed migration) — note ssh/ moved to sshkit rather than platform

## Verification

- `cargo check -p foundation_sshkit` — new modules compile
- `cargo check -p foundation_deployment_platform` — consumers compile with new imports
- `cargo test -p foundation_sshkit --lib` — unit tests pass
- Existing sshkit tests still pass
