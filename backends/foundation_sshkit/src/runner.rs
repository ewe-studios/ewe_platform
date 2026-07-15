//! Runner strategies for executing commands across multiple hosts.

use crate::backends::Backend;
use crate::command::{Command, CommandResult};
use crate::host::Host;
use std::time::Duration;

/// Execution strategy for multi-host commands.
pub enum Runner {
    /// Run on all hosts concurrently.
    Parallel,
    /// Run hosts one-by-one with a configurable delay between.
    Sequential { wait_interval: Duration },
    /// Run in batches of N (parallel within, sequential between).
    Group { limit: usize },
}

impl Runner {
    /// Parallel execution (default).
    #[must_use]
    pub fn parallel() -> Self { Self::Parallel }

    /// Sequential execution with delay.
    #[must_use]
    pub fn sequential(wait_interval: Duration) -> Self {
        Self::Sequential { wait_interval }
    }

    /// Grouped execution — N hosts at a time.
    #[must_use]
    pub fn group(limit: usize) -> Self {
        Self::Group { limit }
    }

    /// Execute a command on all hosts using this strategy.
    pub fn run<F>(
        &self,
        hosts: &[Host],
        backend: &dyn Backend,
        command_builder: F,
    ) -> Vec<Result<CommandResult, String>>
    where
        F: Fn(&Host) -> Command,
    {
        match self {
            Self::Parallel => {
                // Sequential for now — proper parallel needs threads/tokio
                hosts.iter().map(|h| backend.execute(h, &command_builder(h))).collect()
            }
            Self::Sequential { wait_interval } => {
                let mut results = Vec::new();
                for host in hosts {
                    results.push(backend.execute(host, &command_builder(host)));
                    std::thread::sleep(*wait_interval);
                }
                results
            }
            Self::Group { limit } => {
                let mut results = Vec::new();
                for chunk in hosts.chunks(*limit) {
                    let batch: Vec<_> = chunk
                        .iter()
                        .map(|h| backend.execute(h, &command_builder(h)))
                        .collect();
                    results.extend(batch);
                }
                results
            }
        }
    }
}
