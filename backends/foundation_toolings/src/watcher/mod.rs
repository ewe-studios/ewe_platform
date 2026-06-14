// FileChange enum + event pump that maps WatchEvent → FileChange.
// Reuses foundation_nativeapis::valtron::FileWatcherTask as-is.

use std::path::PathBuf;

use foundation_nativeapis::shared::WatchEvent;

// -- FileChange enum (preserved from ewe_devserver for API compat)

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum FileChange {
    Rust(PathBuf),
    Javascript(PathBuf),
    Typescript(PathBuf),
    Ruby(PathBuf),
    Any(PathBuf),
}

impl From<&PathBuf> for FileChange {
    fn from(value: &PathBuf) -> Self {
        value.clone().into()
    }
}

impl From<PathBuf> for FileChange {
    fn from(value: PathBuf) -> Self {
        match value.extension() {
            Some(inner) => match inner.to_str() {
                Some("rs") => FileChange::Rust(value),
                Some("js") => FileChange::Javascript(value),
                Some("ts") => FileChange::Typescript(value),
                Some("rb") => FileChange::Ruby(value),
                Some(&_) => FileChange::Any(value),
                None => FileChange::Any(value),
            },
            _ => FileChange::Any(value),
        }
    }
}

impl From<&WatchEvent> for FileChange {
    fn from(event: &WatchEvent) -> Self {
        event.path.clone().into()
    }
}

// -- Event pump: subscribes to FileWatcherTask, maps WatchEvent → FileChange

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::QueueReadiness;
use std::sync::Arc;

pub struct FileChangeEventPump {
    /// Queue that receives FileChange events.
    pub queue: Arc<ConcurrentQueue<FileChange>>,
    /// Readiness signal for valtron Depends.
    pub ready: QueueReadiness<FileChange>,
}

impl FileChangeEventPump {
    pub fn new() -> Self {
        let queue = Arc::new(ConcurrentQueue::unbounded());
        let ready = QueueReadiness::new(queue.clone());
        Self { queue, ready }
    }

    /// Process a WatchEvent from the FileWatcherTask subscriber.
    pub fn push(&self, event: &WatchEvent) {
        let _ = self.queue.push(event.into());
    }
}
