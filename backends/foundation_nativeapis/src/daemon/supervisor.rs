//! The supervisor: spawns, monitors, and stops managed daemons.
//!
//! WHY: Something must own the child processes, bring them up in dependency
//! order, confirm readiness before starting dependents, restart crashed daemons
//! with backoff, and tear everything down in reverse order.
//!
//! WHAT: [`Supervisor`] — an `Arc`-shared singleton that holds the daemon
//! records, per-daemon output queues, a shared exit-event queue, and the
//! precomputed startup levels.
//!
//! HOW: All supervision runs on the valtron pool. Readiness is driven by
//! `execute` + `collect_one` (which blocks *efficiently* on a CondVar, never a
//! spin); process monitoring, stream reading, and restart backoff run on the
//! background job pool. The public API is synchronous because there is no async
//! I/O here — the "async" would be ceremonial and could deadlock a
//! single-threaded driver, so honest blocking that cooperates with the pool is
//! used instead (see spec-58 F02 "sync core" decision).

use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{collect_one, execute, run_background_job};

use super::config::{DaemonDef, ReadinessConfig};
use super::deps::{shutdown_order, startup_order};
use super::error::{DaemonError, SpawnError, StopError};
use super::id::{DaemonId, DaemonStatus, DEFAULT_NAMESPACE};
use super::process::{
    spawn_exit_watcher, spawn_stream_reader, DaemonExitEvent, ManagedDaemon, OutputLine,
    ReadinessTask, StreamKind,
};

/// Default overall timeout for a readiness strategy.
const READINESS_TIMEOUT: Duration = Duration::from_secs(30);

/// The maximum restart backoff in seconds.
const MAX_BACKOFF_SECS: u64 = 30;

/// How often the monitor loop drains exit events.
const MONITOR_POLL: Duration = Duration::from_millis(100);

/// Central process supervisor.
///
/// See the module docs. Construct via [`Supervisor::new`]; drive via
/// [`Supervisor::start_all`]; tear down via [`Supervisor::shutdown`].
pub struct Supervisor {
    namespace: String,
    daemons: Mutex<BTreeMap<DaemonId, ManagedDaemon>>,
    output_queues: BTreeMap<DaemonId, Arc<ConcurrentQueue<OutputLine>>>,
    exit_queue: Arc<ConcurrentQueue<DaemonExitEvent>>,
    levels: Vec<Vec<DaemonId>>,
    shutting_down: Arc<AtomicBool>,
    monitor_started: AtomicBool,
}

impl Supervisor {
    /// Build a supervisor from daemon definitions, validating the dependency graph.
    ///
    /// WHY: Ordering and cycle detection must happen before any process starts.
    ///
    /// WHAT: An `Arc<Supervisor>` with every daemon in `Pending`, its startup
    /// levels precomputed.
    ///
    /// HOW: Runs [`startup_order`] (which errors on a cycle), then builds the
    /// per-daemon records and output queues keyed by `DaemonId`.
    ///
    /// # Errors
    /// Returns [`DaemonError::Cycle`] if the dependencies contain a cycle.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(definitions: Vec<DaemonDef>) -> Result<Arc<Self>, DaemonError> {
        Self::with_namespace(DEFAULT_NAMESPACE, definitions)
    }

    /// Like [`Supervisor::new`] but with an explicit namespace for all ids.
    ///
    /// # Errors
    /// Returns [`DaemonError::Cycle`] on a dependency cycle.
    ///
    /// # Panics
    /// Never panics.
    pub fn with_namespace(
        namespace: impl Into<String>,
        definitions: Vec<DaemonDef>,
    ) -> Result<Arc<Self>, DaemonError> {
        let namespace = namespace.into();
        let levels = startup_order(&definitions)?;

        let mut daemons = BTreeMap::new();
        let mut output_queues = BTreeMap::new();
        for def in definitions {
            let id = DaemonId::new(namespace.clone(), def.name.clone());
            output_queues.insert(id.clone(), Arc::new(ConcurrentQueue::unbounded()));
            daemons.insert(id.clone(), ManagedDaemon::new(id, def));
        }

        // Re-key the precomputed levels into this namespace.
        let levels = levels
            .into_iter()
            .map(|level| {
                level
                    .into_iter()
                    .map(|id| DaemonId::new(namespace.clone(), id.name))
                    .collect()
            })
            .collect();

        Ok(Arc::new(Self {
            namespace,
            daemons: Mutex::new(daemons),
            output_queues,
            exit_queue: Arc::new(ConcurrentQueue::unbounded()),
            levels,
            shutting_down: Arc::new(AtomicBool::new(false)),
            monitor_started: AtomicBool::new(false),
        }))
    }

    /// The namespace all this supervisor's daemons live in.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Start every daemon in dependency order, waiting for each level to become
    /// ready before starting the next, then start the crash monitor.
    ///
    /// WHY: A dependent must not start until its dependencies are ready.
    ///
    /// WHAT: Brings the whole group up; returns once all daemons are `Ready` (or
    /// errors on the first that fails to start or times out).
    ///
    /// HOW: For each level, spawns all daemons (so their readiness overlaps in
    /// wall-clock), then blocks on each daemon's readiness. Finally launches the
    /// background monitor that restarts crashed daemons.
    ///
    /// # Errors
    /// Returns [`DaemonError::Spawn`] if a process fails to spawn, or
    /// [`DaemonError::Readiness`] if one does not become ready in time.
    ///
    /// # Panics
    /// Never panics.
    pub fn start_all(self: &Arc<Self>) -> Result<(), DaemonError> {
        // Clone the level plan up front so we don't hold any lock while spawning.
        let levels = self.levels.clone();
        for level in &levels {
            for id in level {
                self.spawn_daemon(id)?;
            }
            for id in level {
                self.wait_ready(id)?;
            }
        }
        self.spawn_monitor();
        Ok(())
    }

    /// Spawn (or respawn) a single daemon's process.
    ///
    /// WHY: The unit of process creation, shared by start and restart.
    ///
    /// WHAT: Launches the child, wires its stdout/stderr to the output queue and
    /// its exit to the exit queue, and records the pid + `Starting` status.
    ///
    /// HOW: Builds a `std::process::Command`, puts the child in its own process
    /// group (unix) so signals reach the whole tree, pipes its streams, and
    /// submits the exit watcher + stream readers to the background pool.
    ///
    /// # Errors
    /// Returns [`SpawnError::NotFound`] if the id is unknown or [`SpawnError::Io`]
    /// if the process cannot be spawned.
    ///
    /// # Panics
    /// Never panics.
    pub fn spawn_daemon(self: &Arc<Self>, id: &DaemonId) -> Result<(), SpawnError> {
        let def = {
            let daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
            daemons
                .get(id)
                .map(|m| m.def.clone())
                .ok_or_else(|| SpawnError::NotFound(id.clone()))?
        };

        if def.run.is_empty() {
            return Err(SpawnError::Io {
                id: id.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "daemon has an empty `run` command",
                ),
            });
        }

        let mut cmd = Command::new(&def.run[0]);
        cmd.args(&def.run[1..]);
        if let Some(cwd) = &def.cwd {
            cmd.current_dir(cwd);
        }
        for (k, v) in &def.env {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // pgid = pid, so the supervisor can signal the whole subtree.
            cmd.process_group(0);
        }

        let mut child = cmd.spawn().map_err(|source| SpawnError::Io {
            id: id.clone(),
            source,
        })?;
        let pid = child.id();

        let output_queue = self
            .output_queues
            .get(id)
            .cloned()
            .unwrap_or_else(|| Arc::new(ConcurrentQueue::unbounded()));

        if let Some(stdout) = child.stdout.take() {
            spawn_stream_reader(id.clone(), stdout, StreamKind::Stdout, output_queue.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_stream_reader(id.clone(), stderr, StreamKind::Stderr, output_queue.clone());
        }

        spawn_exit_watcher(child, id.clone(), pid, self.exit_queue.clone());

        let mut daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(managed) = daemons.get_mut(id) {
            managed.pid = Some(pid);
            managed.status = DaemonStatus::Starting;
            managed.last_start = Some(Instant::now());
        }
        tracing::info!(daemon = %id, pid, "spawned");
        Ok(())
    }

    /// Block until a daemon becomes ready (or times out), then run its hook.
    ///
    /// WHY: Readiness — not merely "spawned" — is the gate dependents wait on.
    ///
    /// WHAT: Drives the daemon's [`ReadinessTask`] to its single result; marks
    /// the daemon `Ready` and runs `on_ready`, or `Failed` and runs `on_fail`.
    ///
    /// HOW: `execute` schedules the task on the pool; `collect_one` blocks
    /// efficiently (CondVar, no spin) until it yields. `Immediate` short-circuits.
    ///
    /// # Errors
    /// Returns [`DaemonError::Readiness`] on timeout, or a generic
    /// [`DaemonError::NotFound`] if the task closes without a result.
    ///
    /// # Panics
    /// Never panics.
    pub fn wait_ready(self: &Arc<Self>, id: &DaemonId) -> Result<(), DaemonError> {
        let (strategy, hooks) = {
            let daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
            match daemons.get(id) {
                Some(m) => (m.def.readiness.clone(), m.def.hooks.clone()),
                None => return Err(DaemonError::NotFound(id.name.clone())),
            }
        };

        if matches!(strategy, ReadinessConfig::Immediate) {
            self.mark_ready(id);
            self.run_hook(hooks.as_ref().and_then(|h| h.on_ready.clone()), id, "on_ready");
            return Ok(());
        }

        let output_queue = self
            .output_queues
            .get(id)
            .cloned()
            .unwrap_or_else(|| Arc::new(ConcurrentQueue::unbounded()));
        let task = ReadinessTask::new(id.clone(), strategy, output_queue, READINESS_TIMEOUT);

        let stream = execute(task, None)
            .map_err(|e| DaemonError::NotFound(format!("readiness scheduling failed: {e}")))?;

        match collect_one(stream) {
            Some(Ok(())) => {
                self.mark_ready(id);
                self.run_hook(
                    hooks.as_ref().and_then(|h| h.on_ready.clone()),
                    id,
                    "on_ready",
                );
                Ok(())
            }
            Some(Err(timeout)) => {
                self.mark_status(id, DaemonStatus::Failed);
                self.run_hook(hooks.as_ref().and_then(|h| h.on_fail.clone()), id, "on_fail");
                Err(DaemonError::Readiness(timeout))
            }
            None => Err(DaemonError::NotFound(format!(
                "readiness task for {id} closed without a result"
            ))),
        }
    }

    /// Gracefully stop a daemon: SIGTERM, wait, then SIGKILL if needed.
    ///
    /// WHY: A clean shutdown lets the process flush and exit; the KILL escalation
    /// bounds how long teardown can hang.
    ///
    /// WHAT: Moves the daemon to `Stopped` and clears its pid.
    ///
    /// HOW: Sends SIGTERM to the process group, polls liveness (fast then slow)
    /// up to `stop_timeout`, escalates to SIGKILL, and runs `on_stop`. The daemon
    /// mutex is never held across the polling waits.
    ///
    /// # Errors
    /// Returns [`StopError::NotFound`] / [`StopError::NotRunning`] / [`StopError::Signal`].
    ///
    /// # Panics
    /// Never panics.
    pub fn stop_daemon(self: &Arc<Self>, id: &DaemonId) -> Result<(), StopError> {
        let (pid, timeout, hooks) = {
            let mut daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
            let managed = daemons
                .get_mut(id)
                .ok_or_else(|| StopError::NotFound(id.clone()))?;
            let pid = managed.pid.ok_or_else(|| StopError::NotRunning(id.clone()))?;
            managed.status = DaemonStatus::Stopping;
            (pid, managed.def.stop_timeout.unwrap_or(10), managed.def.hooks.clone())
        };

        super::platform::terminate(pid).map_err(|message| StopError::Signal {
            id: id.clone(),
            message,
        })?;

        // Fast poll for a quick clean exit, then slow poll up to the timeout.
        if !Self::await_exit(pid, 10, Duration::from_millis(10))
            && !Self::await_exit(
                pid,
                (timeout * 1000 / 50).max(1) as usize,
                Duration::from_millis(50),
            )
        {
            tracing::warn!(daemon = %id, pid, "graceful stop timed out, sending SIGKILL");
            super::platform::kill(pid).map_err(|message| StopError::Signal {
                id: id.clone(),
                message,
            })?;
            Self::await_exit(pid, 20, Duration::from_millis(10));
        }

        {
            let mut daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(managed) = daemons.get_mut(id) {
                managed.status = DaemonStatus::Stopped;
                managed.pid = None;
            }
        }
        self.run_hook(hooks.as_ref().and_then(|h| h.on_stop.clone()), id, "on_stop");
        tracing::info!(daemon = %id, "stopped");
        Ok(())
    }

    /// Restart a daemon: run `on_retry`, stop it if running, respawn, wait ready.
    ///
    /// # Errors
    /// Returns any [`DaemonError`] from stop/spawn/readiness.
    ///
    /// # Panics
    /// Never panics.
    pub fn restart_daemon(self: &Arc<Self>, id: &DaemonId) -> Result<(), DaemonError> {
        let hooks = {
            let daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
            daemons.get(id).and_then(|m| m.def.hooks.clone())
        };
        self.run_hook(hooks.as_ref().and_then(|h| h.on_retry.clone()), id, "on_retry");

        // A not-running daemon is fine to restart; only propagate real errors.
        match self.stop_daemon(id) {
            Ok(()) | Err(StopError::NotRunning(_)) => {}
            Err(e) => return Err(DaemonError::Stop(e)),
        }
        self.spawn_daemon(id)?;
        self.wait_ready(id)?;
        {
            let mut daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(managed) = daemons.get_mut(id) {
                managed.restart_count += 1;
            }
        }
        Ok(())
    }

    /// Stop every daemon in reverse dependency order.
    ///
    /// WHY: Nothing should lose a dependency it is still using; dependents die
    /// first.
    ///
    /// WHAT: Sets the shutting-down flag (stopping the monitor and suppressing
    /// auto-restart) and stops all daemons.
    ///
    /// HOW: Iterates [`shutdown_order`] of the startup levels; stop errors are
    /// logged, not propagated, so teardown always completes.
    ///
    /// # Panics
    /// Never panics.
    pub fn shutdown(self: &Arc<Self>) {
        self.shutting_down.store(true, Ordering::SeqCst);
        for level in shutdown_order(&self.levels) {
            for id in level {
                match self.stop_daemon(&id) {
                    Ok(()) | Err(StopError::NotRunning(_)) => {}
                    Err(e) => tracing::warn!(daemon = %id, "shutdown stop error: {e}"),
                }
            }
        }
    }

    /// Snapshot a daemon's current record.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn snapshot(&self, id: &DaemonId) -> Option<ManagedDaemon> {
        self.daemons
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .cloned()
    }

    /// All daemon ids known to this supervisor, in name order.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn ids(&self) -> Vec<DaemonId> {
        self.daemons
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .keys()
            .cloned()
            .collect()
    }

    /// Resolve a bare name into this supervisor's namespaced id.
    #[must_use]
    pub fn id_for(&self, name: &str) -> DaemonId {
        DaemonId::new(self.namespace.clone(), name)
    }

    // -- internals --------------------------------------------------------

    fn mark_ready(&self, id: &DaemonId) {
        self.mark_status(id, DaemonStatus::Ready);
        tracing::info!(daemon = %id, "ready");
    }

    fn mark_status(&self, id: &DaemonId, status: DaemonStatus) {
        let mut daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(managed) = daemons.get_mut(id) {
            managed.status = status;
        }
    }

    /// Poll `process_alive` up to `attempts` times, `interval` apart.
    /// Returns `true` as soon as the process is gone.
    fn await_exit(pid: u32, attempts: usize, interval: Duration) -> bool {
        for _ in 0..attempts {
            if !super::platform::process_alive(pid) {
                return true;
            }
            std::thread::sleep(interval);
        }
        !super::platform::process_alive(pid)
    }

    /// Fire a lifecycle hook command on the background pool (best effort).
    fn run_hook(&self, cmd: Option<String>, id: &DaemonId, name: &'static str) {
        let Some(cmd) = cmd else { return };
        let log_id = id.clone();
        let id = id.clone();
        let submit = run_background_job(move || {
            let result = if cfg!(windows) {
                Command::new("cmd").args(["/C", &cmd]).status()
            } else {
                Command::new("sh").args(["-c", &cmd]).status()
            };
            match result {
                Ok(status) if status.success() => {
                    tracing::debug!(daemon = %id, hook = name, "hook ok");
                }
                Ok(status) => {
                    tracing::warn!(daemon = %id, hook = name, "hook exited {status}");
                }
                Err(e) => tracing::warn!(daemon = %id, hook = name, "hook failed: {e}"),
            }
        });
        if let Err(e) = submit {
            tracing::warn!(daemon = %log_id, hook = name, "could not submit hook: {e}");
        }
    }

    /// Launch the crash monitor once: drains exit events and restarts daemons.
    fn spawn_monitor(self: &Arc<Self>) {
        if self.monitor_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let supervisor = self.clone();
        if let Err(e) = run_background_job(move || supervisor.monitor_loop()) {
            tracing::error!("could not start daemon monitor: {e}");
            self.monitor_started.store(false, Ordering::SeqCst);
        }
    }

    fn monitor_loop(self: Arc<Self>) {
        while !self.shutting_down.load(Ordering::SeqCst) {
            while let Ok(event) = self.exit_queue.pop() {
                self.handle_daemon_exit(&event);
            }
            std::thread::sleep(MONITOR_POLL);
        }
    }

    /// React to a child exit: mark stopped, then restart with backoff if allowed.
    fn handle_daemon_exit(self: &Arc<Self>, event: &DaemonExitEvent) {
        let id = &event.id;
        let (should_restart, restart_count, max_restarts, hooks) = {
            let mut daemons = self.daemons.lock().unwrap_or_else(|p| p.into_inner());
            let Some(managed) = daemons.get_mut(id) else {
                return;
            };
            // Ignore a stale exit from a previous instance (e.g. the old process
            // dying just after a restart already spawned a new one).
            if managed.pid != Some(event.pid) {
                return;
            }
            // A deliberate stop already cleared the pid and set Stopping/Stopped.
            if matches!(managed.status, DaemonStatus::Stopping | DaemonStatus::Stopped) {
                managed.pid = None;
                return;
            }
            managed.pid = None;
            managed.status = DaemonStatus::Stopped;
            (
                managed.def.restart,
                managed.restart_count,
                managed.def.max_restarts,
                managed.def.hooks.clone(),
            )
        };

        self.run_hook(hooks.as_ref().and_then(|h| h.on_exit.clone()), id, "on_exit");

        if self.shutting_down.load(Ordering::SeqCst) || !should_restart {
            return;
        }

        if let Some(max) = max_restarts {
            if restart_count >= max {
                tracing::error!(daemon = %id, "exceeded {max} restarts, marking failed");
                self.mark_status(id, DaemonStatus::Failed);
                self.run_hook(hooks.as_ref().and_then(|h| h.on_fail.clone()), id, "on_fail");
                return;
            }
        }

        let backoff = (1u64 << restart_count.min(5)).min(MAX_BACKOFF_SECS);
        tracing::warn!(
            daemon = %id,
            status = ?event.exit_status,
            "exited; restarting in {backoff}s (attempt {})",
            restart_count + 1
        );

        let supervisor = self.clone();
        let log_id = id.clone();
        let id = id.clone();
        let submit = run_background_job(move || {
            std::thread::sleep(Duration::from_secs(backoff));
            if supervisor.shutting_down.load(Ordering::SeqCst) {
                return;
            }
            supervisor.mark_status(&id, DaemonStatus::Restarting);
            {
                let mut daemons = supervisor.daemons.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(managed) = daemons.get_mut(&id) {
                    managed.restart_count += 1;
                }
            }
            if let Err(e) = supervisor.spawn_daemon(&id) {
                tracing::error!(daemon = %id, "restart spawn failed: {e}");
                return;
            }
            if let Err(e) = supervisor.wait_ready(&id) {
                tracing::error!(daemon = %id, "restart readiness failed: {e}");
            }
        });
        if let Err(e) = submit {
            tracing::error!(daemon = %log_id, "could not schedule restart: {e}");
        }
    }
}
