---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F08-valtron-integration"
this_file: "specifications/58-foundation-daemonmaster/features/F08-valtron-integration/feature.md"

status: planned
priority: high
created: 2026-07-18

depends_on:
  - "F02-process-lifecycle"
  - "F04-pid1-mode"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F08 — Valtron executor integration: Supervisor as TaskIterator + signal task + readiness gates

## Overview

The supervisor IS a `TaskIterator` — not wrapped in `from_future`. It naturally
emits valtron states: `Ready(daemon_status_change)`, `Pending(polling)`,
`Depends(signal_bus)` when parked on OS signals. Signal handling reuses
`SignalTask` from `foundation_nativeapis` directly. Daemon monitors are spawned
as sub-tasks via `ExecutionAction`.

[spec](../spec.md).

---

## Part A — Supervisor as TaskIterator

```rust
// foundation_nativeapis/src/daemon/valtron.rs

use foundation_core::valtron::{
    BoxedSendExecutionAction, EventReadiness, ExecutionEngine,
    QueueReadiness, TaskIterator, TaskStatus, NoAction,
};
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;

/// Supervisor implements TaskIterator — the executor drives it by calling
/// next_status() in a loop. It yields daemon status changes and uses
/// Depends() to park when idle, waking on signals or daemon exits.
pub struct Supervisor {
    /// Managed daemons keyed by DaemonId.
    daemons: BTreeMap<DaemonId, ManagedDaemon>,
    /// Config loaded from F01.
    config: DaemonConfig,
    /// Status broadcast queue — Ready(DaemonStatusEvent) emitted here,
    /// picked up by ConnectRPC streaming consumers.
    status_queue: Arc<ConcurrentQueue<DaemonStatusEvent>>,
    status_readiness: QueueReadiness<DaemonStatusEvent>,
    /// Signal task — parks on OS signals (SIGTERM, SIGINT, SIGHUP).
    signal_task: Option<SignalTask>,
    /// Daemon monitor queues — one per daemon, parks until a daemon exits.
    daemon_queues: BTreeMap<DaemonId, Arc<ConcurrentQueue<DaemonExitEvent>>>,
    /// Shutdown flag — set when SIGTERM/SIGINT received.
    shutting_down: bool,
}

/// Event emitted when a daemon exits.
pub struct DaemonExitEvent {
    pub id: DaemonId,
    pub exit_status: Option<std::process::ExitStatus>,
}

/// Status event for ConnectRPC streaming.
pub struct DaemonStatusEvent {
    pub id: DaemonId,
    pub status: DaemonStatus,
    pub timestamp: std::time::Instant,
}
```

### A.2 — TaskIterator implementation

```rust
impl TaskIterator for Supervisor {
    type Ready = DaemonStatusEvent;
    type Pending = SupervisorPending;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // 1. Check signal task first — OS signals are highest priority.
        if let Some(ref mut signal) = self.signal_task {
            // SignalTask: pops from its queue → Ready, else Depends(handle).
            // We can't call next_status on it directly (it's a separate task),
            // but we CAN check our signal queue directly.
            if let Ok(event) = signal.my_queue.pop() {
                match event.kind {
                    SignalKind::Sigterm | SignalKind::Sigint => {
                        self.shutting_down = true;
                        return Some(TaskStatus::Ready(DaemonStatusEvent {
                            id: DaemonId::system(),
                            status: DaemonStatus::Stopping,
                            timestamp: std::time::Instant::now(),
                        }));
                    }
                    SignalKind::Sighup => {
                        // Reload config — emit as status event.
                        return Some(TaskStatus::Ready(DaemonStatusEvent {
                            id: DaemonId::system(),
                            status: DaemonStatus::Restarting,
                            timestamp: std::time::Instant::now(),
                        }));
                    }
                    _ => {}
                }
            }
        }

        // 2. Check daemon exit queues — any daemon that exited?
        for (id, queue) in &self.daemon_queues {
            if let Ok(exit_event) = queue.pop() {
                // Update daemon status.
                if let Some(managed) = self.daemons.get_mut(id) {
                    managed.status = DaemonStatus::Stopped;
                    managed.pid = None;
                    managed.child = None;
                }

                // Spawn restart task if auto-restart is enabled.
                if let Some(managed) = self.daemons.get(id) {
                    if managed.def.restart && !self.shutting_down {
                        let id = id.clone();
                        let queue = self.daemon_queues.get(&id).unwrap().clone();
                        let config = self.config.clone();
                        // Spawn via ExecutionAction.
                        return Some(TaskStatus::Spawn(Box::new(move |_key, engine| {
                            // Spawn the restart monitor task.
                            let monitor = DaemonMonitorTask::new(id, queue, config);
                            engine.broadcast(monitor.into_box_send_execution_iterator())
                        })));
                    }
                }

                // Emit status change.
                return Some(TaskStatus::Ready(DaemonStatusEvent {
                    id: id.clone(),
                    status: DaemonStatus::Stopped,
                    timestamp: std::time::Instant::now(),
                }));
            }
        }

        // 3. Drain status queue for ConnectRPC — emit any pending events.
        if let Ok(event) = self.status_queue.pop() {
            return Some(TaskStatus::Ready(event));
        }

        // 4. Check if shutting down — if all daemons stopped, we're done.
        if self.shutting_down {
            let all_stopped = self.daemons.values()
                .all(|m| matches!(m.status, DaemonStatus::Stopped));
            if all_stopped {
                return Some(TaskStatus::Ready(DaemonStatusEvent {
                    id: DaemonId::system(),
                    status: DaemonStatus::Stopped,
                    timestamp: std::time::Instant::now(),
                }));
                // Then return None on next call — supervisor is done.
            }
        }

        // 5. Nothing ready — park on the union of signal + daemon exit readiness.
        Some(TaskStatus::Depends(Arc::new(AnyReadiness::new(vec![
            // Signal readiness: ready when signal queue non-empty.
            Arc::new(QueueReadiness::new(
                self.signal_task.as_ref().unwrap().my_queue.clone(),
            )),
            // Daemon exit readiness: ready when any daemon exit queue non-empty.
            // We'd create a composite readiness over all daemon queues.
            Arc::new(DaemonExitReadiness::new(
                self.daemon_queues.values().cloned().collect(),
            )),
        ]))))
    }
}

/// Composite readiness: ready when ANY daemon exit queue is non-empty.
pub struct DaemonExitReadiness {
    queues: Vec<Arc<ConcurrentQueue<DaemonExitEvent>>>,
}

impl DaemonExitReadiness {
    pub fn new(queues: Vec<Arc<ConcurrentQueue<DaemonExitEvent>>>) -> Self {
        Self { queues }
    }
}

impl EventReadiness for DaemonExitReadiness {
    fn is_ready(&self, _dur: Option<std::time::Duration>) -> bool {
        self.queues.iter().any(|q| !q.is_empty() || q.is_closed())
    }
}

/// Pending state — emitted while polling daemons.
#[derive(Debug, Clone)]
pub enum SupervisorPending {
    /// Checking daemon health.
    HealthCheck { checked: usize, total: usize },
    /// Polling for changes.
    Polling,
}
```

---

## Part B — DaemonMonitorTask (spawned sub-task)

Each daemon gets a monitor task spawned via `ExecutionAction`. The monitor
watches the daemon's exit queue and reports when it exits.

```rust
/// Monitor task for a single daemon.
///
/// Parks on the daemon's exit queue via Depends(), returns Ready(exit_event)
/// when the daemon exits.
pub struct DaemonMonitorTask {
    pub id: DaemonId,
    exit_queue: Arc<ConcurrentQueue<DaemonExitEvent>>,
}

impl DaemonMonitorTask {
    pub fn new(
        id: DaemonId,
        exit_queue: Arc<ConcurrentQueue<DaemonExitEvent>>,
    ) -> Self {
        Self { id, exit_queue }
    }
}

impl TaskIterator for DaemonMonitorTask {
    type Ready = DaemonExitEvent;
    type Pending = ();
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Check if daemon has exited.
        if let Ok(event) = self.exit_queue.pop() {
            return Some(TaskStatus::Ready(event));
        }
        // Park until exit event arrives.
        Some(TaskStatus::Depends(Arc::new(
            QueueReadiness::new(self.exit_queue.clone()),
        )))
    }
}
```

### B.2 — Daemon process watcher (bridge from std::process::Child)

When a daemon is spawned, we need to bridge the blocking `Child::wait()` into
valtron's non-blocking model:

```rust
/// Bridges std::process::Child::wait() into a daemon exit queue.
/// Runs on a background thread — pushes to the queue when the child exits.
pub fn spawn_exit_watcher(
    mut child: std::process::Child,
    id: DaemonId,
    exit_queue: Arc<ConcurrentQueue<DaemonExitEvent>>,
) {
    std::thread::spawn(move || {
        let exit_status = child.wait().ok();
        let _ = exit_queue.push(DaemonExitEvent {
            id,
            exit_status,
        });
    });
}
```

This is called when the supervisor spawns a daemon process. The thread owns
the `Child` and pushes to the concurrent queue when it exits — the
`DaemonMonitorTask` parks on that queue via `Depends()`.

---

## Part C — Signal task integration

`SignalTask` from `foundation_nativeapis::signal` is already a proper
`TaskIterator`:

```rust
// From foundation_nativeapis/src/signal/task.rs:

impl TaskIterator for SignalTask {
    type Ready = SignalEvent;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if let Ok(event) = self.my_queue.pop() {
            return Some(TaskStatus::Ready(event));
        }
        Some(TaskStatus::Depends(Arc::new(self.handle.clone())))
    }
}
```

The supervisor accesses the signal task's internal queue directly (via
`signal_task.my_queue`) to check for signals in its own `next_status()`.
No separate valtron task needed for signals — the queue is shared, and
`SignalHandle` (the OS registration) fires into the same queue.

**Setup at supervisor creation:**

```rust
impl Supervisor {
    pub fn new(config: DaemonConfig) -> Result<Self, SignalError> {
        let (signal_task, signal_bus) = foundation_nativeapis::signal_task()?;

        let status_queue = Arc::new(ConcurrentQueue::unbounded());
        let status_readiness = QueueReadiness::new(status_queue.clone());

        Ok(Self {
            daemons: BTreeMap::new(),
            config,
            status_queue,
            status_readiness,
            signal_task: Some(signal_task),
            daemon_queues: BTreeMap::new(),
            shutting_down: false,
        })
    }
}
```

---

## Part D — Running under #[valtron]

The supervisor runs as a TaskIterator under `#[valtron]`:

```rust
use foundation_core::valtron::{valtron, execute, collect_result, Stream};

#[valtron]
fn main() {
    let config = DaemonConfig::load_from_path(&std::env::current_dir().unwrap())?;
    let supervisor = Supervisor::new(config)?;

    // Start all daemons (dependency-ordered, F02).
    // This is done synchronously before the main loop.
    // Each daemon gets:
    //   1. std::process::Child spawned
    //   2. exit_watcher thread spawned (pushes to exit_queue on child exit)
    //   3. DaemonMonitorTask spawned via valtron send()
    supervisor.start_all()?;

    // Drive the supervisor via valtron.
    let stream = execute(supervisor, None)?;

    // Collect all status events — this blocks until supervisor returns None.
    let events: Vec<DaemonStatusEvent> = collect_result(stream);

    // Process final events (shutdown sequence, etc.).
    for event in events {
        tracing::info!("daemon event: {:?} -> {:?}", event.id, event.status);
    }
}
```

### D.2 — start_all (spawns daemons + monitors)

```rust
impl Supervisor {
    /// Start all daemons in dependency order.
    pub fn start_all(&mut self) -> Result<(), DaemonError> {
        let levels = startup_order(&self.config.daemons)?;

        for level in levels {
            // Start daemons in this level concurrently.
            for id in level {
                self.start_daemon(&id)?;
            }
        }

        Ok(())
    }

    fn start_daemon(&mut self, id: &DaemonId) -> Result<(), DaemonError> {
        let def = self.config.daemons.get(id.name.as_str())
            .ok_or(DaemonError::NotFound(id.clone()))?;

        // Spawn the process.
        let mut child = std::process::Command::new(&def.run[0])
            .args(&def.run[1..])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let pid = child.id();

        // Create exit queue for this daemon.
        let exit_queue = Arc::new(ConcurrentQueue::unbounded());

        // Spawn exit watcher on a background thread.
        spawn_exit_watcher(child, id.clone(), exit_queue.clone());

        // Store daemon state.
        self.daemons.insert(id.clone(), ManagedDaemon {
            id: id.clone(),
            def: def.clone(),
            pid: Some(pid),
            status: DaemonStatus::Running,
            restart_count: 0,
            last_start: Some(std::time::Instant::now()),
            child: None, // child moved to exit watcher thread
        });

        // Store exit queue.
        self.daemon_queues.insert(id.clone(), exit_queue);

        // Emit status event.
        let _ = self.status_queue.push(DaemonStatusEvent {
            id: id.clone(),
            status: DaemonStatus::Running,
            timestamp: std::time::Instant::now(),
        });

        Ok(())
    }
}
```

---

## Part E — CompositeReadiness gates

When a daemon depends on other daemons being ready before it starts:

```rust
/// Composite readiness: ready when ALL child readiness signals are ready.
pub struct AllReadiness(Vec<EventReadinessPtr>);

impl AllReadiness {
    pub fn new(children: Vec<EventReadinessPtr>) -> Self {
        Self(children)
    }

    pub fn all(signals: Vec<Arc<dyn EventReadiness + Send + Sync>>) -> Self {
        Self(signals)
    }
}

impl EventReadiness for AllReadiness {
    fn is_ready(&self, dur: Option<std::time::Duration>) -> bool {
        self.0.iter().all(|child| child.is_ready(dur))
    }
}

/// Usage: daemon waits until its own readiness + all deps are ready.
impl Supervisor {
    fn readiness_gate_for(&self, id: &DaemonId) -> Arc<dyn EventReadiness + Send + Sync> {
        let def = &self.config.daemons.get(&id.name).unwrap();
        let mut signals: Vec<Arc<dyn EventReadiness + Send + Sync>> = Vec::new();

        // This daemon's own readiness (from its readiness strategy).
        // Each daemon has a readiness queue that fires when ready.
        if let Some(queue) = self.daemon_readiness_queues.get(id) {
            signals.push(Arc::new(QueueReadiness::new(queue.clone())));
        }

        // Dependency readiness.
        for dep_name in &def.depends {
            let dep_id = DaemonId { namespace: id.namespace.clone(), name: dep_name.clone() };
            if let Some(queue) = self.daemon_readiness_queues.get(&dep_id) {
                signals.push(Arc::new(QueueReadiness::new(queue.clone())));
            }
        }

        Arc::new(AllReadiness::new(signals))
    }
}
```

---

## Part F — Process group stats (batch O(N + ΣDᵢ))

```rust
/// Batch compute process stats for all daemons.
///
/// Builds a parent→children map once (O(N)) then BFS from each root (O(ΣDᵢ)).
pub fn batch_process_stats(
    daemons: &BTreeMap<DaemonId, ManagedDaemon>,
    system: &sysinfo::System,
) -> BTreeMap<DaemonId, DaemonStats> {
    // Build parent→children map once.
    let mut parent_map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (pid, proc) in system.processes() {
        if let Some(parent) = proc.parent() {
            parent_map.entry(parent.as_raw() as u32)
                .or_default()
                .push(pid.as_raw() as u32);
        }
    }

    // BFS from each daemon's PID.
    let mut stats = BTreeMap::new();
    for (id, managed) in daemons {
        let Some(pid) = managed.pid else { continue };
        let mut total_memory = 0u64;
        let mut total_cpu = 0f32;
        let mut count = 0u32;

        let mut queue = vec![pid];
        let mut visited = HashSet::new();
        while let Some(current) = queue.pop() {
            if visited.contains(&current) { continue; }
            visited.insert(current);

            if let Some(proc) = system.process(sysinfo::Pid::from_raw(current as _)) {
                total_memory += proc.memory();
                total_cpu += proc.cpu_usage();
                count += 1;
            }

            if let Some(children) = parent_map.get(&current) {
                queue.extend(children);
            }
        }

        stats.insert(id.clone(), DaemonStats {
            total_memory_kb: total_memory,
            total_cpu_percent: total_cpu,
            process_count: count,
        });
    }
    stats
}

pub struct DaemonStats {
    pub total_memory_kb: u64,
    pub total_cpu_percent: f32,
    pub process_count: u32,
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon -- daemon::valtron
```

Tests cover:
- Supervisor as TaskIterator: next_status returns Ready on signal, Pending while idle
- SignalTask integration: SIGTERM → supervisor emits Stopping, then None
- DaemonMonitorTask: parks on exit queue via Depends, returns Ready on exit
- spawn_exit_watcher: child exit → push to queue → monitor wakes
- CompositeReadiness: AllReadiness only ready when all children ready
- Batch process stats: correct totals for multi-process daemons
- Supervisor under #[valtron]: execute + collect_result drives to completion
