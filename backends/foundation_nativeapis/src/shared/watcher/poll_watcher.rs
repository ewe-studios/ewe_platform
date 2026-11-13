/// `PollWatcher` fallback — stdlib-only metadata polling.
///
/// Walks watched paths, records metadata (mtime, size, exists),
/// and diffs on each poll call. Slow but reliable.
///
/// For a directory, the snapshot also holds the direct children (name, mtime
/// and size). A create, delete or change of a child then gives an event for
/// that child, even when the directory's own mtime did not move. On Windows
/// the file time moves in steps of about 15.6 ms, so a file created right
/// after the snapshot can leave the directory mtime unchanged.
///
/// `has_events()` and `poll()` share a cache so that readiness checks
/// don't duplicate scan work: `has_events()` scans and caches, `poll()`
/// drains the cache.
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
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
    /// The direct children of a directory, by name: (mtime, size).
    /// `None` for a file or a missing path.
    children: Option<BTreeMap<OsString, (std::time::SystemTime, u64)>>,
}

/// Read the direct children of `dir`. A child that goes away during the read
/// is left out; the next scan sees the change.
fn read_children(dir: &Path) -> Result<BTreeMap<OsString, (std::time::SystemTime, u64)>> {
    let mut children = BTreeMap::new();
    for entry in fs::read_dir(dir).map_err(WatchError::Io)? {
        let Ok(entry) = entry else { continue };
        let Ok(meta) = entry.metadata() else { continue };
        let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        children.insert(entry.file_name(), (mtime, meta.len()));
    }
    Ok(children)
}

impl PathSnapshot {
    fn from_path(path: &Path) -> Result<Self> {
        match fs::metadata(path) {
            Ok(meta) => Ok(PathSnapshot {
                exists: true,
                mtime: meta.modified().map_err(WatchError::Io)?,
                size: meta.len(),
                children: if meta.is_dir() {
                    Some(read_children(path)?)
                } else {
                    None
                },
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PathSnapshot {
                exists: false,
                mtime: std::time::UNIX_EPOCH,
                size: 0,
                children: None,
            }),
            Err(e) => Err(WatchError::Io(e)),
        }
    }

    /// Events for the children of `root` that changed since `old`.
    fn child_changes_since(&self, old: &PathSnapshot, root: &Path) -> Vec<WatchEvent> {
        let (Some(new), Some(old)) = (&self.children, &old.children) else {
            return Vec::new();
        };
        let mut events = Vec::new();
        for (name, now) in new {
            let kind = match old.get(name) {
                None => Some(WatchEventKind::Created),
                Some(before) if before != now => Some(WatchEventKind::Modified),
                Some(_) => None,
            };
            if let Some(kind) = kind {
                events.push(WatchEvent {
                    kind,
                    path: root.join(name),
                });
            }
        }
        for name in old.keys().filter(|name| !new.contains_key(*name)) {
            events.push(WatchEvent {
                kind: WatchEventKind::Removed,
                path: root.join(name),
            });
        }
        events
    }

    fn changes_since(&self, other: &PathSnapshot) -> Option<WatchEventKind> {
        if !self.exists && other.exists {
            return Some(WatchEventKind::Removed);
        }
        if self.exists && !other.exists {
            return Some(WatchEventKind::Created);
        }
        if self.exists && other.exists && (self.mtime != other.mtime || self.size != other.size) {
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
    #[must_use]
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
                    let child_events = new.child_changes_since(old, path);
                    if child_events.is_empty() {
                        if let Some(kind) = new.changes_since(old) {
                            events.push(WatchEvent {
                                kind,
                                path: path.clone(),
                            });
                        }
                    } else {
                        events.extend(child_events);
                    }
                    updates.push((path.clone(), new));
                }
                Err(WatchError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
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

impl Default for PollWatcher {
    fn default() -> Self {
        Self::new()
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
