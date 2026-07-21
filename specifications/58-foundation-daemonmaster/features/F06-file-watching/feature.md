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

Auto-restart daemons when source files change using foundation_nativeapis's
existing `NativeWatcher` and `FileWatcherTask`. Cron scheduling uses background
threads that push restart events to the supervisor's event queue.

[spec](../spec.md).

---

## Part A — File watching (uses foundation_nativeapis)

```rust
// foundation_nativeapis/src/daemon/watcher.rs

use foundation_nativeapis::{NativeWatcher, WatchEvent, CompositeReadiness};
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;

/// File watcher for a single daemon — wraps foundation_nativeapis::NativeWatcher.
pub struct DaemonFileWatcher {
    pub daemon_id: DaemonId,
    pub patterns: Vec<String>,       // glob patterns: "src/**/*.rs"
    watcher: NativeWatcher,
    event_queue: Arc<ConcurrentQueue<WatchEvent>>,
}

impl DaemonFileWatcher {
    /// Create a file watcher for a daemon.
    pub fn new(
        daemon_id: DaemonId,
        patterns: Vec<String>,
    ) -> Result<Self, WatchError> {
        let watcher = foundation_nativeapis::native_watcher()?;
        let event_queue = Arc::new(ConcurrentQueue::unbounded());

        // Add each pattern to the watcher.
        for pattern in &patterns {
            watcher.add_watch(pattern)?;
        }

        Ok(Self {
            daemon_id,
            patterns,
            watcher,
            event_queue,
        })
    }

    /// Watch for events — pushes to event_queue, used by FileWatcherTask.
    pub fn poll(&self) -> Result<(), WatchError> {
        for event in self.watcher.poll()? {
            let _ = self.event_queue.push(event);
        }
        Ok(())
    }

    /// Readiness signal — ready when event queue is non-empty.
    pub fn readiness(&self) -> QueueReadiness<WatchEvent> {
        QueueReadiness::new(self.event_queue.clone())
    }
}
```

### A.2 — FileWatcherTask (TaskIterator)

```rust
use foundation_core::valtron::{TaskIterator, TaskStatus, NoAction, QueueReadiness};

/// File watcher task — implements TaskIterator for valtron.
///
/// Returns:
///   Ready(WatchEvent) — file changed
///   Delayed(d) — poll again after d
///   Depends(readiness) — park until file event arrives
pub struct FileWatcherTask {
    watcher: Arc<DaemonFileWatcher>,
    state: FileWatcherState,
}

enum FileWatcherState {
    Init,
    Polling,
}

impl FileWatcherTask {
    pub fn new(watcher: Arc<DaemonFileWatcher>) -> Self {
        Self {
            watcher,
            state: FileWatcherState::Init,
        }
    }
}

impl TaskIterator for FileWatcherTask {
    type Ready = WatchEvent;
    type Pending = ();
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Check for events.
        self.watcher.poll().ok();
        if let Ok(event) = self.watcher.event_queue.pop() {
            return Some(TaskStatus::Ready(event));
        }

        // No events — park on readiness.
        self.state = FileWatcherState::Polling;
        Some(TaskStatus::Depends(Arc::new(self.watcher.readiness())))
    }
}
```

### A.3 — Supervisor integration

The supervisor watches file changes and restarts affected daemons:

```rust
impl Supervisor {
    /// Install file watchers for all daemons that have watch patterns.
    pub fn install_watchers(&mut self) -> Result<(), WatchError> {
        let daemons = self.daemons.lock().unwrap();
        for (id, managed) in daemons.iter() {
            if !managed.def.watch.is_empty() {
                let watcher = Arc::new(DaemonFileWatcher::new(
                    id.clone(),
                    managed.def.watch.clone(),
                )?);
                self.file_watchers.insert(id.clone(), watcher);
            }
        }
        Ok(())
    }

    /// Handle a file change event — restart the affected daemon.
    fn handle_file_change(&self, daemon_id: &DaemonId) {
        tracing::info!(daemon = %daemon_id, "file change detected, restarting");
        let _ = self.restart_daemon(daemon_id);
    }
}
```

---

## Part B — Cron scheduling

Cron tasks run on background threads and push restart events to the supervisor.

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
    pub fn install_cron_schedules(&self) {
        let daemons = self.daemons.lock().unwrap();
        for (id, managed) in daemons.iter() {
            if let Some(expr) = &managed.def.cron_schedule {
                if let Ok(schedule) = cron::Schedule::from_str(expr) {
                    let cron = CronSchedule {
                        daemon_id: id.clone(),
                        schedule,
                        retrigger: CronRetrigger::Always, // default
                    };
                    self.spawn_cron_thread(cron);
                }
            }
        }
    }

    /// Spawn a background job (via valtron's BackgroundJobRegistry) that waits
    /// for the cron time and triggers restart.
    fn spawn_cron_thread(&self, cron: CronSchedule, bg_jobs: &BackgroundJobRegistry) {
        let daemon_id = cron.daemon_id.clone();
        let supervisor = self.clone();

        bg_jobs.submit(move || {
            let next = cron.schedule.upcoming(chrono::Utc).next();
            if let Some(datetime) = next {
                let delay = datetime - chrono::Utc::now();
                if let Ok(dur) = delay.to_std() {
                    std::thread::sleep(dur);

                    let should_restart = match cron.retrigger {
                        CronRetrigger::IfStopped => {
                            let daemons = supervisor.daemons.lock().unwrap();
                            daemons.get(&daemon_id)
                                .map(|m| m.status == DaemonStatus::Stopped)
                                .unwrap_or(false)
                        }
                        CronRetrigger::Always => true,
                        CronRetrigger::OnSuccess => true,
                        CronRetrigger::OnFailure => true,
                    };

                    if should_restart {
                        let _ = supervisor.restart_daemon(&daemon_id);
                    }
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
- NativeWatcher integration: add_watch, poll, event delivery
- FileWatcherTask: returns Ready(event) on file change, Depends on idle
- Cron schedule parses valid expressions, rejects invalid ones
- CronRetrigger::IfStopped only restarts stopped daemons
- Pattern resolution: glob expansion to concrete paths
