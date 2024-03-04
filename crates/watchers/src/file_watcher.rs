// module implementation details

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::new_debouncer;
use std::{
    fmt::{Debug, Display},
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct FileWatcher {
    target_location: PathBuf,
}

impl FileWatcher {
    pub fn new(target_location: PathBuf) -> Self {
        Self { target_location }
    }
}

pub fn watch<P: AsRef<Path> + Debug>(path: P) -> notify::Result<()> {
    info!("Creating a file watcher for path: {:?}", path);

    let (tx, rx) = std::sync::mpsc::channel();

    // generate recommender watcher that automatically
    // selects best implmenetation to use.
    let mut watcher = new_debouncer(Duration::from_millis(800), None, tx)?;

    // watch target path
    watcher
        .watcher()
        .watch(path.as_ref(), notify::RecursiveMode::Recursive)?;

    // listen for change events
    for change_event in rx {
        match change_event {
            Err(err) => error!("Error: {err:?}"),
            Ok(event) => info!("Changed: {event:?}"),
        }
    }

    Ok(())
}

// module for tests
#[cfg(test)]
mod tests {}
