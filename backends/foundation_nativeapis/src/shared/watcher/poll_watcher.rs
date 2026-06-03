/// PollWatcher fallback — stdlib-only metadata polling.
///
/// Walks watched paths, records metadata (mtime, size, exists),
/// and diffs on each poll call. Slow but reliable.
///
/// `has_events()` and `poll()` share a cache so that readiness checks
/// don't duplicate scan work: `has_events()` scans and caches, `poll()`
/// drains the cache.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, thread};

use super::super::error::{Result, WatchError};
use super::super::event::{WatchEvent, WatchEventKind};
use super::NativeWatcher;

/// A snapshot of a path's metadata at a point in time.
#[derive(Debug, Clone)]
struct PathSnapshot {
    exists: bool,
    mtime: std::time::SystemTime,
    size: u64,
}

impl PathSnapshot {
    fn from_path(path: &Path) -> Result<Self> {
        match fs::metadata(path) {
            Ok(meta) => Ok(PathSnapshot {
                exists: true,
                mtime: meta.modified().map_err(WatchError::Io)?,
                size: meta.len(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PathSnapshot {
                exists: false,
                mtime: std::time::UNIX_EPOCH,
                size: 0,
            }),
            Err(e) => Err(WatchError::Io(e)),
        }
    }

    fn changes_since(&self, other: &PathSnapshot) -> Option<WatchEventKind> {
        if !self.exists && other.exists {
            return Some(WatchEventKind::Removed);
        }
        if self.exists && !other.exists {
            return Some(WatchEventKind::Created);
        }
        if self.exists && other.exists
            && (self.mtime != other.mtime || self.size != other.size)
        {
            return Some(WatchEventKind::Modified);
        }
        None
    }
}

/// Poll-based file watcher.
pub struct PollWatcher {
    watches: HashMap<PathBuf, PathSnapshot>,
    /// Events cached by `has_events()` so `poll()` can drain them.
    cached_events: Vec<WatchEvent>,
}

impl PollWatcher {
    pub fn new() -> Self {
        Self {
            watches: HashMap::new(),
            cached_events: Vec::new(),
        }
    }

    /// Shared scan: check all watches, update snapshots, return events.
    fn scan_watches(&mut self) -> Vec<WatchEvent> {
        let mut events = Vec::new();
        let mut stale = Vec::new();
        let mut updates = Vec::new();

        for (path, old) in &self.watches {
            match PathSnapshot::from_path(path) {
                Ok(new) => {
                    if let Some(kind) = new.changes_since(old) {
                        events.push(WatchEvent {
                            kind,
                            path: path.clone(),
                        });
                    }
                    updates.push((path.clone(), new));
                }
                Err(WatchError::Io(e))
                    if e.kind() == std::io::ErrorKind::NotFound =>
                {
                    if old.exists {
                        events.push(WatchEvent {
                            kind: WatchEventKind::Removed,
                            path: path.clone(),
                        });
                        stale.push(path.clone());
                    }
                }
                Err(_) => {}
            }
        }

        for (path, new) in updates {
            self.watches.insert(path, new);
        }
        for path in stale {
            self.watches.remove(&path);
        }

        events
    }
}

impl NativeWatcher for PollWatcher {
    fn watch(&mut self, path: &Path, _recursive: bool) -> Result<()> {
        let snapshot = PathSnapshot::from_path(path)?;
        self.watches.insert(path.to_path_buf(), snapshot);
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        if self.watches.remove(path).is_some() {
            Ok(())
        } else {
            Err(WatchError::NotWatched)
        }
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        // Drain cached events first (populated by has_events).
        if !self.cached_events.is_empty() {
            return Ok(std::mem::take(&mut self.cached_events));
        }
        // No cached events — sleep then scan.
        thread::sleep(timeout);
        self.cached_events = self.scan_watches();
        Ok(std::mem::take(&mut self.cached_events))
    }

    fn clear(&mut self) -> Result<()> {
        self.watches.clear();
        self.cached_events.clear();
        Ok(())
    }

    fn has_events(&mut self, timeout: Option<Duration>) -> bool {
        // If we already have cached events from a prior scan, return immediately.
        if !self.cached_events.is_empty() {
            return true;
        }
        // Scan for changes. Use the provided timeout to wait (if any).
        if let Some(dur) = timeout {
            thread::sleep(dur);
        }
        self.cached_events = self.scan_watches();
        // Return true ONLY when events were found. Returning false is correct —
        // it avoids busy-looping. The executor parks the task and re-checks later.
        !self.cached_events.is_empty()
    }
}
