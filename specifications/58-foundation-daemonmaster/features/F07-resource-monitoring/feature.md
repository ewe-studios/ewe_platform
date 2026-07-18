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
Runs as a TaskIterator that the supervisor's event loop drives.

[spec](../spec.md).

---

## Part A — Resource monitoring

```rust
// foundation_nativeapis/src/daemon/resource.rs

use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use foundation_core::valtron::{TaskIterator, TaskStatus, NoAction};

/// Resource limits for a daemon.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum RSS in bytes. Kill if exceeded.
    pub max_memory_bytes: Option<u64>,
    /// Maximum CPU percentage (0-100 per core). Kill if exceeded.
    pub max_cpu_percent: Option<f32>,
}

impl ResourceLimits {
    /// Parse from config string (e.g. "500MB", "1GB", "200%").
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
pub struct ResourceMonitorTask {
    system: System,
    check_interval: std::time::Duration,
    last_check: std::time::Instant,
    daemon_pids: BTreeMap<DaemonId, Option<u32>>,
    limits: BTreeMap<DaemonId, ResourceLimits>,
}

#[derive(Debug, Clone)]
pub enum ResourcePending {
    Waiting { remaining: std::time::Duration },
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
            last_check: std::time::Instant::now(),
            daemon_pids,
            limits,
        }
    }

    /// Check all daemons — returns list of daemons that exceeded limits.
    fn check_all(&mut self) -> Vec<ResourceViolation> {
        // Refresh process stats.
        self.system.refresh_processes_specifics(
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );

        let mut violations = Vec::new();

        for (id, pid) in &self.daemon_pids {
            let Some(pid) = pid else { continue };
            let Some(limits) = self.limits.get(id) else { continue };

            if let Some(proc) = self.system.process(sysinfo::Pid::from_raw(*pid as _)) {
                // Check memory.
                if let Some(max_bytes) = limits.max_memory_bytes {
                    let rss = proc.memory() * 1024; // sysinfo returns KB
                    if rss > max_bytes {
                        violations.push(ResourceViolation {
                            id: id.clone(),
                            kind: ResourceViolationKind::Memory {
                                rss_bytes: rss,
                                max_bytes,
                            },
                        });
                    }
                }

                // Check CPU.
                if let Some(max_cpu) = limits.max_cpu_percent {
                    let cpu = proc.cpu_usage();
                    if cpu > max_cpu {
                        violations.push(ResourceViolation {
                            id: id.clone(),
                            kind: ResourceViolationKind::Cpu {
                                cpu_percent: cpu,
                                max_percent: max_cpu,
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

### A.2 — TaskIterator impl

```rust
impl TaskIterator for ResourceMonitorTask {
    type Ready = Vec<ResourceViolation>;
    type Pending = ResourcePending;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let elapsed = self.last_check.elapsed();
        if elapsed >= self.check_interval {
            self.last_check = std::time::Instant::now();
            let violations = self.check_all();
            Some(TaskStatus::Ready(violations))
        } else {
            let remaining = self.check_interval.saturating_sub(elapsed);
            Some(TaskStatus::Delayed(remaining))
        }
    }
}
```

---

## Part B — Batch process stats (O(N + ΣDᵢ))

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
cargo test --package foundation_nativeapis --features daemon -- daemon::resource
```

Tests cover:
- Memory limit parsing: "500MB" → 524288000, "1GB" → 1073741824
- ResourceMonitorTask: returns violations when limits exceeded, Delayed while waiting
- Resource monitor detects process exceeding memory limit
- Resource monitor detects process exceeding CPU limit
- Daemon is killed when limit is exceeded, status updated to Failed
- Batch process stats: correct totals for multi-process daemons
