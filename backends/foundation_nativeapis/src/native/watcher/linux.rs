/// Linux inotify-based file watcher.
///
/// Uses the native inotify API for real-time file system event monitoring.
/// Integrates with our epoll-based poll selector for readiness polling.

use std::collections::HashMap;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, mem};

use crate::shared::error::{Result, WatchError};
use crate::shared::event::{WatchEvent, WatchEventKind};
use crate::native::poll::{Interest, Poll, SourceFd, Token};
use crate::shared::watcher::NativeWatcher;

/// Maximum inotify buffer size for a single read.
const INOTIFY_BUF_LEN: usize = 4096;

/// Linux inotify-based file watcher.
pub struct InotifyWatcher {
    /// The raw inotify fd.
    inotify_fd: OwnedFd,
    /// The poll selector for readiness polling.
    poll: Poll,
    /// The token for our inotify fd.
    token: Token,
    /// Map from watch descriptor (wd) to path.
    wd_to_path: HashMap<i32, PathBuf>,
    /// Map from path to watch descriptor.
    path_to_wd: HashMap<PathBuf, i32>,
    /// Counter for generating unique tokens (future use).
    next_wd: i32,
    /// Buffer for reading inotify events.
    buffer: [u8; INOTIFY_BUF_LEN],
}

impl InotifyWatcher {
    /// Create a new InotifyWatcher.
    pub fn new() -> Result<Self> {
        let inotify_fd = unsafe {
            let fd = libc::inotify_init1(libc::IN_CLOEXEC);
            if fd < 0 {
                return Err(WatchError::Io(io::Error::last_os_error()));
            }
            OwnedFd::from_raw_fd(fd)
        };

        // Set nonblocking so reads return EAGAIN when no data
        unsafe {
            let flags = libc::fcntl(inotify_fd.as_raw_fd(), libc::F_GETFL, 0);
            if flags < 0 {
                return Err(WatchError::Io(io::Error::last_os_error()));
            }
            let r = libc::fcntl(
                inotify_fd.as_raw_fd(),
                libc::F_SETFL,
                flags | libc::O_NONBLOCK,
            );
            if r < 0 {
                return Err(WatchError::Io(io::Error::last_os_error()));
            }
        }

        let poll = Poll::new()?;
        let token = Token(0); // inotify always uses token 0

        poll.registry().register_fd(
            inotify_fd.as_raw_fd(),
            token,
            Interest::READABLE,
        )?;

        Ok(Self {
            inotify_fd,
            poll,
            token,
            wd_to_path: HashMap::new(),
            path_to_wd: HashMap::new(),
            next_wd: 1,
            buffer: [0u8; INOTIFY_BUF_LEN],
        })
    }

    /// Decode inotify events from the raw buffer.
    /// Takes the wd→path map as a mutable reference to handle DELETE_SELF,
    /// and the inotify fd for removing watches when paths are deleted.
    ///
    /// Returns `(events, error)` — successfully decoded events, and an optional
    /// error if any event could not be decoded (e.g., unknown watch descriptor
    /// or queue overflow). The caller should log the error and continue polling.
    fn decode_events(
        wd_to_path: &mut HashMap<i32, PathBuf>,
        path_to_wd: &mut HashMap<PathBuf, i32>,
        inotify_fd: std::os::fd::RawFd,
        buf: &[u8],
    ) -> (Vec<WatchEvent>, Option<WatchError>) {
        let mut events = Vec::new();
        let mut offset = 0;
        let mut decode_error = None;

        // Track renamed-from events for cookie matching
        let mut cookie_from: HashMap<u32, PathBuf> = HashMap::new();

        while offset + mem::size_of::<libc::inotify_event>() <= buf.len() {
            let event_ptr = buf[offset..].as_ptr() as *const libc::inotify_event;
            let event = unsafe { &*event_ptr };

            let name = if event.len > 0 {
                let name_ptr = unsafe {
                    (event_ptr as *const u8).offset(mem::size_of::<libc::inotify_event>() as isize)
                };
                let name_bytes = unsafe { std::slice::from_raw_parts(name_ptr, event.len as usize) };
                // Find the null terminator — if none found, treat all bytes as the name
                let null_pos = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
                String::from_utf8_lossy(&name_bytes[..null_pos]).to_string()
            } else {
                String::new()
            };

            let dir_path = match wd_to_path.get(&event.wd).cloned() {
                Some(p) => p,
                None => {
                    // The watch descriptor is not in our map — this shouldn't happen
                    // unless there's a bug or the watch was removed without us knowing.
                    // Skip this event but record the error so the caller can log it.
                    decode_error = Some(WatchError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("inotify event for unknown watch descriptor {}", event.wd),
                    )));
                    offset += mem::size_of::<libc::inotify_event>() + event.len as usize;
                    continue;
                }
            };

            let full_path = if name.is_empty() {
                dir_path.clone()
            } else {
                dir_path.join(&name)
            };

            let mask = event.mask;

            if mask & libc::IN_IGNORED != 0 {
                // Watch was removed by kernel — this is expected when the user
                // calls unwatch() or when the watched file is deleted.
                offset += mem::size_of::<libc::inotify_event>() + event.len as usize;
                continue;
            }

            if mask & libc::IN_Q_OVERFLOW != 0 {
                // Queue overflow — inotify's buffer was full and some events were lost.
                // We cannot recover the lost events, so return an error to the caller.
                decode_error = Some(WatchError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    "inotify queue overflow — some events may have been lost",
                )));
                offset += mem::size_of::<libc::inotify_event>() + event.len as usize;
                continue;
            }

            // Handle rename cookie matching
            if mask & libc::IN_MOVED_FROM != 0 && event.cookie != 0 {
                cookie_from.insert(event.cookie, full_path.clone());
            } else if mask & libc::IN_MOVED_TO != 0 && event.cookie != 0 {
                if let Some(from_path) = cookie_from.remove(&event.cookie) {
                    events.push(WatchEvent {
                        kind: WatchEventKind::Renamed {
                            from: from_path,
                            to: full_path.clone(),
                        },
                        path: full_path.clone(),
                    });
                } else {
                    // No matching MOVED_FROM — treat as Created
                    events.push(WatchEvent {
                        kind: WatchEventKind::Created,
                        path: full_path.clone(),
                    });
                }
            } else if mask & libc::IN_CREATE != 0 {
                events.push(WatchEvent {
                    kind: WatchEventKind::Created,
                    path: full_path.clone(),
                });
            } else if mask & libc::IN_MODIFY != 0 {
                events.push(WatchEvent {
                    kind: WatchEventKind::Modified,
                    path: full_path.clone(),
                });
            } else if mask & libc::IN_DELETE != 0 {
                events.push(WatchEvent {
                    kind: WatchEventKind::Removed,
                    path: full_path.clone(),
                });
            } else if mask & libc::IN_DELETE_SELF != 0 {
                // The watched path itself was deleted
                if let Some(path) = wd_to_path.remove(&event.wd) {
                    if let Some(wd) = path_to_wd.remove(&path) {
                        unsafe { libc::inotify_rm_watch(inotify_fd, wd) };
                    }
                    events.push(WatchEvent {
                        kind: WatchEventKind::Removed,
                        path,
                    });
                }
            }

            offset += mem::size_of::<libc::inotify_event>() + event.len as usize;
        }

        // Any unmatched MOVED_FROM events are treated as Removed
        for (_cookie, path) in cookie_from {
            events.push(WatchEvent {
                kind: WatchEventKind::Removed,
                path,
            });
        }

        (events, decode_error)
    }

    /// Recursively add watches for all subdirectories.
    fn watch_recursive(&mut self, path: &Path) -> Result<()> {
        // First watch the directory itself
        self.add_watch_inner(path)?;

        // Then walk and watch all subdirectories
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    // Recurse into subdirectory
                    self.watch_recursive(&entry_path)?;
                }
            }
        }
        Ok(())
    }

    /// Add a single watch (non-recursive).
    fn add_watch_inner(&mut self, path: &Path) -> Result<i32> {
        let path_cstr = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| WatchError::InvalidPath(path.to_path_buf()))?;

        let wd = unsafe {
            libc::inotify_add_watch(
                self.inotify_fd.as_raw_fd(),
                path_cstr.as_ptr(),
                libc::IN_CREATE
                    | libc::IN_MODIFY
                    | libc::IN_DELETE
                    | libc::IN_MOVED_FROM
                    | libc::IN_MOVED_TO
                    | libc::IN_DELETE_SELF,
            )
        };

        if wd < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOSPC) {
                return Err(WatchError::WatchLimit);
            }
            return Err(WatchError::Io(err));
        }

        let path_buf = path.to_path_buf();
        self.wd_to_path.insert(wd, path_buf.clone());
        self.path_to_wd.insert(path_buf, wd);
        Ok(wd)
    }
}

impl NativeWatcher for InotifyWatcher {
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        if recursive {
            self.watch_recursive(path)
        } else {
            self.add_watch_inner(path)?;
            Ok(())
        }
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        if let Some(&wd) = self.path_to_wd.get(path) {
            unsafe {
                libc::inotify_rm_watch(self.inotify_fd.as_raw_fd(), wd);
            }
            self.wd_to_path.remove(&wd);
            self.path_to_wd.remove(path);
            Ok(())
        } else {
            Err(WatchError::NotWatched)
        }
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        let mut events = crate::native::poll::Events::with_capacity(16);

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

        // Read inotify events from the fd
        let n = unsafe {
            libc::read(
                self.inotify_fd.as_raw_fd(),
                self.buffer.as_mut_ptr() as *mut libc::c_void,
                self.buffer.len(),
            )
        };

        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                // No data available yet
                return Ok(Vec::new());
            }
            return Err(WatchError::Io(err));
        }

        // Decode the buffer into WatchEvents (borrows &mut self.wd_to_path, &mut self.path_to_wd)
        let (events, error) = Self::decode_events(
            &mut self.wd_to_path,
            &mut self.path_to_wd,
            self.inotify_fd.as_raw_fd(),
            &self.buffer[..n as usize],
        );

        // If there was a decode error, log it but still return any events we did get.
        // Queue overflow means some events were lost — the caller should know.
        if let Some(e) = error {
            tracing::error!("InotifyWatcher decode error: {}", e);
        }

        Ok(events)
    }

    fn clear(&mut self) -> Result<()> {
        // inotify_rm_watch all watches
        for (&wd, _path) in &self.wd_to_path {
            unsafe {
                libc::inotify_rm_watch(self.inotify_fd.as_raw_fd(), wd);
            }
        }
        self.wd_to_path.clear();
        self.path_to_wd.clear();
        Ok(())
    }

    fn has_events(&mut self, timeout: Option<Duration>) -> bool {
        let mut events = crate::native::poll::Events::with_capacity(1);
        // Use zero timeout when None — this is a readiness check, not a blocking poll.
        // The epoll fd tells the kernel "is the inotify fd readable right now?"
        // We don't want to block waiting for new events.
        let effective = timeout.unwrap_or(Duration::ZERO);
        self.poll.poll(&mut events, Some(effective)).is_ok() && !events.is_empty()
    }
}
