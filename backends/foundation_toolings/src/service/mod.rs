// DevService — valtron-coordinated top-level dev server.
// Wires watchers, builder, runner, and HTTP proxy together.

use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;
use foundation_core::synca::OnSignal;
use foundation_core::valtron;
use foundation_nativeapis::shared::WatchEvent;
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

    /// Start the dev service. Spawns watchers/builder/runner as valtron
    /// TaskIterators, then blocks until shutdown signal.
    ///
    /// # Errors
    /// Returns an error if any component fails to spawn.
    pub fn start(
        &self,
        shutdown: &Arc<OnSignal>,
    ) -> Result<(), ToolingError> {
        // -- Shared queues
        let _build_changes_queue: Arc<ConcurrentQueue<WatchEvent>> =
            Arc::new(ConcurrentQueue::unbounded());
        let build_complete_queue: Arc<ConcurrentQueue<()>> =
            Arc::new(ConcurrentQueue::unbounded());
        let _running_queue: Arc<ConcurrentQueue<()>> =
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
            _running_queue,
        );
        valtron::execute(runner, None)
            .map_err(|e| ToolingError::Run(e.to_string()))?;

        // -- Block until shutdown signal
        tracing::info!("Dev service started — waiting for shutdown signal");
        loop {
            if shutdown.probe() {
                tracing::info!("Shutdown signal received, stopping dev service");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
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
    let shutdown = Arc::new(OnSignal::new());

    let _guard = valtron::initialize_pool(42, None);
    dev_service.start(&shutdown)
}
