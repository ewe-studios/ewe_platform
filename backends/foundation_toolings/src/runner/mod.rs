// BinaryRunnerTask — manages std::process::Child lifecycle.
// Spawns to background, uses Depends to park between state transitions.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{EventReadiness, NoSpawner, QueueReadiness, TaskIterator, TaskStatus};
use foundation_nativeapis::valtron::CompositeReadiness;

use crate::types::ProjectDefinition;
use crate::ToolingError;

// -- DurationSleeper: EventReadiness for time-based waits

#[derive(Clone)]
pub struct DurationSleeper {
    deadline: Arc<AtomicU64>, // epoch millis, 0 = not started
}

impl DurationSleeper {
    pub fn new() -> Self {
        Self {
            deadline: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(&mut self, duration: Duration) {
        // Store deadline as epoch millis
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
            + duration.as_millis() as u64;
        self.deadline.store(epoch, Ordering::SeqCst);
    }

    pub fn is_done(&self) -> bool {
        let deadline = self.deadline.load(Ordering::SeqCst);
        if deadline == 0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now >= deadline
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
    build_queue: Arc<ConcurrentQueue<()>>,
    build_ready: QueueReadiness<()>,
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
        build_queue: Arc<ConcurrentQueue<()>>,
        running_tx: Arc<ConcurrentQueue<()>>,
    ) -> Self {
        let build_ready = QueueReadiness::new(build_queue.clone());
        Self {
            project: project.clone(),
            build_queue,
            build_ready,
            running_tx,
            child: None,
            wait_before_reload: project.wait_before_reload,
            reload_sleeper: DurationSleeper::new(),
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

    pub fn readiness(&self) -> Arc<dyn EventReadiness> {
        match self.state {
            RunnerState::Idle => Arc::new(self.build_ready.clone()),
            RunnerState::ReloadDelay => Arc::new(self.reload_sleeper.clone()),
            RunnerState::Running => Arc::new(self.build_ready.clone()),
        }
    }
}

impl TaskIterator for BinaryRunnerTask {
    type Ready = ();
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
                Some(TaskStatus::Depends(Arc::new(self.build_ready.clone())))
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
                Some(TaskStatus::Depends(Arc::new(self.build_ready.clone())))
            }
        }
    }
}
