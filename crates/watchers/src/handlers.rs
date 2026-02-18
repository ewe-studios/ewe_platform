use std::{
    fs::File,
    io::Read,
    path::{self, Path},
    process,
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::anyhow;
use tracing::{error, info};

use notify::{EventKind, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};

use crate::config::{self, CommandExpectation};

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

/// Creates a debounced notify watcher for the given `target_path`.
///
/// # Errors
///
/// Returns an error if the underlying notify debouncer fails to be created
/// or if adding the watch to the underlying watcher fails.
pub fn create_notify_watcher(
    target_path: &path::Path,
    debounce: u64,
    sender: std::sync::mpsc::Sender<DebounceEventResult>,
) -> crate::watcher::Result<NotifyWatcher> {
    let mut watcher: NotifyWatcher = new_debouncer(Duration::from_millis(debounce), None, sender)?;

    let watcher_path = Path::new(target_path);

    // watch target path
    watcher
        .watcher()
        .watch(watcher_path.as_ref(), notify::RecursiveMode::Recursive)?;

    Ok(watcher)
}

/// Create and run a watcher for the given configuration.
///
/// # Errors
///
/// Returns an error if the underlying notify watcher cannot be created or
/// initialized (propagates errors from `create_notify_watcher`).
pub fn watch_path(
    config: crate::config::Watcher,
    handler: crate::watcher::ChangeHandler,
) -> crate::watcher::Result<WatchHandle<()>> {
    info!("Creating a file watcher for path: {:?}", config.path());

    let (tx, rx) = std::sync::mpsc::channel();

    let target_path = config.path();
    let watcher_path = Path::new(&target_path);
    let watcher = create_notify_watcher(watcher_path, config.debounce().into(), tx)?;

    // listen for change events
    let join_handler = thread::spawn(move || {
        for events in rx.into_iter().flatten() {
            for event in events {
                match event.kind {
                    EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => {
                        if let Err(failed) =
                            handler(config.clone(), event.time, event.kind, event.paths.clone())
                        {
                            error!("Failed execution of update: {}", failed);
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(WatchHandle(join_handler, watcher))
}

type ExecResult<T> = std::result::Result<T, anyhow::Error>;

/// Execute a configured command description.
///
/// # Panics
///
/// This function will panic if `command.command` is empty because it calls
/// `first().unwrap()` to obtain the command binary.
///
/// # Errors
///
/// Returns an error if spawning or executing the command fails, or if the
/// command completes with a non-success status that is considered a failure
/// by `CommandExpectation`.
pub fn execute_command(mut command: config::CommandDescription) -> ExecResult<()> {
    let command_binary = command.command.first().unwrap().clone();
    let command_arguments = command.command.split_off(1);

    tracing::info!(
        "Executing shell command: bin={}, arguments={:?}",
        command_binary.clone(),
        command_arguments.clone()
    );

    let mut commander = process::Command::new(command_binary.clone());

    match commander
        .args(command_arguments.clone())
        .env("LS_COLORS", "rs=0:di=38;5;27:mh=44;38;5;15")
        .output()
    {
        Ok(result) => {
            if result.status.success() {
                return Ok(());
            }

            let output = String::from_utf8(result.stdout).unwrap();
            let error_output = String::from_utf8(result.stderr).unwrap();

            if command.if_failed == Some(CommandExpectation::Exit) {
                return Err(anyhow!(
                    r"
    Command: {command_binary}, args={command_arguments:?}

    Output:
        {output}

    Error:
        {error_output}

    [end]
                    ",
                ));
            }

            Ok(())
        }
        Err(err) => Err(anyhow!("failed to execute command: {err}")),
    }
}

/// Execute all configured commands for the provided watcher.
///
/// # Errors
///
/// Returns an error if any individual command execution returns an error.
pub fn execute_commands(watcher: &config::Watcher) -> ExecResult<()> {
    if let Some(watcher_commands) = watcher.commands() {
        for command in watcher_commands {
            execute_command(command)?;
        }
    }

    tracing::info!("Done executing!!!");
    Ok(())
}

/// Loads configuration from file path.
///
/// # Errors
///
/// Returns `ConfigError` if:
/// - File not found
/// - Failed to read file
/// - Unknown file format (only JSON supported)
/// - JSON parsing fails
///
/// # Panics
///
/// Panics if file extension is not valid UTF-8.
pub fn load_config(target: &path::Path) -> crate::config::Result<crate::config::Config> {
    let extension: &str = target.extension().unwrap().to_str().unwrap();
    match extension {
        "json" => {
            let mut file =
                File::open(target).map_err(|_| crate::config::ConfigError::FileNotFound)?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|_| crate::config::ConfigError::FailedReading)?;
            crate::config::Config::json(content.as_str())
        }
        _ => Err(crate::config::ConfigError::UnknownFormat),
    }
}
