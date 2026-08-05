---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F02-process-lifecycle"
this_file: "specifications/58-foundation-daemonmaster/features/F02-process-lifecycle/feature.md"

status: complete
priority: high
created: 2026-07-18
completed: 2026-07-31

depends_on:
  - "F01-daemon-config"

tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# F02 — Supervisor + daemon start/stop/monitor + readiness detection + graceful kill

## Overview

The supervisor singleton that manages daemon lifecycles: spawn processes, detect
readiness, monitor health, restart on failure, and perform graceful two-phase kill.
Builds on F01's config and dependency graph.

[spec](../spec.md).

---

## Delivered design (authoritative — reconciles the sketch below)

Shipped in `foundation_nativeapis::daemon::{supervisor,process,platform}`. The
sketch below is the design exploration; the following decisions **win** where
they differ:

1. **Synchronous supervisor** (see F01 Delivered design #1). Readiness is driven
   by `execute` + `collect_one`; there is no async surface.

2. **No `bg_jobs` field.** Background work uses the global
   `foundation_core::valtron::run_background_job` accessor (requires
   `initialize_pool`), not a per-supervisor `Arc<BackgroundJobRegistry>`.

3. **Readiness tasks use `Spawner = BoxedSendExecutionAction`, never `NoAction`**
   (house law: valtron tasks always use `BoxedExecutionAction`/`BoxedSendExecutionAction`).

4. **Readiness reuses F01's `ReadinessConfig`** — there is no separate
   `ReadinessStrategy` enum.

5. **HTTP readiness is a dependency-free `TcpStream` GET probe**, not
   `foundation_netio` (which would risk a crate cycle with `foundation_nativeapis`).

6. **`DaemonExitEvent` carries the exited pid.** The monitor ignores a stale exit
   from a previous instance (old process dying just after a restart spawned a new
   one) by comparing the event pid against the daemon's current pid — without
   this, a restart could trigger a spurious second restart.

7. **The stop path never holds the daemon mutex across the kill/poll waits** (the
   sketch held the lock across `sleep`); locks are released before polling
   liveness and re-acquired to record `Stopped`.

8. **Process-group signalling is isolated in `daemon::platform`** (`process_alive`,
   `terminate`, `kill`): unix via `nix::killpg` on the child's own process group
   (`process_group(0)`), Windows via `TerminateProcess`/`GetExitCodeProcess`.

9. **A background monitor loop** (started by `start_all`) drains the exit queue and
   drives restart-with-backoff (`min(2^n, 30)s`), suppressed while shutting down.

Verification: `cargo test -p foundation_nativeapis --features daemon --test daemon`
(readiness + real-process lifecycle tests, including stop-kills-process,
restart-replaces-pid, and drop-tears-down-all-children).

---

## Part A — Supervisor singleton

```rust
// foundation_nativeapis/src/daemon/supervisor.rs

use std::collections::BTreeMap;
use std::sync::Arc;
use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::BackgroundJobRegistry;

/// Central supervisor — manages all daemon child processes.
pub struct Supervisor {
    config: Arc<DaemonConfig>,
    daemons: std::sync::Mutex<BTreeMap<DaemonId, ManagedDaemon>>,
    shutting_down: std::sync::Arc<std::sync::atomic::AtomicBool>,
    bg_jobs: Arc<BackgroundJobRegistry>,
}

pub struct ManagedDaemon {
    pub id: DaemonId,
    pub def: DaemonDef,
    pub pid: Option<u32>,
    pub status: DaemonStatus,
    pub restart_count: u32,
    pub last_start: Option<std::time::Instant>,
}
```

---

## Part B — Process spawning

### B.1 — Spawn

```rust
impl Supervisor {
    fn spawn_daemon(&self, id: &DaemonId) -> Result<(), SpawnError> {
        let def = self.config.daemons.get(id.name.as_str())
            .ok_or(SpawnError::NotFound(id.clone()))?;

        let mut cmd = std::process::Command::new(&def.run[0]);
        cmd.args(&def.run[1..]);
        if let Some(cwd) = &def.cwd { cmd.current_dir(cwd); }
        for (k, v) in &def.env { cmd.env(k, v); }
        #[cfg(unix)]
        cmd.process_group(0);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;
        let pid = child.id();

        // Submit exit watcher to valtron's BackgroundJobRegistry.
        let exit_queue = self.exit_queue_for(id)?;
        spawn_exit_watcher(child, id.clone(), exit_queue, &self.bg_jobs);

        // Submit stream readers to valtron's BackgroundJobRegistry.
        if let Some(stdout) = child.stdout.take() {
            spawn_stream_reader(id.clone(), stdout, StreamKind::Stdout,
                self.readiness_queue_for(id), &self.bg_jobs);
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_stream_reader(id.clone(), stderr, StreamKind::Stderr,
                self.readiness_queue_for(id), &self.bg_jobs);
        }

        let mut daemons = self.daemons.lock().unwrap();
        let managed = daemons.entry(id.clone()).or_insert_with(|| ManagedDaemon {
            id: id.clone(), def: def.clone(), pid: None,
            status: DaemonStatus::Starting, restart_count: 0, last_start: None,
        });
        managed.pid = pid;
        managed.status = DaemonStatus::Starting;
        managed.last_start = Some(std::time::Instant::now());

        Ok(())
    }
}
```

### B.2 — Exit watcher + stream reader (via BackgroundJobRegistry)

```rust
/// Submitted to BackgroundJobRegistry — owns Child, pushes to queue on exit.
pub fn spawn_exit_watcher(
    child: std::process::Child,
    id: DaemonId,
    queue: Arc<ConcurrentQueue<DaemonExitEvent>>,
    bg_jobs: &BackgroundJobRegistry,
) {
    bg_jobs.submit(move || {
        let exit_status = child.wait().ok();
        let _ = queue.push(DaemonExitEvent { id, exit_status });
    }).expect("BackgroundJobRegistry closed");
}

/// Submitted to BackgroundJobRegistry — reads stream, pushes to readiness queue.
pub fn spawn_stream_reader(
    id: DaemonId,
    stream: std::process::ChildStdout,
    kind: StreamKind,
    readiness_queue: Arc<ConcurrentQueue<OutputLine>>,
    bg_jobs: &BackgroundJobRegistry,
) {
    bg_jobs.submit(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stream).lines() {
            match line {
                Ok(line) => {
                    let _ = readiness_queue.push(OutputLine {
                        text: line.clone(), stream: kind,
                    });
                    tracing::info!(daemon = %id, stream = ?kind, "{line}");
                }
                Err(e) => {
                    tracing::warn!(daemon = %id, "stream read error: {e}");
                    break;
                }
            }
        }
    }).expect("BackgroundJobRegistry closed");
}
```

---

## Part C — Readiness detection (5 strategies)

All strategies are `TaskIterator` implementations that use `Depends(readiness)`
to park — never `Delayed` or sleep. Time-based strategies use `TimerReadiness`
(a `BoolSignal` that fires after a duration). Output-based strategies use
`QueueReadiness`.

> **House law (delivered):** `Spawner = BoxedSendExecutionAction`, never
> `NoAction`. The snippet below predates that rule; the shipped `ReadinessTask`
> uses `BoxedSendExecutionAction`.

```rust
use foundation_core::valtron::{
    TaskIterator, TaskStatus, BoxedSendExecutionAction, EventReadiness, BoolSignal,
};

/// Timer readiness — BoolSignal that fires after a duration.
/// A background thread sleeps for the duration then sets the bool.
pub struct TimerReadiness {
    signal: Arc<std::sync::atomic::AtomicBool>,
}

impl TimerReadiness {
    pub fn new() -> Self {
        Self {
            signal: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Arm the timer. Background thread sleeps then fires the signal.
    pub fn arm(&self, dur: std::time::Duration) {
        let signal = self.signal.clone();
        std::thread::spawn(move || {
            std::thread::sleep(dur);
            signal.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }

    /// Reset and re-arm.
    pub fn reset(&self, dur: std::time::Duration) {
        self.signal.store(false, std::sync::atomic::Ordering::SeqCst);
        self.arm(dur);
    }
}

impl EventReadiness for TimerReadiness {
    fn is_ready(&self, _dur: Option<std::time::Duration>) -> bool {
        self.signal.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// ReadinessTask — TaskIterator for all 5 strategies.
pub struct ReadinessTask {
    strategy: ReadinessStrategy,
    readiness_queue: ReadinessQueue,   // for Output strategy
    timer: TimerReadiness,             // for time-based strategies
    started: std::time::Instant,
    timeout: std::time::Duration,
    done: bool,
}

impl ReadinessTask {
    fn timed_out(&self) -> bool {
        self.started.elapsed() >= self.timeout
    }

    fn http_check(url: &str) -> bool {
        use foundation_netio::HttpClientBuilder;
        HttpClientBuilder::new().build().get(url).send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn port_check(port: u16) -> bool {
        std::net::TcpStream::connect_timeout(
            &("127.0.0.1", port),
            std::time::Duration::from_millis(50),
        ).is_ok()
    }

    fn cmd_check(args: &[String]) -> bool {
        std::process::Command::new(&args[0]).args(&args[1..])
            .status().map(|s| s.success()).unwrap_or(false)
    }
}

impl TaskIterator for ReadinessTask {
    type Ready = Result<(), ReadinessTimeout>;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction; // house law — never NoAction

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.done { return None; }
        if self.timed_out() {
            self.done = true;
            return Some(TaskStatus::Ready(Err(ReadinessTimeout)));
        }

        match &self.strategy {
            ReadinessStrategy::Immediate => {
                self.done = true;
                Some(TaskStatus::Ready(Ok(())))
            }

            // Delay: park on timer, check on wake.
            ReadinessStrategy::Delay(target) => {
                if self.started.elapsed() >= *target {
                    self.done = true;
                    Some(TaskStatus::Ready(Ok(())))
                } else {
                    self.timer.reset(target.saturating_sub(self.started.elapsed()));
                    Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
                }
            }

            // Output: park on queue, check each line.
            ReadinessStrategy::Output(pattern) => {
                while let Some(line) = self.readiness_queue.pop() {
                    if pattern.is_match(&line.text) {
                        self.done = true;
                        return Some(TaskStatus::Ready(Ok(())));
                    }
                }
                Some(TaskStatus::Depends(Arc::new(self.readiness_queue.readiness())))
            }

            // HTTP: check, if not ready park on timer, recheck on wake.
            ReadinessStrategy::Http(url) => {
                let url = url.clone();
                if Self::http_check(&url) {
                    self.done = true;
                    return Some(TaskStatus::Ready(Ok(())));
                }
                self.timer.reset(std::time::Duration::from_secs(1));
                Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
            }

            // Port: check, if not ready park on timer, recheck on wake.
            ReadinessStrategy::Port(port) => {
                let port = *port;
                if Self::port_check(port) {
                    self.done = true;
                    return Some(TaskStatus::Ready(Ok(())));
                }
                self.timer.reset(std::time::Duration::from_millis(100));
                Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
            }

            // Cmd: check, if not ready park on timer, recheck on wake.
            ReadinessStrategy::Cmd(args) => {
                let args = args.clone();
                if Self::cmd_check(&args) {
                    self.done = true;
                    return Some(TaskStatus::Ready(Ok(())));
                }
                self.timer.reset(std::time::Duration::from_secs(1));
                Some(TaskStatus::Depends(Arc::new(self.timer.clone())))
            }
        }
    }
}
```

---

## Part D — Graceful two-phase kill

```rust
impl Supervisor {
    fn stop_daemon(&self, id: &DaemonId) -> Result<(), StopError> {
        let mut daemons = self.daemons.lock().unwrap();
        let managed = daemons.get_mut(id).ok_or(StopError::NotFound(id.clone()))?;
        managed.status = DaemonStatus::Stopping;
        let pid = managed.pid.ok_or(StopError::NotRunning(id.clone()))?;
        let timeout = managed.def.stop_timeout.unwrap_or(10);

        // SIGTERM → poll fast → poll slow → SIGKILL.
        #[cfg(unix)]
        nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(-(pid as i32)),
            nix::sys::signal::Signal::SIGTERM,
        )?;

        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if !Self::process_alive(pid) {
                managed.status = DaemonStatus::Stopped;
                managed.pid = None;
                return Ok(());
            }
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if !Self::process_alive(pid) {
                managed.status = DaemonStatus::Stopped;
                managed.pid = None;
                return Ok(());
            }
        }

        #[cfg(unix)]
        nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(-(pid as i32)),
            nix::sys::signal::Signal::SIGKILL,
        )?;
        std::thread::sleep(std::time::Duration::from_millis(100));

        managed.status = DaemonStatus::Stopped;
        managed.pid = None;
        Ok(())
    }
}
```

---

## Part E — Auto-restart with backoff

```rust
impl Supervisor {
    fn handle_daemon_exit(&self, id: &DaemonId, exit_status: Option<std::process::ExitStatus>) {
        let mut daemons = self.daemons.lock().unwrap();
        if let Some(m) = daemons.get_mut(id) {
            m.pid = None;
            m.status = DaemonStatus::Stopped;
        }

        let managed = daemons.get(id).cloned();
        drop(daemons);

        if let Some(managed) = managed {
            if managed.def.restart
                && !self.shutting_down.load(std::sync::atomic::Ordering::SeqCst)
            {
                if self.check_restart_limit(id) {
                    let backoff = std::cmp::min(2_u64.pow(managed.restart_count), 30);
                    let id = id.clone();
                    let supervisor = self.clone();
                    let bg_jobs = self.bg_jobs.clone();

                    // Submit backoff + restart to BackgroundJobRegistry.
                    bg_jobs.submit(move || {
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
                    }).expect("BackgroundJobRegistry closed");
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
