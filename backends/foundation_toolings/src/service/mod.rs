// DevService — valtron-coordinated top-level dev server.
// Wires watchers, builder, runner, signal handling together.

use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{self, Stream};
use foundation_nativeapis::signal::{signal_task, SignalKind};
use foundation_nativeapis::valtron::FileWatcherTask;

use crate::builder::{CargoBuilder, ProjectBuilderTask};
use crate::runner::BinaryRunnerTask;
use crate::types::ProjectDefinition;
use crate::ToolingError;

pub struct DevService {
    pub project: ProjectDefinition,
}

impl DevService {
    #[must_use]
    pub fn new(project: ProjectDefinition) -> Self {
        Self { project }
    }

    /// Start the dev service. Spawns watchers/builder/runner/signal as valtron
    /// TaskIterators, then waits for a signal event (SIGINT/SIGTERM).
    ///
    /// # Errors
    /// Returns an error if any component fails to spawn.
    pub fn start(&self) -> Result<(), ToolingError> {
        // -- Shared queues
        let build_complete_queue: Arc<ConcurrentQueue<()>> =
            Arc::new(ConcurrentQueue::unbounded());
        let running_queue: Arc<ConcurrentQueue<()>> =
            Arc::new(ConcurrentQueue::unbounded());

        // -- File watcher (build dirs)
        let mut build_watcher = FileWatcherTask::new()
            .map_err(|e| ToolingError::Watch(e.to_string()))?;
        for dir in &self.project.build_directories {
            build_watcher
                .watch(std::path::Path::new(dir), true)
                .map_err(|e| ToolingError::Watch(e.to_string()))?;
        }
        let build_events_rx = build_watcher.subscribe();
        valtron::execute(build_watcher, None)
            .map_err(|e| ToolingError::Watch(e.to_string()))?;

        // -- File watcher (reload dirs)
        let mut reload_watcher = FileWatcherTask::new()
            .map_err(|e| ToolingError::Watch(e.to_string()))?;
        for dir in &self.project.reload_directories {
            reload_watcher
                .watch(std::path::Path::new(dir), true)
                .map_err(|e| ToolingError::Watch(e.to_string()))?;
        }
        valtron::execute(reload_watcher, None)
            .map_err(|e| ToolingError::Watch(e.to_string()))?;

        // -- Project builder (cargo)
        let builder = ProjectBuilderTask::new(build_events_rx, build_complete_queue.clone())
            .builder(CargoBuilder {
                workspace_root: self.project.workspace_root.clone(),
                crate_name: self.project.crate_name.clone(),
                build_args: self.project.build_arguments.clone(),
                skip_check: self.project.skip_rust_checks,
            })
            .with_stop_on_failure(self.project.stop_on_failure);
        valtron::execute(builder, None)
            .map_err(|e| ToolingError::Build(e.to_string()))?;

        // -- Binary runner
        let runner = BinaryRunnerTask::new(
            self.project.clone(),
            build_complete_queue,
            running_queue,
        );
        valtron::execute(runner, None)
            .map_err(|e| ToolingError::Run(e.to_string()))?;

        // -- Signal handler (Ctrl+C / SIGTERM)
        // This replaces the old shutdown.probe() + sleep(100ms) polling loop.
        // signal_task parks on epoll/kqueue — zero CPU until a signal arrives.
        let (sig_task, _bus) = signal_task()
            .map_err(|e| ToolingError::Watch(e.to_string()))?;
        let mut sig_stream = valtron::execute(sig_task, None)
            .map_err(|e| ToolingError::Watch(e.to_string()))?;

        tracing::info!("Dev service started — press Ctrl+C to stop");

        // Drive the signal stream — the valtron engine interleaves this with
        // all other tasks. The stream yields Stream::Pending/Delayed/Ignore
        // while waiting; only Stream::Next carries the actual SignalEvent.
        // We loop until a shutdown signal arrives.
        for item in &mut sig_stream {
            let Stream::Next(event) = item else { continue };
            match event.kind {
                SignalKind::Interrupt | SignalKind::Terminate => {
                    tracing::info!("Received {}, shutting down", event.kind);
                    break;
                }
                SignalKind::Hangup => {
                    tracing::info!("Received SIGHUP — reload not yet implemented");
                }
                SignalKind::Quit => {
                    tracing::info!("Received SIGQUIT — shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

/// Convenience wrapper for CLI consumers.
///
/// # Errors
/// Returns an error if any component fails to start.
pub fn run_dev_server(project: ProjectDefinition) -> Result<(), ToolingError> {
    let dev_service = DevService::new(project);
    let _guard = valtron::initialize_pool(42, None);
    dev_service.start()
}
