//! Shell command execution with Valtron streaming integration.
//!
//! WHY: Build steps (cargo build, docker build, etc.) need structured progress
//! updates via Valtron's execution engine. A streaming interface lets callers
//! observe process lifecycle events as they happen.
//!
//! WHAT: `ShellExecutor` builder that spawns a shell command and returns a
//! Valtron `Stream<ShellDone, ShellPending>` iterator via `execute()`.
//!
//! HOW: Wraps a `std::process::Command` in a `TaskIterator` implementation,
//! executes it via `valtron::execute()`, and streams `Spawning` -> `Running`
//! -> `Success`/`Failed` states.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use foundation_core::valtron::{
    execute, NoAction, Stream, TaskIterator, TaskStatus,
};

// Re-export collect_one from foundation_core for use by providers
pub use foundation_core::valtron::collect_one;

use crate::error::DeploymentError;

// ===========================================================================
// Pending Type: Progress states during execution
// ===========================================================================

/// Progress states yielded while the shell command is running.
#[derive(Debug, Clone)]
pub enum ShellPending {
    /// Process is about to spawn.
    Spawning,
    /// Process spawned successfully, waiting for output.
    Running {
        /// OS process ID.
        pid: u32,
    },
}

// ===========================================================================
// Done Type: Final completion states
// ===========================================================================

/// Completion states yielded when the shell command finishes.
#[derive(Debug, Clone)]
pub enum ShellDone {
    /// Process completed successfully (exit code 0).
    Success {
        /// Exit code (always 0 for this variant).
        exit_code: i32,
        /// Captured stdout.
        stdout: String,
        /// Captured stderr.
        stderr: String,
    },
    /// Process failed (non-zero exit or signal).
    Failed {
        /// Exit code, or `None` if killed by signal.
        exit_code: Option<i32>,
        /// Captured stdout.
        stdout: String,
        /// Captured stderr.
        stderr: String,
    },
}

// ===========================================================================
// ShellExecutor Builder
// ===========================================================================

/// Builder for executing shell commands via Valtron.
///
/// The `execute()` method schedules the task on Valtron's thread pool
/// and returns an iterator that yields `Stream<ShellDone, ShellPending>`.
pub struct ShellExecutor {
    command: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    working_dir: Option<PathBuf>,
}

impl ShellExecutor {
    /// Create a new executor for the given command.
    pub fn new(command: &str) -> Self {
        Self {
            command: command.to_string(),
            args: Vec::new(),
            envs: Vec::new(),
            working_dir: None,
        }
    }

    /// Append a single argument.
    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.args.push(arg.as_ref().to_string_lossy().into_owned());
        self
    }

    /// Append multiple arguments.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for a in args {
            self.args.push(a.as_ref().to_string_lossy().into_owned());
        }
        self
    }

    /// Set an environment variable for the child process.
    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(mut self, key: K, val: V) -> Self {
        self.envs.push((
            key.as_ref().to_string_lossy().into_owned(),
            val.as_ref().to_string_lossy().into_owned(),
        ));
        self
    }

    /// Set the working directory for the child process.
    pub fn current_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.working_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Execute the shell command and return a streaming iterator.
    ///
    /// The returned iterator yields `Stream<ShellDone, ShellPending>` values:
    /// - `Stream::Pending(ShellPending::Spawning)` — about to start
    /// - `Stream::Pending(ShellPending::Running { pid })` — process running
    /// - `Stream::Next(ShellDone::Success { .. })` — completed with exit code 0
    /// - `Stream::Next(ShellDone::Failed { .. })` — completed with non-zero exit
    ///
    /// # Errors
    ///
    /// Returns `Err` if Valtron fails to schedule the task on its thread pool.
    pub fn execute(
        self,
    ) -> Result<impl Iterator<Item = Stream<ShellDone, ShellPending>>, DeploymentError> {
        let task = ShellTask {
            command: self.command,
            args: self.args,
            envs: self.envs,
            working_dir: self.working_dir,
            phase: ShellPhase::NotStarted,
        };
        execute(task, None).map_err(|e| DeploymentError::ExecutorError {
            reason: e.to_string(),
        })
    }
}

// ===========================================================================
// ShellTask: TaskIterator implementation
// ===========================================================================

enum ShellPhase {
    NotStarted,
    Spawned { child: std::process::Child },
    Done,
}

struct ShellTask {
    command: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    working_dir: Option<PathBuf>,
    phase: ShellPhase,
}

impl TaskIterator for ShellTask {
    type Ready = ShellDone;
    type Pending = ShellPending;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<ShellDone, ShellPending, NoAction>> {
        match std::mem::replace(&mut self.phase, ShellPhase::Done) {
            ShellPhase::NotStarted => {
                let mut cmd = Command::new(&self.command);
                cmd.args(&self.args);
                for (k, v) in &self.envs {
                    cmd.env(k, v);
                }
                if let Some(dir) = &self.working_dir {
                    cmd.current_dir(dir);
                }
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());

                match cmd.spawn() {
                    Ok(child) => {
                        let pid = child.id();
                        self.phase = ShellPhase::Spawned { child };
                        Some(TaskStatus::Pending(ShellPending::Running { pid }))
                    }
                    Err(e) => {
                        self.phase = ShellPhase::Done;
                        Some(TaskStatus::Ready(ShellDone::Failed {
                            exit_code: None,
                            stdout: String::new(),
                            stderr: e.to_string(),
                        }))
                    }
                }
            }
            ShellPhase::Spawned { child } => {
                self.phase = ShellPhase::Done;
                match child.wait_with_output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                        let code = output.status.code();
                        if output.status.success() {
                            Some(TaskStatus::Ready(ShellDone::Success {
                                exit_code: code.unwrap_or(0),
                                stdout,
                                stderr,
                            }))
                        } else {
                            Some(TaskStatus::Ready(ShellDone::Failed {
                                exit_code: code,
                                stdout,
                                stderr,
                            }))
                        }
                    }
                    Err(e) => Some(TaskStatus::Ready(ShellDone::Failed {
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })),
                }
            }
            ShellPhase::Done => None,
        }
    }
}

// ===========================================================================
// Helpers: Boundary collection
// ===========================================================================

// Note: collect_one is re-exported from foundation_core::valtron above.
// collect_result is omitted for now - add back if multi-value collection is needed.

/// Collected output from a shell command (for simple non-streaming use cases).
#[derive(Debug, Clone)]
pub struct CollectedOutput {
    /// Process exit code, or `None` if killed by signal.
    pub exit_code: Option<i32>,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Whether the process exited successfully.
    pub success: bool,
}

/// Execute a shell command and block until it finishes, returning collected output.
///
/// This is a convenience wrapper for cases that don't need streaming progress.
/// Prefer consuming the stream directly when you need progress reporting.
///
/// # Errors
///
/// Returns `DeploymentError::ProcessFailed` if scheduling fails or the stream
/// produces no result. Returns `Ok(CollectedOutput)` in all other cases
/// (check `success` field for the process exit status).
pub fn execute_and_collect(executor: ShellExecutor) -> Result<CollectedOutput, DeploymentError> {
    let stream = executor.execute()?;

    let result = collect_one(stream).ok_or_else(|| DeploymentError::ProcessFailed {
        command: "shell".to_string(),
        exit_code: None,
        stdout: String::new(),
        stderr: "no result from stream".to_string(),
    })?;

    match result {
        ShellDone::Success {
            exit_code,
            stdout,
            stderr,
        } => Ok(CollectedOutput {
            exit_code: Some(exit_code),
            stdout,
            stderr,
            success: true,
        }),
        ShellDone::Failed {
            exit_code,
            stdout,
            stderr,
        } => Ok(CollectedOutput {
            exit_code,
            stdout,
            stderr,
            success: false,
        }),
    }
}
