pub use notify::EventKind;

use std::{
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::Duration,
    time::Instant,
};

use notify::Watcher;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};

pub type Result<T> = std::result::Result<T, anyhow::Error>;

#[cfg(all(target_os = "macos", not(feature = "macos_kqueue")))]
pub type NotifyWatcher = notify_debouncer_full::Debouncer<
    notify::fsevent::FsEventWatcher,
    notify_debouncer_full::FileIdMap,
>;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub type NotifyWatcher =
    notify_debouncer_full::Debouncer<notify::INotifyWatcher, notify_debouncer_full::FileIdMap>;

#[cfg(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "ios",
    all(target_os = "macos", feature = "macos_kqueue")
))]
pub type NotifyWatcher = notify_debouncer_full::Debouncer<
    notify::kqueue::KqueueWatcher,
    notify_debouncer_full::FileIdMap,
>;

pub struct WatchHandle<T>(pub JoinHandle<T>, pub NotifyWatcher);

/// # Errors
///
/// Returns an error if the file watcher cannot be initialized.
pub fn create_notify_watcher(
    debounce_millis: u64,
    sender: std::sync::mpsc::Sender<DebounceEventResult>,
) -> Result<NotifyWatcher> {
    let watcher: NotifyWatcher =
        new_debouncer(Duration::from_millis(debounce_millis), None, sender)?;
    Ok(watcher)
}

/// # Errors
///
/// Returns an error if:
/// - The file watcher cannot be initialized
/// - The specified paths cannot be watched
/// - The handler function returns an error
pub fn watch_path(
    debounce_millis: u64,
    target_paths: Vec<String>,
    be_recursive: bool,
    handler: impl Fn(Instant, EventKind, Vec<PathBuf>) -> Result<()> + Send + Sync + 'static,
) -> Result<WatchHandle<()>> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = create_notify_watcher(debounce_millis, tx)?;

    let r_mode = if be_recursive {
        notify::RecursiveMode::Recursive
    } else {
        notify::RecursiveMode::NonRecursive
    };

    // watch target path
    for watcher_path in target_paths {
        watcher
            .watcher()
            .watch(Path::new(&watcher_path).as_ref(), r_mode)?;
    }

    // listen for change events
    let join_handler = thread::spawn(move || {
        for events in rx.into_iter().flatten() {
            for event in events {
                match event.kind {
                    EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => {
                        if let Err(failed) =
                            handler(event.time, event.kind, event.paths.clone())
                        {
                            ewe_trace::error!("Failed execution of update: {}", failed);
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(WatchHandle(join_handler, watcher))
}
