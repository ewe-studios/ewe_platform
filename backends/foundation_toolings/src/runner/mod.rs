// BinaryRunnerTask — manages std::process::Child lifecycle.
// Spawns to background, uses Depends to park between state transitions.

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_compact::Instant;
use foundation_core::valtron::{Clock, Ctx, EventQueue, EventReadiness, TaskIterator, TaskStatus};

use crate::types::ProjectDefinition;

// -- DurationSleeper: EventReadiness for time-based waits

/// A deadline wait that reads the injected [`Ctx`] clock.
///
/// `None` means the wait has not started. `start` stores the deadline, and
/// `is_done` compares the clock to it, so a simulated clock drives the wait.
#[derive(Clone)]
pub struct DurationSleeper {
    clock: Arc<dyn Clock>,
    deadline: Arc<Mutex<Option<Instant>>>,
}

impl DurationSleeper {
    /// A sleeper that reads the clock carried by `ctx`.
    #[must_use]
    pub fn new(ctx: Ctx) -> Self {
        Self {
            clock: ctx.clock(),
            deadline: Arc::new(Mutex::new(None)),
        }
    }

    /// Start a wait of `duration` from the current clock instant.
    pub fn start(&mut self, duration: Duration) {
        let deadline = self.clock.now() + duration;
        let mut slot = self
            .deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = Some(deadline);
    }

    /// Whether the clock has reached the deadline.
    #[must_use]
    pub fn is_done(&self) -> bool {
        let deadline = *self
            .deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        deadline.is_some_and(|deadline| self.clock.now() >= deadline)
    }
}

impl EventReadiness for DurationSleeper {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.is_done()
    }
}

// -- BinaryRunnerTask

pub struct BinaryRunnerTask {
    project: ProjectDefinition,
    /// Build-complete signals. A push wakes the task parked on it.
    build_queue: EventQueue<()>,
    running_tx: Arc<ConcurrentQueue<()>>,
    child: Option<std::process::Child>,
    wait_before_reload: Duration,
    reload_sleeper: DurationSleeper,
    state: RunnerState,
}

#[derive(Clone, Copy, PartialEq)]
enum RunnerState {
    Idle,
    ReloadDelay,
    Running,
}

impl BinaryRunnerTask {
    pub fn new(
        project: ProjectDefinition,
        build_queue: EventQueue<()>,
        running_tx: Arc<ConcurrentQueue<()>>,
        ctx: Ctx,
    ) -> Self {
        Self {
            project: project.clone(),
            build_queue,
            running_tx,
            child: None,
            wait_before_reload: project.wait_before_reload,
            reload_sleeper: DurationSleeper::new(ctx),
            state: RunnerState::Idle,
        }
    }

    /// Spawn the built binary as a background process.
    fn spawn_binary(&self) -> std::io::Result<std::process::Child> {
        let mut args = self.project.run_arguments.clone();
        if args.is_empty() {
            return Err(std::io::Error::other("no run arguments"));
        }
        let binary = args.remove(0);

        Command::new(&binary)
            .current_dir(&self.project.workspace_root)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
    }

    pub fn readiness(&self) -> Arc<dyn EventReadiness + Send + Sync> {
        match self.state {
            RunnerState::Idle | RunnerState::Running => Arc::new(self.build_queue.readiness()),
            RunnerState::ReloadDelay => Arc::new(self.reload_sleeper.clone()),
        }
    }
}

impl TaskIterator for BinaryRunnerTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.state {
            RunnerState::Idle => {
                // Check for build complete signal
                if self.build_queue.pop().is_ok() {
                    // Kill old binary
                    if let Some(mut child) = self.child.take() {
                        let _ = child.kill();
                    }
                    // Spawn new binary
                    match self.spawn_binary() {
                        Ok(child) => {
                            self.child = Some(child);
                            self.state = RunnerState::ReloadDelay;
                            self.reload_sleeper.start(self.wait_before_reload);
                            return Some(TaskStatus::Depends(self.readiness()));
                        }
                        Err(e) => {
                            tracing::error!("failed to spawn binary: {e}");
                            self.state = RunnerState::Idle;
                        }
                    }
                }
                Some(TaskStatus::Depends(self.readiness()))
            }
            RunnerState::ReloadDelay => {
                if self.reload_sleeper.is_done() {
                    let _ = self.running_tx.push(());
                    self.state = RunnerState::Running;
                    return Some(TaskStatus::Ready(()));
                }
                Some(TaskStatus::Depends(self.readiness()))
            }
            RunnerState::Running => {
                // Non-blocking check: did child exit?
                if let Some(ref mut child) = self.child {
                    if let Ok(Some(status)) = child.try_wait() {
                        tracing::warn!("binary exited with status: {status:?}");
                        self.child = None;
                    }
                }
                // Back to waiting for next build
                self.state = RunnerState::Idle;
                Some(TaskStatus::Depends(self.readiness()))
            }
        }
    }
}
