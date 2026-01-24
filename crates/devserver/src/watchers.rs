use itertools::Itertools;
use std::path::PathBuf;

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

#[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
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
                Some("js") => FileChange::Javascript(value),
                Some("ts") => FileChange::Typescript(value),
                Some("rs") => FileChange::Rust(value),
                Some("rb") => FileChange::Ruby(value),
                Some(&_) => FileChange::Any(value),
                None => FileChange::Any(value),
            },
            _ => FileChange::Any(value),
        }
    }
}

pub struct DirectoryWatcher {
    pub directories: Vec<String>,
    pub file_change_sender: broadcast::Sender<FileChange>,
}

// -- Core Details

impl Operator for DirectoryWatcher {
    fn run(&self, mut cancel_signal: broadcast::Receiver<()>) -> crate::types::JoinHandle<()> {
        let sender_copy = self.file_change_sender.clone();

        let watcher_handler = watch_path(
            300,
            self.directories.clone(),
            true,
            move |_, _, change_list| {
                for change_item in change_list.iter().unique() {
                    match sender_copy.send(change_item.into()) {
                        Ok(_) => {}
                        Err(err) => {
                            tracing::error!("Failed to deliver notification: {err:?}");
                        }
                    }
                }
                Ok(())
            },
        )
        .expect("should create watcher");

        tokio::spawn(async move {
            let _ = cancel_signal.recv().await;
            watcher_handler.1.stop();
        });

        tokio::task::spawn_blocking(move || match watcher_handler.0.join() {
            Ok(()) => Ok(()),
            Err(err) => {
                ewe_trace::error!("Failed to correct destroy directory watcher: {:?}", err);
                Err(Box::new(DirectoryWatcherError::FailedToFinishedCorrectly).into())
            }
        })
    }
}

// -- Constructors

impl DirectoryWatcher {
    pub fn new<S>(directory: S, file_change_sender: broadcast::Sender<FileChange>) -> Self
    where
        S: Into<Vec<String>>,
    {
        Self {
            file_change_sender,
            directories: directory.into(),
        }
    }
}
