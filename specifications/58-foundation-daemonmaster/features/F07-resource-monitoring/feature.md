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

Periodic resource monitoring per daemon with enforcement: kill process if RSS or CPU%
exceeds configured limits. Uses `sysinfo` for cross-platform process stats.

[spec](../spec.md).

---

## Part A — Resource monitoring

```rust
// foundation_nativeapis/src/daemon/resource.rs

use sysinfo::{ProcessRefreshKind, RefreshKind, System};

/// Resource limits for a daemon.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum RSS in bytes. Kill if exceeded.
    pub max_memory_bytes: Option<u64>,
    /// Maximum CPU percentage (0-100 per core). Kill if exceeded.
    /// E.g., 80 = 80% of one core; 200 = 200% of one core (2 cores).
    pub max_cpu_percent: Option<f32>,
}

impl ResourceLimits {
    /// Parse from config string (e.g., "500MB", "1GB", "200%").
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

/// Resource monitor — periodic ticker that checks all daemons.
pub struct ResourceMonitor {
    system: Mutex<System>,
    check_interval: std::time::Duration,
}

impl ResourceMonitor {
    /// Run the monitoring loop.
    pub async fn run(&self, supervisor: &Supervisor) {
        let mut interval = tokio::time::interval(self.check_interval);
        loop {
            interval.tick().await;
            self.check_all(supervisor).await;
        }
    }

    async fn check_all(&self, supervisor: &Supervisor) {
        let mut sys = self.system.lock();
        sys.refresh_processes_specifics(
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory(),
        );

        let daemons = supervisor.daemons.lock().await;
        for (id, managed) in daemons.iter() {
            if managed.status != DaemonStatus::Running && managed.status != DaemonStatus::Ready {
                continue;
            }

            let Some(pid) = managed.pid else { continue };

            if let Some(proc) = sys.process(sysinfo::Pid::from_raw(pid as _)) {
                // Check memory.
                if let Some(limit) = managed.def.memory_limit.as_ref() {
                    let max_bytes = ResourceLimits::parse_memory(limit).ok();
                    if let Some(max) = max_bytes {
                        let rss = proc.memory() * 1024; // sysinfo returns KB
                        if rss > max {
                            tracing::warn!(
                                daemon = %id,
                                rss_mb = rss / (1024 * 1024),
                                limit_mb = max / (1024 * 1024),
                                "memory limit exceeded, killing daemon"
                            );
                            drop(daemons);
                            supervisor.kill_daemon(id).await;
                            return;
                        }
                    }
                }

                // Check CPU.
                if let Some(max_cpu) = managed.def.cpu_limit {
                    let cpu = proc.cpu_usage();
                    if cpu > max_cpu {
                        tracing::warn!(
                            daemon = %id,
                            cpu_percent = cpu,
                            limit_percent = max_cpu,
                            "CPU limit exceeded, killing daemon"
                        );
                        drop(daemons);
                        supervisor.kill_daemon(id).await;
                        return;
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
cargo test --package foundation_nativeapis --features daemon -- daemon::resource
```

Tests cover:
- Memory limit parsing: "500MB" → 524288000, "1GB" → 1073741824
- Resource monitor detects process exceeding memory limit
- Resource monitor detects process exceeding CPU limit
- Daemon is killed when limit is exceeded, status updated to Failed
