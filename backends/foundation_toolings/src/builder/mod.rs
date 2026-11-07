// ProjectBuilder trait — pluggable builder interface.

pub mod cargo;
pub mod wasm;
pub mod wasm_pack;

pub use cargo::CargoBuilder;
pub use wasm::WasmBuilder;
pub use wasm_pack::WasmPackBuilder;

use std::sync::Arc;

use foundation_core::valtron::run_background_job;
use foundation_core::valtron::{EventQueue, QueueReadiness};
use foundation_nativeapis::shared::WatchEvent;
use foundation_nativeapis::valtron::CompositeReadiness;

use crate::watcher::FileChange;
use crate::ToolingError;

// -- BuildResult

pub type BuildResult =
    std::result::Result<BuildOutput, foundation_errstacks::ErrorTrace<ToolingError>>;

#[derive(Debug, Clone)]
pub enum BuildOutput {
    CheckPassed,
    BuildComplete { binary: String },
}

// -- ProjectBuilder trait

/// A build target that decides whether it should run for a given file change.
pub trait ProjectBuilder: Send + Sync + std::fmt::Debug {
    /// Returns the builder's human-readable name.
    fn name(&self) -> &str;

    /// Decide whether this file change should trigger a build.
    fn should_build(&self, change: &FileChange) -> bool;

    /// Run the build. Called by background job — blocking is fine.
    fn build(&self, change: &FileChange) -> BuildResult;
}

// -- ProjectBuilderTask

pub struct ProjectBuilderTask {
    builders: Vec<Arc<dyn ProjectBuilder>>,
    change_rx: foundation_core::synca::mpp::Receiver<WatchEvent>,
    /// Woken when the watcher sends a watch event, so the parked task runs.
    change_ready: QueueReadiness<WatchEvent>,
    /// Build results from the background jobs. Their push wakes the parked task.
    result_queue: EventQueue<BuildResult>,
    build_complete_queue: EventQueue<()>,
    stop_on_failure: bool,
}

impl ProjectBuilderTask {
    pub fn new(
        change_rx: foundation_core::synca::mpp::Receiver<WatchEvent>,
        build_complete_queue: EventQueue<()>,
    ) -> Self {
        let change_ready = change_rx.readiness();
        Self {
            builders: Vec::new(),
            change_rx,
            change_ready,
            result_queue: EventQueue::unbounded(),
            build_complete_queue,
            stop_on_failure: false,
        }
    }

    pub fn builder(mut self, b: impl ProjectBuilder + 'static) -> Self {
        self.builders.push(Arc::new(b));
        self
    }

    pub fn with_stop_on_failure(mut self, stop: bool) -> Self {
        self.stop_on_failure = stop;
        self
    }

    /// A readiness that is ready when the watcher has an unprocessed event. A
    /// send on the watcher channel wakes the task parked on it.
    #[must_use]
    pub fn change_readiness(&self) -> Arc<QueueReadiness<WatchEvent>> {
        Arc::new(self.change_ready.clone())
    }

    /// A readiness that is ready when a build result is queued. A push from the
    /// background job wakes the task parked on it.
    #[must_use]
    pub fn result_readiness(&self) -> Arc<QueueReadiness<BuildResult>> {
        Arc::new(self.result_queue.readiness())
    }

    /// The readiness of the whole task: ready when a file change **or** a build
    /// result arrives.
    pub fn readiness(&self) -> Arc<CompositeReadiness> {
        Arc::new(CompositeReadiness::new(
            self.change_readiness(),
            self.result_readiness(),
        ))
    }
}

impl ProjectBuilderTask {
    /// Drain watch events, dispatch to matching builders.
    ///
    /// `try_iter` pops each event from the watch channel, so the change
    /// readiness drops as the events are consumed.
    fn drain_watch_events(&mut self) {
        for event in self.change_rx.try_iter() {
            let change: FileChange = (&event).into();
            for builder in &self.builders {
                if builder.should_build(&change) {
                    self.submit_build(builder.clone(), change.clone());
                }
            }
        }
    }

    /// Submit a build to BackgroundJobRegistry.
    fn submit_build(&self, builder: Arc<dyn ProjectBuilder>, _change: FileChange) {
        let result_queue = self.result_queue.clone();
        let complete_queue = self.build_complete_queue.clone();

        run_background_job(move || {
            tracing::info!("{}: building for {:?}", builder.name(), _change);
            let result = builder.build(&_change);
            let _ = result_queue.push(result);
            let _ = complete_queue.push(());
        })
        .expect("background job pool available");
    }

    /// Drain build results (fire-and-forget: log, signal).
    fn drain_results(&mut self) -> Option<BuildResult> {
        let mut last_error = None;
        while let Ok(result) = self.result_queue.pop() {
            match &result {
                Ok(output) => tracing::info!("build complete: {output:?}"),
                Err(e) => {
                    tracing::error!("build failed: {e}");
                    if self.stop_on_failure {
                        last_error = Some(result);
                    }
                }
            }
        }
        last_error
    }
}

// -- TaskIterator impl

use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};

impl TaskIterator for ProjectBuilderTask {
    type Ready = BuildResult;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // 1. Drain watch events → dispatch to builders
        self.drain_watch_events();

        // 2. Drain build results
        if let Some(error) = self.drain_results() {
            return Some(TaskStatus::Ready(error));
        }

        // 3. Park until file changes or build results arrive
        Some(TaskStatus::Depends(self.readiness()))
    }
}
