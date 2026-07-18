---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F07-resource-monitoring"
this_file: "specifications/58-foundation-daemonmaster/features/F07-resource-monitoring/feature.md"

status: planned
priority: medium
created: 2026-07-18

depends_on:
  - "F02-process-lifecycle"

tasks:
  completed: 0
  uncompleted: 3
  total: 3
  completion_percentage: 0%
---

# F07 — CPU/memory enforcement limits + health checks

## Overview

Periodic resource monitoring per daemon with enforcement: kill process if RSS or
CPU% exceeds configured limits. Uses `sysinfo` for cross-platform process stats.
Runs as a TaskIterator that parks via `Depends(timer_readiness)` between checks.

[spec](../spec.md).

---

## Part A — Resource monitoring

```rust
// foundation_nativeapis/src/daemon/resource.rs

use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use foundation_core::valtron::{TaskIterator, TaskStatus, NoAction, EventReadiness};

/// Resource limits for a daemon.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_bytes: Option<u64>,
    pub max_cpu_percent: Option<f32>,
}

impl ResourceLimits {
    pub fn parse_memory(s: &str) -> Result<u64, ResourceError> {
        let s = s.trim().to_lowercase();
        if let Some(n) = s.strip_suffix("mb") {
            Ok(n.trim().parse::<u64>()? * 1024 * 1024)
        } else if let Some(n) = s.strip_suffix("gb") {
            Ok(n.trim().parse::<u64>()? * 1024 * 1024 * 1024)
        } else if let Some(n) = s.strip_suffix("kb") {
            Ok(n.trim().parse::<u64>()? * 1024)
        } else {
            Ok(s.parse::<u64>()?)
        }
    }
}

/// Resource monitor — TaskIterator that checks daemons periodically.
/// Parks via Depends(timer_readiness) between checks — never Delayed.
pub struct ResourceMonitorTask {
    system: System,
    check_interval: std::time::Duration,
    timer: TimerReadiness,
    daemon_pids: BTreeMap<DaemonId, Option<u32>>,
    limits: BTreeMap<DaemonId, ResourceLimits>,
}

impl ResourceMonitorTask {
    pub fn new(
        daemon_pids: BTreeMap<DaemonId, Option<u32>>,
        limits: BTreeMap<DaemonId, ResourceLimits>,
        check_interval: std::time::Duration,
    ) -> Self {
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );
        Self {
            system,
            check_interval,
            timer: TimerReadiness::new(),
            daemon_pids,
            limits,
        }
    }

    /// Check all daemons — returns list of violations.
    fn check_all(&mut self) -> Vec<ResourceViolation> {
        self.system.refresh_processes_specifics(
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );

        let mut violations = Vec::new();
        for (id, pid) in &self.daemon_pids {
            let Some(pid) = pid else { continue };
            let Some(limits) = self.limits.get(id) else { continue };

            if let Some(proc) = self.system.process(sysinfo::Pid::from_raw(*pid as _)) {
                if let Some(max_bytes) = limits.max_memory_bytes {
                    let rss = proc.memory() * 1024;
                    if rss > max_bytes {
                        violations.push(ResourceViolation {
                            id: id.clone(),
                            kind: ResourceViolationKind::Memory { rss_bytes: rss, max_bytes },
                        });
                    }
                }
                if let Some(max_cpu) = limits.max_cpu_percent {
                    let cpu = proc.cpu_usage();
                    if cpu > max_cpu {
                        violations.push(ResourceViolation {
                            id: id.clone(),
                            kind: ResourceViolationKind::Cpu {
                                cpu_percent: cpu, max_percent: max_cpu,
                            },
                        });
                    }
                }
            }
        }
        violations
    }
}

pub struct ResourceViolation {
    pub id: DaemonId,
    pub kind: ResourceViolationKind,
}

pub enum ResourceViolationKind {
    Memory { rss_bytes: u64, max_bytes: u64 },
    Cpu { cpu_percent: f32, max_percent: f32 },
}
```

### A.2 — TaskIterator impl (Depends on timer, never Delayed)

```rust
impl TaskIterator for ResourceMonitorTask {
    type Ready = Vec<ResourceViolation>;
    type Pending = ();
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Check violations.
        let violations = self.check_all();

        // Schedule next check and park on timer readiness.
        self.timer.reset(self.check_interval);
        Some(TaskStatus::Depends(Arc::new(self.timer.clone())))

        // When the timer fires, next_status is called again and
        // check_all runs. Violations are delivered as Ready.
    }
}
```

---

## Part B — Batch process stats (O(N + ΣDᵢ))

```rust
/// Batch compute process stats for all daemons.
pub fn batch_process_stats(
    daemons: &BTreeMap<DaemonId, ManagedDaemon>,
    system: &System,
) -> BTreeMap<DaemonId, DaemonStats> {
    let mut parent_map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (pid, proc) in system.processes() {
        if let Some(parent) = proc.parent() {
            parent_map.entry(parent.as_raw() as u32)
                .or_default().push(pid.as_raw() as u32);
        }
    }

    let mut stats = BTreeMap::new();
    for (id, managed) in daemons {
        let Some(pid) = managed.pid else { continue };
        let (mut total_memory, mut total_cpu, mut count) = (0u64, 0f32, 0u32);
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
cargo test --package foundation_nativeapis --features daemon -- daemon::resource
```
