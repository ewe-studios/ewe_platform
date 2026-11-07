//! A missing background pool makes the builder task report a typed error (#348).
//!
//! WHY a separate test binary: the background pool is process-global, and
//! other tests start one; here nothing does.

#![cfg(not(target_family = "wasm"))]

use std::path::PathBuf;

use foundation_core::synca::mpp;
use foundation_core::valtron::{Ctx, EventQueue, TaskIterator, TaskStatus};
use foundation_nativeapis::shared::{WatchEvent, WatchEventKind};
use foundation_toolings::builder::{BuildOutput, BuildResult, ProjectBuilder, ProjectBuilderTask};
use foundation_toolings::watcher::FileChange;

/// A builder that always accepts a change, so the task tries to submit a build.
#[derive(Debug)]
struct AlwaysBuild;

impl ProjectBuilder for AlwaysBuild {
    fn name(&self) -> &'static str {
        "always"
    }

    fn should_build(&self, _change: &FileChange) -> bool {
        true
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        Ok(BuildOutput::CheckPassed)
    }
}

#[test]
fn a_missing_background_pool_becomes_a_ready_error() {
    let (tx, rx) = mpp::unbounded::<WatchEvent>();
    let mut task = ProjectBuilderTask::new(rx, EventQueue::unbounded())
        .builder(AlwaysBuild)
        .with_stop_on_failure(true);

    tx.send(WatchEvent {
        kind: WatchEventKind::Modified,
        path: PathBuf::from("src/main.rs"),
    })
    .expect("send the watch event");

    let status = task.next_status(Ctx::host());

    match status {
        Some(TaskStatus::Ready(Err(error))) => {
            let message = error.to_string();
            assert!(
                message.contains("Background job pool not initialized"),
                "the error must name the missing pool, got: {message}"
            );
        }
        other => panic!("expected TaskStatus::Ready(Err(..)), got {other:?}"),
    }
}
