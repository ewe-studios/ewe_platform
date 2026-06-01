---
feature: "Binary Runner"
description: "Replace BinaryApp (tokio::spawn loops) with valtron TaskIterator managing std::process::Child lifecycle"
status: "pending"
priority: "high"
depends_on: ["02-task-operators", "04-cargo-builder"]
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: Binary Runner

## Problem

Current `BinaryApp` in `crates/devserver/src/builders.rs`:
- Uses `tokio::spawn` with infinite `loop { tokio::select! { ... } }`
- Waits for build notifier (`broadcast::Receiver<()>`)
- Kills old binary (`Child::kill()`) and spawns new one
- Uses `std::process::Command::spawn()` (sync) but inside tokio runtime
- Sends running notifications via `broadcast::Sender<().send_in()` (tokio delayed send)

## Solution

Replace with `BinaryRunnerTask` — a valtron `TaskIterator` that manages `std::process::Child`:

```rust
pub struct BinaryRunnerTask {
    project: ProjectDefinition,
    build_complete_flag: Arc<AtomicBool>,
    running_queue: Arc<ConcurrentQueue<()>>,  // signals "binary is running"
    child: Option<std::process::Child>,
    state: RunnerState,
    wait_before_reload: Duration,
    reload_timer_start: Option<Instant>,
}

enum RunnerState {
    WaitingForBuild,
    KillingOldBinary,
    WaitingForKill,
    SpawningNewBinary,
    WaitingForReload,  // timer countdown
    Running,
}

impl TaskIterator for BinaryRunnerTask {
    fn next_status(&mut self) -> Option<TaskStatus<...>> {
        match self.state {
            RunnerState::WaitingForBuild => {
                if self.build_complete_flag.swap(false, Ordering::Relaxed) {
                    self.state = RunnerState::KillingOldBinary;
                    return Some(TaskStatus::Wait(Duration::ZERO));
                }
                Some(TaskStatus::Wait(Duration::from_millis(50)))
            }
            RunnerState::KillingOldBinary => {
                if let Some(child) = self.child.take() {
                    let _ = child.kill();
                    // Don't wait() — let it die, we're replacing it
                }
                self.state = RunnerState::SpawningNewBinary;
                Some(TaskStatus::Wait(Duration::ZERO))
            }
            RunnerState::SpawningNewBinary => {
                match self.spawn_binary() {
                    Ok(child) => {
                        self.child = Some(child);
                        self.state = RunnerState::WaitingForReload;
                        self.reload_timer_start = Some(Instant::now());
                    }
                    Err(e) => {
                        tracing::error!("failed to spawn binary: {}", e);
                        self.state = RunnerState::WaitingForBuild;
                    }
                }
                Some(TaskStatus::Wait(Duration::ZERO))
            }
            RunnerState::WaitingForReload => {
                if Instant::now() - self.reload_timer_start.unwrap() >= self.wait_before_reload {
                    let _ = self.running_queue.push(());
                    self.state = RunnerState::Running;
                }
                Some(TaskStatus::Wait(Duration::from_millis(10)))
            }
            RunnerState::Running => {
                // Check if child exited (optional: auto-restart)
                if let Some(ref mut child) = self.child {
                    if let Some(status) = child.try_wait().ok().flatten() {
                        tracing::warn!("binary exited with status: {:?}", status);
                        self.child = None;
                    }
                }
                self.state = RunnerState::WaitingForBuild;
                Some(TaskStatus::Wait(Duration::from_millis(50)))
            }
        }
    }
}
```

### Task Breakdown

1. [ ] Define `BinaryRunnerTask` struct
2. [ ] Implement `spawn_binary()` using `std::process::Command::spawn()`
3. [ ] Implement `TaskIterator` with state machine
4. [ ] Implement child lifecycle management (kill, wait, spawn)
5. [ ] Implement reload timer (sync `Instant::now()` comparison, no tokio::time::sleep)
6. [ ] Write tests: verify spawn arguments, verify kill-before-spawn ordering

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/runner/mod.rs` | Create — BinaryRunnerTask |

---

_Created: 2026-06-01_
