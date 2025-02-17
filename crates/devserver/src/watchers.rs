use derive_more::derive::From;
use tokio::sync::broadcast;

use crate::operators::Operator;
use ewe_watch_utils::watch_path;

#[derive(Debug, From)]
pub enum DirectoryWatcherError {
    FailedToFinishedCorrectly,
}

impl std::error::Error for DirectoryWatcherError {}

impl core::fmt::Display for DirectoryWatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct DirectoryWatcher {
    pub directories: Vec<String>,
    pub file_change_sender: broadcast::Sender<()>,
}

// -- Core Details

impl Operator for DirectoryWatcher {
    fn run(&self, mut cancel_signal: broadcast::Receiver<()>) -> crate::types::JoinHandle<()> {
        let sender_copy = self.file_change_sender.clone();
        let watch_callback = move |_, _, _| {
            sender_copy.send(()).expect("should deliver notification");
            Ok(())
        };

        let watcher_handler = watch_path(300, self.directories.clone(), true, watch_callback)
            .expect("should create watcher");

        let _ = tokio::spawn(async move {
            let _ = cancel_signal.recv().await;
            watcher_handler.1.stop();
        });

        tokio::task::spawn_blocking(move || match watcher_handler.0.join() {
            Ok(_) => Ok(()),
            Err(err) => {
                ewe_trace::error!("Failed to correct destroy directory watcher: {:?}", err);
                Err(Box::new(DirectoryWatcherError::FailedToFinishedCorrectly).into())
            }
        })
    }
}

// -- Constructors

impl DirectoryWatcher {
    pub fn new<S>(directory: S, file_change_sender: broadcast::Sender<()>) -> Self
    where
        S: Into<Vec<String>>,
    {
        Self {
            directories: directory.into(),
            file_change_sender,
        }
    }
}
