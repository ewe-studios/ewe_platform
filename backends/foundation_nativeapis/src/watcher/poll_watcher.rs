/// PollWatcher fallback — stdlib-only metadata polling.
///
/// Walks watched paths, records metadata (mtime, size, exists),
/// and diffs on each poll call. Slow but reliable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, thread};

use crate::error::{Result, WatchError};
use crate::event::{WatchEvent, WatchEventKind};
use crate::watcher::NativeWatcher;

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
            Ok(meta) => {
                Ok(PathSnapshot {
                    exists: true,
                    mtime: meta.modified().map_err(WatchError::Io)?,
                    size: meta.len(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(PathSnapshot {
                    exists: false,
                    mtime: std::time::UNIX_EPOCH,
                    size: 0,
                })
            }
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
        if self.exists && other.exists {
            if self.mtime != other.mtime || self.size != other.size {
                return Some(WatchEventKind::Modified);
            }
        }
        None
    }
}

/// Poll-based file watcher — uses filesystem metadata polling.
pub struct PollWatcher {
    /// Map of watched paths to their last known snapshot.
    watches: HashMap<PathBuf, PathSnapshot>,
}

impl PollWatcher {
    /// Create a new empty PollWatcher.
    pub fn new() -> Self {
        Self {
            watches: HashMap::new(),
        }
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
        // Sleep for the requested timeout before checking for changes
        thread::sleep(timeout);

        let mut events = Vec::new();
        let mut stale = Vec::new();
        // Buffer updates to apply after the immutable borrow ends
        let mut updates = Vec::new();

        // Check each watched path for changes
        for (path, old) in &self.watches {
            match PathSnapshot::from_path(path) {
                Ok(new) => {
                    // Emit an event if the path changed since last poll
                    if let Some(kind) = new.changes_since(old) {
                        events.push(WatchEvent {
                            kind,
                            path: path.clone(),
                        });
                    }
                    // Queue the snapshot update (can't mutate self.watches while borrowed above)
                    updates.push((path.clone(), new));
                }
                Err(WatchError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Path no longer exists — emit Removed if it previously existed
                    if old.exists {
                        events.push(WatchEvent {
                            kind: WatchEventKind::Removed,
                            path: path.clone(),
                        });
                        stale.push(path.clone());
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // Apply snapshot updates after the immutable borrow ends
        for (path, new) in updates {
            self.watches.insert(path, new);
        }

        // Clean up removed paths from the watch list
        for path in stale {
            self.watches.remove(&path);
        }

        Ok(events)
    }

    fn clear(&mut self) -> Result<()> {
        self.watches.clear();
        Ok(())
    }
}
