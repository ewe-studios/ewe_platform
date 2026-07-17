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

# F08 — Valtron executor integration + signal task + readiness gates

## Overview

The supervisor runs as a valtron task. Signal handling reuses `signal_task()` and
`SignalBus` from `foundation_nativeapis` directly. Readiness gates compose with
valtron's task model so daemon startup blocks on both their own readiness and
their dependencies being ready.

[spec](../spec.md).

---

## Part A — Supervisor as a valtron task

```rust
// foundation_nativeapis/src/daemon/valtron.rs

use foundation_nativeapis::valtron::{BoxedExecutionAction, StopSignal};

/// Supervisor runs as a valtron task.
pub struct DaemonSupervisorTask {
    config: DaemonConfig,
}

impl DaemonSupervisorTask {
    pub fn new(config: DaemonConfig) -> Self {
        Self { config }
    }

    /// Run the supervisor as a valtron task.
    ///
    /// This is the main event loop — driven by valtron's executor.
    /// Exits when StopSignal fires or all daemons exit permanently.
    pub async fn run(self, stop: StopSignal) -> Result<(), SupervisorError> {
        let supervisor = Supervisor::new(self.config).await?;

        // PID 1 mode if applicable.
        if am_i_pid1() {
            let managed_pids = supervisor.managed_pids();
            let _pid1_guard = init_pid1_mode(managed_pids);
        }

        // Install signal handler (uses foundation_nativeapis::signal).
        supervisor.install_signal_handler()?;

        // Start all daemons (dependency-ordered).
        supervisor.start_all().await?;

        // Install background tasks: file watchers (F06), cron (F06),
        // resource monitor (F07).
        supervisor.install_background().await?;

        // Run the event loop — valtron drives this until StopSignal fires.
        supervisor.run_event_loop(stop).await
    }
}
```

### A.2 — Event loop (valtron-driven)

```rust
impl Supervisor {
    /// The main event loop. Runs under valtron until the stop signal fires
    /// or all daemons have permanently exited.
    ///
    /// This is a single long-running valtron task that drives:
    /// - Daemon exit monitoring (restart logic)
    /// - Signal event dispatch
    /// - Status change broadcasting (for ConnectRPC streaming)
    async fn run_event_loop(&self, stop: StopSignal) -> Result<(), SupervisorError> {
        use futures::{select, FutureExt};

        // Subscribe to signal events (from foundation_nativeapis::signal).
        let mut signal_rx = self.signal_bus.subscribe();

        // Subscribe to daemon status changes (for ConnectRPC streaming).
        let mut status_rx = self.status_bus.subscribe();

        loop {
            select! {
                // Stop signal from valtron: graceful shutdown.
                _ = stop.wait().fuse() => {
                    tracing::info!("stop signal received, shutting down supervisor");
                    return self.shutdown().await;
                }

                // OS signal delivery (SIGTERM, SIGINT, SIGHUP).
                event = signal_rx.recv().fuse() => {
                    if let Some(event) = event {
                        match event.kind {
                            SignalKind::Sigterm | SignalKind::Sigint => {
                                tracing::info!("received {:?}, initiating shutdown", event.kind);
                                return self.shutdown().await;
                            }
                            SignalKind::Sighup => {
                                tracing::info!("received SIGHUP, reloading config");
                                self.reload_config().await?;
                            }
                            _ => {}
                        }
                    }
                }

                // Daemon status change — broadcast to ConnectRPC watchers.
                status = status_rx.recv().fuse() => {
                    if let Some(status) = status {
                        self.broadcast_status_change(status).await;
                    }
                }

                // Periodic health check ticker (valtron sleep).
                _ = foundation_core::valtron::time::sleep(
                    std::time::Duration::from_secs(10),
                ).fuse() => {
                    self.health_check_all().await;
                }
            }
        }
    }
}
```

---

## Part B — Signal task integration

Uses `foundation_nativeapis::signal::signal_task()` and `SignalBus` directly —
no new signal infrastructure needed.

```rust
impl Supervisor {
    /// Install the OS signal handler.
    ///
    /// Uses foundation_nativeapis::signal_task() which:
    /// 1. Registers OS signal handlers (sigaction/SetConsoleCtrlHandler)
    /// 2. Creates a SignalBus that delivers signals to subscribers
    /// 3. Returns a SignalTask that must be driven (reads from OS)
    pub fn install_signal_handler(&self) -> Result<(), SignalError> {
        let (signal_task, signal_bus) = foundation_nativeapis::signal_task()?;

        // Store the bus for the event loop to subscribe.
        self.signal_bus = signal_bus;

        // The signal_task is driven as part of the supervisor's valtron task —
        // it reads OS signal delivery and feeds the bus.
        // We store it and run it inside the event loop.
        // The task runs inline: when a signal fires, the bus gets notified.
        signal_task.run_once()?; // registers handlers

        Ok(())
    }
}
```

The `signal_task.run_once()` call installs the OS-level handlers. After that,
signals arrive on `signal_bus.subscribe()` and are dispatched in the event loop
(Part A.2). No separate valtron task needed for signals — the bus is shared.

---

## Part C — CompositeReadiness gates

Daemons can wait on multiple readiness sources. The supervisor uses valtron's
task model to compose: a daemon is "ready" when its own strategy fires AND
all its dependencies are ready.

```rust
/// Build a composite readiness gate for a daemon.
pub async fn wait_composite_ready(
    daemon_id: &DaemonId,
    own_ready: OneshotReceiver<()>,    // daemon's own readiness (from F02)
    deps: Vec<WatchReceiver<DaemonStatus>>, // dependency status watchers
) -> Result<(), ReadinessTimeout> {
    use futures::{select, FutureExt};

    // All dependencies must be Ready (or at least not Failed/Stopped).
    let deps_ready = async {
        loop {
            let all_ready = deps.iter().all(|rx| {
                matches!(rx.borrow(), DaemonStatus::Ready)
            });
            if all_ready {
                return;
            }
            // Wait for any dep to change, then recheck.
            futures::future::select_all(
                deps.iter().map(|rx| rx.changed()).collect::<Vec<_>>()
            ).await;
        }
    };

    select! {
        _ = own_ready.fuse() => {
            // Own strategy fired — check deps.
            deps_ready.await;
        }
        _ = deps_ready.fuse() => {
            // All deps ready — wait for own.
            own_ready.await.ok();
        }
        _ = foundation_core::valtron::time::sleep(
            std::time::Duration::from_secs(60),
        ).fuse() => {
            return Err(ReadinessTimeout);
        }
    }

    Ok(())
}
```

---

## Part D — Process group stats (batch O(N + ΣDᵢ))

Instead of O(D × N) per-daemon process tree checks:

```rust
/// Batch compute process stats for all daemons.
///
/// Builds a parent→children map once (O(N)) then BFS from each root (O(ΣDᵢ)).
pub fn batch_process_stats(
    daemons: &BTreeMap<DaemonId, ManagedDaemon>,
    system: &System,
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
- Supervisor runs as valtron task, responds to StopSignal
- Signal integration: SIGTERM → supervisor shutdown
- Signal integration: SIGHUP → config reload
- CompositeReadiness: daemon ready when own + deps ready
- CompositeReadiness: daemon NOT ready if any dep not ready
- Batch process stats: correct totals for multi-process daemons
