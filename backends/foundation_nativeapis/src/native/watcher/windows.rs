/// Windows ReadDirectoryChangesW-based file watcher.
///
/// Uses CreateFile to open directory handles, then ReadDirectoryChangesW
/// with overlapped I/O to receive file system notifications via IOCP.

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{io, mem, ptr};

use concurrent_queue::ConcurrentQueue;

use crate::shared::error::{Result, WatchError};
use crate::shared::event::{WatchEvent, WatchEventKind};
use crate::native::poll::Poll;

use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::IO::*,
};

/// Per-watch state for ReadDirectoryChangesW.
struct WatchState {
    /// Directory handle.
    dir_handle: HANDLE,
    /// Buffer for FILE_NOTIFY_INFORMATION.
    buffer: Vec<u8>,
    /// Overlapped I/O state.
    overlapped: Box<OVERLAPPED>,
}

/// Windows IOCP-based file watcher.
pub struct WinWatcher {
    /// The poll selector (owns the IOCP handle).
    poll: Poll,
    /// Map from path to watch state.
    watches: HashMap<PathBuf, WatchState>,
    /// Reusable event buffer.
    events_buf: Vec<u8>,
    /// Cached events — populated by `is_ready()` so they aren't lost,
    /// drained by `poll()`. IOCP is dequeue-based; this cache lets us
    /// peek without consuming.
    event_cache: Arc<ConcurrentQueue<WatchEvent>>,
}

impl WinWatcher {
    /// Create a new WinWatcher.
    pub fn new() -> Result<Self> {
        let poll = Poll::new()?;
        Ok(Self {
            poll,
            watches: HashMap::new(),
            events_buf: Vec::with_capacity(64 << 10), // 64KB
            event_cache: Arc::new(ConcurrentQueue::unbounded()),
        })
    }

    /// Open a directory handle for file change notifications.
    fn open_directory(path: &Path) -> io::Result<HANDLE> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let path_wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let handle = CreateFileW(
                path_wide.as_ptr(),
                FILE_LIST_DIRECTORY,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
                0,
            );

            if handle == INVALID_HANDLE_VALUE {
                Err(io::Error::last_os_error())
            } else {
                Ok(handle)
            }
        }
    }

    /// Start watching a directory via ReadDirectoryChangesW.
    fn start_watch(path: &Path, recursive: bool) -> io::Result<WatchState> {
        let dir_handle = Self::open_directory(path)?;
        let buffer = vec![0u8; 64 << 10]; // 64KB notification buffer
        let overlapped: Box<OVERLAPPED> = Box::new(unsafe { mem::zeroed() });

        // Issue the initial ReadDirectoryChangesW
        unsafe {
            let result = ReadDirectoryChangesW(
                dir_handle,
                buffer.as_ptr() as *mut _,
                buffer.len() as u32,
                if recursive { 1 } else { 0 },
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_SIZE
                    | FILE_NOTIFY_CHANGE_LAST_WRITE,
                ptr::null_mut(),
                overlapped.as_ref() as *const _ as *mut _,
                None,
            );

            if result == 0 {
                let err = io::Error::last_os_error();
                CloseHandle(dir_handle);
                return Err(err);
            }
        }

        Ok(WatchState {
            dir_handle,
            buffer,
            overlapped,
        })
    }

    /// Decode FILE_NOTIFY_INFORMATION structs from the buffer.
    fn decode_events(buffer: &[u8]) -> Vec<(WatchEventKind, OsString)> {
        let mut events = Vec::new();
        let mut offset = 0;

        while offset + mem::size_of::<FILE_NOTIFY_INFORMATION>() <= buffer.len() {
            let info_ptr = &buffer[offset] as *const u8 as *const FILE_NOTIFY_INFORMATION;
            let info = unsafe { &*info_ptr };

            // Decode the filename from UTF-16
            let name_slice = &buffer[offset + mem::size_of::<FILE_NOTIFY_INFORMATION>()
                ..offset + mem::size_of::<FILE_NOTIFY_INFORMATION>() + info.FileNameLength as usize];
            let name = {
                let utf16: Vec<u16> = name_slice
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                OsString::from_wide(&utf16)
            };

            let kind = match info.Action {
                FILE_ACTION_ADDED => WatchEventKind::Created,
                FILE_ACTION_MODIFIED => WatchEventKind::Modified,
                FILE_ACTION_REMOVED => WatchEventKind::Removed,
                FILE_ACTION_RENAMED_OLD_NAME => {
                    // The new name follows in the next FILE_NOTIFY_INFORMATION
                    // For now, treat as Removed — rename pairing requires buffering
                    WatchEventKind::Removed
                }
                FILE_ACTION_RENAMED_NEW_NAME => WatchEventKind::Created,
                _ => WatchEventKind::Modified,
            };

            events.push((kind, name));

            if info.NextEntryOffset == 0 {
                break;
            }
            offset += info.NextEntryOffset as usize;
        }

        events
    }
}

impl NativeWatcher for WinWatcher {
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        let state = Self::start_watch(path, recursive)?;
        self.watches.insert(path.to_path_buf(), state);
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        if let Some(state) = self.watches.remove(path) {
            unsafe {
                // Cancel pending I/O
                CancelIoEx(state.dir_handle, state.overlapped.as_ref() as *const _ as *mut _);
                CloseHandle(state.dir_handle);
            }
            Ok(())
        } else {
            Err(WatchError::NotWatched)
        }
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        use crate::native::poll::Events;

        // Drain any cached events from a prior is_ready() call first.
        // This ensures events peeked by is_ready() are not lost.
        let cached: Vec<WatchEvent> = self
            .event_cache
            .try_iter()
            .take_while(|_| !self.event_cache.is_empty())
            .collect();
        if !cached.is_empty() {
            return Ok(cached);
        }

        // For each watch, check if the overlapped I/O has completed
        let mut all_events = Vec::new();
        let mut completed_paths: Vec<PathBuf> = Vec::new();

        for (path, state) in &mut self.watches {
            let mut bytes_transferred: u32 = 0;

            unsafe {
                let result = GetOverlappedResult(
                    state.dir_handle,
                    state.overlapped.as_ref() as *const _ as *mut _,
                    &mut bytes_transferred,
                    if timeout.is_zero() { 0 } else { 1 },
                );

                if result != 0 && bytes_transferred > 0 {
                    // I/O completed — decode events
                    let buf = &state.buffer[..bytes_transferred as usize];
                    let decoded = Self::decode_events(buf);

                    for (kind, name) in decoded {
                        let full_path = if name.is_empty() {
                            path.clone()
                        } else {
                            path.join(&name)
                        };
                        all_events.push(WatchEvent {
                            kind,
                            path: full_path,
                        });
                    }

                    // Re-issue ReadDirectoryChangesW for next poll
                    let result = ReadDirectoryChangesW(
                        state.dir_handle,
                        state.buffer.as_ptr() as *mut _,
                        state.buffer.len() as u32,
                        0,
                        FILE_NOTIFY_CHANGE_FILE_NAME
                            | FILE_NOTIFY_CHANGE_DIR_NAME
                            | FILE_NOTIFY_CHANGE_SIZE
                            | FILE_NOTIFY_CHANGE_LAST_WRITE,
                        ptr::null_mut(),
                        state.overlapped.as_ref() as *const _ as *mut _,
                        None,
                    );

                    if result == 0 {
                        tracing::warn!(
                            "WinWatcher: ReadDirectoryChangesW failed for {:?}: {}",
                            path,
                            io::Error::last_os_error()
                        );
                        completed_paths.push(path.clone());
                    }
                }
            }
        }

        // Remove watches that failed to re-issue
        for path in completed_paths {
            if let Some(state) = self.watches.remove(&path) {
                unsafe {
                    CancelIoEx(state.dir_handle, state.overlapped.as_ref() as *const _ as *mut _);
                    CloseHandle(state.dir_handle);
                }
            }
        }

        Ok(all_events)
    }

    fn clear(&mut self) -> Result<()> {
        let paths: Vec<PathBuf> = self.watches.keys().cloned().collect();
        for path in paths {
            let _ = self.unwatch(&path);
        }
        Ok(())
    }

    fn has_events(&mut self, timeout: Option<Duration>) -> bool {
        // IOCP is dequeue-based — peek the cache first.
        if !self.event_cache.is_empty() {
            return true;
        }

        // Cache is empty, so we poll each watch with the given timeout
        // to see if anything is ready. We use GetOverlappedResult with
        // bWait=0 (non-blocking) since the caller can pass a timeout
        // here if they want to wait — but for IOCP, waiting requires
        // actually consuming, so we just check what's already queued.
        if let Some(dur) = timeout {
            if !dur.is_zero() {
                std::thread::sleep(dur);
            }
        }

        for (_path, state) in &self.watches {
            let mut bytes_transferred: u32 = 0;
            unsafe {
                let result = GetOverlappedResult(
                    state.dir_handle,
                    state.overlapped.as_ref() as *const _ as *mut _,
                    &mut bytes_transferred,
                    0, // don't wait
                );
                if result != 0 && bytes_transferred > 0 {
                    return true;
                }
            }
        }
        false
    }
}
