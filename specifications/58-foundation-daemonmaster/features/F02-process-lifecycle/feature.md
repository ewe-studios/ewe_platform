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
use concurrent_queue::ConcurrentQueue;

/// Central supervisor — manages all daemon child processes.
///
/// Communicated with via ConnectRPC (F03). Only one supervisor per user session.
pub struct Supervisor {
    /// Config loaded from F01.
    config: Arc<DaemonConfig>,
    /// Managed daemons keyed by DaemonId.
    daemons: std::sync::Mutex<BTreeMap<DaemonId, ManagedDaemon>>,
    /// Shutdown signal — atomic bool, checked in event loop.
    shutting_down: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// A daemon under supervision.
pub struct ManagedDaemon {
    pub id: DaemonId,
    pub def: DaemonDef,
    pub pid: Option<u32>,
    pub status: DaemonStatus,
    pub restart_count: u32,
    pub last_start: Option<std::time::Instant>,
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
    fn spawn_daemon(&self, id: &DaemonId) -> Result<(), SpawnError> {
        let def = self.config.daemons.get(id.name.as_str())
            .ok_or(SpawnError::NotFound(id.clone()))?;

        let mut cmd = std::process::Command::new(&def.run[0]);
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

        // Spawn exit watcher thread — pushes to exit queue when child exits.
        let exit_queue = self.exit_queue_for(id)?;
        spawn_exit_watcher(child, id.clone(), exit_queue);

        // Spawn stream readers for stdout/stderr → feeds readiness queue.
        if let Some(stdout) = child.stdout.take() {
            spawn_stream_reader(id.clone(), stdout, StreamKind::Stdout, self);
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_stream_reader(id.clone(), stderr, StreamKind::Stderr, self);
        }

        // Update managed daemon state.
        let mut daemons = self.daemons.lock().unwrap();
        let managed = daemons.entry(id.clone()).or_insert_with(|| ManagedDaemon {
            id: id.clone(),
            def: def.clone(),
            pid: None,
            status: DaemonStatus::Starting,
            restart_count: 0,
            last_start: None,
        });
        managed.pid = pid;
        managed.status = DaemonStatus::Starting;
        managed.last_start = Some(std::time::Instant::now());

        Ok(())
    }
}
```

### B.2 — Exit watcher (background thread)

```rust
/// Bridges std::process::Child::wait() into a valtron concurrent queue.
/// Runs on a background thread — pushes to the queue when the child exits.
pub fn spawn_exit_watcher(
    child: std::process::Child,
    id: DaemonId,
    queue: Arc<ConcurrentQueue<DaemonExitEvent>>,
) {
    std::thread::spawn(move || {
        let exit_status = child.wait().ok();
        let _ = queue.push(DaemonExitEvent { id, exit_status });
    });
}
```

### B.3 — Stream reader (background thread)

```rust
/// Reads a process stream, pushes lines to the daemon's readiness queue
/// and log buffer.
pub fn spawn_stream_reader(
    id: DaemonId,
    stream: std::process::ChildStdout,
    kind: StreamKind,
    supervisor: &Supervisor,
) {
    let queue = supervisor.readiness_queue_for(&id);
    std::thread::spawn(move || {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    let _ = queue.push(OutputLine {
                        text: line.clone(),
                        stream: kind,
                    });
                    tracing::info!(daemon = %id, stream = ?kind, "{line}");
                }
                Err(e) => {
                    tracing::warn!(daemon = %id, "stream read error: {e}");
                    break;
                }
            }
        }
    });
}
```

---

## Part C — Readiness detection (5 strategies)

All strategies are modeled as `TaskIterator` implementations — the executor
drives them via `next_status()`, returning `Ready(())`, `Pending`, `Delayed`,
or `Depends` naturally.

```rust
// foundation_nativeapis/src/daemon/process.rs

use foundation_core::valtron::{
    NoAction, TaskIterator, TaskStatus,
    QueueReadiness, AnyReadiness, EventReadiness,
};
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;

/// Readiness queue — output stream pushes lines here, ReadinessTask pops them.
pub struct ReadinessQueue {
    queue: Arc<ConcurrentQueue<OutputLine>>,
}

impl ReadinessQueue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(ConcurrentQueue::unbounded()),
        }
    }

    pub fn push(&self, line: OutputLine) {
        let _ = self.queue.push(line);
    }

    pub fn pop(&self) -> Option<OutputLine> {
        self.queue.pop().ok()
    }

    pub fn readiness(&self) -> QueueReadiness<OutputLine> {
        QueueReadiness::new(self.queue.clone())
    }
}

pub struct OutputLine {
    pub text: String,
    pub stream: StreamKind,
}

pub enum StreamKind { Stdout, Stderr }

/// Readiness strategy — declarative, maps to ReadinessTask at runtime.
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

/// Readiness task — implements TaskIterator so valtron drives it properly.
pub struct ReadinessTask {
    strategy: ReadinessStrategy,
    queue: ReadinessQueue,
    started: std::time::Instant,
    timeout: std::time::Duration,
    // Strategy-specific state.
    state: ReadinessState,
}

enum ReadinessState {
    Init,
    Delaying,
    HttpChecking { last_check: std::time::Instant },
    PortChecking { last_probe: std::time::Instant },
    CmdChecking { last_cmd: std::time::Instant },
    Done,
}

#[derive(Debug)]
pub enum ReadinessPending {
    Waiting { remaining: std::time::Duration },
    Checking,
}

#[derive(Debug)]
pub struct ReadinessTimeout;
```

### C.2 — TaskIterator impl

```rust
impl ReadinessTask {
    pub fn new(
        strategy: ReadinessStrategy,
        queue: ReadinessQueue,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            strategy,
            queue,
            started: std::time::Instant::now(),
            timeout,
            state: ReadinessState::Init,
        }
    }

    fn elapsed(&self) -> std::time::Duration {
        self.started.elapsed()
    }

    fn timed_out(&self) -> bool {
        self.elapsed() >= self.timeout
    }
}

impl TaskIterator for ReadinessTask {
    type Ready = Result<(), ReadinessTimeout>;
    type Pending = ReadinessPending;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if matches!(self.state, ReadinessState::Done) {
            return None;
        }

        // Check timeout on every poll.
        if self.timed_out() {
            self.state = ReadinessState::Done;
            return Some(TaskStatus::Ready(Err(ReadinessTimeout)));
        }

        match &self.strategy {
            ReadinessStrategy::Immediate => {
                self.state = ReadinessState::Done;
                Some(TaskStatus::Ready(Ok(())))
            }

            ReadinessStrategy::Delay(target) => {
                if self.elapsed() >= *target {
                    self.state = ReadinessState::Done;
                    Some(TaskStatus::Ready(Ok(())))
                } else {
                    let remaining = target.saturating_sub(self.elapsed());
                    self.state = ReadinessState::Delaying;
                    Some(TaskStatus::Delayed(remaining))
                }
            }

            ReadinessStrategy::Output(pattern) => {
                // Check queue for matching output line.
                while let Some(line) = self.queue.pop() {
                    if pattern.is_match(&line.text) {
                        self.state = ReadinessState::Done;
                        return Some(TaskStatus::Ready(Ok(())));
                    }
                }
                // No matching line — park on queue readiness.
                Some(TaskStatus::Depends(Arc::new(self.queue.readiness())))
            }

            ReadinessStrategy::Http(url) => {
                let poll_interval = std::time::Duration::from_secs(1);
                match &mut self.state {
                    ReadinessState::Init => {
                        if Self::http_check(url) {
                            self.state = ReadinessState::Done;
                            return Some(TaskStatus::Ready(Ok(())));
                        }
                        self.state = ReadinessState::HttpChecking {
                            last_check: std::time::Instant::now(),
                        };
                        Some(TaskStatus::Delayed(poll_interval))
                    }
                    ReadinessState::HttpChecking { last_check } => {
                        if last_check.elapsed() >= poll_interval {
                            *last_check = std::time::Instant::now();
                            if Self::http_check(url) {
                                self.state = ReadinessState::Done;
                                return Some(TaskStatus::Ready(Ok(())));
                            }
                        }
                        let remaining = poll_interval.saturating_sub(last_check.elapsed());
                        Some(TaskStatus::Delayed(remaining))
                    }
                    _ => {
                        self.state = ReadinessState::Done;
                        None
                    }
                }
            }

            ReadinessStrategy::Port(port) => {
                let probe_interval = std::time::Duration::from_millis(100);
                match &mut self.state {
                    ReadinessState::Init => {
                        if Self::port_check(*port) {
                            self.state = ReadinessState::Done;
                            return Some(TaskStatus::Ready(Ok(())));
                        }
                        self.state = ReadinessState::PortChecking {
                            last_probe: std::time::Instant::now(),
                        };
                        Some(TaskStatus::Delayed(probe_interval))
                    }
                    ReadinessState::PortChecking { last_probe } => {
                        if last_probe.elapsed() >= probe_interval {
                            *last_probe = std::time::Instant::now();
                            if Self::port_check(*port) {
                                self.state = ReadinessState::Done;
                                return Some(TaskStatus::Ready(Ok(())));
                            }
                        }
                        let remaining = probe_interval.saturating_sub(last_probe.elapsed());
                        Some(TaskStatus::Delayed(remaining))
                    }
                    _ => {
                        self.state = ReadinessState::Done;
                        None
                    }
                }
            }

            ReadinessStrategy::Cmd(args) => {
                let check_interval = std::time::Duration::from_secs(1);
                match &mut self.state {
                    ReadinessState::Init => {
                        if Self::cmd_check(args) {
                            self.state = ReadinessState::Done;
                            return Some(TaskStatus::Ready(Ok(())));
                        }
                        self.state = ReadinessState::CmdChecking {
                            last_cmd: std::time::Instant::now(),
                        };
                        Some(TaskStatus::Delayed(check_interval))
                    }
                    ReadinessState::CmdChecking { last_cmd } => {
                        if last_cmd.elapsed() >= check_interval {
                            *last_cmd = std::time::Instant::now();
                            if Self::cmd_check(args) {
                                self.state = ReadinessState::Done;
                                return Some(TaskStatus::Ready(Ok(())));
                            }
                        }
                        let remaining = check_interval.saturating_sub(last_cmd.elapsed());
                        Some(TaskStatus::Delayed(remaining))
                    }
                    _ => {
                        self.state = ReadinessState::Done;
                        None
                    }
                }
            }
        }
    }
}

impl ReadinessTask {
    fn http_check(url: &str) -> bool {
        // Uses foundation_netio's HTTP client (not reqwest).
        use foundation_netio::HttpClientBuilder;
        match HttpClientBuilder::new().build().get(url).send() {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    fn port_check(port: u16) -> bool {
        std::net::TcpStream::connect_timeout(
            &("127.0.0.1", port),
            std::time::Duration::from_millis(50),
        ).is_ok()
    }

    fn cmd_check(args: &[String]) -> bool {
        std::process::Command::new(&args[0])
            .args(&args[1..])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
```

---

## Part D — Graceful two-phase kill

```rust
impl Supervisor {
    /// Gracefully stop a daemon: SIGTERM → poll → SIGKILL.
    fn stop_daemon(&self, id: &DaemonId) -> Result<(), StopError> {
        let mut daemons = self.daemons.lock().unwrap();
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
            std::thread::sleep(std::time::Duration::from_millis(10));
            if !Self::process_alive(pid) {
                managed.status = DaemonStatus::Stopped;
                managed.pid = None;
                return Ok(());
            }
        }

        // Phase 3: Slow poll (50ms intervals) for remainder of timeout.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if !Self::process_alive(pid) {
                managed.status = DaemonStatus::Stopped;
                managed.pid = None;
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
        std::thread::sleep(std::time::Duration::from_millis(100));

        managed.status = DaemonStatus::Stopped;
        managed.pid = None;
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
1. Set shutting_down flag → all watchers see it
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
    /// Called by the supervisor's TaskIterator when a daemon exit event arrives.
    fn handle_daemon_exit(&self, id: &DaemonId, exit_status: Option<std::process::ExitStatus>) {
        let mut daemons = self.daemons.lock().unwrap();
        if let Some(m) = daemons.get_mut(id) {
            m.pid = None;
            m.status = DaemonStatus::Stopped;
        }

        // Auto-restart logic.
        let managed = daemons.get(id).cloned();
        drop(daemons);

        if let Some(managed) = managed {
            if managed.def.restart && !self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
                // Check max restarts in a 60s window.
                let can_restart = self.check_restart_limit(id);
                if can_restart {
                    // Exponential backoff: 1s, 2s, 4s, 8s, 16s (cap 30s).
                    let backoff = std::cmp::min(
                        2_u64.pow(managed.restart_count),
                        30,
                    );
                    tracing::info!(daemon = %id, "restarting in {}s", backoff);

                    // Schedule restart via backoff watcher thread.
                    let id = id.clone();
                    let supervisor = self.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(backoff));
                        let mut daemons = supervisor.daemons.lock().unwrap();
                        if let Some(m) = daemons.get_mut(&id) {
                            m.status = DaemonStatus::Restarting;
                            m.restart_count += 1;
                        }
                        drop(daemons);

                        if let Err(e) = supervisor.spawn_daemon(&id) {
                            tracing::error!(daemon = %id, "restart failed: {e}");
                        }
                    });
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
