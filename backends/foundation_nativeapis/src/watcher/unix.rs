/// macOS/BSD kqueue-based file watcher.
///
/// Uses EVFILT_VNODE to track file system changes: WRITE, DELETE, EXTEND, RENAME, REVOKE.
/// The kqueue fd is shared with our poll selector — EVFILT_VNODE events are distinguished
/// by their filter field from EVFILT_READ/WRITE events.

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, mem};

use crate::error::{Result, WatchError};
use crate::event::{WatchEvent, WatchEventKind};
use crate::poll::{Poll, Registry, Token};
use crate::watcher::NativeWatcher;

/// macOS/BSD kqueue-based file watcher.
///
/// Each watched path is opened as an fd and registered with EVFILT_VNODE.
/// The kqueue selector handles both socket readiness (EVFILT_READ/WRITE) and
/// file watching (EVFILT_VNODE).
pub struct KqueueWatcher {
    /// The poll selector (owns the kqueue fd).
    poll: Poll,
    /// Token for file watching events (separate from socket readiness tokens).
    vnode_token: Token,
    /// Map from fd to path. We need the fd open for kqueue to track the vnode.
    fd_to_path: HashMap<i32, PathBuf>,
    /// Map from path to fd.
    path_to_fd: HashMap<PathBuf, i32>,
    /// Counter for generating unique tokens.
    next_token: usize,
}

impl KqueueWatcher {
    /// Create a new KqueueWatcher.
    pub fn new() -> Result<Self> {
        let poll = Poll::new()?;

        Ok(Self {
            poll,
            vnode_token: Token(usize::MAX), // Use a unique token for vnode events
            fd_to_path: HashMap::new(),
            path_to_fd: HashMap::new(),
            next_token: 1,
        })
    }

    /// Open a file for EVFILT_VNODE monitoring.
    fn open_vnode(path: &Path) -> io::Result<OwnedFd> {
        use std::ffi::CString;
        use std::os::unix::io::{FromRawFd, OwnedFd};

        let path_cstr = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid path"))?;

        // O_EVTONLY opens the file for event notification only (no read/write access needed)
        let fd = unsafe { libc::open(path_cstr.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    /// Add a single watch (non-recursive).
    fn add_watch_inner(&mut self, path: &Path) -> Result<()> {
        // Open the file for vnode monitoring
        let fd = Self::open_vnode(path).map_err(WatchError::Io)?;
        let raw_fd = fd.as_raw_fd();

        // Register with the kqueue selector via EVFILT_VNODE
        self.poll
            .registry()
            .selector()
            .register_vnode(raw_fd, self.vnode_token)
            .map_err(WatchError::Io)?;

        // Store the fd so it stays open (kqueue tracks by fd, not path)
        self.fd_to_path.insert(raw_fd, path.to_path_buf());
        self.path_to_fd.insert(path.to_path_buf(), raw_fd);

        // Leak the OwnedFd — we need it to stay open for the lifetime of the watch.
        // The fd will be closed when we explicitly close it in unwatch/clear.
        mem::forget(fd);

        Ok(())
    }

    /// Recursively add watches for all subdirectories.
    fn watch_recursive(&mut self, path: &Path) -> Result<()> {
        self.add_watch_inner(path)?;

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    self.watch_recursive(&entry_path)?;
                }
            }
        }
        Ok(())
    }

    /// Close an fd and remove it from tracking.
    fn close_fd(&mut self, fd: i32, path: &Path) {
        let _ = self
            .poll
            .registry()
            .selector()
            .deregister_vnode(fd);
        unsafe { libc::close(fd) };
        self.fd_to_path.remove(&fd);
        self.path_to_fd.remove(path);
    }
}

impl NativeWatcher for KqueueWatcher {
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        if recursive {
            self.watch_recursive(path)
        } else {
            self.add_watch_inner(path)
        }
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        if let Some(&fd) = self.path_to_fd.get(path) {
            self.close_fd(fd, path);
            Ok(())
        } else {
            Err(WatchError::NotWatched)
        }
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        use crate::poll::Events;

        let mut events = Events::with_capacity(16);

        match self.poll.poll(&mut events, Some(timeout)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                return Ok(Vec::new());
            }
            Err(e) => return Err(WatchError::Io(e)),
        }

        if events.is_empty() {
            return Ok(Vec::new());
        }

        let mut watch_events = Vec::new();

        for event in events.iter() {
            // Only process vnode events (they use our vnode_token)
            if event.token() != self.vnode_token {
                continue;
            }

            // Extract fd from the event
            // On kqueue, the ident field is the fd
            let fd = event.ident() as i32;

            let path = match self.fd_to_path.get(&fd) {
                Some(p) => p.clone(),
                None => continue,
            };

            // Decode EVFILT_VNODE fflags
            let fflags = event.fflags();

            if fflags & libc::NOTE_DELETE != 0 {
                watch_events.push(WatchEvent {
                    kind: WatchEventKind::Removed,
                    path,
                });
            } else if fflags & libc::NOTE_WRITE != 0 {
                watch_events.push(WatchEvent {
                    kind: WatchEventKind::Modified,
                    path,
                });
            } else if fflags & libc::NOTE_EXTEND != 0 {
                watch_events.push(WatchEvent {
                    kind: WatchEventKind::Modified,
                    path,
                });
            } else if fflags & libc::NOTE_RENAME != 0 {
                // kqueue doesn't give us the target path for renames
                watch_events.push(WatchEvent {
                    kind: WatchEventKind::Removed,
                    path,
                });
            } else if fflags & libc::NOTE_REVOKE != 0 {
                watch_events.push(WatchEvent {
                    kind: WatchEventKind::Removed,
                    path,
                });
            }
        }

        Ok(watch_events)
    }

    fn clear(&mut self) -> Result<()> {
        // Clone the path_to_fd map to avoid borrow issues during iteration
        let paths: Vec<PathBuf> = self.path_to_fd.keys().cloned().collect();
        for path in paths {
            if let Some(&fd) = self.path_to_fd.get(&path) {
                self.close_fd(fd, &path);
            }
        }
        Ok(())
    }
}
