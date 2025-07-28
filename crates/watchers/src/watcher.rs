use notify::EventKind;
use std::{
    path::{self, PathBuf},
    thread,
    time::Instant,
};

pub type Result<T> = std::result::Result<T, anyhow::Error>;

pub type ChangeHandler =
    fn(crate::config::Watcher, when: Instant, kind: EventKind, paths: Vec<PathBuf>) -> Result<()>;

pub struct Watchers {
    config: Option<crate::config::Config>,
    watchers: Vec<Option<crate::handlers::WatchHandle<()>>>,
}

impl Default for Watchers {
    fn default() -> Self {
        Self::new()
    }
}

impl Watchers {
    pub fn new() -> Self {
        Self {
            config: None,
            watchers: Vec::with_capacity(5),
        }
    }

    pub(crate) fn reload(&mut self, config: crate::config::Config) {
        self.close_watchers();
        self.config.replace(config);
        self.start_watchers();
    }

    fn close_watchers(&mut self) {
        for watcher in &mut self.watchers {
            let crate::handlers::WatchHandle(join_handler, watcher) = watcher.take().unwrap();
            watcher.stop();
            join_handler.join().expect("finished watcher handler");
        }
        self.watchers.clear();
    }

    fn start_watchers(&mut self) {
        if let Some(config) = self.config.take() {
            for watcher in &config.watchers {
                self.watchers.push(Some(
                    crate::handlers::watch_path(
                        watcher.clone(),
                        |watcher_config, _, kind, changed| {
                            tracing::info!(
                                "Files changed with type of change: {:?} for files: {:?}",
                                kind,
                                changed,
                            );
                            match crate::handlers::execute_commands(watcher_config.clone()) {
                                Ok(()) => Ok(()),
                                Err(err) => {
                                    tracing::error!(
                                        "Failed to execute watchers command: {:?}",
                                        err
                                    );
                                    Ok(())
                                }
                            }
                        },
                    )
                    .expect("created watcher for path"),
                ));
            }
            self.config.replace(config);
        }
    }
}

pub struct ConfigWatcher {
    watcher_file: Box<path::Path>,
    watcher: std::sync::Arc<std::sync::Mutex<Watchers>>,
    debounce: u64,
}

impl ConfigWatcher {
    pub fn new(watcher_file: Box<path::Path>, watcher: Watchers, debounce: u64) -> Self {
        Self {
            watcher_file,
            debounce,
            watcher: std::sync::Arc::new(std::sync::Mutex::new(watcher)),
        }
    }

    pub fn listen(&self) -> Result<()> {
        // start watcher with initial load of config
        self.start_watcher()?;

        // start listening for config changes
        self.listen_for_change()?;
        Ok(())
    }

    fn start_watcher(&self) -> Result<()> {
        let target_file = self.watcher_file.clone();
        let target_watcher = self.watcher.clone();
        let loaded_config =
            crate::handlers::load_config(&target_file).expect("should load config correctly");
        target_watcher.lock().unwrap().reload(loaded_config);
        Ok(())
    }

    fn listen_for_change(&self) -> Result<()> {
        let debounce = self.debounce;
        let target_file = self.watcher_file.clone();
        let target_watcher = self.watcher.clone();

        thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();

            // generate recommender watcher that automatically
            // selects best implmenetation to use.
            let watcher = crate::handlers::create_notify_watcher(&target_file, debounce, tx)
                .expect("should have created watcher");

            for event_result in rx {
                match event_result {
                    Ok(_) => {
                        let reloaded_config = crate::handlers::load_config(&target_file)
                            .expect("should load config correctly");
                        target_watcher.lock().unwrap().reload(reloaded_config);
                    }
                    Err(err) => {
                        tracing::error!("Error occured watching config file: {:?}", err);
                        continue;
                    }
                }
            }

            watcher.stop();
        })
        .join()
        .map_err(|err| anyhow::anyhow!("Failed to join thread: {:?}", err))
    }
}
