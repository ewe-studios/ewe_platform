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

# F08 — Valtron executor tasks + signal task + CompositeReadiness gates

## Overview

Integrate the daemon supervisor with the valtron async executor: supervisor runs as a
valtron task, signal handling uses the existing `SignalTask` + `SignalBus`, readiness
gates use `CompositeReadiness` so daemons can block on multiple readiness sources.

[spec](../spec.md).

---

## Part A — Supervisor as valtron task

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

    /// Run the supervisor in a valtron executor pool.
    pub async fn run(self, stop: StopSignal) -> Result<(), SupervisorError> {
        let supervisor = Supervisor::new(self.config).await?;

        // PID 1 mode if applicable.
        if am_i_pid1() {
            let managed_pids = supervisor.managed_pids();
            let _pid1_guard = init_pid1_mode(managed_pids);
        }

        // Start all daemons (dependency-ordered).
        supervisor.start_all().await?;

        // Install background tasks: file watchers, cron, resource monitor.
        supervisor.install_background().await?;

        // Run until stop signal.
        tokio::select! {
            _ = stop.wait() => {
                tracing::info!("stop signal received, shutting down supervisor");
                supervisor.shutdown().await?;
                Ok(())
            }
            result = supervisor.run_event_loop() => {
                result
            }
        }
    }
}
```

---

## Part B — Signal task integration

Reuse the existing `signal_task()` from foundation_nativeapis:

```rust
impl Supervisor {
    /// Install signal handler using foundation_nativeapis::signal module.
    pub fn install_signal_handler(&self) -> Result<(), SignalError> {
        let (signal_task, signal_bus) = foundation_nativeapis::signal::signal_task()?;

        // Spawn the signal task in valtron.
        let stop = self.shutdown_rx.clone();
        tokio::spawn(async move {
            let mut stream = signal_bus.subscribe();
            loop {
                tokio::select! {
                    event = stream.recv() => {
                        if let Some(event) = event {
                            tracing::info!(?event.kind, "signal received");
                            match event.kind {
                                SignalKind::Sigterm | SignalKind::Sigint => {
                                    // Graceful shutdown.
                                }
                                SignalKind::Sighup => {
                                    // Reload config.
                                }
                                _ => {}
                            }
                        }
                    }
                    _ = stop.changed() => break,
                }
            }
        });

        // Spawn the signal task (reads from OS).
        tokio::spawn(async move {
            signal_task.run().await;
        });

        Ok(())
    }
}
```

---

## Part C — CompositeReadiness gates

Daemons can wait on multiple readiness sources using `CompositeReadiness`:

```rust
use foundation_nativeapis::valtron::CompositeReadiness;

/// Build a composite readiness gate for a daemon.
///
/// A daemon is "ready" when:
/// 1. Its own readiness strategy fires (delay, output, HTTP, port, cmd)
/// 2. All its dependencies are Ready
impl ManagedDaemon {
    pub fn readiness_gate(&self, dependencies: Vec<Arc<ManagedDaemon>>) -> CompositeReadiness {
        let mut gate = CompositeReadiness::new();

        // Own readiness.
        gate.add_source(format!("{}-self", self.id));

        // Dependencies.
        for dep in dependencies {
            gate.add_source(format!("{}-dep-{}", self.id, dep.id));
        }

        gate
    }
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

        // BFS from daemon's PID.
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
- CompositeReadiness: daemon ready when own + deps ready
- CompositeReadiness: daemon NOT ready if any dep not ready
- Batch process stats: correct totals for multi-process daemons
