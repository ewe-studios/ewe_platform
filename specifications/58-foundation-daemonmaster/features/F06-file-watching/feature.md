---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F06-file-watching"
this_file: "specifications/58-foundation-daemonmaster/features/F06-file-watching/feature.md"

status: planned
priority: medium
created: 2026-07-18

depends_on:
  - "F02-process-lifecycle"

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# F06 — File watching + interval/cron scheduling

## Overview

Auto-restart daemons when source files change (native file watcher + poll fallback)
and schedule periodic restarts via cron or fixed intervals. Reuses the existing
foundation_nativeapis file watching and valtron task infrastructure.

[spec](../spec.md).

---

## Part A — File watching

```rust
// foundation_nativeapis/src/daemon/watcher.rs

use foundation_nativeapis::{NativeWatcher, WatchEvent};

/// File watcher for a single daemon.
pub struct DaemonFileWatcher {
    pub daemon_id: DaemonId,
    pub patterns: Vec<String>,       // glob patterns: "src/**/*.rs"
    pub watcher: NativeWatcher,      // inotify/FSEvents/ReadDirectoryChangesW
    mode: WatchMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchMode {
    /// Platform-specific native watcher (inotify, FSEvents, etc.).
    Native,
    /// Periodic filesystem polling (for networked filesystems).
    Poll(std::time::Duration),
    /// Try native, fall back to poll if native setup fails.
    Auto,
}

impl DaemonFileWatcher {
    /// Start watching. Returns a stream of change events.
    pub async fn watch(&self, tx: mpsc::Sender<WatchEvent>) -> Result<(), WatchError> {
        match self.mode {
            WatchMode::Native => self.watch_native(tx).await,
            WatchMode::Poll(interval) => self.watch_poll(interval, tx).await,
            WatchMode::Auto => {
                match self.watch_native(tx.clone()).await {
                    Ok(()) => Ok(()),
                    Err(_) => self.watch_poll(std::time::Duration::from_secs(1), tx).await,
                }
            }
        }
    }

    /// Resolve glob patterns to concrete file paths.
    pub fn resolve_patterns(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for pattern in &self.patterns {
            if let Ok(glob) = glob::glob(pattern) {
                for entry in glob.flatten() {
                    paths.push(entry);
                }
            }
        }
        paths
    }
}
```

### A.2 — Supervisor integration

```rust
impl Supervisor {
    /// Install file watchers for all daemons that have watch patterns.
    pub async fn install_watchers(&self) -> Result<(), WatchError> {
        let daemons = self.daemons.lock().await;
        for (id, managed) in daemons.iter() {
            if !managed.def.watch.is_empty() {
                let watcher = DaemonFileWatcher {
                    daemon_id: id.clone(),
                    patterns: managed.def.watch.clone(),
                    watcher: foundation_nativeapis::native_watcher()?,
                    mode: WatchMode::Auto,
                };
                let (tx, mut rx) = tokio::sync::mpsc::channel(100);
                watcher.watch(tx).await?;

                // Watch → restart loop.
                let supervisor = self.clone();
                let daemon_id = id.clone();
                tokio::spawn(async move {
                    while let Some(_event) = rx.recv().await {
                        tracing::info!(daemon = %daemon_id, "file change detected, restarting");
                        let _ = supervisor.restart_daemon(&daemon_id).await;
                    }
                });
            }
        }
        Ok(())
    }
}
```

---

## Part B — Cron scheduling

```rust
/// Cron-triggered restart for a daemon.
pub struct CronSchedule {
    pub daemon_id: DaemonId,
    pub schedule: cron::Schedule,    // parsed cron expression
    pub retrigger: CronRetrigger,    // when to restart
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronRetrigger {
    /// Restart only if the daemon is stopped.
    IfStopped,
    /// Always restart on schedule (even if running).
    Always,
    /// Restart only if the last run succeeded.
    OnSuccess,
    /// Restart only if the last run failed.
    OnFailure,
}

impl Supervisor {
    /// Install cron schedules for all daemons that have cron_schedule defined.
    pub async fn install_cron_schedules(&self) {
        let daemons = self.daemons.lock().await;
        for (id, managed) in daemons.iter() {
            if let Some(expr) = &managed.def.cron_schedule {
                if let Ok(schedule) = cron::Schedule::from_str(expr) {
                    let cron = CronSchedule {
                        daemon_id: id.clone(),
                        schedule,
                        retrigger: CronRetrigger::Always, // default
                    };
                    self.spawn_cron_task(cron).await;
                }
            }
        }
    }

    async fn spawn_cron_task(&self, cron: CronSchedule) {
        let supervisor = self.clone();
        tokio::spawn(async move {
            for datetime in cron.schedule.upcoming(chrono::Utc).take(1) {
                let delay = datetime - chrono::Utc::now();
                tokio::time::sleep(delay.to_std().unwrap()).await;

                let should_restart = match cron.retrigger {
                    CronRetrigger::IfStopped => {
                        let daemons = supervisor.daemons.lock().await;
                        daemons.get(&cron.daemon_id)
                            .map(|m| m.status == DaemonStatus::Stopped)
                            .unwrap_or(false)
                    }
                    CronRetrigger::Always => true,
                    CronRetrigger::OnSuccess => true, // simplified
                    CronRetrigger::OnFailure => true, // simplified
                };

                if should_restart {
                    let _ = supervisor.restart_daemon(&cron.daemon_id).await;
                }
            }
        });
    }
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon -- daemon::watcher
```

Tests cover:
- File watcher detects changes to matched patterns
- Auto mode: native succeeds → watcher runs; native fails → poll fallback
- Cron schedule parses valid expressions, rejects invalid ones
- CronRetrigger::IfStopped only restarts stopped daemons
- Pattern resolution: glob expansion to concrete paths
