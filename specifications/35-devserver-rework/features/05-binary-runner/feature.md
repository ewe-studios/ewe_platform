---
feature: "Binary Runner"
description: "BinaryRunnerTask manages std::process::Child lifecycle — spawns to background, uses Depends to park between state transitions"
status: "pending"
priority: "high"
depends_on: ["02-task-operators", "04-cargo-builder"]
estimated_effort: "small"
created: "2026-06-01"
last_updated: "2026-06-14"
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

Replace with `BinaryRunnerTask` — a valtron `TaskIterator` that manages `std::process::Child`.
The spawned process runs **independently** in the background — we hold a `Child` handle only
for `kill()` and `try_wait()`. The task uses `Depends` to park between states, never spinning.

```rust
pub struct BinaryRunnerTask {
    project: ProjectDefinition,
    // Build complete notifications (from ProjectBuilderTask)
    build_queue: Arc<ConcurrentQueue<()>>,
    build_ready: QueueReadiness<()>,
    // Running notification (for SSE reload to know when binary is ready)
    running_tx: mpp::Sender<()>,
    // Child process lifecycle
    child: Option<std::process::Child>,
    state: RunnerState,
    wait_before_reload: Duration,
    // Used for reload-delay polling — avoids spinning
    reload_timer: DurationSleeper,
}

enum RunnerState {
    Idle,                   // Waiting for build complete signal → Depends(build_ready)
    Killing,                // Killing old binary → instant transition to Spawning
    Spawning,               // Spawning new binary → instant transition to ReloadDelay
    ReloadDelay,            // wait_before_reload countdown → Depends(reload_timer)
    Running,                // Binary running, monitoring exit → Depends(build_ready)
}
```

### spawn_binary — process runs in background

```rust
impl BinaryRunnerTask {
    /// Spawn the built binary as a background process.
    ///
    /// The process runs independently — we hold the `Child` handle only for:
    /// - `kill()` when a new build arrives
    /// - `try_wait()` to detect if it exited unexpectedly
    ///
    /// Stdout/stderr are inherited from the devserver process so build logs
    /// appear in the terminal. Stdin is null (no interactive input).
    fn spawn_binary(&self) -> std::io::Result<std::process::Child> {
        let mut args = self.project.run_arguments.clone();
        let binary = args.remove(0); // First arg is the binary path

        std::process::Command::new(&binary)
            .current_dir(&self.project.workspace_root)
            .args(&args)
            .stdin(std::process::Stdio::null())    // No interactive input
            .stdout(std::process::Stdio::inherit()) // Logs go to terminal
            .stderr(std::process::Stdio::inherit())
            .spawn()                                // ← Returns immediately, process runs in background
    }
}
```

### TaskIterator — Depends everywhere, no Pending spam

```rust
impl TaskIterator for BinaryRunnerTask {
    type Ready = ();
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state {
            RunnerState::Idle => {
                // Check for build complete signal
                if self.build_queue.try_pop().is_ok() {
                    // Kill old binary (instant transition — no park needed)
                    if let Some(mut child) = self.child.take() {
                        let _ = child.kill();
                        // Don't wait() — let OS clean up, we're replacing it
                    }
                    // Spawn new binary (instant transition)
                    match self.spawn_binary() {
                        Ok(child) => {
                            self.child = Some(child);
                            self.state = RunnerState::ReloadDelay;
                            self.reload_timer.start(self.wait_before_reload);
                            // Park on reload timer — no spinning
                            return Some(TaskStatus::Depends(Arc::new(self.reload_timer.clone())));
                        }
                        Err(e) => {
                            tracing::error!("failed to spawn binary: {e}");
                            self.state = RunnerState::Idle;
                        }
                    }
                }
                // No build yet — park until build queue has a message
                Some(TaskStatus::Depends(Arc::new(self.build_ready.clone())))
            }
            RunnerState::ReloadDelay => {
                if self.reload_timer.is_done() {
                    // Notify listeners that binary is running
                    let _ = self.running_tx.send(());
                    self.state = RunnerState::Running;
                    // Park waiting for next build signal
                    return Some(TaskStatus::Ready(()));
                }
                // Not yet — park on the timer (executor wakes when duration expires)
                Some(TaskStatus::Depends(Arc::new(self.reload_timer.clone())))
            }
            RunnerState::Running => {
                // Non-blocking check: did child exit?
                if let Some(ref mut child) = self.child {
                    if let Ok(Some(status)) = child.try_wait() {
                        tracing::warn!("binary exited with status: {status:?}");
                        self.child = None;
                    }
                }
                // Back to waiting for next build — park, don't spin
                self.state = RunnerState::Idle;
                Some(TaskStatus::Depends(Arc::new(self.build_ready.clone())))
            }
        }
    }
}
```

### State transition diagram

```
                    build arrives (queue non-empty)
                              ↓
     ┌───────── Depends ──── Idle ──────────────────────────────────────┐
     │                              │                                   │
     │                              │ pop() → kill old → spawn new      │
     │                              ↓                                   │
     │                      ReloadDelay (Depends on timer)              │
     │                              │                                   │
     │                              │ timer expires                     │
     │                              ↓                                   │
     │                         Running ──→ child exited? ──┐            │
     │                              │                      │            │
     │                              │ Ready(())            │            │
     │                              ↓                      ↓            │
     └───── Depends(build_ready) ← Idle ←─────────────────┘            │
                                                                       │
     (next build arrives → depends wakes → loop restarts) ←────────────┘
```

### Why Depends everywhere

| State | Depends on | Why |
|-------|-----------|-----|
| `Idle` | `QueueReadiness(build_queue)` | Park until ProjectBuilderTask pushes a build result |
| `ReloadDelay` | `DurationSleeper` | Park until `wait_before_reload` expires (no spinning) |
| `Running` | `QueueReadiness(build_queue)` | Park until next build arrives OR check child on wake |
| `Killing` | _(instant)_ | `child.kill()` is <1ms — no park needed |
| `Spawning` | _(instant)_ | `Command::spawn()` is <10ms — no park needed |

**No `Pending` spam.** The only states that return `Pending` would be states that need multiple executor ticks to complete — but Killing and Spawning are instant, so they transition immediately to the next state and then `Depends`.

### DurationSleeper

A simple `EventReadiness` impl for time-based waits (avoids `TaskStatus::Wait` spinning):

```rust
#[derive(Clone)]
pub struct DurationSleeper {
    deadline: Arc<Mutex<Option<Instant>>>,
}

impl DurationSleeper {
    pub fn start(&mut self, duration: Duration) {
        *self.deadline.lock().unwrap() = Some(Instant::now() + duration);
    }

    pub fn is_done(&self) -> bool {
        self.deadline.lock().unwrap().is_some_and(|d| Instant::now() >= d)
    }
}

impl EventReadiness for DurationSleeper {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.is_done()
    }
}
```

### Task Breakdown

1. [ ] Define `BinaryRunnerTask` struct with state machine
2. [ ] Implement `spawn_binary()` using `std::process::Command::spawn()` (background process)
3. [ ] Implement `DurationSleeper` as `EventReadiness` for reload delay
4. [ ] Implement `TaskIterator` — `Depends` on build queue, reload timer, never `Pending` spam
5. [ ] Implement child lifecycle management (kill, spawn, try_wait)
6. [ ] Write tests: verify spawn arguments, verify kill-before-spawn ordering

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/runner/mod.rs` | Create — BinaryRunnerTask + DurationSleeper |

---

_Created: 2026-06-01_
