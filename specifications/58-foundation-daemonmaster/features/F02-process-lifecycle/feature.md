---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F02-process-lifecycle"
this_file: "specifications/58-foundation-daemonmaster/features/F02-process-lifecycle/feature.md"

status: planned
priority: high
created: 2026-07-18

depends_on:
  - "F01-daemon-config"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F02 — Supervisor + daemon start/stop/monitor + readiness detection + graceful kill

## Overview

The supervisor singleton that manages daemon lifecycles: spawn processes, detect
readiness, monitor health, restart on failure, and perform graceful two-phase kill.
Builds on F01's config and dependency graph.

[spec](../spec.md).

---

## Part A — Supervisor singleton

```rust
// foundation_nativeapis/src/daemon/supervisor.rs

use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Central supervisor — manages all daemon child processes.
///
/// Communicated with via ConnectRPC (F03). Only one supervisor per user session.
pub struct Supervisor {
    /// Config loaded from F01.
    config: Arc<DaemonConfig>,
    /// Managed daemons keyed by DaemonId.
    daemons: Mutex<BTreeMap<DaemonId, ManagedDaemon>>,
    /// Shutdown signal — when fired, supervisor stops all daemons.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

/// A daemon under supervision.
pub struct ManagedDaemon {
    pub id: DaemonId,
    pub def: DaemonDef,
    pub pid: Option<u32>,
    pub status: DaemonStatus,
    pub restart_count: u32,
    pub last_start: Option<std::time::Instant>,
    /// Child process handle (for wait + signal).
    pub child: Option<tokio::process::Child>,
    /// Readiness future — resolves when ready or timed out.
    pub ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    Stopped,
    Starting,
    Running,
    Ready,
    Stopping,
    Failed,
    Restarting,
}
```

### A.2 — Startup sequence

```
Supervisor::start()
1. Load config (F01::load_from_path)
2. Compute startup order (F01::startup_order → levels)
3. For each level (concurrent within level):
   a. Spawn daemon process
   b. Wait for readiness (or timeout)
   c. Mark Ready or Failed
4. Install background tasks:
   - Signal handler (SIGTERM/SIGINT → graceful shutdown)
   - SIGHUP → reload config + restart changed daemons
   - Resource monitor ticker (F07)
   - File watcher ticker (F06)
```

---

## Part B — Process spawning

```rust
impl Supervisor {
    /// Spawn a single daemon process.
    async fn spawn_daemon(&self, id: &DaemonId) -> Result<(), SpawnError> {
        let def = self.config.daemons.get(id.name.as_str())
            .ok_or(SpawnError::NotFound(id.clone()))?;

        let mut cmd = tokio::process::Command::new(&def.run[0]);
        cmd.args(&def.run[1..]);

        // Working directory.
        if let Some(cwd) = &def.cwd {
            cmd.current_dir(cwd);
        } else {
            cmd.current_dir(self.config.base_dir());
        }

        // Environment injection.
        for (k, v) in &def.env {
            cmd.env(k, v);
        }

        // Process group: spawn with new session so PID == PGID for group killing.
        #[cfg(unix)]
        cmd.process_group(0);

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;
        let pid = child.id();

        // Capture stdout/stderr for readiness detection and log streaming.
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Update managed daemon state.
        let mut daemons = self.daemons.lock().await;
        let managed = daemons.entry(id.clone()).or_insert_with(|| ManagedDaemon {
            id: id.clone(),
            def: def.clone(),
            pid: None,
            status: DaemonStatus::Starting,
            restart_count: 0,
            last_start: None,
            child: None,
            ready_tx: None,
        });
        managed.pid = pid;
        managed.status = DaemonStatus::Starting;
        managed.child = Some(child);
        managed.last_start = Some(std::time::Instant::now());

        // Spawn stdout/stderr readers for readiness + log streaming.
        if let Some(stdout) = stdout {
            tokio::spawn(Self::read_stream(id.clone(), stdout, StreamKind::Stdout));
        }
        if let Some(stderr) = stderr {
            tokio::spawn(Self::read_stream(id.clone(), stderr, StreamKind::Stderr));
        }

        Ok(())
    }
}
```

---

## Part C — Readiness detection (5 strategies)

All strategies funnel into a `oneshot::Sender<()>` — when fired, the daemon is Ready.

```rust
// foundation_nativeapis/src/daemon/process.rs

use tokio::sync::mpsc;

/// Readiness notifier — pushed to by stream readers.
pub struct ReadinessNotifier {
    tx: tokio::sync::oneshot::Sender<()>,
}

impl ReadinessNotifier {
    pub fn new() -> (Self, tokio::sync::oneshot::Receiver<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (ReadinessNotifier { tx }, rx)
    }

    pub fn mark_ready(self) {
        // oneshot::send consumes self — can only fire once.
        let _ = self.tx.send(());
    }
}

/// Readiness strategies.
pub enum ReadinessStrategy {
    /// Wait N seconds then mark ready.
    Delay(std::time::Duration),
    /// Regex match on stdout/stderr output.
    Output(regex::Regex),
    /// HTTP 2xx on this URL.
    Http(String),
    /// TCP port open (connect succeeds).
    Port(u16),
    /// Command exits 0.
    Cmd(Vec<String>),
    /// Immediate — no check.
    Immediate,
}

impl ReadinessStrategy {
    /// Run the readiness check. Returns when ready or times out.
    pub async fn wait_ready(
        &self,
        notifier: ReadinessNotifier,
        output_rx: mpsc::Receiver<OutputLine>,
        timeout: std::time::Duration,
    ) -> Result<(), ReadinessTimeout> {
        match self {
            Self::Immediate => { notifier.mark_ready(); Ok(()) }
            Self::Delay(d) => {
                tokio::select! {
                    _ = tokio::time::sleep(*d) => { notifier.mark_ready(); Ok(()) }
                    _ = tokio::time::sleep(timeout) => Err(ReadinessTimeout),
                }
            }
            Self::Output(pattern) => {
                // Watch stdout/stderr for regex match.
                let mut output_rx = output_rx;
                tokio::select! {
                    _ = async {
                        while let Some(line) = output_rx.recv().await {
                            if pattern.is_match(&line.text) {
                                notifier.mark_ready();
                                return;
                            }
                        }
                    } => Ok(()),
                    _ = tokio::time::sleep(timeout) => Err(ReadinessTimeout),
                }
            }
            Self::Http(url) => {
                // Simple HTTP GET loop.
                let client = reqwest::Client::new();
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                            if let Ok(resp) = client.get(url).send().await {
                                if resp.status().is_success() {
                                    notifier.mark_ready();
                                    return Ok(());
                                }
                            }
                        }
                        _ = tokio::time::sleep(timeout) => return Err(ReadinessTimeout),
                    }
                }
            }
            Self::Port(port) => {
                tokio::select! {
                    _ = async {
                        loop {
                            if tokio::net::TcpStream::connect(("127.0.0.1", *port)).await.is_ok() {
                                notifier.mark_ready();
                                return;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    } => Ok(()),
                    _ = tokio::time::sleep(timeout) => Err(ReadinessTimeout),
                }
            }
            Self::Cmd(args) => {
                tokio::select! {
                    r = async {
                        loop {
                            let status = tokio::process::Command::new(&args[0])
                                .args(&args[1..]).status().await;
                            if let Ok(s) = status {
                                if s.success() {
                                    notifier.mark_ready();
                                    return;
                                }
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    } => r,
                    _ = tokio::time::sleep(timeout) => Err(ReadinessTimeout),
                }
            }
        }
    }
}

pub struct OutputLine {
    pub text: String,
    pub stream: StreamKind,
}

pub enum StreamKind { Stdout, Stderr }

#[derive(Debug)]
pub struct ReadinessTimeout;
```

### C.2 — Stream reader (feeds readiness + log buffer)

```rust
impl Supervisor {
    /// Read a process stream, feed lines to readiness notifier and log buffer.
    async fn read_stream(
        id: DaemonId,
        stream: tokio::process::ChildStdout, // or ChildStderr
        kind: StreamKind,
    ) {
        use tokio::io::AsyncBufReadExt;
        let mut reader = tokio::io::BufReader::new(stream).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // Log lines are stored for RPC streaming (F03).
            // If readiness is Output-based, the notifier checks here.
            tracing::info!(daemon = %id, stream = ?kind, "{line}");
        }
    }
}
```

---

## Part D — Graceful two-phase kill

```rust
impl Supervisor {
    /// Gracefully stop a daemon: SIGTERM → poll → SIGKILL.
    async fn stop_daemon(&self, id: &DaemonId) -> Result<(), StopError> {
        let mut daemons = self.daemons.lock().await;
        let managed = daemons.get_mut(id).ok_or(StopError::NotFound(id.clone()))?;

        managed.status = DaemonStatus::Stopping;
        let pid = managed.pid.ok_or(StopError::NotRunning(id.clone()))?;
        let timeout = managed.def.stop_timeout.unwrap_or(10);

        // Phase 1: SIGTERM to process group.
        #[cfg(unix)]
        nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(-(pid as i32)),
            nix::sys::signal::Signal::SIGTERM,
        )?;

        // Phase 2: Fast poll (10ms intervals for ~100ms).
        for _ in 0..10 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            if !Self::process_alive(pid) {
                managed.status = DaemonStatus::Stopped;
                managed.child = None;
                return Ok(());
            }
        }

        // Phase 3: Slow poll (50ms intervals) for remainder of timeout.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
        while std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if !Self::process_alive(pid) {
                managed.status = DaemonStatus::Stopped;
                managed.child = None;
                return Ok(());
            }
        }

        // Phase 4: SIGKILL.
        #[cfg(unix)]
        nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(-(pid as i32)),
            nix::sys::signal::Signal::SIGKILL,
        )?;

        // Brief wait for SIGKILL to take effect.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        managed.status = DaemonStatus::Stopped;
        managed.child = None;
        Ok(())
    }

    #[cfg(unix)]
    fn process_alive(pid: u32) -> bool {
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid as i32),
            None,
        ).is_ok()
    }
}
```

### D.2 — Shutdown sequence

```
Supervisor::shutdown()
1. Signal shutdown_tx → all watchers see it
2. Compute shutdown order (reverse topological from F01)
3. For each level (concurrent within level):
   - Stop all daemons in level via stop_daemon()
4. Wait for hook tasks to complete (30s timeout each)
5. Clean up IPC socket (F03)
```

---

## Part E — Auto-restart with backoff

```rust
impl Supervisor {
    /// Monitor a daemon and restart on failure (if restart is enabled).
    async fn monitor_daemon(&self, id: &DaemonId) {
        let mut daemons = self.daemons.lock().await;
        let managed = daemons.get(id).cloned();
        drop(daemons);

        if let Some(managed) = managed {
            // Wait for child to exit.
            if let Some(mut child) = managed.child {
                let exit_status = child.wait().await;
                tracing::info!(daemon = %id, ?exit_status, "daemon exited");

                // Update status.
                let mut daemons = self.daemons.lock().await;
                if let Some(m) = daemons.get_mut(&id) {
                    m.child = None;
                    m.pid = None;
                    m.status = DaemonStatus::Stopped;
                }

                // Auto-restart logic.
                if managed.def.restart {
                    // Check max restarts in a 60s window.
                    let can_restart = self.check_restart_limit(&id);
                    if can_restart {
                        // Exponential backoff: 1s, 2s, 4s, 8s, 16s (cap 30s).
                        let backoff = std::cmp::min(
                            2_u64.pow(managed.restart_count),
                            30,
                        );
                        tracing::info!(daemon = %id, "restarting in {}s", backoff);
                        tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;

                        let mut daemons = self.daemons.lock().await;
                        if let Some(m) = daemons.get_mut(&id) {
                            m.status = DaemonStatus::Restarting;
                            m.restart_count += 1;
                        }
                        drop(daemons);

                        if let Err(e) = self.spawn_daemon(&id).await {
                            tracing::error!(daemon = %id, "restart failed: {e}");
                        }
                    }
                }
            }
        }
    }
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon -- daemon::supervisor
cargo test --package foundation_nativeapis --features daemon -- daemon::process
```

Tests cover:
- Spawn a process, verify PID and status transitions
- Readiness: Delay, Output (regex match), Port, Immediate
- Two-phase kill: SIGTERM → process exits → no SIGKILL
- Two-phase kill: SIGTERM → process ignores → SIGKILL after timeout
- Auto-restart with exponential backoff
- Restart limit enforcement (max 5 in 60s)
- Shutdown in reverse dependency order
