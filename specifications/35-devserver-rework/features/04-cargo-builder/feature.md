---
feature: "Cargo Builder"
description: "Replace CargoShellBuilder (tokio::process) with sync spawning + valtron task iteration for cargo check/build"
status: "pending"
priority: "high"
depends_on: ["02-task-operators", "03-native-watching"]
estimated_effort: "medium"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: Cargo Builder

## Problem

Current `CargoShellBuilder` in `crates/devserver/src/builders.rs`:
- Uses `tokio::process::Command` for `cargo check` and `cargo build`
- Listens on `tokio::sync::broadcast::Receiver` for file change events
- Uses `tokio::select!` to wait for either file changes or cancel signal
- `tokio::spawn` loops indefinitely, rebuilding on each Rust file change

## Solution

Replace with `CargoBuilderTask` — a valtron `TaskIterator` that:

1. **Receives file change events** from a `ConcurrentQueue<FileChange>` (pushed by FileWatcherTask)
2. **Runs cargo check** (sync `std::process::Command::output()`)
3. **Runs cargo build** (sync `std::process::Command::output()`)
4. **Signals completion** via a flag/queue that BinaryRunnerTask polls

```rust
pub struct CargoBuilderTask {
    project: ProjectDefinition,
    skip_check: bool,
    stop_on_failure: bool,
    change_queue: Arc<ConcurrentQueue<FileChange>>,
    build_complete_flag: Arc<AtomicBool>,  // signals BinaryRunnerTask
    state: BuilderState,
}

enum BuilderState {
    Idle,
    Checking,
    Building,
    Complete,
    Failed,
}

impl TaskIterator for CargoBuilderTask {
    fn next_status(&mut self) -> Option<TaskStatus<...>> {
        match self.state {
            BuilderState::Idle => {
                // Check for new file changes
                if let Ok(FileChange::Rust(_)) = self.change_queue.pop() {
                    self.state = if self.skip_check {
                        BuilderState::Building
                    } else {
                        BuilderState::Checking
                    };
                }
                Some(TaskStatus::Wait(Duration::from_millis(100)))
            }
            BuilderState::Checking => {
                // Run cargo check (blocking, but fast ~1-5s)
                match self.run_cargo_check() {
                    Ok(()) => { self.state = BuilderState::Building; }
                    Err(e) => {
                        tracing::error!("cargo check failed: {}", e);
                        if self.stop_on_failure {
                            return Some(TaskStatus::Ready(Err(e)));
                        }
                        self.state = BuilderState::Idle;
                    }
                }
                Some(TaskStatus::Wait(Duration::ZERO))
            }
            BuilderState::Building => {
                match self.run_cargo_build() {
                    Ok(()) => {
                        self.build_complete_flag.store(true, Ordering::Relaxed);
                        self.state = BuilderState::Idle;
                    }
                    Err(e) => { /* similar error handling */ }
                }
                Some(TaskStatus::Wait(Duration::ZERO))
            }
            BuilderState::Complete | BuilderState::Failed => {
                Some(TaskStatus::Ready(()))
            }
        }
    }
}
```

### Key Differences from Current Implementation

| Current | New |
|---------|-----|
| `tokio::process::Command.output().await` | `std::process::Command.output()` (blocking) |
| `tokio::select!` for event waiting | Poll `ConcurrentQueue` per tick |
| `broadcast::Receiver<FileChange>` | `ConcurrentQueue<FileChange>` |
| Infinite async loop | `TaskIterator::next_status()` yields per tick |
| `broadcast::Sender<()>` for build complete | `AtomicBool` flag + `ConcurrentQueue<()>` |

### Task Breakdown

1. [ ] Define `CargoBuilderTask` struct with all fields
2. [ ] Implement `run_cargo_check()` using `std::process::Command`
3. [ ] Implement `run_cargo_build()` using `std::process::Command`
4. [ ] Implement `TaskIterator` with state machine (Idle → Checking → Building → Idle)
5. [ ] Handle `stop_on_failure` and `skip_check` flags
6. [ ] Wire up `change_queue` and `build_complete_flag`
7. [ ] Write tests: verify cargo check/build commands are constructed correctly

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/builder/mod.rs` | Create — CargoBuilderTask |

---

_Created: 2026-06-01_
