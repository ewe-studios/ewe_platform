use notify::EventKind;

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

pub fn create_notify_watcher(
    target_path: String,
    debounce_millis: u64,
    be_recursive: bool,
    sender: std::sync::mpsc::Sender<DebounceEventResult>,
) -> Result<NotifyWatcher> {
    let mut watcher: NotifyWatcher =
        new_debouncer(Duration::from_millis(debounce_millis), None, sender)?;

    let watcher_path = Path::new(&target_path);

    let r_mode = if be_recursive {
        notify::RecursiveMode::Recursive
    } else {
        notify::RecursiveMode::NonRecursive
    };

    // watch target path
    watcher.watcher().watch(watcher_path.as_ref(), r_mode)?;

    Ok(watcher)
}

pub fn watch_path(
    debounce_millis: u64,
    target_path: String,
    be_recursive: bool,
    handler: impl Fn(String, Instant, EventKind, Vec<PathBuf>) -> Result<()> + Send + Sync + 'static,
) -> Result<WatchHandle<()>> {
    let (tx, rx) = std::sync::mpsc::channel();

    let watcher = create_notify_watcher(target_path.clone(), debounce_millis, be_recursive, tx)?;

    // listen for change events
    let join_handler = thread::spawn(move || {
        for event_result in rx {
            match event_result {
                Ok(events) => {
                    for event in events {
                        match event.kind {
                            EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => {
                                if let Err(failed) = handler(
                                    target_path.clone(),
                                    event.time,
                                    event.kind,
                                    event.paths.clone(),
                                ) {
                                    ewe_logs::error!("Failed execution of update: {}", failed);
                                }
                            }
                            _ => continue,
                        }
                    }
                }
                Err(_) => continue,
            }
        }
    });

    Ok(WatchHandle(join_handler, watcher))
}
