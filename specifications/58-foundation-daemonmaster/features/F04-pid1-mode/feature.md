---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F04-pid1-mode"
this_file: "specifications/58-foundation-daemonmaster/features/F04-pid1-mode/feature.md"

status: planned
priority: medium
created: 2026-07-18

depends_on:
  - "F02-process-lifecycle"

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# F04 — PID 1 container mode (zombie reaping + signal forwarding)

## Overview

Run the supervisor as PID 1 in containers with proper zombie reaping and signal
forwarding to managed daemons. Uses `waitid(WNOWAIT)` peek-then-reap on Linux to
avoid races with Tokio's `Child::wait()`, and a stash fallback on other Unix.

[spec](../spec.md).

---

## Part A — Zombie reaping

```rust
// foundation_nativeapis/src/daemon/pid1.rs

use std::sync::atomic::{AtomicBool, Ordering};

static IS_PID1: AtomicBool = AtomicBool::new(false);

/// Check if we are PID 1 (should be called at startup).
pub fn am_i_pid1() -> bool {
    std::process::id() == 1
}

/// Initialize PID 1 mode — installs SIGCHLD handler.
///
/// # Safety
/// Must be called before any child processes are spawned.
pub fn init_pid1_mode(managed_pids: Arc<Mutex<HashSet<u32>>>) -> Pid1Guard {
    IS_PID1.store(true, Ordering::SeqCst);
    install_sigchld_handler(managed_pids);
    Pid1Guard(())
}

pub struct Pid1Guard(());

impl Drop for Pid1Guard {
    fn drop(&mut self) {
        IS_PID1.store(false, Ordering::SeqCst);
        // Restore default SIGCHLD handler.
    }
}

/// SIGCHLD handler — reap zombies.
///
/// On Linux: use `waitid(Id::All, WNOHANG | WNOWAIT | WEXITED)` to peek,
/// then selectively reap only unmanaged PIDs.
///
/// On other Unix: fallback to `waitpid(-1, WNOHANG)` with stash mechanism.
fn install_sigchld_handler(managed_pids: Arc<Mutex<HashSet<u32>>>) {
    // Signal registration uses foundation_nativeapis::signal module.
    // On SIGCHLD delivery:
    //   1. Loop: waitid(WNOHANG | WNOWAIT | WEXITED) → get zombie info
    //   2. If PID is managed (in managed_pids set), skip — let Child::wait() reap
    //   3. If PID is unmanaged, reap via waitid(WNOHANG) to consume
    //   4. If no more zombies, exit loop
    //
    // The WNOWAIT flag is critical: it peeks at the zombie without consuming
    // its exit status, so the managed Child's .wait() still works.
}

#[cfg(target_os = "linux")]
fn reap_zombies(managed_pids: &HashSet<u32>) -> ReapResult {
    use nix::sys::wait::{waitid, Id, WaitPidFlag};

    let mut reaped = Vec::new();
    loop {
        match waitid(Id::All, WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT | WaitPidFlag::WEXITED) {
            Ok(Some(info)) => {
                let pid = info.pid();
                if !managed_pids.contains(&pid) {
                    // Unmanaged zombie — reap it.
                    if let Ok(info) = waitid(Id::Pid(nix::unistd::Pid::from_raw(pid)), WaitPidFlag::WNOHANG) {
                        reaped.push((pid, info.and_then(|i| i.status())));
                    }
                }
                // Managed zombie — let Child::wait() handle it.
            }
            Ok(None) | Err(_) => break, // No more zombies
        }
    }
    ReapResult { reaped }
}

#[cfg(not(target_os = "linux"))]
fn reap_zombies_non_linux(managed_pids: &HashSet<u32>) -> ReapResult {
    use nix::sys::wait::waitpid;
    use nix::sys::wait::WaitPidFlag;

    let mut reaped = Vec::new();
    loop {
        match waitpid(None, Some(WaitPidFlag::WNOHANG)) {
            Ok(nix::libc::pid_t) if nix::libc::pid_t == 0 => break, // No zombies
            Ok(pid) => {
                let pid = pid.as_raw() as u32;
                if !managed_pids.contains(&pid) {
                    reaped.push(pid);
                }
                // If managed, stash to REAPED_STATUSES for Child::wait() recovery.
            }
            Err(nix::errno::Errno::ECHILD) => break,
            Err(_) => continue,
        }
    }
    ReapResult { reaped }
}

struct ReapResult {
    reaped: Vec<(u32, Option<std::process::ExitStatus>)>,
}
```

---

## Part B — Signal forwarding

```rust
/// When running as PID 1, forward signals to managed daemons.
///
/// SIGTERM/SIGINT → forward to all managed daemons (graceful shutdown).
/// SIGHUP → forward to all managed daemons (config reload).
/// SIGQUIT → forward to all managed daemons (immediate shutdown).
pub fn install_signal_forwarder(daemons: Arc<Supervisor>) {
    // Use foundation_nativeapis::signal module.
    // On signal delivery:
    //   1. Determine signal type
    //   2. Iterate managed daemons
    //   3. Forward signal to each daemon's process group (killpg)
}

impl Supervisor {
    /// Forward a signal to all running managed daemons.
    pub fn forward_signal(&self, kind: SignalKind) {
        let daemons = self.daemons.blocking_lock();
        for (_, managed) in daemons.iter() {
            if let Some(pid) = managed.pid {
                #[cfg(unix)]
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(-(pid as i32)),
                    match kind {
                        SignalKind::Sigterm => nix::sys::signal::Signal::SIGTERM,
                        SignalKind::Sigint => nix::sys::signal::Signal::SIGINT,
                        SignalKind::Sighup => nix::sys::signal::Signal::SIGHUP,
                        SignalKind::Sigquit => nix::sys::signal::Signal::SIGQUIT,
                        _ => continue,
                    },
                );
            }
        }
    }
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon -- daemon::pid1
```

Tests cover:
- `am_i_pid1()` returns correct value
- Zombie reaping: spawn a child that spawns a grandchild, kill child → reap grandchild
- Managed PID is NOT reaped by SIGCHLD handler (Child::wait() still works)
- Signal forwarding: send SIGTERM to PID 1 → forwarded to all daemons
- Non-Linux stash mechanism (simulated)
